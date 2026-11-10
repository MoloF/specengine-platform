//! docs/features/trace-views.md (RM-0048), the CLI half: `stage:`, the folded
//! `start` and `end`, `spec flow --trace`, the `spec trace` lanes and `spec
//! card list --marks`, through the binary on scratch git repositories of
//! `fixtures/trace-almanac` (a village almanac: two committees over five
//! steps, the stages cutting across them, one step with no stage), with
//! `fixtures/flow-relay`, `flow-hire`, `flow-gaps`, `card-press` and `spec-c`
//! for the bytes and the genre. Cards are cut through the library. The tool
//! is MCP `mcp_flow.rs`, `mcp_trace.rs`, `mcp_cards.rs`; the daemon http
//! `flow.rs`, `trace.rs`, `cards.rs`; the page the UI's tests.
//!
//! - AC-01: a stage written, changed or taken away moves no `norm_hash`, `rev`
//!   or mark, no `unknown-key`, and `spec flow` prints it. M: counting the key
//!   in `norm_input`.
//! - AC-02: a folded node's `start` and `end`, at every depth; gap rows
//!   byte-equal to the unfolded read. M: `steps.starts.contains(&file)` kept
//!   in `shown`.
//! - AC-03: no `--trace`: the bytes of `trace_views_before.json`, recorded
//!   with the binary before this slice, but `stage`, `rollup` and `counts`
//!   `null`; `--trace`: each node's values `spec trace`'s. M: `rollup` filled
//!   without the flag.
//! - AC-06, the read: a `no card` step stands in its lane, and the lane fills.
//! - AC-07: `--stage`, `--summary`, lanes by least `distance`, two file orders.
//! - AC-09, the read: `list --marks` equals `spec trace`'s marks, a gone
//!   target included.
//! - The owner's readings of the lanes, settled after the spec: a lane holds
//!   the nodes carrying its stage, the last lane and `--stage -` the flow steps
//!   with none, so a group with no stage stands in no lane and `stages` can be
//!   `[]`; a queue this build cannot read gives `rollup` and `counts` `null`
//!   with a note.
//! - docs/features/trace-truth.md AC-07 moves the counts these pin: a holder
//!   with no card of its own is counted nowhere, its row word still `no card`,
//!   so the almanac's lanes now sum to the header and a holder standing in a
//!   lane leaves its count below its rows.
//! - "Rules and edge cases": no read changes a status, an exit or a file; a
//!   queue of another schema; a bad `[flow]`; a control character in a stage.
//!
//! Every expected value below is worked out from the spec and the fixture,
//! never read back from a run. Non-Latin characters in Rust sources are
//! Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::fs;
use std::path::{Path, PathBuf};

use common::graph::spec30;
use common::keys::{FLOW_KEYS, without_keys};
use common::proposal::{LATER, Pair};
use common::{Run, Scratch, fixture, index, md_files, read, read_text, replace, write};
use serde_json::{Value, json};
use specengine_cli::{
    CardAcceptRequest, CardAssignRequest, CardClearOutcome, CardClearRequest, CardMoveRequest,
    CardNewRequest, Env, Exit, Globals, card_accept, card_assign, card_clear, card_move, card_new,
};
use task_common::Ordered;

const ALMANAC: &str = "trace-almanac";
const BOOK: &str = "BOOK-ALMANAC";
const GATHER: &str = "PART-GATHER";
const PRINT: &str = "PART-PRINT";
const CALL: &str = "STEP-CALL";
const SORT: &str = "STEP-SORT";
const READ: &str = "STEP-READ";
const SET: &str = "STEP-SET";
const BIND: &str = "STEP-BIND";
const PROOF: &str = "STEP-SET-PROOF";

const CALL_MD: &str = "docs/spec/step-call.md";
const SORT_MD: &str = "docs/spec/step-sort.md";
const READ_MD: &str = "docs/spec/step-read.md";
const SET_MD: &str = "docs/spec/step-set.md";
const BIND_MD: &str = "docs/spec/step-bind.md";

/// The stage lines of a tree of `fixtures/trace-almanac`.
const GATHER_STAGE: &str = "stage: gather\n";
const PRINT_STAGE: &str = "stage: print\n";

/// A line each document carries: a stage goes after it.
const REVIEWED: &str = "reviewed: 2026-10-01\n";

/// The fixture's nine nodes.
const NODES: [&str; 9] = [BOOK, GATHER, PRINT, CALL, SORT, READ, SET, PROOF, BIND];

/// Meaning changes of each step's own text.
const CALL_MEANING: (&str, &str) = ("on the first of the month", "on the last of the month");
const SET_MEANING: (&str, &str) = ("opens on a right-hand page", "opens on a left-hand page");
const PROOF_MEANING: (&str, &str) = ("One proof goes", "Two proofs go");

/// `spec flow --json`'s node, in order ("Routes and types": 15 keys; `stage`
/// after `group`; docs/features/flow-scenario.md: 16, `in_flow` after `end`).
const FLOW_NODE_KEYS: [&str; 16] = [
    "id", "kind", "title", "path", "line", "distance", "group", "stage", "start", "end", "in_flow",
    "holds", "inner", "gaps", "rollup", "counts",
];

/// `spec trace --json`'s lane.
const STAGE_KEYS: [&str; 3] = ["stage", "distance", "counts"];

/// The five states' counts.
const COUNT_KEYS: [&str; 5] = ["no_card", "in_work", "suspect", "departed", "confirmed"];

/// What `--marks` adds to a row, `spec trace`'s keys of a card.
const MARK_KEYS: [&str; 4] = ["mark", "on", "discrepancy", "correction"];

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

/// `spec --json args`, exit 0: the document.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let done = run(pair, &all);
    done.code(0);
    done.json()
}

/// The library's `Env` for the main worktree.
fn env(pair: &Pair) -> Env {
    pair.env(&pair.main)
}

/// A card of the almanac's flow on `targets`: its ID.
fn cut(pair: &Pair, title: &str, targets: &[&str]) -> String {
    let outcome = card_new(
        &env(pair),
        &Globals::default(),
        &CardNewRequest {
            title: title.to_owned(),
            discipline: Some("editing".to_owned()),
            targets: targets.iter().map(|target| (*target).to_owned()).collect(),
            now: LATER.to_owned(),
            ..CardNewRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("card new {title}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome.id.expect("a card's ID")
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

/// A card accepted `--briefed` on `targets`: its ID.
fn accepted(pair: &Pair, title: &str, targets: &[&str]) -> String {
    let id = cut(pair, title, targets);
    to_review(pair, &id);
    accept(
        pair,
        &id,
        CardAcceptRequest {
            briefed: true,
            ..CardAcceptRequest::default()
        },
    );
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

/// `(from, to)` on `target` proposed and approved in the main worktree.
fn approved(pair: &Pair, target: &str, (from, to): (&str, &str)) {
    let id = pair.propose_edit(&pair.main, target, from, to);
    pair.approve_ok(&pair.main, &id);
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

/// `{no_card, in_work, suspect, departed, confirmed}`.
fn counts(no_card: u64, in_work: u64, suspect: u64, departed: u64, confirmed: u64) -> Value {
    json!({"no_card": no_card, "in_work": in_work, "suspect": suspect,
        "departed": departed, "confirmed": confirmed})
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

/// The IDs of a document's nodes, in order.
fn ids(document: &Value) -> Vec<String> {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["id"].as_str().expect("an ID").to_owned())
        .collect()
}

/// The lane of `stage`, `None` the one with none.
fn lane<'a>(document: &'a Value, stage: Option<&str>) -> &'a Value {
    let wanted = stage.map_or(Value::Null, |stage| json!(stage));
    document["stages"]
        .as_array()
        .expect("stages")
        .iter()
        .find(|row| row["stage"] == wanted)
        .unwrap_or_else(|| panic!("no lane {stage:?}: {document}"))
}

/// The field list of a text line.
fn fields(line: &str) -> Vec<&str> {
    line.split(" | ").collect()
}

/// The one node line of `spec flow`'s `stdout`: `<distance or -> <name> | ...`.
fn line_of<'a>(stdout: &'a str, name: &str) -> &'a str {
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| {
            let head = line.split(" | ").next().unwrap_or_default();
            head.split_once(' ').is_some_and(|(distance, shown)| {
                shown == name && (distance == "-" || distance.chars().all(|c| c.is_ascii_digit()))
            })
        })
        .collect();
    assert_eq!(lines.len(), 1, "{name} in:\n{stdout}");
    lines[0]
}

/// The `gap ...` lines of a text.
fn gap_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.starts_with("gap "))
        .collect()
}

/// `spec trace`'s `stage ...` lines.
fn stage_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.starts_with("stage "))
        .collect()
}

/// `spec check --json`'s findings of `code`.
fn findings_of(pair: &Pair, code: &str) -> Vec<Value> {
    doc(pair, &["check"])["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| finding["code"] == code)
        .cloned()
        .collect()
}

/// A scratch copy of `fixtures/<name>`, its `HOME` and a neutral `cwd`.
struct Copy {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
}

impl Copy {
    fn new(label: &str, name: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.copy(name, "copy");
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self {
            _scratch: scratch,
            root,
            home,
            cwd,
        }
    }

    /// `spec <args> --root <root>`.
    fn run(&self, args: &[&str]) -> Run {
        let mut all = args.to_vec();
        all.extend_from_slice(&["--root", self.root.to_str().expect("UTF-8 root")]);
        spec30(&self.home, &self.cwd, &all)
    }

    /// [`Copy::run`], exit 0: stdout.
    fn ok(&self, args: &[&str]) -> String {
        let done = self.run(args);
        done.code(0);
        done.stdout
    }

    /// [`Copy::run`] with `--json`, exit 0: the document.
    fn json(&self, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let done = self.run(&all);
        done.code(0);
        done.json()
    }

    /// The copy's config with `extra` appended.
    fn configure(&self, extra: &str) {
        let config = read_text(&self.root, "specengine.toml");
        write(
            &self.root,
            "specengine.toml",
            format!("{config}{extra}").as_bytes(),
        );
    }
}

/// A document of the almanac with `stage` in its front-matter.
fn plain_document(id: &str, stage: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\ntitle: {id}\nowner: owner\nreviewed: 2026-10-01\n\
         stage: {stage}\n---\n\n# {id}\n\nA body.\n"
    )
}

