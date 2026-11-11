//! docs/features/card-trace.md, the tool: `get_trace {ref?}`, a read, answers
//! what the CLI library answers for the same request (text = `stderr_lines` +
//! `render_text`; exit 2 = the `CliError` line; `structuredContent` = the
//! parsed `render_json`, absent on exit 2; `isError` on exit 1 and 2), in both
//! protocol eras, over a scratch git repository of `fixtures/card-press` with
//! a confirmed, a suspect and a departed node, and every document meets the
//! tool's output schema under the strict walker of `common/read.rs`.
//!
//! - AC-09: suspect and departed rows are no error, the bytes the CLI's.
//! - AC-10: `tools/list` holds no clear; the sources hold no way to one.
//! - "Re-estimating": `edit_card` gains `reason?`, acting as `agent`.
//!
//! - docs/features/trace-truth.md AC-01, AC-08: a nested edit reads `suspect`
//!   on `list_cards` and `get_trace`; `get_card` is `spec card show --marks`,
//!   its row's four keys, `within_hash`, no new argument, an index error an
//!   error result.
//!
//! - docs/features/hub-confirm.md "Data": `get_proposal`'s `author` has two
//!   closed shapes, four keys and a hub's departing accept's five.
//!
//! The door (`mcp_door.rs`), the genre scan (`mcp_genre.rs`) and the
//! `INSTRUCTIONS` pins hold the rest.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::PathBuf;

use common::read::{
    ERAS, Era, Session, TOOLS, assert_bad_arguments, checked_call, cli_env, conforms, content_text,
    output_schemas,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    CardAcceptRequest, CardAssignRequest, CardCancelRequest, CardMoveRequest, CardNewRequest, Env,
    Globals, card_accept, card_assign, card_cancel, card_move, card_new,
};

const CLOCK: &str = "2026-10-30T09:00:00Z";

/// `get_trace`'s reads-line clause (docs/features/card-trace.md).
const TRACE_READ: &str = "; get_trace(ref?) = trace";

/// A scratch git repository of `fixtures/card-press`, committed.
struct Repo {
    /// Removed on drop, with the repository in it.
    _scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        Self::of(label, "card-press")
    }

    /// A scratch git repository of `fixtures/<fixture>`, committed.
    fn of(label: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy(fixture, "repo");
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the fixture");
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            git,
            root,
            home,
        }
    }

    fn env(&self) -> Env {
        cli_env(&self.root, Some(&self.home))
    }

    fn session(&self, era: Era) -> Session {
        Session::open(era, &[], Some(&self.root), Home::At(&self.home))
    }

    /// A card of the `copy` flow on `targets`, at `proof`, `ben` its reviewer:
    /// its ID.
    fn in_proof(&self, title: &str, targets: &[&str]) -> String {
        let (env, globals) = (self.env(), Globals::default());
        let made = card_new(
            &env,
            &globals,
            &CardNewRequest {
                title: title.to_owned(),
                discipline: Some("copy".to_owned()),
                targets: targets.iter().map(|target| (*target).to_owned()).collect(),
                now: CLOCK.to_owned(),
                ..CardNewRequest::default()
            },
        )
        .expect("a card");
        let id = made.card.expect("a card").id;
        card_assign(
            &env,
            &globals,
            &CardAssignRequest {
                id: id.clone(),
                reviewer: Some("ben".to_owned()),
                now: CLOCK.to_owned(),
                ..CardAssignRequest::default()
            },
        )
        .expect("assigned");
        for (column, actor) in [("edit", "desk"), ("proof", "desk")] {
            card_move(
                &env,
                &globals,
                &CardMoveRequest {
                    id: id.clone(),
                    to: Some(column.to_owned()),
                    actor: Some(actor.to_owned()),
                    now: CLOCK.to_owned(),
                    ..CardMoveRequest::default()
                },
            )
            .expect("moved");
        }
        id
    }

    /// `id` accepted by `ben`, `request` the verdict.
    fn accept(&self, id: &str, request: CardAcceptRequest) {
        let outcome = card_accept(
            &self.env(),
            &Globals::default(),
            &CardAcceptRequest {
                id: id.to_owned(),
                actor: "ben".to_owned(),
                now: CLOCK.to_owned(),
                ..request
            },
            &mut |_| true,
        )
        .expect("the accept");
        assert_eq!(outcome.refusal, None, "{outcome:?}");
    }

    /// `from` for `to` in `path`, by hand, committed.
    fn hand_edit(&self, path: &str, from: &str, to: &str) {
        replace(&self.root, path, from, to);
        self.git.add_all(&self.root);
        self.git.commit(&self.root, "by hand");
    }

    /// `git status --porcelain --ignored`, `HEAD`.
    fn git_side(&self) -> (String, String) {
        (
            self.git
                .git_text(&self.root, &["status", "--porcelain", "--ignored"]),
            self.git.git_text(&self.root, &["rev-parse", "HEAD"]),
        )
    }

    /// The `card.*` events of the queue, `(type, payload)`.
    fn events(&self) -> Vec<(String, Value)> {
        let db = data_dir(&self.home).join("card-press.db");
        let output = std::process::Command::new("/usr/bin/sqlite3")
            .arg(&db)
            .arg("SELECT type || char(9) || payload FROM events WHERE type LIKE 'card.%' ORDER BY seq;")
            .stdin(std::process::Stdio::null())
            .output()
            .expect("sqlite3 runs");
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout)
            .expect("UTF-8")
            .lines()
            .map(|line| {
                let (kind, payload) = line.split_once('\t').expect("two fields");
                (
                    kind.to_owned(),
                    serde_json::from_str(payload).expect("a payload"),
                )
            })
            .collect()
    }
}

