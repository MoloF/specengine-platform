//! docs/features/collision-pairs.md (RM-0088, the pair half)
//! "Acceptance criteria", the daemon's half of PP1 and PP8: the real daemon over a
//! scratch git repository of `fixtures/spec-a` holding the spec's example
//! (its hit DEC-0023 named) and a three-target variant (its hits named),
//! both raised by the `spec` binary.
//!
//! - PP1: `GET …/proposals/PR-0001` serves the spec's `pair`, right after
//!   `evidence`, in 54 keys, the document byte-equal to `spec review
//!   PR-0001 --json` under the daemon's `HOME`; the `…/round` item the same
//!   object after `evidence`, in 22 keys; the three-target item `null` in
//!   both. M: evidence grouped by position, not `file`.
//! - The orchestrator's ruling, pinned: a staged decision answers its review
//!   document with `pair` null; the next `GET` still serves the pair.
//!
//! The key sets themselves, `fixtures/daemon-keys.json` against the daemon,
//! are `daemon_keys.rs`'s.

#![cfg(unix)]

mod common;

use common::{Scratch, Server, run_product, spec_bin, spec_json};
use serde_json::{Value, json};

/// The project's routes.
const P: &str = "/api/projects/lantern-keep";

/// The spec's pair for the example.
fn data_pair() -> Value {
    json!({"left": {"id": "RULE-SPRINT-COST", "title": "Cost",
            "path": "docs/spec/movement/sprint.md", "evidence": [0]},
        "right": {"id": "RULE-STAM-REGEN", "title": "Regeneration",
            "path": "docs/spec/movement/stamina.md", "evidence": [1]},
        "other": [2]})
}

/// The example's `--input` document over `targets`, `distinct` named.
fn example(targets: &[&str], summary: &str, distinct: &[&str]) -> String {
    json!({"node_ids": targets, "gap_type": "contradicts", "severity": "normal",
        "summary": summary,
        "evidence": [
            {"file": "docs/spec/movement/sprint.md", "lines": "18-20",
                "observed": "Sprinting pauses regeneration",
                "documented": "Cost is drained per second"},
            {"file": "docs/spec/movement/stamina.md", "lines": "21-23",
                "observed": "Regeneration never pauses",
                "documented": "Refill starts after 1.5 s"},
            {"file": "src/stamina.rs", "qpath": "stamina::regen", "lines": "40-52",
                "observed": "regen runs during sprint", "documented": "a pause is specified"}],
        "options": [{"label": "Pause while sprinting", "effect": "Sprint drains faster",
                "price": "Regeneration rewritten"},
            {"label": "Never pause", "effect": "Sprint lasts longer",
                "price": "Sprint cost rewritten"}],
        "recommendation": 0, "distinct_from": distinct})
    .to_string()
}

/// The keys of the object at the start of `raw` (compact JSON), in order,
/// read from the text: the daemon's order, which a parsed `Value` keeps not.
fn top_keys(raw: &str) -> Vec<String> {
    let value: Value = serde_json::from_str(raw).expect("JSON");
    let mut keys: Vec<(usize, String)> = value
        .as_object()
        .expect("an object")
        .keys()
        .map(|key| {
            let at = raw
                .find(&format!("\"{key}\":"))
                .unwrap_or_else(|| panic!("{key} in {raw}"));
            (at, key.clone())
        })
        .collect();
    keys.sort();
    keys.into_iter().map(|(_, key)| key).collect()
}

/// PP1, PP8: the daemon serves the pair where the review and the round
/// serve it, byte-equal to the CLI's review; a staged decision answers
/// `null`.
#[test]
fn pp1_the_daemon_serves_the_pair() {
    let scratch = Scratch::new("cpair-daemon");
    let root = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let root_text = root.to_str().expect("a UTF-8 root");
    let raise = |document: &str| {
        let run = run_product(
            &spec_bin(),
            &home,
            &cwd,
            &[
                "--root",
                root_text,
                "propose",
                "discrepancy",
                "--input",
                "-",
                "--author-role",
                "developer",
            ],
            document.as_bytes(),
        );
        run.code(0);
        run.stdout.lines().next().unwrap_or_default().to_owned()
    };
    let id = raise(&example(
        &["RULE-SPRINT-COST", "RULE-STAM-REGEN"],
        "Sprint cost and regeneration disagree on when stamina refills",
        &["DEC-0023"],
    ));
    assert_eq!(id, "PR-0001");
    let three = raise(&example(
        &["RULE-SPRINT-COST", "RULE-STAM-REGEN", "MEC-SPRINT"],
        "Three documents disagree on when stamina refills",
        &["DEC-0023"],
    ));
    assert_eq!(three, "PR-0002");

    let server = Server::serve(&home, &cwd, &[&root]);
    let reply = server.get(&format!("{P}/proposals/PR-0001"));
    reply.status(200);
    let raw = reply.text().to_owned();
    let keys = top_keys(&raw);
    assert_eq!(keys.len(), 54, "{raw}");
    let at = keys
        .iter()
        .position(|key| key == "evidence")
        .expect("evidence");
    assert_eq!(keys[at..at + 3], ["evidence", "pair", "options"]);
    let document = reply.json();
    assert_eq!(document["pair"], data_pair(), "{raw}");
    let cli = spec_json(&home, &cwd, &root, &["review", "PR-0001"]);
    assert_eq!(
        serde_json::from_str::<Value>(&cli.stdout).expect("JSON")["pair"],
        document["pair"],
        "the CLI's review, the daemon's HOME"
    );
    let other = server
        .get(&format!("{P}/proposals/PR-0002"))
        .status(200)
        .json();
    assert_eq!(other["pair"], Value::Null);

    let round = server.get(&format!("{P}/round")).status(200).json();
    let items = round["items"].as_array().expect("items");
    for item in items {
        assert_eq!(item.as_object().map(serde_json::Map::len), Some(22));
    }
    let item = |id: &str| {
        items
            .iter()
            .find(|item| item["id"] == json!(id))
            .unwrap_or_else(|| panic!("no {id} in {round}"))
            .clone()
    };
    assert_eq!(item("PR-0001")["pair"], data_pair());
    assert_eq!(item("PR-0002")["pair"], Value::Null);

    // The ruling: a stage answers `pair` null; the next read serves it.
    let updated_at = document["updated_at"].as_str().expect("updated_at");
    let staged = server.stage(
        &format!("{P}/proposals/PR-0001/decision"),
        &format!("{{\"decision\":\"approve\",\"option\":0,\"updated_at\":\"{updated_at}\"}}"),
    );
    assert_eq!(staged.status, 200, "{}", staged.text());
    let staged = staged.json();
    assert_eq!(staged.as_object().map(serde_json::Map::len), Some(54));
    assert_eq!(staged["pair"], Value::Null, "{staged}");
    let again = server
        .get(&format!("{P}/proposals/PR-0001"))
        .status(200)
        .json();
    assert_eq!(again["pair"], data_pair());
    assert!(again["staged"].is_object(), "{again}");
}