// ------------------------------------------------------------- the fixture

/// "Fixture": `fixtures/trace-almanac` reads as its `expected.json` says, by
/// `spec flow --json` and `spec trace --json`, in the project's own words.
#[test]
fn the_almanac_reads_as_expected() {
    let expected: Value = serde_json::from_slice(&read(&fixture(ALMANAC), "expected.json"))
        .expect("expected.json is JSON");
    let copy = Copy::new("tv-fixture", ALMANAC);
    let flow = copy.json(&["flow"]);
    let check = copy.json(&["check"]);
    assert_eq!(check["counts"]["errors"], json!(0), "{check}");
    assert_eq!(
        check["counts"]["warnings"],
        json!(0),
        "no unknown-key: {check}"
    );

    assert_eq!(flow["gaps"], expected["gaps"], "{flow}");
    assert_eq!(flow["edges"], expected["edges"], "{flow}");
    assert_eq!(
        ids(&flow),
        expected["walk"]
            .as_array()
            .expect("walk")
            .iter()
            .map(|id| id.as_str().expect("an ID").to_owned())
            .collect::<Vec<_>>()
    );
    let files = expected["files"].as_object().expect("files");
    assert_eq!(files.len(), 8);
    for (path, want) in files {
        let shown = flow["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .find(|node| node["path"] == *path)
            .unwrap_or_else(|| panic!("{path}: {flow}"));
        for key in ["id", "kind", "title", "stage", "distance", "start", "end"] {
            assert_eq!(shown[key], want[key], "{path}: {key}: {shown}");
        }
        assert_eq!(shown["group"], want["parent"], "{path}: {shown}");
    }

    let trace = copy.json(&["trace"]);
    assert_eq!(trace["stages"], expected["stages"], "{trace}");
    assert_eq!(trace["counts"], counts(5, 0, 0, 0, 0), "{trace}");
    let (prefixes, kinds) = (expected["prefixes"].clone(), expected["kinds"].clone());
    assert_eq!(prefixes, json!(["BOOK", "PART", "STEP"]));
    assert_eq!(kinds, json!(["committee", "errand", "yearbook"]));
    assert_eq!(
        files[SET_MD]["sections"],
        json!([PROOF]),
        "the one section: {expected}"
    );
    copy.run(&["show", PROOF]).code(0);
}

// ------------------------------------------------------------------- AC-01

/// What a stage must leave alone: each node's `norm_hash` and `rev`, and each
/// `spec trace` row with its cards and their marks.
fn standing(pair: &Pair) -> Vec<String> {
    let mut rows: Vec<String> = NODES
        .iter()
        .map(|id| {
            let node = pair.node(&pair.main, id);
            assert!(node.norm_hash.is_some(), "{id} is hashed");
            format!("{id} | rev {:?} | {:?}", node.rev, node.norm_hash)
        })
        .collect();
    let trace = doc(pair, &["trace"]);
    rows.extend(
        trace["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| {
                format!(
                    "{} | {} | {} | {} | {}",
                    node["id"], node["rollup"], node["counts"], node["cards"], node["uses"]
                )
            }),
    );
    rows
}

/// `file` with its stage line `from` turned to `to`; `from` empty: `to`
/// written after the file's `reviewed:` line.
fn recut(pair: &Pair, file: &str, from: &str, to: &str) {
    if from.is_empty() {
        replace(&pair.main, file, REVIEWED, &format!("{REVIEWED}{to}"));
    } else {
        replace(&pair.main, file, from, to);
    }
}

/// AC-01: on a project with an accepted, an open and a section's card, a
/// stage changed, written, taken away, in another script, an ID and a value
/// naming nothing: every `norm_hash`, `rev` and mark as it was; no
/// `unknown-key`, no other finding; `spec flow` prints the value. M: counting
/// the key in `norm_input`.
#[test]
fn ac01_a_stage_moves_no_hash_no_revision_and_no_mark() {
    const OTHER_SCRIPT: &str = "\u{0441}\u{0431}\u{043e}\u{0440}";
    let pair = Pair::new("tv-ac01", ALMANAC);
    let confirmed = accepted(&pair, "Notice posted", &[CALL]);
    let open = cut(&pair, "Pages set", &[SET]);
    let section = accepted(&pair, "Proof pulled", &[PROOF]);
    let before = standing(&pair);
    assert!(
        before.iter().any(|row| row.contains(&confirmed)),
        "{before:?}"
    );
    assert!(before.iter().any(|row| row.contains(&open)), "{before:?}");
    assert!(
        before.iter().any(|row| row.contains(&section)),
        "{before:?}"
    );

    let cases: [(&str, &str, String, Value); 5] = [
        (
            CALL_MD,
            GATHER_STAGE,
            "stage: harvest\n".to_owned(),
            json!("harvest"),
        ),
        (READ_MD, "", "stage: print\n".to_owned(), json!("print")),
        (BIND_MD, PRINT_STAGE, String::new(), Value::Null),
        (
            SORT_MD,
            GATHER_STAGE,
            format!("stage: {OTHER_SCRIPT}\n"),
            json!(OTHER_SCRIPT),
        ),
        (
            SET_MD,
            PRINT_STAGE,
            "stage: RM-0048\n".to_owned(),
            json!("RM-0048"),
        ),
    ];
    let id_of = |file: &str| {
        [
            (CALL_MD, CALL),
            (READ_MD, READ),
            (BIND_MD, BIND),
            (SORT_MD, SORT),
            (SET_MD, SET),
        ]
        .iter()
        .find(|(path, _)| *path == file)
        .expect("a step's file")
        .1
    };
    for (file, from, to, read_as) in &cases {
        recut(&pair, file, from, to);
        assert_eq!(standing(&pair), before, "{file}: {from:?} -> {to:?}");
        let check = doc(&pair, &["check"]);
        assert_eq!(check["counts"]["errors"], json!(0), "{file}: {check}");
        assert_eq!(check["counts"]["warnings"], json!(0), "{file}: {check}");
        assert_eq!(findings_of(&pair, "unknown-key"), Vec::<Value>::new());
        let flow = doc(&pair, &["flow"]);
        assert_eq!(node_of(&flow, id_of(file))["stage"], *read_as, "{flow}");
    }
    let text = ok(&pair, &["flow"]);
    let harvest = line_of(&text, CALL);
    assert!(fields(harvest).contains(&"stage harvest"), "{harvest}");
    let bound = line_of(&text, BIND);
    assert!(
        !fields(bound)
            .iter()
            .any(|field| field.starts_with("stage ")),
        "a stage taken away prints no field: {bound}"
    );
    assert!(
        fields(line_of(&text, SORT)).contains(&format!("stage {OTHER_SCRIPT}").as_str()),
        "{text}"
    );

    // A commit holding only the stages raises nothing either: nothing is
    // listed against HEAD, in the working tree or in the git index.
    pair.git.add_all(&pair.main);
    let bump = doc(&pair, &["bump", "--staged"]);
    assert_eq!(bump["rows"], json!([]), "{bump}");
    pair.git.commit(&pair.main, "re-cut the stages");
    assert_eq!(standing(&pair), before, "committed");
}

/// "The key `stage:`": an ID in a stage is one value a project may write:
/// the document holding it gets no link, no reference and no finding, and the
/// node it names no incoming row.
#[test]
fn ac01_an_id_in_a_stage_is_only_a_word() {
    let copy = Copy::new("tv-id", ALMANAC);
    let before = copy.json(&["flow"])["edges"].clone();
    let refs = copy.ok(&["show", BIND, "--links"]);
    for value in [
        "STEP-BIND",
        "STEP-NOWHERE",
        // A Cyrillic letter in a Latin ID, which an ID is refused for.
        "\u{0421}TEP-BIND",
    ] {
        replace(
            &copy.root,
            SET_MD,
            &read_text(&copy.root, SET_MD)
                .lines()
                .find(|line| line.starts_with("stage: "))
                .map(|line| format!("{line}\n"))
                .expect("a stage line"),
            &format!("stage: {value}\n"),
        );
        let check = copy.json(&["check"]);
        assert_eq!(check["counts"]["errors"], json!(0), "{value}: {check}");
        assert_eq!(check["counts"]["warnings"], json!(0), "{value}: {check}");
        assert_eq!(copy.json(&["flow"])["edges"], before, "{value}");
        assert_eq!(copy.ok(&["show", BIND, "--links"]), refs, "{value}");
    }
}