/// A board with a confirmed node, a suspect one and a departed one.
fn marked(label: &str) -> Repo {
    let repo = Repo::new(label);
    let lead = repo.in_proof("Lead story", &["SHT-FRONT"]);
    repo.accept(
        &lead,
        CardAcceptRequest {
            briefed: true,
            ..CardAcceptRequest::default()
        },
    );
    let notices = repo.in_proof("Notices", &["SHT-BACK"]);
    repo.accept(
        &notices,
        CardAcceptRequest {
            briefed: true,
            ..CardAcceptRequest::default()
        },
    );
    let long = repo.in_proof("Long read", &["SHT-INNER"]);
    repo.accept(
        &long,
        CardAcceptRequest {
            departs: vec!["SHT-INNER".to_owned()],
            summary: Some("The result left the brief.".to_owned()),
            observed: Some("The lead runs below the fold.".to_owned()),
            documented: Some("The lead runs above the fold.".to_owned()),
            ..CardAcceptRequest::default()
        },
    );
    repo.hand_edit("docs/spec/sht-back.md", "the tide table", "the moon table");
    repo
}

/// Every call of `calls` in both eras against the CLI and the schema; the
/// first era's results.
fn parity(repo: &Repo, calls: &[Value]) -> Vec<Value> {
    let env = repo.env();
    let globals = Globals::default();
    let mut first = Vec::new();
    for (round, era) in ERAS.into_iter().enumerate() {
        let mut session = repo.session(era);
        let schemas = output_schemas(&session.tools());
        for args in calls {
            let result = checked_call(
                &mut session,
                &schemas,
                "get_trace",
                args.clone(),
                &env,
                &globals,
            );
            if round == 0 {
                first.push(result);
            }
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
    first
}

/// AC-09: a whole read, a tree, each state's node, each form of `ref`: the
/// CLI's bytes, a schema-true document, suspect and departed rows no error;
/// git as it was: a read.
#[test]
fn ac09_get_trace_equals_the_cli_within_its_schema() {
    let repo = marked("mcp-trace");
    let git = repo.git_side();
    let results = parity(
        &repo,
        &[
            json!({}),
            json!({"ref": null}),
            json!({"ref": "PRN-WEEKLY"}),
            json!({"ref": "SHT-FRONT"}),
            json!({"ref": "SHT-BACK"}),
            json!({"ref": "SHT-INNER"}),
            json!({"ref": "SHT-FRONT-HEAD"}),
            json!({"ref": "docs/spec/sht-inner.md"}),
            json!({"ref": "SHT-FRONT#SHT-FRONT-HEAD"}),
        ],
    );
    assert_eq!(repo.git_side(), git, "a read touches no file");
    let whole = &results[2];
    assert_eq!(whole["isError"], json!(false), "{whole}");
    let text = content_text(whole);
    assert!(
        text.starts_with(
            "trace PRN-WEEKLY | nodes 5, cards 3 | no card 1, in work 0, suspect 1, departed 1, \
             confirmed 1\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("  card C-0002 | copy/filed | suspect SHT-BACK | Notices\n"),
        "{text}"
    );
    assert!(
        text.contains("  card C-0003 | copy/filed | departed PR-0001 | Long read\n"),
        "{text}"
    );
    assert_eq!(
        whole["structuredContent"]["counts"],
        json!({"no_card": 1, "in_work": 0, "suspect": 1, "departed": 1, "confirmed": 1})
    );
}

/// docs/features/trace-views.md AC-09, the tool: `list_cards {marks: true}` is
/// `spec card list --marks`' bytes within the schema, each row's mark what
/// `get_trace` reads; no `marks`, every mark `null`; this project has no step
/// and no stage, so `get_trace` holds no lane; git as it was: a read.
/// M: a tile drawn without its mark.
#[test]
fn trace_views_list_cards_marks_equal_the_trace() {
    let repo = marked("mcp-trace-marks");
    let git = repo.git_side();
    let env = repo.env();
    let globals = Globals::default();
    for era in ERAS {
        let mut session = repo.session(era);
        let schemas = output_schemas(&session.tools());
        let mut call = |tool: &str, args: Value| {
            let result = checked_call(&mut session, &schemas, tool, args, &env, &globals);
            assert_eq!(result["isError"], json!(false), "{tool}: {result}");
            result["structuredContent"].clone()
        };
        let marked = call("list_cards", json!({"marks": true}));
        let plain = call("list_cards", json!({}));
        assert_eq!(
            plain,
            call("list_cards", json!({"marks": false})),
            "{era:?}"
        );
        let trace = call("get_trace", json!({}));
        let rows = marked["cards"].as_array().expect("cards");
        assert_eq!(rows.len(), 3, "{marked}");
        for (row, node, word) in [
            (&rows[0], "SHT-FRONT", "accepted"),
            (&rows[1], "SHT-BACK", "suspect"),
            (&rows[2], "SHT-INNER", "departed"),
        ] {
            assert_eq!(row["mark"], word, "{row}");
            let shown = trace["nodes"]
                .as_array()
                .expect("nodes")
                .iter()
                .find(|shown| shown["id"] == node)
                .expect("the node");
            let card = &shown["cards"][0];
            assert_eq!(card["id"], row["id"], "{shown}");
            for key in ["mark", "on", "discrepancy", "correction"] {
                assert_eq!(row[key], card[key], "{key}: {row}");
            }
        }
        for row in plain["cards"].as_array().expect("cards") {
            for key in ["mark", "on", "discrepancy", "correction"] {
                assert_eq!(row[key], Value::Null, "{key}: {row}");
            }
        }
        assert_eq!(trace["stages"], json!([]), "no step, no stage: {trace}");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
    assert_eq!(repo.git_side(), git, "a read touches no file");
}

/// docs/features/trace-truth.md AC-01, the tool: text appended under
/// `STEP-SET-PROOF`, the card on `STEP-SET` accepted: `list_cards {marks: true}`
/// and `get_trace` read `suspect STEP-SET`, within the schemas, the bytes the
/// CLI's. M: `within_hash` over the target alone.
#[test]
fn trace_truth_a_nested_edit_marks_on_the_tools() {
    let repo = Repo::of("mcp-truth-nested", "trace-almanac");
    let (env, globals) = (repo.env(), Globals::default());
    let made = card_new(
        &env,
        &globals,
        &CardNewRequest {
            title: "Pages set".to_owned(),
            discipline: Some("editing".to_owned()),
            targets: vec!["STEP-SET".to_owned()],
            now: CLOCK.to_owned(),
            ..CardNewRequest::default()
        },
    )
    .expect("a card");
    let id = made.card.expect("a card").id;
    card_assign(
        &env,
        &globals,
        &CardAssignRequest {
            id: id.clone(),
            reviewer: Some("ben".to_owned()),
            now: CLOCK.to_owned(),
            ..CardAssignRequest::default()
        },
    )
    .expect("assigned");
    card_move(
        &env,
        &globals,
        &CardMoveRequest {
            id: id.clone(),
            to: Some("review".to_owned()),
            actor: Some("ana".to_owned()),
            now: CLOCK.to_owned(),
            ..CardMoveRequest::default()
        },
    )
    .expect("moved");
    repo.accept(
        &id,
        CardAcceptRequest {
            briefed: true,
            ..CardAcceptRequest::default()
        },
    );
    repo.hand_edit(
        "docs/spec/step-set.md",
        "before any copy is pulled.\n",
        "before any copy is pulled.\nA second proof goes to the printer.\n",
    );

    let mut session = repo.session(ERAS[0]);
    let schemas = output_schemas(&session.tools());
    let mut call = |tool: &str, args: Value| {
        let result = checked_call(&mut session, &schemas, tool, args, &env, &globals);
        assert_eq!(result["isError"], json!(false), "{tool}: {result}");
        result["structuredContent"].clone()
    };
    let rows = call("list_cards", json!({"marks": true}));
    let row = &rows["cards"][0];
    assert_eq!(row["mark"], "suspect", "{row}");
    assert_eq!(row["on"], "STEP-SET", "{row}");
    let trace = call("get_trace", json!({"ref": "STEP-SET"}));
    let node = &trace["nodes"][0];
    assert_eq!(node["rollup"], "suspect", "{trace}");
    assert_eq!(node["cards"][0]["mark"], "suspect", "{trace}");
    assert_eq!(node["cards"][0]["on"], "STEP-SET", "{trace}");
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
}

/// docs/features/trace-truth.md AC-08, the tool: `get_card` always asks the
/// mark, so each card equals its `list_cards {marks: true}` row on the four
/// keys, a suspect, a departed and an accepted one, in both eras, the bytes of
/// `spec card show --marks` within the strict walker's schema, `within_hash` in
/// its snapshot; it takes no new argument. Nothing is written. M: keys not
/// joined in `run_show`.
#[test]
fn trace_truth_get_card_equals_its_list_row() {
    let repo = marked("mcp-truth-get-card");
    let git = repo.git_side();
    let env = repo.env();
    let globals = Globals::default();
    for era in ERAS {
        let mut session = repo.session(era);
        let schemas = output_schemas(&session.tools());
        let mut call = |tool: &str, args: Value| {
            let result = checked_call(&mut session, &schemas, tool, args, &env, &globals);
            assert_eq!(result["isError"], json!(false), "{tool}: {result}");
            result["structuredContent"].clone()
        };
        let list = call("list_cards", json!({"marks": true}));
        let rows = list["cards"].as_array().expect("cards");
        assert_eq!(rows.len(), 3, "{list}");
        let mut words = Vec::new();
        for row in rows {
            let id = row["id"].as_str().expect("an ID");
            let card = call("get_card", json!({"card_id": id}));
            for key in ["mark", "on", "discrepancy", "correction"] {
                assert_eq!(card[key], row[key], "{id}: {key}: {card}");
            }
            assert!(
                card["snapshot"][0]["within_hash"].is_string(),
                "{id}: {card}"
            );
            words.push(row["mark"].as_str().expect("a mark").to_owned());
        }
        assert_eq!(words, ["accepted", "suspect", "departed"], "{list}");
        let reply = session.call("get_card", json!({"card_id": "C-0001", "marks": true}));
        assert_bad_arguments(&reply, "", "get_card takes no `marks`");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
    assert_eq!(repo.git_side(), git, "a read touches no file");
}

/// docs/features/trace-truth.md "Data": a card frozen on a broken document
/// holds a null `within_hash`, and a cancelled card null marks: both within
/// the schema. D8: `get_card` over an index that cannot be read is an error
/// result, never a clean card.
#[test]
fn trace_truth_get_card_nulls_and_a_broken_index() {
    let repo = Repo::new("mcp-truth-nulls");
    let (env, globals) = (repo.env(), Globals::default());
    repo.hand_edit(
        "docs/spec/sht-front.md",
        "reviewed: 2026-10-01\n",
        "reviewed: 2026-10-01\nlinks: 7\n",
    );
    let cancelled = repo.in_proof("Pulled", &["SHT-BACK"]);
    card_cancel(
        &env,
        &globals,
        &CardCancelRequest {
            id: cancelled.clone(),
            reason: "Pulled.".to_owned(),
            now: CLOCK.to_owned(),
            ..CardCancelRequest::default()
        },
    )
    .expect("cancelled");
    let broken = repo.in_proof("Lead story", &["SHT-FRONT"]);

    let mut session = repo.session(ERAS[0]);
    let schemas = output_schemas(&session.tools());
    let mut call = |tool: &str, args: Value| {
        let result = checked_call(&mut session, &schemas, tool, args, &env, &globals);
        assert_eq!(result["isError"], json!(false), "{tool}: {result}");
        result["structuredContent"].clone()
    };
    let card = call("get_card", json!({"card_id": broken}));
    assert_eq!(card["snapshot"][0]["within_hash"], Value::Null, "{card}");
    assert_eq!(card["mark"], "open", "{card}");
    let card = call("get_card", json!({"card_id": cancelled}));
    for key in ["mark", "on", "discrepancy", "correction"] {
        assert_eq!(card[key], Value::Null, "{key}: {card}");
    }
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);

    let mut session = repo.session(ERAS[0]);
    let database = data_dir(&repo.home).join("card-press.db");
    let renamed = std::process::Command::new("/usr/bin/sqlite3")
        .arg(database)
        .arg("ALTER TABLE files RENAME TO files_gone;")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(renamed.status.success(), "{renamed:?}");
    let reply = session.call("get_card", json!({"card_id": broken}));
    let result = &reply["result"];
    assert_eq!(result["isError"], json!(true), "no clean card: {reply}");
    assert!(result.get("structuredContent").is_none(), "{reply}");
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
}

/// The IDs of a document's nodes, in order.
fn node_ids(document: &Value) -> Vec<&str> {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["id"].as_str().expect("an ID"))
        .collect()
}

/// docs/features/trace-views.md "`spec trace`, extended", the tool: `stage` and
/// `summary` are `spec trace`'s flags: each call the CLI's bytes within the
/// schema, in both eras, the rows of one stage across both parts, `-` the step
/// with none, `summary` no row; the lanes whole every time, the book and its
/// parts in none. A wrong type is a bad argument; git as it was: a read.
#[test]
fn trace_views_get_trace_takes_stage_and_summary() {
    let repo = Repo::of("mcp-trace-lanes", "trace-almanac");
    let (env, globals) = (repo.env(), Globals::default());
    card_new(
        &env,
        &globals,
        &CardNewRequest {
            title: "Notice posted".to_owned(),
            discipline: Some("editing".to_owned()),
            targets: vec!["STEP-CALL".to_owned()],
            now: CLOCK.to_owned(),
            ..CardNewRequest::default()
        },
    )
    .expect("a card");
    let git = repo.git_side();
    let results = parity(
        &repo,
        &[
            json!({}),
            json!({"stage": "print"}),
            json!({"stage": "-"}),
            json!({"stage": "nowhere"}),
            json!({"summary": true}),
            json!({"summary": false}),
            json!({"stage": null, "summary": null}),
            json!({"stage": "print", "summary": true}),
            json!({"ref": "PART-GATHER", "stage": "print"}),
            json!({"ref": "STEP-NOWHERE", "stage": "print"}),
        ],
    );
    assert_eq!(repo.git_side(), git, "a read touches no file");
    let whole = &results[0]["structuredContent"];
    let in_work = json!({"no_card": 1, "in_work": 1, "suspect": 0, "departed": 0, "confirmed": 0});
    let empty = json!({"no_card": 2, "in_work": 0, "suspect": 0, "departed": 0, "confirmed": 0});
    let one = json!({"no_card": 1, "in_work": 0, "suspect": 0, "departed": 0, "confirmed": 0});
    assert_eq!(
        whole["stages"],
        json!([
            {"stage": "gather", "distance": 0, "counts": in_work},
            {"stage": "print", "distance": 3, "counts": empty},
            {"stage": null, "distance": 2, "counts": one},
        ]),
        "{whole}"
    );
    for (result, rows) in [
        (&results[1], vec!["STEP-BIND", "STEP-SET"]),
        (&results[2], vec!["STEP-READ"]),
        (&results[3], vec![]),
        (&results[4], vec![]),
        (&results[7], vec![]),
        (&results[8], vec!["STEP-SET"]),
    ] {
        assert_eq!(result["isError"], json!(false), "{result}");
        let document = &result["structuredContent"];
        assert_eq!(node_ids(document), rows, "{document}");
    }
    for at in [1, 2, 3, 4, 7] {
        let document = &results[at]["structuredContent"];
        assert_eq!(document["stages"], whole["stages"], "always whole");
        assert_eq!(document["counts"], whole["counts"], "the header whole");
    }
    assert_eq!(
        content_text(&results[4]).lines().count(),
        4,
        "the summary line and three lanes: {}",
        content_text(&results[4])
    );
    assert_eq!(results[5], results[0], "`false` is the plain read");
    assert_eq!(results[6], results[0], "`null` is the plain read");
    assert_eq!(results[9]["isError"], json!(true), "{}", results[9]);

    let mut session = repo.session(ERAS[0]);
    for args in [json!({"stage": 3}), json!({"summary": "yes"})] {
        let reply = session.call("get_trace", args.clone());
        assert_bad_arguments(&reply, "", &format!("get_trace {args}"));
    }
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
}

/// "Exit 1 only a REF naming nothing, 2 as `spec flow`": a REF naming nothing
/// is an error result with the document, what cannot run has none; a wrong
/// type or an unknown field is a bad argument.
#[test]
fn a_ref_naming_nothing_is_an_error_with_its_document() {
    let repo = Repo::new("mcp-trace-exits");
    let results = parity(
        &repo,
        &[
            json!({"ref": "SHT-NOPE"}),
            json!({"ref": "S\u{041d}T-FRONT"}),
        ],
    );
    assert_eq!(results[0]["isError"], json!(true), "{}", results[0]);
    assert_eq!(results[0]["structuredContent"]["ref"], json!("SHT-NOPE"));
    assert!(results[0]["structuredContent"]["reason"].is_string());
    assert_eq!(results[1]["isError"], json!(true), "{}", results[1]);
    assert!(results[1].get("structuredContent").is_none());
}

/// "Rules": no queue: `no card`, no table made, and the tool's document meets
/// its schema, which requires each key: a node without `rollup`, or a card
/// without `mark`, does not conform.
#[test]
fn no_queue_reads_no_card_within_the_schema() {
    let repo = Repo::new("mcp-trace-empty");
    let results = parity(&repo, &[json!({}), json!({"ref": "PRN-WEEKLY"})]);
    assert_eq!(
        results[0]["structuredContent"]["counts"],
        json!({"no_card": 5, "in_work": 0, "suspect": 0, "departed": 0, "confirmed": 0})
    );

    let repo = marked("mcp-trace-schema");
    let results = parity(&repo, &[json!({"ref": "SHT-FRONT"})]);
    let mut session = repo.session(ERAS[0]);
    let schemas = output_schemas(&session.tools());
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
    let schema = &schemas["get_trace"];
    let served = &results[0]["structuredContent"];
    assert_eq!(conforms(schema, served, "get_trace"), Ok(()));
    let mut without = served.clone();
    assert!(
        without["nodes"][0]
            .as_object_mut()
            .expect("a node")
            .remove("rollup")
            .is_some()
    );
    assert!(
        conforms(schema, &without, "get_trace").is_err(),
        "{without}"
    );
    let mut without = served.clone();
    assert!(
        without["nodes"][0]["cards"][0]
            .as_object_mut()
            .expect("a card")
            .remove("mark")
            .is_some()
    );
    assert!(
        conforms(schema, &without, "get_trace").is_err(),
        "{without}"
    );
}

/// AC-10: `tools/list` holds no clear, in both eras, by name or description,
/// and `get_trace` is a read: `readOnlyHint`, one optional input, `ref`.
/// `INSTRUCTIONS` holds its clause on the reads' line, 2 048 bytes since
/// docs/features/trace-views.md (`get_flow`'s `, trace?`).
#[test]
fn ac10_no_tool_clears_and_get_trace_is_a_read() {
    // docs/features/department-split.md S8: thirty with `split_card`.
    assert_eq!(TOOLS.len(), 30);
    assert!(TOOLS.contains(&"get_trace"));
    let instructions = specengine_mcp::INSTRUCTIONS;
    assert_eq!(instructions.len(), 2048);
    assert_eq!(
        instructions.matches(TRACE_READ).count(),
        1,
        "{instructions}"
    );
    assert!(
        !instructions.contains("evidence computed, never typed"),
        "cut for room"
    );
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        assert_eq!(tool_names(&list), TOOLS, "{era:?}");
        for entry in list["tools"].as_array().expect("tools") {
            let name = entry["name"].as_str().expect("a name");
            assert!(!name.contains("clear"), "{era:?}: {name}");
            let description = entry["description"].as_str().unwrap_or_default();
            assert!(
                !description.contains("card clear"),
                "{era:?}: {name}: {description}"
            );
        }
        let trace = tool(&list, "get_trace");
        assert_eq!(trace["annotations"]["readOnlyHint"], json!(true), "{trace}");
        assert_eq!(
            trace["inputSchema"]["properties"]
                .as_object()
                .expect("properties")
                .keys()
                .collect::<Vec<_>>(),
            ["ref", "stage", "summary"],
            "{trace}"
        );
        assert_eq!(trace["inputSchema"].get("required"), None, "{trace}");
        drop(session.finish());
    }
}

/// AC-10: the sources of the MCP crate hold no way to a clear.
#[test]
fn ac10_the_mcp_sources_hold_no_clear() {
    let dir = repository_root().join("crates/specengine-mcp/src");
    let mut read = 0;
    for entry in fs::read_dir(dir).expect("src") {
        let path = entry.expect("an entry").path();
        let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        read += 1;
        for name in ["card_clear", "clear_card", "clear_cards", "CardClear"] {
            assert!(!text.contains(name), "{}: {name}", path.display());
        }
    }
    assert!(read > 5, "{read} sources read");
}

/// "Re-estimating": `edit_card` takes `reason`: a card holding an estimate
/// refuses a change without it, nothing else beside it, and records
/// `card.reestimated` as `agent`; a first estimate needs none.
#[test]
fn edit_card_takes_a_reason() {
    let repo = Repo::new("mcp-trace-estimate");
    let mut session = repo.session(Era::Stateless);
    let list = session.tools();
    let keys = tool(&list, "edit_card")["inputSchema"]["properties"]
        .as_object()
        .expect("properties")
        .clone();
    assert!(keys.contains_key("reason"), "{keys:?}");
    let reply = session.call(
        "create_card",
        json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"]}),
    );
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    let reply = session.call("edit_card", json!({"card_id": "C-0001", "estimate": "2"}));
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    let refused = session.call("edit_card", json!({"card_id": "C-0001", "estimate": "3"}));
    assert_eq!(result(&refused)["isError"], json!(true), "{refused}");
    assert!(
        content_text(result(&refused)).contains("--reason"),
        "{refused}"
    );
    let beside = session.call(
        "edit_card",
        json!({"card_id": "C-0001", "estimate": "3", "reason": "Bigger.", "goal": "G."}),
    );
    assert_eq!(result(&beside)["isError"], json!(true), "{beside}");
    let reply = session.call(
        "edit_card",
        json!({"card_id": "C-0001", "estimate": "3", "reason": "Bigger."}),
    );
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    drop(session.finish());
    assert_eq!(
        repo.events().pop().expect("an event"),
        (
            "card.reestimated".to_owned(),
            json!({"id": "C-0001", "old": "2", "new": "3", "reason": "Bigger.", "as": "agent"})
        )
    );
}

/// "Snapshot and suspect": two `edit_card` calls cannot clear a mark, a tool
/// having no clear: the target dropped and named again finds its old hash.
#[test]
fn two_edit_card_calls_cannot_clear_a_mark() {
    let repo = Repo::new("mcp-trace-retarget");
    let mut session = repo.session(Era::Stateless);
    let reply = session.call(
        "create_card",
        json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"]}),
    );
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    repo.hand_edit(
        "docs/spec/sht-front.md",
        "the masthead and the lead story",
        "the masthead, the lead story and the weather",
    );
    let mark = |session: &mut Session| {
        let reply = session.call("get_trace", json!({"ref": "SHT-FRONT"}));
        result(&reply)["structuredContent"]["nodes"][0]["cards"][0]["mark"].clone()
    };
    assert_eq!(mark(&mut session), json!("suspect"));
    for target in ["SHT-BACK", "SHT-FRONT"] {
        let reply = session.call(
            "edit_card",
            json!({"card_id": "C-0001", "target": [target]}),
        );
        assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    }
    assert_eq!(mark(&mut session), json!("suspect"), "the mark stays");
    drop(session.finish());
}

/// "Accepting": a tool cannot reach an accept's verdict either: `move_card`
/// to `accept.to` stays refused and a cancelled card reads `no card`.
#[test]
fn no_tool_reaches_a_verdict() {
    let repo = Repo::new("mcp-trace-verdict");
    let id = repo.in_proof("Lead story", &["SHT-FRONT"]);
    let mut session = repo.session(Era::Stateless);
    let refused = session.call("move_card", json!({"card_id": id, "to": "filed"}));
    assert_eq!(result(&refused)["isError"], json!(true), "{refused}");
    card_cancel(
        &repo.env(),
        &Globals::default(),
        &CardCancelRequest {
            id: id.clone(),
            reason: "Pulled.".to_owned(),
            now: CLOCK.to_owned(),
            ..CardCancelRequest::default()
        },
    )
    .expect("cancelled");
    let reply = session.call("get_trace", json!({"ref": "SHT-FRONT"}));
    assert_eq!(
        result(&reply)["structuredContent"]["nodes"][0]["rollup"],
        json!("no card"),
        "{reply}"
    );
    drop(session.finish());
}

/// An accept departing from `node`, `login` a hub's login when there is one.
fn departure(node: &str, login: Option<&str>) -> CardAcceptRequest {
    CardAcceptRequest {
        departs: vec![node.to_owned()],
        summary: Some("The result left the brief.".to_owned()),
        observed: Some("The lead runs below the fold.".to_owned()),
        documented: Some("The lead runs above the fold.".to_owned()),
        login: login.map(str::to_owned),
        ..CardAcceptRequest::default()
    }
}

/// docs/features/hub-confirm.md "Data": `login` is the one key the CLI leaves out
/// rather than writes `null`, so an `Author` has two shapes. A hub's departing
/// accept, its five keys, and a local one, its four, each answer `get_proposal`
/// as the CLI does, in both eras, within the tool's output schema under the
/// strict walker, `login` last. Each shape is closed: the schema takes no `null`
/// login and no sixth key. M: `ProposalAuthor` of four keys alone; `login`
/// optional in one shape.
#[test]
fn a_hubs_five_key_author_conforms_to_get_proposals_schema() {
    let repo = Repo::new("mcp-author-login");
    let hub = repo.in_proof("Long read", &["SHT-INNER"]);
    repo.accept(&hub, departure("SHT-INNER", Some("tomas")));
    let local = repo.in_proof("Notices", &["SHT-BACK"]);
    repo.accept(&local, departure("SHT-BACK", None));
    let four = json!({"type": "human", "role": null, "model": null, "run": null});
    let mut five = four.clone();
    five["login"] = json!("tomas");
    let (env, globals) = (repo.env(), Globals::default());
    for era in ERAS {
        let mut session = repo.session(era);
        let schemas = output_schemas(&session.tools());
        for (id, author) in [("PR-0001", &five), ("PR-0002", &four)] {
            let result = checked_call(
                &mut session,
                &schemas,
                "get_proposal",
                json!({"proposal_id": id}),
                &env,
                &globals,
            );
            assert_eq!(result["structuredContent"]["author"], *author, "{era:?}");
        }
        let schema = &schemas["get_proposal"]["properties"]["author"];
        conforms(schema, &five, "$").expect("five keys");
        conforms(schema, &four, "$").expect("four keys");
        let mut null_login = four.clone();
        null_login["login"] = Value::Null;
        conforms(schema, &null_login, "$").expect_err("a `null` login");
        let mut sixth = five.clone();
        sixth["seat"] = json!("desk");
        conforms(schema, &sixth, "$").expect_err("a sixth key");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}
