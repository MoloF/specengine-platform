//! docs/features/plain-questions.md (RM-0088, the question half)
//! "Acceptance criteria", PQ10, the daemon's half: the real daemon over a
//! scratch git repository of `fixtures/spec-a` holding the "Data" example
//! (its hit DEC-0023 named: spec-a's accepted decision answers both
//! targets), a card's question with a gist and `matters`, and a question
//! without them, all raised by the `spec` binary. `GET …/proposals/PR-0001`
//! serves 53 keys, `gist` and `matters` after `summary`, the four fields
//! byte-equal to the input; `…/inbox` entries 13 keys, `gist` after
//! `summary`; `…/round` items 21; the card's `items` 18; every other item
//! `null`. M: `matters` not served.
//!
//! The key sets themselves, `fixtures/daemon-keys.json` against the daemon,
//! are `daemon_keys.rs`'s.

#![cfg(unix)]

mod common;

use common::{Scratch, Server, run_product, spec_bin};
use serde_json::{Value, json};

/// The project's routes.
const P: &str = "/api/projects/lantern-keep";

const TEXT: &str =
    "Does RULE-STAM-REGEN's delay restart when stamina_regen() is called mid-sprint?";
const GIST: &str = "Should resting restart the wait before stamina refills?";
const MATTERS: &str = "Sprint tuning and two tasks wait on it.";
const OPTIONS: &str = "[{\"label\": \"Restart the wait\", \"effect\": \"Short rests do not \
                       refill\", \"price\": \"none: as built\"},\n {\"label\": \"Keep the wait \
                       running\", \"effect\": \"Tapping rest refills faster\", \"price\": \
                       \"R-28 rebalanced\"}]\n";

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

/// PQ10: the daemon serves the fields where the documents gain them.
#[test]
fn pq10_the_daemon_serves_gist_and_matters() {
    let scratch = Scratch::new("pq10");
    let root = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let root_text = root.to_str().expect("a UTF-8 root");
    let spec = |args: &[&str], stdin: &[u8]| {
        let mut all = vec!["--root", root_text];
        all.extend_from_slice(args);
        let run = run_product(&spec_bin(), &home, &cwd, &all, stdin);
        run.code(0);
        run.stdout.lines().next().unwrap_or_default().to_owned()
    };
    let asked = spec(
        &[
            "propose",
            "question",
            "MEC-STAMINA",
            "RULE-STAM-REGEN",
            "--text",
            TEXT,
            "--working-answer",
            "yes, the 1.5 s delay restarts",
            "--price-of-other",
            "R-28 rebalanced",
            "--gist",
            GIST,
            "--matters",
            MATTERS,
            "--options",
            "-",
            "--recommendation",
            "0",
            "--distinct-from",
            "DEC-0023",
            "--author-role",
            "developer",
        ],
        OPTIONS.as_bytes(),
    );
    assert_eq!(asked, "PR-0001");
    spec(&["card", "new", "--title", "Lamp room"], b"");
    let carded = spec(
        &[
            "propose",
            "question",
            "MEC-SPRINT",
            "--text",
            "Does a sprint restart the wait?",
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
    assert_eq!(carded, "PR-0002");
    let bare = spec(
        &[
            "propose",
            "question",
            "EDGE-STAM-ZERO",
            "--text",
            "Does the lamp stay lit?",
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
    assert_eq!(bare, "PR-0003");

    let server = Server::serve(&home, &cwd, &[&root]);
    let reply = server.get(&format!("{P}/proposals/PR-0001"));
    reply.status(200);
    let raw = reply.text().to_owned();
    let document = reply.json();
    let keys = top_keys(&raw);
    assert_eq!(keys.len(), 53, "{raw}");
    let summary = keys
        .iter()
        .position(|key| key == "summary")
        .expect("summary");
    assert_eq!(
        keys[summary..summary + 4],
        ["summary", "gist", "matters", "working_answer"]
    );
    assert_eq!(document["gist"], json!(GIST));
    assert_eq!(document["matters"], json!(MATTERS));
    let options: Value = serde_json::from_str(OPTIONS).expect("options");
    assert_eq!(document["options"], options);
    assert_eq!(document["recommendation"], json!(0));
    let read = server
        .get(&format!("{P}/proposals/PR-0003"))
        .status(200)
        .json();
    assert_eq!(
        (&read["gist"], &read["matters"]),
        (&Value::Null, &Value::Null)
    );

    let inbox = server.get(&format!("{P}/inbox"));
    inbox.status(200);
    let inbox = inbox.json();
    let entries = inbox["proposals"].as_array().expect("entries");
    assert_eq!(entries.len(), 3, "{inbox}");
    for entry in entries {
        assert_eq!(
            entry.as_object().map(serde_json::Map::len),
            Some(13),
            "{entry}"
        );
        assert!(entry.get("matters").is_none(), "{entry}");
    }
    let gists: Vec<&Value> = entries.iter().map(|entry| &entry["gist"]).collect();
    assert_eq!(
        gists,
        [
            &json!(GIST),
            &json!("Should a sprint restart the wait?"),
            &Value::Null
        ]
    );

    let round = server.get(&format!("{P}/round")).status(200).json();
    let items = round["items"].as_array().expect("items");
    assert_eq!(items.len(), 3, "{round}");
    for item in items {
        assert_eq!(
            item.as_object().map(serde_json::Map::len),
            Some(21),
            "{item}"
        );
    }
    let first = items
        .iter()
        .find(|item| item["id"] == json!("PR-0001"))
        .expect("PR-0001");
    assert_eq!(
        (&first["gist"], &first["matters"]),
        (&json!(GIST), &json!(MATTERS))
    );

    let card = server.get(&format!("{P}/cards/C-0001")).status(200).json();
    let items = card["items"].as_array().expect("items");
    assert_eq!(items.len(), 2, "{card}");
    for item in items {
        assert_eq!(
            item.as_object().map(serde_json::Map::len),
            Some(18),
            "{item}"
        );
    }
    assert_eq!(items[0]["gist"], json!("Should a sprint restart the wait?"));
    assert_eq!(items[0]["matters"], json!("One task waits."));
    assert_eq!(
        (&items[1]["gist"], &items[1]["matters"]),
        (&Value::Null, &Value::Null)
    );
}