/// "The key `stage:`": a mistyped value is `frontmatter-type` naming the file
/// and the line, as any typed key, and it stops no read: the node stands with
/// no stage.
#[test]
fn ac01_a_mistyped_stage_is_a_finding_at_its_line_and_stops_no_read() {
    let copy = Copy::new("tv-mistyped", ALMANAC);
    replace(&copy.root, CALL_MD, GATHER_STAGE, "stage: [gather]\n");
    let check = copy.run(&["check", "--json"]);
    let document = check.json();
    let found: Vec<(Value, Value, Value)> = document["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|finding| {
            (
                finding["code"].clone(),
                finding["path"].clone(),
                finding["line"].clone(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [(json!("frontmatter-type"), json!(CALL_MD), json!(8))],
        "{document}"
    );
    let flow = copy.json(&["flow"]);
    assert_eq!(node_of(&flow, CALL)["stage"], Value::Null, "{flow}");
    let trace = copy.json(&["trace"]);
    assert_eq!(node_of(&trace, CALL)["stage"], Value::Null, "{trace}");
    assert_eq!(
        lane(&trace, Some("gather"))["counts"],
        counts(1, 0, 0, 0, 0)
    );
}

/// "`spec trace`, extended": a section's `stage` is its document's, and its
/// `distance` too: documents only walk.
#[test]
fn ac01_a_section_reads_its_documents_stage() {
    let pair = Pair::new("tv-section", ALMANAC);
    cut(&pair, "Proof pulled", &[PROOF]);
    let trace = doc(&pair, &["trace"]);
    let row = node_of(&trace, PROOF);
    assert_eq!(row["stage"], "print", "{row}");
    assert_eq!(row["distance"], json!(3), "{row}");
    let print = doc(&pair, &["trace", "--stage", "print"]);
    assert_eq!(ids(&print), [BIND, SET, PROOF], "{print}");
}

// ------------------------------------------------------------------- AC-02

/// AC-02: `spec flow BOOK-ALMANAC --depth 1`: the part holding `STEP-CALL` is
/// `start` and not `end`, the one holding `STEP-BIND` `end` and not `start`;
/// the book, holding nothing at this depth, neither. At depth 0 the book holds
/// both. Unfolded the marks stand on the steps alone. Gap rows byte-equal to
/// the unfolded read. M: `steps.starts.contains(&file)` kept in `shown`.
#[test]
fn ac02_a_folded_node_is_a_start_or_an_end_by_what_it_holds() {
    let copy = Copy::new("tv-ac02", ALMANAC);
    let unfolded_text = copy.ok(&["flow", BOOK]);
    let unfolded = copy.json(&["flow", BOOK]);

    let parts = copy.json(&["flow", BOOK, "--depth", "1"]);
    assert_eq!(ids(&parts), [GATHER, PRINT, BOOK], "{parts}");
    let flags = |document: &Value, id: &str| {
        let node = node_of(document, id);
        (node["start"].clone(), node["end"].clone())
    };
    assert_eq!(
        flags(&parts, GATHER),
        (json!(true), json!(false)),
        "{parts}"
    );
    assert_eq!(flags(&parts, PRINT), (json!(false), json!(true)), "{parts}");
    assert_eq!(flags(&parts, BOOK), (json!(false), json!(false)), "{parts}");

    let text = copy.ok(&["flow", BOOK, "--depth", "1"]);
    assert_eq!(
        line_of(&text, GATHER),
        "0 PART-GATHER | committee | The gathering committee | docs/spec/part-gather.md:1 \
         | start | group BOOK-ALMANAC | holds 2, inner 0",
    );
    assert_eq!(
        line_of(&text, PRINT),
        "1 PART-PRINT | committee | The printing committee | docs/spec/part-print.md:1 \
         | end | group BOOK-ALMANAC | holds 3, inner 1",
    );
    assert_eq!(
        line_of(&text, BOOK),
        "- BOOK-ALMANAC | yearbook | The village almanac | docs/spec/book-almanac.md:1",
    );
    assert_eq!(gap_lines(&text), gap_lines(&unfolded_text));
    assert_eq!(parts["gaps"], unfolded["gaps"]);

    let whole = copy.json(&["flow", BOOK, "--depth", "0"]);
    assert_eq!(ids(&whole), [BOOK], "{whole}");
    assert_eq!(flags(&whole, BOOK), (json!(true), json!(true)), "{whole}");
    let text = copy.ok(&["flow", BOOK, "--depth", "0"]);
    assert_eq!(
        line_of(&text, BOOK),
        "0 BOOK-ALMANAC | yearbook | The village almanac | docs/spec/book-almanac.md:1 \
         | start | end | holds 7, inner 4",
    );
    assert_eq!(gap_lines(&text), gap_lines(&unfolded_text));
    assert_eq!(whole["gaps"], unfolded["gaps"]);

    assert_eq!(flags(&unfolded, CALL), (json!(true), json!(false)));
    assert_eq!(flags(&unfolded, BIND), (json!(false), json!(true)));
    for id in [BOOK, GATHER, PRINT, SORT, READ, SET] {
        assert_eq!(
            flags(&unfolded, id),
            (json!(false), json!(false)),
            "nothing folded into {id}: {unfolded}"
        );
    }
    // A depth past the deepest document folds nothing.
    assert_eq!(
        copy.ok(&["flow", BOOK, "--depth", "2"]),
        unfolded_text.replacen(
            "flow BOOK-ALMANAC | nodes 8, edges 4, depth -",
            "flow BOOK-ALMANAC | nodes 8, edges 4, depth 2",
            1
        )
    );
}

/// AC-02: a start with no mark folds too (`spec-flow.md` "Readings": `start`
/// means the walk's source, derived starts included). Under `PART-PRINT` no
/// marked start is in scope, so `STEP-SORT` is the scope's start; under
/// `PART-GATHER` no end is, and `STEP-CALL` is its marked start.
#[test]
fn ac02_a_derived_start_folds_and_a_scope_with_no_end_marks_none() {
    let copy = Copy::new("tv-ac02-scope", ALMANAC);
    let print = copy.json(&["flow", PRINT, "--depth", "0"]);
    assert_eq!(ids(&print), [PRINT], "{print}");
    assert_eq!(node_of(&print, PRINT)["start"], json!(true), "{print}");
    assert_eq!(node_of(&print, PRINT)["end"], json!(true), "{print}");
    let gather = copy.json(&["flow", GATHER, "--depth", "0"]);
    assert_eq!(node_of(&gather, GATHER)["start"], json!(true), "{gather}");
    assert_eq!(node_of(&gather, GATHER)["end"], json!(false), "{gather}");
}

/// AC-02 on a project of another nature: the lending wing folded is the walk's
/// start and its end, the whole-library read keeps them apart, and a `[flow]`
/// with no rule leaves a fold's derived start standing.
#[test]
fn ac02_the_fold_on_a_library_and_a_relay() {
    let gaps = Copy::new("tv-ac02-gaps", "flow-gaps");
    let lend = gaps.json(&["flow", "WING-LEND", "--depth", "0"]);
    assert_eq!(node_of(&lend, "WING-LEND")["start"], json!(true), "{lend}");
    assert_eq!(node_of(&lend, "WING-LEND")["end"], json!(true), "{lend}");
    let library = gaps.json(&["flow", "WING-LIBRARY", "--depth", "1"]);
    assert_eq!(
        library["gaps"],
        gaps.json(&["flow", "WING-LIBRARY"])["gaps"]
    );
    assert_eq!(
        node_of(&library, "WING-LIBRARY")["start"],
        json!(false),
        "at this depth it holds nothing: {library}"
    );
    assert_eq!(
        node_of(&library, "WING-HEADLESS")["start"],
        json!(false),
        "its steps are reached by nothing marked: {library}"
    );
}

/// AC-02 and "Flow badges": `spec flow`'s node keys, `stage` after `group`, a
/// folded node showing its own stage, never what it holds.
#[test]
fn ac02_a_folded_node_shows_its_own_stage() {
    let copy = Copy::new("tv-own-stage", ALMANAC);
    let done = copy.run(&["flow", "--json"]);
    done.code(0);
    let ordered = Ordered::of(&done.stdout);
    let Ordered::Array(nodes) = ordered.get("nodes") else {
        panic!("{}", done.stdout);
    };
    for node in nodes {
        assert_eq!(node.keys(), FLOW_NODE_KEYS, "{}", done.stdout);
    }
    let parts = copy.json(&["flow", BOOK, "--depth", "1"]);
    assert_eq!(node_of(&parts, GATHER)["stage"], Value::Null, "{parts}");
    assert_eq!(node_of(&parts, PRINT)["stage"], Value::Null, "{parts}");

    replace(
        &copy.root,
        "docs/spec/part-gather.md",
        REVIEWED,
        &format!("{REVIEWED}stage: gather\n"),
    );
    let parts = copy.json(&["flow", BOOK, "--depth", "1"]);
    assert_eq!(node_of(&parts, GATHER)["stage"], "gather", "{parts}");
    let text = copy.ok(&["flow", BOOK, "--depth", "1"]);
    assert!(
        fields(line_of(&text, GATHER)).contains(&"stage gather"),
        "{text}"
    );
}

// ------------------------------------------------------------------- AC-03

/// One row of `trace_views_before.json`.
#[derive(serde::Deserialize)]
struct Before {
    fixture: String,
    args: Vec<String>,
    text: String,
    json: String,
}

fn before_rows() -> Vec<Before> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trace_views_before.json");
    serde_json::from_slice(&fs::read(&path).expect("trace_views_before.json"))
        .expect("trace_views_before.json is JSON")
}

/// `stdout` with the three keys of this slice taken out, each `null`: what
/// the binary before it wrote. A key left with another value stays, and the
/// comparison shows it. docs/features/flow-scenario.md (RM-0094), its test
/// churn: the three flow keys of that slice are taken out first, whatever their value
/// ([`without_keys`]).
fn without_this_slice(stdout: &str) -> String {
    without_keys(stdout, &FLOW_KEYS)
        .replace("\"stage\":null,", "")
        .replace("\"rollup\":null,", "")
        .replace("\"counts\":null,", "")
        .replace(",\"rollup\":null", "")
        .replace(",\"counts\":null", "")
}

/// docs/features/flow-scope.md FS-2 (a): `before` less what read
/// `depends_on`, which no flow reads since that slice: in a text, each such
/// edge line, the header's `edges` lowered by as many; in a JSON document,
/// each object of that type with one comma. Bytes holding none are returned
/// as they are.
fn without_depends_on(before: &str) -> String {
    if before.starts_with('{') {
        let mut out = before.to_owned();
        while let Some(at) = out.find("\"type\":\"depends_on\"") {
            let start = out[..at]
                .rfind('{')
                .expect("an object opens before its type");
            let end = at + out[at..].find('}').expect("the object closes") + 1;
            if out[end..].starts_with(',') {
                out.replace_range(start..=end, "");
            } else if out[..start].ends_with(',') {
                out.replace_range(start - 1..end, "");
            } else {
                out.replace_range(start..end, "");
            }
        }
        return out;
    }
    let edge = |line: &str| line.contains(" -> ") && line.contains(" | depends_on | ");
    let removed = before.lines().filter(|line| edge(line)).count();
    let mut out = String::new();
    for (at, line) in before.split_inclusive('\n').enumerate() {
        if edge(line) {
            continue;
        }
        if at == 0 && removed > 0 {
            let (head, rest) = line.split_once(", edges ").expect("a header");
            let (count, tail) = rest.split_once(", depth ").expect("its depth");
            let count: usize = count.parse().expect("an edge count");
            out.push_str(&format!("{head}, edges {}, depth {tail}", count - removed));
        } else {
            out.push_str(line);
        }
    }
    out
}

/// AC-03: no `--trace`: the text is the bytes of the binary before this
/// slice, and the JSON too but `stage`, `rollup` and `counts` `null`, each
/// present. M: `rollup` filled without the flag. docs/features/flow-scope.md
/// FS-2 (a): the bytes before less their `depends_on` edge
/// ([`without_depends_on`]); the golden keeps what trace-views' parent
/// printed.
#[test]
fn ac03_no_flag_keeps_the_bytes_but_three_nulls() {
    let rows = before_rows();
    assert_eq!(rows.len(), 6);
    for row in rows {
        let copy = Copy::new("tv-ac03-bytes", &row.fixture);
        let args: Vec<&str> = std::iter::once("flow")
            .chain(row.args.iter().map(String::as_str))
            .collect();
        assert_eq!(
            copy.ok(&args),
            without_depends_on(&row.text),
            "{} {:?}: the text",
            row.fixture,
            row.args
        );
        let mut asked = args.clone();
        asked.push("--json");
        let done = copy.run(&asked);
        done.code(0);
        assert_eq!(
            without_this_slice(&done.stdout),
            without_depends_on(&row.json),
            "{} {:?}: the JSON",
            row.fixture,
            row.args
        );
        for node in done.json()["nodes"].as_array().expect("nodes") {
            for key in ["stage", "rollup", "counts"] {
                assert_eq!(
                    node.get(key),
                    Some(&Value::Null),
                    "{} {:?}: {key}: {node}",
                    row.fixture,
                    row.args
                );
            }
        }
    }
}

/// AC-03: a project with a queue and marks, no flag: `rollup` and `counts`
/// stay `null`, the text holds no rollup. M: `rollup` filled without the flag.
#[test]
fn ac03_no_flag_reads_no_queue() {
    let pair = Pair::new("tv-ac03-null", ALMANAC);
    accepted(&pair, "Notice posted", &[CALL]);
    cut(&pair, "Pages set", &[SET]);
    approved(&pair, CALL, CALL_MEANING);
    for args in [
        vec!["flow"],
        vec!["flow", BOOK, "--depth", "1"],
        vec!["flow", BOOK, "--depth", "0"],
    ] {
        let flow = doc(&pair, &args);
        for node in flow["nodes"].as_array().expect("nodes") {
            assert_eq!(node["rollup"], Value::Null, "{args:?}: {node}");
            assert_eq!(node["counts"], Value::Null, "{args:?}: {node}");
        }
        let text = ok(&pair, &args);
        assert!(!text.contains(" | rollup "), "{args:?}: {text}");
    }
}

/// The work of a whole cycle on the almanac: `CALL` confirmed, then its brief
/// changed: suspect; `SORT` confirmed; `READ` departed; `SET` open; `BIND` no
/// card.
fn a_cycle(pair: &Pair) {
    accepted(pair, "Notice posted", &[CALL]);
    accepted(pair, "Entries sorted", &[SORT]);
    departed(pair, "Entries read", READ);
    cut(pair, "Pages set", &[SET]);
    approved(pair, CALL, CALL_MEANING);
}

/// AC-03: `--trace`: at every depth and scope each shown node's `rollup` and
/// `counts` are `spec trace`'s, and they are what the spec's rules give; the
/// rest of the document is the bytes of the read with no flag.
#[test]
fn ac03_the_flag_gives_each_node_spec_traces_values() {
    let pair = Pair::new("tv-ac03", ALMANAC);
    a_cycle(&pair);
    let trace = doc(&pair, &["trace"]);
    for args in [
        vec!["flow"],
        vec!["flow", BOOK],
        vec!["flow", BOOK, "--depth", "1"],
        vec!["flow", BOOK, "--depth", "0"],
        vec!["flow", GATHER],
        vec!["flow", PRINT, "--depth", "0"],
    ] {
        let mut traced = args.clone();
        traced.push("--trace");
        let flow = doc(&pair, &traced);
        let nodes = flow["nodes"].as_array().expect("nodes");
        assert!(!nodes.is_empty(), "{args:?}");
        for node in nodes {
            let id = node["id"].as_str().expect("an ID");
            let row = node_of(&trace, id);
            assert_eq!(node["rollup"], row["rollup"], "{args:?}: {id}");
            assert_eq!(node["counts"], row["counts"], "{args:?}: {id}");
        }
        let mut quiet = flow.clone();
        for node in quiet["nodes"].as_array_mut().expect("nodes") {
            node["rollup"] = Value::Null;
            node["counts"] = Value::Null;
        }
        assert_eq!(quiet, doc(&pair, &args), "{args:?}: the rest of the read");
    }

    let flow = doc(&pair, &["flow", "--trace"]);
    assert_eq!(node_of(&flow, CALL)["rollup"], "suspect");
    assert_eq!(node_of(&flow, SORT)["rollup"], "confirmed");
    assert_eq!(node_of(&flow, READ)["rollup"], "departed");
    assert_eq!(node_of(&flow, SET)["rollup"], "in work");
    assert_eq!(node_of(&flow, BIND)["rollup"], "no card");

    // A group's own row has no card, so it counts nowhere; its `counts` hold
    // what it holds, so folding moves nothing and the book is its parts' sum.
    // PART-GATHER: CALL, SET. PART-PRINT: SORT, READ, BIND. The book: those.
    let unfolded = doc(&pair, &["flow", BOOK, "--trace"]);
    let parts = doc(&pair, &["flow", BOOK, "--depth", "1", "--trace"]);
    let book = doc(&pair, &["flow", BOOK, "--depth", "0", "--trace"]);
    assert_eq!(node_of(&parts, GATHER)["rollup"], "no card", "{parts}");
    assert_eq!(node_of(&parts, GATHER)["counts"], counts(0, 1, 1, 0, 0));
    assert_eq!(node_of(&parts, PRINT)["counts"], counts(1, 0, 0, 1, 1));
    assert_eq!(node_of(&book, BOOK)["counts"], counts(1, 1, 1, 1, 1));
    assert_eq!(node_of(&unfolded, GATHER)["counts"], counts(0, 1, 1, 0, 0));
    assert_eq!(node_of(&unfolded, PRINT)["counts"], counts(1, 0, 0, 1, 1));
    assert_eq!(node_of(&unfolded, BOOK)["counts"], counts(1, 1, 1, 1, 1));
}

/// AC-03: the text of `--trace`: ` | rollup <state>` on every node line and
/// no other line changed.
#[test]
fn ac03_the_flag_adds_one_field_to_a_node_line() {
    let pair = Pair::new("tv-ac03-text", ALMANAC);
    a_cycle(&pair);
    let plain = ok(&pair, &["flow", BOOK]);
    let traced = ok(&pair, &["flow", BOOK, "--trace"]);
    assert_eq!(plain.lines().count(), traced.lines().count(), "{traced}");
    for (id, state) in [
        (CALL, "suspect"),
        (SORT, "confirmed"),
        (READ, "departed"),
        (SET, "in work"),
        (BIND, "no card"),
        (GATHER, "no card"),
        (PRINT, "no card"),
        (BOOK, "no card"),
    ] {
        let line = line_of(&traced, id);
        assert!(line.contains(&format!(" | rollup {state}")), "{id}: {line}");
        assert_eq!(
            line.replacen(&format!(" | rollup {state}"), "", 1),
            line_of(&plain, id),
            "{id}: only the field"
        );
    }
    for (with, without) in traced.lines().zip(plain.lines()) {
        if !with.contains(" | rollup ") {
            assert_eq!(with, without);
        }
    }
    assert_eq!(gap_lines(&traced), gap_lines(&plain));
}

/// AC-03: `--trace` with no queue at all reads `no card` as `spec trace`
/// does, a ROOT naming nothing is exit 1 as ever, and `--depth` bad is exit 2.
#[test]
fn ac03_the_flag_takes_the_reads_exits() {
    let pair = Pair::new("tv-ac03-exits", ALMANAC);
    let flow = doc(&pair, &["flow", "--trace"]);
    for node in flow["nodes"].as_array().expect("nodes") {
        assert_eq!(node["rollup"], "no card", "{node}");
        assert_eq!(node["counts"]["confirmed"], json!(0), "{node}");
    }
    assert_eq!(
        pair.sql("SELECT name FROM sqlite_master WHERE name IN ('cards', 'proposals', 'tasks');"),
        "",
        "no queue table after a read"
    );
    let missing = run(&pair, &["--json", "flow", "STEP-NOWHERE", "--trace"]);
    missing.code(1);
    assert!(missing.json()["reason"].is_string(), "{}", missing.show());
    run(&pair, &["flow", "--trace", "--depth", "x"]).code(2);
}

/// "Rules and edge cases": `--trace` with a newer queue never fails the flow,
/// in the text and the JSON alike. What this build cannot read it does not
/// call `no card`: `rollup` and `counts` stay `null`, a note says why, and a
/// screen draws its boxes bare.
#[test]
fn ac03_a_newer_queue_gives_no_rollup_and_a_note() {
    let pair = Pair::new("tv-schema", ALMANAC);
    accepted(&pair, "Notice posted", &[CALL]);
    pair.sql("PRAGMA user_version = 9999;");
    let done = run(&pair, &["--json", "flow", "--trace"]);
    done.code(0);
    let flow = done.json();
    for node in flow["nodes"].as_array().expect("nodes") {
        assert_eq!(node["rollup"], Value::Null, "{node}");
        assert_eq!(node["counts"], Value::Null, "{node}");
    }
    let notes = flow["notes"].as_array().expect("notes");
    assert_eq!(notes.len(), 1, "{flow}");
    let text = run(&pair, &["flow", "--trace"]);
    text.code(0);
    assert!(
        !line_of(&text.stdout, CALL).contains(" | rollup "),
        "{}",
        text.show()
    );
    assert!(
        text.stderr.contains(notes[0].as_str().expect("a note")),
        "the note reaches a person: {}",
        text.show()
    );
    // No flag: no queue read, no note.
    let plain = run(&pair, &["--json", "flow"]);
    plain.code(0);
    assert_eq!(plain.json()["notes"], json!([]), "{}", plain.show());
}

/// "Rules and edge cases", the other way: a queue from before the cards reads
/// `no card` and a note, and one a step behind reads its cards with a note:
/// neither fails the flow or runs a schema step.
#[test]
fn ac03_an_older_queue_is_read_as_it_stands() {
    let pair = Pair::new("tv-schema-old", ALMANAC);
    accepted(&pair, "Notice posted", &[CALL]);
    pair.sql("PRAGMA user_version = 8;");
    let done = run(&pair, &["--json", "flow", "--trace"]);
    done.code(0);
    let flow = done.json();
    assert_eq!(node_of(&flow, CALL)["rollup"], "confirmed", "{flow}");
    assert_eq!(flow["notes"].as_array().map(Vec::len), Some(1), "{flow}");

    pair.sql("DROP TABLE cards; PRAGMA user_version = 3;");
    let done = run(&pair, &["--json", "flow", "--trace"]);
    done.code(0);
    let flow = done.json();
    assert_eq!(node_of(&flow, CALL)["rollup"], "no card", "{flow}");
    assert_eq!(flow["notes"].as_array().map(Vec::len), Some(1), "{flow}");
    assert_eq!(
        pair.sql("PRAGMA user_version;").trim(),
        "3",
        "no schema step"
    );
}

/// AC-03 on projects of other natures, a queue or none: each node's values
/// are `spec trace`'s, by ID or by path; a card on a section reaches its
/// document's row; no project and no document needs a word of the core's.
#[test]
fn ac03_the_flag_on_every_project() {
    for name in [
        "flow-relay",
        "flow-hire",
        "flow-gaps",
        "spec-c",
        "card-press",
    ] {
        let copy = Copy::new("tv-ac03-genre", name);
        let trace = copy.json(&["trace"]);
        let flow = copy.json(&["flow", "--trace"]);
        for node in flow["nodes"].as_array().expect("nodes") {
            let shown = node["id"].as_str().map_or_else(
                || node["path"].as_str().expect("a path").to_owned(),
                str::to_owned,
            );
            let row = node_of(&trace, &shown);
            assert_eq!(node["rollup"], row["rollup"], "{name}: {shown}");
            assert_eq!(node["counts"], row["counts"], "{name}: {shown}");
        }
    }
    let pair = Pair::new("tv-ac03-section", ALMANAC);
    cut(&pair, "Proof pulled", &[PROOF]);
    let flow = doc(&pair, &["flow", "--trace"]);
    assert_eq!(node_of(&flow, SET)["rollup"], "in work", "{flow}");
    let empty = Copy::new("tv-ac03-empty", ALMANAC);
    for file in md_files(&empty.root.join("docs")) {
        fs::remove_file(empty.root.join("docs").join(file)).expect("a document");
    }
    assert_eq!(empty.json(&["flow", "--trace"])["nodes"], json!([]));
}

/// AC-03 past the cap: a node line's rollup counts against it, the text stays
/// within `OUTPUT_CAP_CHARS` and a tail, JSON holds the printed rows, each
/// with its badge, and the gap rows are whole.
#[test]
fn ac03_the_flag_stays_within_the_cap() {
    const FILES: usize = 1_400;
    let copy = Copy::new("tv-ac03-cap", ALMANAC);
    copy.configure("\n[[flow.step]]\nkinds = [\"errand\"]\n");
    for number in 0..FILES {
        write(
            &copy.root,
            &format!("docs/spec/bulk-{number:04}.md"),
            plain_document(&format!("STEP-BULK{number}"), "gather"),
        );
    }
    let text = copy.ok(&["flow", "--trace"]);
    let lines: Vec<&str> = text.lines().collect();
    let tail = lines.last().expect("a tail");
    assert!(tail.starts_with("[truncated: "), "{tail}");
    assert!(
        text.chars().count() - tail.chars().count() <= specengine_cli::OUTPUT_CAP_CHARS,
        "{} characters",
        text.chars().count()
    );
    assert_eq!(gap_lines(&text).len(), 10, "{tail}");
    let flow = copy.json(&["flow", "--trace"]);
    assert_eq!(flow["truncated"], json!(true), "{tail}");
    let nodes = flow["nodes"].as_array().expect("nodes");
    assert!(nodes.len() < 1_408, "{}", nodes.len());
    assert!(
        nodes.iter().all(|node| node["rollup"] == "no card"),
        "every printed row its badge"
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains(" | rollup "))
            .count(),
        nodes.len(),
        "text and JSON print alike"
    );
}

