//! docs/features/intake-similar.md (RM-0059) "Acceptance criteria", IS10,
//! the daemon's half: the real daemon over a scratch git repository of
//! `fixtures/spec-a` holding a create stored with `--distinct-from` (its
//! propose answer listed `similar`), an update and a question raised by the
//! `spec` binary. `GET …/proposals/<id>` serves `similar: null` for each,
//! among 51 keys, `similar` after `why` and before `notes`; no inbox entry
//! holds it. M: a route serving a list.
//!
//! The `Proposal` key set itself, `fixtures/daemon-keys.json` against the
//! daemon, is `daemon_keys.rs`'s.

#![cfg(unix)]

mod common;

use common::{Scratch, Server, ask, propose_update, run_product, spec_bin};
use serde_json::{Value, json};

/// The project's routes.
const P: &str = "/api/projects/lantern-keep";

/// "Text"'s example: a new mechanic titled as MEC-STAMINA.
const ENDURANCE: &str = "---\nid: MEC-ENDURANCE\nclass: canon\ntier: 2\ntitle: STAMINA\n\
                         parent: DOM-MOVEMENT\nstatus: accepted\nowner: owner\nreviewed: \
                         2026-09-20\nlinks:\n  depends_on: [MEC-SPRINT]\n  derived_from: \
                         [R-12]\n---\n\n# STAMINA\n\nA body of plain words.\n";

/// IS10: each review document the daemon serves holds `similar: null`,
/// 51 keys, `similar` between `why` and `notes`; the inbox's entries hold
/// none. M: a route serving a list.
#[test]
fn is10_the_daemon_serves_similar_null_among_51_keys() {
    let scratch = Scratch::new("is10");
    let root = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let root_text = root.to_str().expect("a UTF-8 root");

    let created = run_product(
        &spec_bin(),
        &home,
        &cwd,
        &[
            "--root",
            root_text,
            "propose",
            "create",
            "docs/spec/movement/endurance.md",
            "--text-file",
            "-",
            "--rationale",
            "A new mechanic.",
            "--distinct-from",
            "MEC-STAMINA",
            "--json",
        ],
        ENDURANCE.as_bytes(),
    );
    created.code(0);
    let answer = created.json();
    assert_eq!(answer["id"], json!("PR-0001"), "{}", created.show());
    assert_eq!(answer["similar"][0]["id"], json!("MEC-STAMINA"), "{answer}");
    let updated = propose_update(
        &home,
        &cwd,
        &root,
        "EDGE-STAM-ZERO",
        "## Depletion {#EDGE-STAM-ZERO}\n- Stamina reaches 0: `Exhausted` at once.\n",
    );
    assert_eq!(updated, "PR-0002");
    let asked = ask(
        &home,
        &root,
        &["EDGE-STAM-ZERO"],
        "Does zero end the sprint at once?",
    );
    assert_eq!(asked, "PR-0003");

    let server = Server::serve(&home, &cwd, &[&root]);
    for id in ["PR-0001", "PR-0002", "PR-0003"] {
        let reply = server.get(&format!("{P}/proposals/{id}"));
        reply.status(200);
        let raw = reply.text().to_owned();
        let document: Value = reply.json();
        let object = document.as_object().expect("an object");
        // 53 since docs/features/plain-questions.md PQ10 (`gist`, `matters`).
        assert_eq!(object.len(), 53, "{id}: {raw}");
        assert_eq!(document["similar"], Value::Null, "{id}: {raw}");
        let why = raw.find("\"why\":").expect("why");
        let similar = raw.find("\"similar\":").expect("similar");
        let notes = raw.find("\"notes\":").expect("notes");
        assert!(why < similar && similar < notes, "{id}: {raw}");
        assert!(raw.contains(",\"similar\":null,\"notes\":"), "{id}: {raw}");
    }
    let inbox = server.get(&format!("{P}/inbox")).status(200).json();
    let entries = inbox["proposals"].as_array().expect("entries");
    assert_eq!(entries.len(), 3, "{inbox}");
    for entry in entries {
        assert!(entry.get("similar").is_none(), "{entry}");
    }
}
