//! docs/features/norm-hash.md TE-8, the HTTP half ("spec show and
//! mirrors": HTTP passes show's JSON through): `nodes/<REF>` is `spec
//! --root R show REF --json` byte for byte, its node carrying `norm_hash`
//! between `sections` and `span_hash` (a string, or `null` for a document
//! whose front-matter may have lost a link and for a file that is not
//! UTF-8); the document's top level stays `NodeView`'s four keys, as
//! `fixtures/daemon-keys.json` lists them (unchanged by this slice; since
//! docs/features/ui-node-bindings.md TE-1 the fixture lists the node's
//! keys too, `ShownNode`, its one `norm_hash` between `sections` and
//! `span_hash`). M: the key at `NodeView`'s top level.

mod common;

use common::{Scratch, Server, encode_component, repository_root, spec_json, write};
use serde_json::Value;

#[test]
fn te8_nodes_carry_norm_hash_as_spec_show_does() {
    let scratch = Scratch::new("norm-http");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    write(
        &a,
        "docs/spec/broken.md",
        b"---\nid: MEC-BROKEN\n---\n\n# Broken\n\nbad \xff byte\n".as_slice(),
    );
    let server = Server::serve(&home, &cwd, &[&a]);
    for (reference, norm) in [
        (
            "RULE-SPRINT-COST",
            "\"b3:bb97b1d5b795143baa4c6e9b3cfc3d7f7c798239339680cf8ed0f3af1623ca4d\"",
        ),
        (
            "MEC-STAMINA",
            "\"b3:d28068144ffb79d558aa83cb5da6756bf2e5284b685d7df96671f2091218ce06\"",
        ),
        ("docs/features/stamina-tuning.md", "null"),
        ("docs/spec/broken.md", "null"),
    ] {
        let run = spec_json(&home, &cwd, &a, &["show", reference]);
        run.code(0);
        let reply = server.get(&format!(
            "/api/projects/lantern-keep/nodes/{}",
            encode_component(reference)
        ));
        assert_eq!(reply.status, 200, "{reference}: {}", reply.text());
        assert_eq!(reply.text(), run.document(), "{reference}: byte for byte");
        let text = reply.text();
        assert!(
            text.contains(&format!("],\"norm_hash\":{norm},\"span_hash\":\"b3:")),
            "{reference}: norm_hash after sections, before span_hash: {text}"
        );
        let json: Value = serde_json::from_str(text).expect("JSON");
        let top: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(top, ["nodes", "notes", "reason", "ref"], "{reference}");
    }
    drop(server);

    let keys = std::fs::read_to_string(repository_root().join("fixtures/daemon-keys.json"))
        .expect("daemon-keys.json");
    assert!(
        keys.contains("\n  \"NodeView\": [\"ref\", \"reason\", \"notes\", \"nodes\"],\n"),
        "NodeView lists its four top-level keys only"
    );
    // ui-node-bindings (TE-1): the node's set holds the key, between
    // `sections` and `span_hash`; no other set does.
    let fixture: Value = serde_json::from_str(&keys).expect("daemon-keys.json");
    let holding: Vec<&String> = fixture
        .as_object()
        .expect("sets")
        .iter()
        .filter(|(_, set)| {
            set.as_array()
                .expect("a set")
                .contains(&Value::from("norm_hash"))
        })
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        holding,
        ["ShownNode"],
        "the norm key in the node's set alone"
    );
    let node: Vec<&str> = fixture["ShownNode"]
        .as_array()
        .expect("ShownNode")
        .iter()
        .map(|key| key.as_str().expect("a key"))
        .collect();
    let at = node
        .iter()
        .position(|key| *key == "norm_hash")
        .expect("norm_hash");
    assert_eq!(
        node[at - 1..=at + 1],
        ["sections", "norm_hash", "span_hash"]
    );
}