// ------------------------------------------------------------------- AC-06

/// AC-06, the read: a step with no card stands in its lane as `no card`, and
/// the last card's accept moves the lane to all confirmed: `gather` holds
/// two steps, `0 of 2`, `1 of 2`, `2 of 2`. M: hiding `no_card` steps.
#[test]
fn ac06_a_step_with_no_card_keeps_its_place_and_the_lane_fills() {
    let pair = Pair::new("tv-ac06", ALMANAC);
    let rows = |pair: &Pair| -> Vec<(Value, Value)> {
        let trace = doc(pair, &["trace", "--stage", "gather"]);
        ids(&trace)
            .into_iter()
            .map(|id| {
                let node = node_of(&trace, &id);
                (json!(id), node["rollup"].clone())
            })
            .collect()
    };
    let gather = |pair: &Pair| doc(pair, &["trace"])["stages"][0].clone();
    assert_eq!(gather(&pair)["stage"], "gather");
    assert_eq!(gather(&pair)["counts"], counts(2, 0, 0, 0, 0));
    assert_eq!(
        rows(&pair),
        [
            (json!(CALL), json!("no card")),
            (json!(SORT), json!("no card"))
        ]
    );

    accepted(&pair, "Notice posted", &[CALL]);
    assert_eq!(gather(&pair)["counts"], counts(1, 0, 0, 0, 1));
    assert_eq!(
        rows(&pair),
        [
            (json!(CALL), json!("confirmed")),
            (json!(SORT), json!("no card"))
        ]
    );

    let last = cut(&pair, "Entries sorted", &[SORT]);
    assert_eq!(gather(&pair)["counts"], counts(0, 1, 0, 0, 1));
    to_review(&pair, &last);
    accept(
        &pair,
        &last,
        CardAcceptRequest {
            briefed: true,
            ..CardAcceptRequest::default()
        },
    );
    assert_eq!(gather(&pair)["counts"], counts(0, 0, 0, 0, 2));
}

// ------------------------------------------------------------------- AC-07

/// AC-07: `spec trace` whole, hand-worked: the header over every document but
/// the holders with no card (trace-truth AC-07), the lanes by least `distance`,
/// the `null` lane last though it stands before `print` in the walk, a `no card`
/// step counted and the book and its parts in no lane, the rows by path.
#[test]
fn ac07_the_stage_lines_follow_the_summary_line() {
    let copy = Copy::new("tv-ac07-lines", ALMANAC);
    assert_eq!(
        copy.ok(&["trace"]),
        "trace all | nodes 8, cards 0 | no card 5, in work 0, suspect 0, departed 0, confirmed 0\n\
         stage gather | distance 0 | no card 2, in work 0, suspect 0, departed 0, confirmed 0\n\
         stage print | distance 3 | no card 2, in work 0, suspect 0, departed 0, confirmed 0\n\
         stage - | distance 2 | no card 1, in work 0, suspect 0, departed 0, confirmed 0\n\
         no card | BOOK-ALMANAC | The village almanac | docs/spec/book-almanac.md | cards 0\n\
         no card | PART-GATHER | The gathering committee | docs/spec/part-gather.md | cards 0\n\
         no card | PART-PRINT | The printing committee | docs/spec/part-print.md | cards 0\n\
         no card | STEP-BIND | Bind the copies | docs/spec/step-bind.md | cards 0\n\
         no card | STEP-CALL | Call for entries | docs/spec/step-call.md | cards 0\n\
         no card | STEP-READ | Read the entries aloud | docs/spec/step-read.md | cards 0\n\
         no card | STEP-SET | Set the pages | docs/spec/step-set.md | cards 0\n\
         no card | STEP-SORT | Sort the entries | docs/spec/step-sort.md | cards 0\n"
    );
    let trace = copy.json(&["trace"]);
    for row in trace["stages"].as_array().expect("stages") {
        let keys: Vec<&String> = row.as_object().expect("a lane").keys().collect();
        assert_eq!(keys.len(), STAGE_KEYS.len(), "{row}");
        for key in STAGE_KEYS {
            assert!(row.get(key).is_some(), "{key}: {row}");
        }
        for key in COUNT_KEYS {
            assert!(row["counts"].get(key).is_some(), "{key}: {row}");
        }
    }
    assert_eq!(
        trace["stages"],
        json!([
            {"stage": "gather", "distance": 0, "counts": counts(2, 0, 0, 0, 0)},
            {"stage": "print", "distance": 3, "counts": counts(2, 0, 0, 0, 0)},
            {"stage": null, "distance": 2, "counts": counts(1, 0, 0, 0, 0)},
        ])
    );
    for (id, stage, distance) in [
        (BOOK, Value::Null, Value::Null),
        (GATHER, Value::Null, Value::Null),
        (PRINT, Value::Null, Value::Null),
        (CALL, json!("gather"), json!(0)),
        (SORT, json!("gather"), json!(1)),
        (READ, Value::Null, json!(2)),
        (SET, json!("print"), json!(3)),
        (BIND, json!("print"), json!(4)),
    ] {
        let node = node_of(&trace, id);
        assert_eq!(node["stage"], stage, "{node}");
        assert_eq!(node["distance"], distance, "{node}");
    }
}

/// AC-07: `--stage print` lists its two steps, in two parts; `--stage gather`
/// likewise, crossing the other way; `--stage -` the unstaged step alone: the
/// book and its parts are no steps. The stage lines stay whole under every
/// cut, and a filter leaves the header whole.
#[test]
fn ac07_stage_keeps_one_stages_rows_across_the_parts() {
    let copy = Copy::new("tv-ac07-stage", ALMANAC);
    let whole = copy.json(&["trace"]);
    let print = copy.json(&["trace", "--stage", "print"]);
    assert_eq!(ids(&print), [BIND, SET], "{print}");
    let groups: Vec<Value> = ids(&print)
        .iter()
        .map(|id| node_of(&copy.json(&["flow"]), id)["group"].clone())
        .collect();
    assert_eq!(groups, [json!(PRINT), json!(GATHER)], "across both parts");

    let gather = copy.json(&["trace", "--stage", "gather"]);
    assert_eq!(ids(&gather), [CALL, SORT], "{gather}");

    let none = copy.json(&["trace", "--stage", "-"]);
    assert_eq!(ids(&none), [READ], "{none}");

    assert_eq!(print["stages"], whole["stages"], "always whole");
    assert_eq!(none["stages"], whole["stages"], "always whole");
    assert_eq!(print["counts"], whole["counts"], "the header stays whole");
    assert_eq!(none["counts"], whole["counts"], "the header stays whole");
    let text = copy.ok(&["trace", "--stage", "print"]);
    assert_eq!(stage_lines(&text).len(), 3, "{text}");
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("no card | "))
            .count(),
        2,
        "{text}"
    );

    let nothing = copy.json(&["trace", "--stage", "nowhere"]);
    assert_eq!(ids(&nothing), Vec::<String>::new(), "{nothing}");
    assert_eq!(nothing["stages"], whole["stages"]);
}

/// AC-07: `--summary` prints no node row, in the text and the JSON; the
/// summary line and the stage lines stay whole, and `--stage` beside it adds
/// nothing back.
#[test]
fn ac07_summary_prints_no_node_row() {
    let pair = Pair::new("tv-ac07-summary", ALMANAC);
    a_cycle(&pair);
    let whole = doc(&pair, &["trace"]);
    let summary = doc(&pair, &["trace", "--summary"]);
    assert_eq!(summary["nodes"], json!([]), "{summary}");
    assert_eq!(summary["stages"], whole["stages"]);
    assert_eq!(summary["counts"], whole["counts"]);
    assert_eq!(summary["truncated"], json!(false));
    assert_eq!(summary["omitted"], Value::Null);

    let text = ok(&pair, &["trace", "--summary"]);
    let plain = ok(&pair, &["trace"]);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "{text}");
    assert!(plain.starts_with(&text), "{text}\nof\n{plain}");

    let both = doc(&pair, &["trace", "--summary", "--stage", "print"]);
    assert_eq!(both["nodes"], json!([]), "{both}");
    assert_eq!(both["stages"], whole["stages"]);
}

/// AC-07: `stages` by (`distance`, unreached last, `stage`), the `null` stage
/// last: a stage's place is its least step, not its name; two unreached
/// stages by name, after every reached one and before the `null` lane.
#[test]
fn ac07_lanes_sort_by_their_least_step() {
    let copy = Copy::new("tv-ac07-order", ALMANAC);
    // `STEP-CALL` into `print`: `print` now least at 0, `gather` at 1.
    replace(&copy.root, CALL_MD, GATHER_STAGE, PRINT_STAGE);
    write(
        &copy.root,
        "docs/spec/step-zinc.md",
        plain_document("STEP-ZINC", "zinc"),
    );
    write(
        &copy.root,
        "docs/spec/step-attic.md",
        plain_document("STEP-ATTIC", "attic"),
    );
    let trace = copy.json(&["trace"]);
    assert_eq!(
        trace["stages"],
        json!([
            {"stage": "print", "distance": 0, "counts": counts(3, 0, 0, 0, 0)},
            {"stage": "gather", "distance": 1, "counts": counts(1, 0, 0, 0, 0)},
            {"stage": "attic", "distance": null, "counts": counts(1, 0, 0, 0, 0)},
            {"stage": "zinc", "distance": null, "counts": counts(1, 0, 0, 0, 0)},
            {"stage": null, "distance": 2, "counts": counts(1, 0, 0, 0, 0)},
        ]),
        "{trace}"
    );
}

/// AC-07: a card and a mark count where their step stands: `stage` and
/// `counts` of each lane over the cycle, hand-worked. The lanes now sum to the
/// summary line (trace-truth AC-07): what stands in none, the book and two
/// parts, has no card and is counted nowhere.
#[test]
fn ac07_a_lane_counts_its_nodes_rollups() {
    let pair = Pair::new("tv-ac07-counts", ALMANAC);
    a_cycle(&pair);
    let trace = doc(&pair, &["trace"]);
    assert_eq!(
        trace["stages"],
        json!([
            {"stage": "gather", "distance": 0, "counts": counts(0, 0, 1, 0, 1)},
            {"stage": "print", "distance": 3, "counts": counts(1, 1, 0, 0, 0)},
            {"stage": null, "distance": 2, "counts": counts(0, 0, 0, 1, 0)},
        ]),
        "{trace}"
    );
    assert_eq!(trace["counts"], counts(1, 1, 1, 1, 1), "{trace}");
    for key in COUNT_KEYS {
        let sum: u64 = trace["stages"]
            .as_array()
            .expect("stages")
            .iter()
            .map(|row| row["counts"][key].as_u64().expect("a count"))
            .sum();
        assert_eq!(json!(sum), trace["counts"][key], "{key}: {trace}");
    }
}

/// The owner's reading of a lane: a group with no stage stands in none, one
/// carrying a stage stands in its lane; a section stands where its document
/// does, in the steps' lane or in none. What a step is stays the project's: a
/// `[[flow.step]]` rule puts the parts in the last lane, and no core word does.
#[test]
fn ac07_a_group_stands_in_a_lane_only_by_its_own_stage() {
    let pair = Pair::new("tv-ac07-group", ALMANAC);
    replace(
        &pair.main,
        READ_MD,
        "and only there.\n",
        "and only there.\n\n## The minutes {#STEP-READ-MINUTES}\n\nA clerk takes them.\n",
    );
    replace(
        &pair.main,
        "docs/spec/part-print.md",
        "reads them out and binds.\n",
        "reads them out and binds.\n\n## The press {#PART-PRINT-PRESS}\n\nIt stands in the hall.\n",
    );
    cut(&pair, "Minutes taken", &["STEP-READ-MINUTES"]);
    cut(&pair, "Press oiled", &["PART-PRINT-PRESS"]);
    let none = doc(&pair, &["trace", "--stage", "-"]);
    assert_eq!(ids(&none), [READ, "STEP-READ-MINUTES"], "{none}");
    let trace = doc(&pair, &["trace"]);
    assert!(
        ids(&trace).contains(&"PART-PRINT-PRESS".to_owned()),
        "a row, in no lane: {trace}"
    );
    assert_eq!(
        lane(&trace, None)["counts"],
        counts(0, 2, 0, 0, 0),
        "the step and its section: {trace}"
    );
    assert_eq!(
        trace["counts"],
        counts(4, 4, 0, 0, 0),
        "ten rows: PART-PRINT, a holder with a card, READ and their sections in work; BIND, \
         CALL, SET, SORT no card; the book and PART-GATHER counted nowhere: {trace}"
    );

    let copy = Copy::new("tv-ac07-group-stage", ALMANAC);
    replace(
        &copy.root,
        "docs/spec/part-gather.md",
        REVIEWED,
        &format!("{REVIEWED}stage: gather\n"),
    );
    let staged = copy.json(&["trace", "--stage", "gather"]);
    assert_eq!(ids(&staged), [GATHER, CALL, SORT], "{staged}");
    assert_eq!(
        lane(&staged, Some("gather"))["counts"],
        counts(2, 0, 0, 0, 0),
        "a holder stands in the lane and counts nowhere (trace-truth): {staged}"
    );
    assert_eq!(
        lane(&staged, Some("gather"))["distance"],
        json!(0),
        "the group's own `null` moves no lane"
    );

    let ruled = Copy::new("tv-ac07-group-rule", ALMANAC);
    ruled.configure("\n[[flow.step]]\nkinds = [\"committee\"]\n");
    let none = ruled.json(&["trace", "--stage", "-"]);
    assert_eq!(ids(&none), [GATHER, PRINT, READ], "{none}");
    assert_eq!(lane(&none, None)["counts"], counts(1, 0, 0, 0, 0), "{none}");
    assert_eq!(lane(&none, None)["distance"], json!(2), "{none}");
}

/// AC-07: a REF narrows the rows as it did, `--stage` inside it; a REF naming
/// nothing is exit 1 under `--stage` and `--summary`; usage is exit 2.
#[test]
fn ac07_stage_beside_a_ref_and_the_exits() {
    let copy = Copy::new("tv-ac07-ref", ALMANAC);
    let held = copy.json(&["trace", GATHER, "--stage", "print"]);
    assert_eq!(ids(&held), [SET], "{held}");
    let inside = copy.json(&["trace", PRINT, "--stage", "print"]);
    assert_eq!(ids(&inside), [BIND], "{inside}");
    // Under a REF the walk is that scope's, as `spec flow`'s under a ROOT: the
    // lanes and a node's `distance` read the read they stand in.
    for reference in [BOOK, GATHER, PRINT, SET] {
        let trace = copy.json(&["trace", reference]);
        let flow = copy.json(&["flow", reference]);
        for node in flow["nodes"].as_array().expect("nodes") {
            let id = node["id"].as_str().expect("an ID");
            assert_eq!(
                node_of(&trace, id)["distance"],
                node["distance"],
                "{reference}: {id}"
            );
        }
    }
    for args in [
        vec!["trace", "STEP-NOWHERE", "--stage", "print", "--json"],
        vec!["trace", "STEP-NOWHERE", "--summary", "--json"],
    ] {
        let done = copy.run(&args);
        done.code(1);
        assert!(done.json()["reason"].is_string(), "{}", done.show());
    }
    copy.run(&["trace", "--stage"]).code(2);
}

/// "`spec trace`, extended" with the cut: past the cap the stage lines and the
/// summary line stay whole, the rows are what is cut, the text stays within the
/// cap; `--summary` has nothing to cut; `--stage` cuts what it keeps.
#[test]
fn ac07_the_cut_leaves_the_lanes_whole() {
    const FILES: usize = 1_400;
    let copy = Copy::new("tv-ac07-cut", ALMANAC);
    for number in 0..FILES {
        write(
            &copy.root,
            &format!("docs/spec/bulk-{number:04}.md"),
            plain_document(
                &format!("STEP-BULK{number}"),
                ["gather", "print"][number % 2],
            ),
        );
    }
    let whole = copy.json(&["trace"]);
    assert_eq!(whole["truncated"], json!(true), "past the cap");
    assert_eq!(
        lane(&whole, Some("gather"))["counts"],
        counts(702, 0, 0, 0, 0),
        "{}",
        whole["stages"]
    );
    assert_eq!(
        lane(&whole, Some("print"))["counts"],
        counts(702, 0, 0, 0, 0)
    );
    assert_eq!(lane(&whole, None)["counts"], counts(1, 0, 0, 0, 0));
    let shown = whole["nodes"].as_array().expect("nodes").len();
    assert!(shown < 1_408, "{shown}");
    assert_eq!(whole["omitted"], json!(1_408 - shown));
    assert_eq!(
        whole["counts"],
        counts(1_405, 0, 0, 0, 0),
        "the header whole, the book and its parts counted nowhere"
    );

    let text = copy.ok(&["trace"]);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(stage_lines(&text).len(), 3, "{}", lines[0]);
    assert!(lines[1].starts_with("stage gather | "), "{}", lines[1]);
    assert!(lines[3].starts_with("stage - | "), "{}", lines[3]);
    assert!(
        lines.last().expect("a tail").starts_with("[truncated: "),
        "{}",
        lines.last().expect("a tail")
    );
    let summary = copy.json(&["trace", "--summary"]);
    assert_eq!(summary["truncated"], json!(false), "no row to cut");
    assert_eq!(summary["omitted"], Value::Null);
    assert_eq!(summary["stages"], whole["stages"]);
    let print = copy.json(&["trace", "--stage", "print"]);
    assert_eq!(print["stages"], whole["stages"]);
    assert!(
        print["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .all(|node| node["stage"] == "print"),
        "the cut keeps the stage's rows"
    );
}

/// AC-07: two file orders, byte-identical, and a repeat run too, over every
/// form of the read.
#[test]
fn ac07_two_file_orders_are_byte_identical() {
    let scratch = Scratch::new("tv-ac07-orders");
    let (home_a, home_b) = (scratch.home("a"), scratch.home("b"));
    let forward = written_in_order(&scratch, "forward", &home_a, false);
    let reverse = written_in_order(&scratch, "other/reverse", &home_b, true);
    for args in [
        vec!["trace"],
        vec!["--json", "trace"],
        vec!["trace", "--stage", "print"],
        vec!["--json", "trace", "--stage", "print"],
        vec!["trace", "--stage", "-"],
        vec!["trace", "--summary"],
        vec!["--json", "trace", "--summary"],
        vec!["trace", BOOK],
        vec!["flow"],
        vec!["--json", "flow"],
        vec!["flow", "--trace"],
        vec!["--json", "flow", "--trace"],
        vec!["flow", BOOK, "--depth", "1"],
        vec!["--json", "flow", BOOK, "--depth", "1", "--trace"],
    ] {
        let one = spec30(&home_a, &forward, &args);
        let other = spec30(&home_b, &reverse, &args);
        one.code(0);
        assert_eq!(
            (&one.stdout, &one.stderr),
            (&other.stdout, &other.stderr),
            "{args:?}"
        );
        let again = spec30(&home_a, &forward, &args);
        assert_eq!(
            (&one.stdout, &one.stderr),
            (&again.stdout, &again.stderr),
            "{args:?} repeated"
        );
        assert!(
            !one.stdout.contains(forward.to_str().expect("UTF-8 root")),
            "no absolute root: {}",
            one.show()
        );
    }
}

/// A copy of the almanac whose documents are written and indexed one at a
/// time, forward or reversed, so its database holds rows in that order.
fn written_in_order(scratch: &Scratch, dir: &str, home: &Path, reverse: bool) -> PathBuf {
    let source = scratch.dir(&format!("source-{dir}"));
    common::copy_dir(&fixture(ALMANAC), &source);
    let root = scratch.dir(dir);
    write(&root, "specengine.toml", read(&source, "specengine.toml"));
    let mut files: Vec<String> = md_files(&source.join("docs"))
        .into_iter()
        .map(|file| format!("docs/{file}"))
        .collect();
    if reverse {
        files.reverse();
    }
    index(home, &root);
    for file in files {
        write(&root, &file, read(&source, &file));
        index(home, &root);
    }
    root
}

// ----------------------------------------------- "States": a bad `[flow]`

/// "States": a bad `[flow]`: `distance` null, lanes by name, the `null` lane
/// last, and a note naming the config's line; exit 0. The table's own error
/// stays beside each read, never a failed trace.
#[test]
fn a_bad_flow_table_gives_lanes_by_name() {
    for (extra, line) in [
        ("\n[[flow.step]]\nkinds = 3\n", "specengine.toml:"),
        ("\n[flow]\nflavour = 1\n", "specengine.toml:"),
    ] {
        let copy = Copy::new("tv-bad-flow", ALMANAC);
        copy.configure(extra);
        let trace = copy.json(&["trace"]);
        assert_eq!(
            trace["stages"],
            json!([
                {"stage": "gather", "distance": null, "counts": counts(2, 0, 0, 0, 0)},
                {"stage": "print", "distance": null, "counts": counts(2, 0, 0, 0, 0)},
                {"stage": null, "distance": null, "counts": counts(1, 0, 0, 0, 0)},
            ]),
            "{extra}: {trace}"
        );
        for node in trace["nodes"].as_array().expect("nodes") {
            assert_eq!(node["distance"], Value::Null, "{extra}: {node}");
        }
        let notes = trace["notes"].as_array().expect("notes");
        assert!(
            notes
                .iter()
                .any(|note| note.as_str().is_some_and(|note| note.contains(line))),
            "{extra}: {trace}"
        );
    }
}

/// "States": no `stage:` anywhere, on projects of several natures: flow steps
/// make one lane, the `null` one, of the steps alone; no step and no stage make
/// none, `stages` `[]` and no stage line. No document at all answers.
#[test]
fn no_stage_anywhere_is_one_lane_of_steps_or_none() {
    for (name, steps) in [
        ("flow-hire", Some(6)),
        ("flow-relay", Some(6)),
        ("spec-a", None),
        ("spec-b", None),
        ("spec-c", None),
        ("card-press", None),
    ] {
        let copy = Copy::new("tv-genre", name);
        let trace = copy.json(&["trace"]);
        let stages = trace["stages"].as_array().expect("stages");
        let text = copy.ok(&["trace", "--summary"]);
        match steps {
            Some(steps) => {
                assert_eq!(stages.len(), 1, "{name}: {trace}");
                assert_eq!(stages[0]["stage"], Value::Null, "{name}: {trace}");
                assert_eq!(
                    stages[0]["counts"],
                    counts(steps, 0, 0, 0, 0),
                    "{name}: {trace}"
                );
                assert_eq!(stage_lines(&text).len(), 1, "{name}: {text}");
                assert!(stage_lines(&text)[0].starts_with("stage - | "), "{text}");
                assert_eq!(
                    ids(&copy.json(&["trace", "--stage", "-"])).len(),
                    usize::try_from(steps).expect("a count"),
                    "{name}"
                );
            }
            None => {
                assert_eq!(trace["stages"], json!([]), "{name}: {trace}");
                assert_eq!(stage_lines(&text), Vec::<&str>::new(), "{name}: {text}");
                assert_eq!(text.lines().count(), 1, "the header alone: {text}");
                assert_eq!(ids(&copy.json(&["trace", "--stage", "-"])).len(), 0);
            }
        }
        assert_eq!(ids(&copy.json(&["trace", "--stage", "gather"])).len(), 0);
    }
    let scratch = Scratch::new("tv-empty");
    let root = scratch.dir("empty");
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"empty-almanac\"\n",
    );
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let done = spec30(
        &home,
        &cwd,
        &[
            "trace",
            "--json",
            "--root",
            root.to_str().expect("UTF-8 root"),
        ],
    );
    done.code(0);
    assert_eq!(done.json()["stages"], json!([]), "{}", done.show());
}

/// A stage is a word of the project, in any script: `--stage` compares it
/// exactly; the text prints it; JSON keeps it as written. A control character
/// never forges a line: `spec trace` escapes it, as a title's.
#[test]
fn a_stage_in_any_script_and_with_a_control_character() {
    const OTHER_SCRIPT: &str = "\u{043f}\u{0435}\u{0447}\u{0430}\u{0442}\u{044c}";
    let copy = Copy::new("tv-script", ALMANAC);
    replace(
        &copy.root,
        CALL_MD,
        GATHER_STAGE,
        &format!("stage: {OTHER_SCRIPT}\n"),
    );
    let trace = copy.json(&["trace", "--stage", OTHER_SCRIPT]);
    assert_eq!(ids(&trace), [CALL], "{trace}");
    assert_eq!(lane(&trace, Some(OTHER_SCRIPT))["distance"], json!(0));
    let text = copy.ok(&["trace", "--summary"]);
    assert!(
        stage_lines(&text)
            .iter()
            .any(|line| line.starts_with(&format!("stage {OTHER_SCRIPT} | "))),
        "{text}"
    );

    replace(
        &copy.root,
        CALL_MD,
        &format!("stage: {OTHER_SCRIPT}\n"),
        "stage: \"gather\\nstage forged | distance 0\\u0007\"\n",
    );
    let lines = copy.ok(&["trace", "--summary"]);
    assert_eq!(stage_lines(&lines).len(), 4, "{lines}");
    assert!(!lines.contains('\u{7}'), "{lines:?}");
    assert!(
        !lines.lines().any(|line| line.starts_with("stage forged")),
        "{lines}"
    );
    let flow = copy.ok(&["flow"]);
    assert!(
        !flow.lines().any(|line| line.starts_with("stage forged")),
        "{flow}"
    );
    assert_eq!(
        copy.json(&["flow"])["nodes"][0]["stage"],
        "gather\nstage forged | distance 0\u{7}"
    );
}

// ------------------------------------------------------------------- AC-09

/// A card's row of `spec card list --json`.
fn row_of<'a>(list: &'a Value, id: &str) -> &'a Value {
    list["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["id"] == id)
        .unwrap_or_else(|| panic!("no row {id}: {list}"))
}

/// Card `id` on node `node` of a `spec trace` document.
fn trace_card<'a>(trace: &'a Value, node: &str, id: &str) -> &'a Value {
    node_of(trace, node)["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["id"] == id)
        .unwrap_or_else(|| panic!("no card {id} on {node}: {trace}"))
}

/// AC-09: `list --marks` equals `spec trace`'s marks, on every mark: open,
/// accepted, suspect open or accepted, departed, correcting and the card
/// correcting it. Without the flag `mark` is `null`, each row keeps its other
/// bytes, and the exit stays 0. M: a tile drawn without its mark is the UI's.
#[test]
fn ac09_list_marks_equals_the_traces_marks() {
    let pair = Pair::new("tv-ac09", ALMANAC);
    let open = cut(&pair, "Entries sorted", &[SORT]);
    let clean = accepted(&pair, "Copies bound", &[BIND]);
    let suspect_open = cut(&pair, "Pages set", &[SET]);
    let suspect_accepted = accepted(&pair, "Notice posted", &[CALL]);
    let leaving = departed(&pair, "Entries read", READ);
    let corrected = accepted(&pair, "Proof pulled", &[PROOF]);
    approved(&pair, SET, SET_MEANING);
    approved(&pair, CALL, CALL_MEANING);
    approved(&pair, PROOF, PROOF_MEANING);
    let correction = needs_correction(&pair, &corrected);

    let marked = doc(&pair, &["card", "list", "--marks"]);
    let plain = doc(&pair, &["card", "list"]);
    let trace = doc(&pair, &["trace"]);
    for (id, node, word) in [
        (&open, SORT, "open"),
        (&clean, BIND, "accepted"),
        (&suspect_open, SET, "suspect"),
        (&suspect_accepted, CALL, "suspect"),
        (&leaving, READ, "departed"),
        (&corrected, PROOF, "correcting"),
        (&correction, PROOF, "open"),
    ] {
        let row = row_of(&marked, id);
        assert_eq!(row["mark"], word, "{id}: {row}");
        let card = trace_card(&trace, node, id);
        for key in MARK_KEYS {
            assert_eq!(row[key], card[key], "{id}: {key}");
            assert_eq!(row_of(&plain, id)[key], Value::Null, "{id}: {key}");
        }
    }
    assert_eq!(
        row_of(&marked, &suspect_open)["on"],
        SET,
        "the tile names the target"
    );
    assert!(
        row_of(&marked, &leaving)["discrepancy"].is_string(),
        "the tile names the discrepancy: {marked}"
    );
    assert_eq!(
        row_of(&marked, &corrected)["correction"],
        correction.as_str(),
        "the tile names the correction"
    );
    assert_eq!(
        marked["cards"].as_array().map(Vec::len),
        Some(7),
        "{marked}"
    );
    let mut quiet = marked.clone();
    for row in quiet["cards"].as_array_mut().expect("cards") {
        for key in MARK_KEYS {
            row[key] = Value::Null;
        }
    }
    assert_eq!(quiet, plain, "the list but its marks");

    // The text adds one field, the mark as `spec trace` prints it.
    let text = ok(&pair, &["card", "list", "--marks"]);
    let plain_text = ok(&pair, &["card", "list"]);
    for (id, printed) in [
        (open.clone(), "open".to_owned()),
        (clean.clone(), "accepted".to_owned()),
        (suspect_open.clone(), format!("suspect {SET}")),
        (corrected.clone(), format!("correcting {correction}")),
    ] {
        let line = text
            .lines()
            .find(|line| line.starts_with(&format!("{id} | ")))
            .unwrap_or_else(|| panic!("{id}: {text}"));
        let bare = plain_text
            .lines()
            .find(|line| line.starts_with(&format!("{id} | ")))
            .unwrap_or_else(|| panic!("{id}: {plain_text}"));
        assert_eq!(line, format!("{bare} | mark {printed}"), "{id}");
    }
    assert!(!plain_text.contains(" | mark "), "{plain_text}");
}

/// AC-09 and "The board tile": a card on a gone target reads `suspect` on the
/// list, where `spec trace` has no node to stand it on and says it in a note.
/// A card with no target keeps its plain word. M: the tile read from `GET
/// trace`.
#[test]
fn ac09_a_gone_target_is_marked_on_the_list() {
    let pair = Pair::new("tv-gone", ALMANAC);
    let gone = accepted(&pair, "Proof pulled", &[PROOF]);
    let manual = cut(&pair, "Order paper", &[]);
    let body = read_text(&pair.main, SET_MD);
    let (head, _) = body
        .split_once("## The proof sheet")
        .expect("the proof section");
    write(&pair.main, SET_MD, head.as_bytes());
    let list = doc(&pair, &["card", "list", "--marks"]);
    assert_eq!(row_of(&list, &gone)["mark"], "suspect", "{list}");
    assert_eq!(row_of(&list, &manual)["mark"], "open", "{list}");
    let trace = doc(&pair, &["trace"]);
    assert!(
        !ids(&trace).contains(&PROOF.to_owned()),
        "no node to stand on: {trace}"
    );
    assert!(
        trace["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|note| note.as_str().is_some_and(|note| note.contains(&gone))),
        "a note says it: {trace}"
    );
}

/// "The board tile": no marks, no tiles: an index that cannot be read fails
/// `list --marks` with the store's line, exit 2 and no row, where the plain
/// list, which reads no index, answers; a board with no target reads none.
#[test]
fn ac09_an_index_that_cannot_be_read_draws_no_row() {
    let pair = Pair::new("tv-no-index", ALMANAC);
    cut(&pair, "Notice posted", &[CALL]);
    pair.sql("ALTER TABLE files RENAME TO files_gone;");
    let marked = run(&pair, &["card", "list", "--marks"]);
    marked.code(2);
    assert_eq!(marked.stdout, "", "{}", marked.show());
    assert!(marked.stderr.contains("index"), "{}", marked.show());
    let plain = run(&pair, &["--json", "card", "list"]);
    plain.code(0);
    assert_eq!(plain.json()["cards"].as_array().map(Vec::len), Some(1));
}

/// "Rules and edge cases": no badge, lane or mark changes a status, a control
/// or an exit: every read answers exit 0 over suspect and departed rows,
/// writes no event and no file, and `spec check` and `spec gate` answer as
/// they did.
#[test]
fn no_read_changes_a_status_a_file_or_an_exit() {
    let pair = Pair::new("tv-rules", ALMANAC);
    let check_before = run(&pair, &["check"]);
    a_cycle(&pair);
    let check = run(&pair, &["check"]);
    assert_eq!(
        (check.code, &check.stdout),
        (check_before.code, &check_before.stdout)
    );
    let events = pair.sql("SELECT COUNT(*) FROM events;");
    let cards = pair.sql("SELECT id, revision, updated_at FROM cards ORDER BY id;");
    let status = pair.git.git(&pair.main, &["status", "--porcelain"]);
    for args in [
        vec!["flow", "--trace"],
        vec!["trace"],
        vec!["trace", "--stage", "gather"],
        vec!["trace", "--summary"],
        vec!["card", "list", "--marks"],
    ] {
        run(&pair, &args).code(0);
    }
    assert_eq!(pair.sql("SELECT COUNT(*) FROM events;"), events);
    assert_eq!(
        pair.sql("SELECT id, revision, updated_at FROM cards ORDER BY id;"),
        cards
    );
    assert_eq!(pair.git.git(&pair.main, &["status", "--porcelain"]), status);
    let after = run(&pair, &["check"]);
    assert_eq!((after.code, &after.stdout), (check.code, &check.stdout));
}
