//! AC-09 of docs/features/daemon-read.md, its Rust half: the key sets of
//! the daemon's documents on fixture A, `Project`, `InboxEntry`,
//! `Proposal` (the review document of an update, a question and a
//! discrepancy alike), `NodeView`, `SearchHit`, `BundleView`, in the order
//! the daemon sends them, equal `fixtures/daemon-keys.json`, which
//! `ui/src/api/daemonKeys.test.ts` compares with the UI's types.
//! `SPECENGINE_WRITE_DAEMON_KEYS=1` regenerates the file from the daemon
//! instead of asserting it unchanged. M: a review key missing.

mod common;

use std::fmt;
use std::fs;

use common::{Scratch, Server, ask, propose_update, repository_root, run_product, spec_bin};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};

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

/// The keys of the objects in `texts`, one list for all (asserted equal).
fn one_key_set(name: &str, texts: &[String], ordered: impl Fn(&str) -> Vec<String>) -> Vec<String> {
    let mut sets = texts.iter().map(|text| ordered(text));
    let first = sets.next().unwrap_or_else(|| panic!("{name}: no instance"));
    for other in sets {
        let (mut a, mut b) = (first.clone(), other.clone());
        a.sort();
        b.sort();
        assert_eq!(a, b, "{name}: every instance has the same keys");
    }
    first
}

/// The raw text of each item of `key`'s array in `json` (the order of
/// their keys kept).
fn raw_items(json: &str, key: &str) -> Vec<String> {
    // Re-serializing would sort the keys: cut the raw items out instead.
    let at = json
        .find(&format!("\"{key}\":["))
        .unwrap_or_else(|| panic!("no `{key}` array: {json}"));
    let bytes = json.as_bytes();
    let mut index = at + key.len() + 4;
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    let mut in_string = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'\\' {
                index += 1;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => {
                    if depth == 0 {
                        start = Some(index);
                    }
                    depth += 1;
                }
                b'}' | b']' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        out.push(json[start.take().unwrap()..=index].to_owned());
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    out
}

fn daemon_key_sets() -> Vec<(&'static str, Vec<String>)> {
    let scratch = Scratch::new("daemon-keys");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let question = ask(
        &home,
        &a,
        &["MEC-SPRINT", "EDGE-STAM-ZERO"],
        "Does a sprint end at zero?",
    );
    let update = propose_update(
        &home,
        &cwd,
        &a,
        "EDGE-STAM-ZERO",
        "## Depletion {#EDGE-STAM-ZERO}\n- Stamina reaches 0: `Exhausted` after 0.2 s.\n",
    );
    let input = serde_json::json!({
        "node_ids": ["MEC-SPRINT"], "summary": "Walking drains stamina in the build.",
        "gap_type": "contradicts", "severity": "high",
        "evidence": [{"file": "src/stamina.rs", "qpath": "stamina::drain", "lines": "3-9",
            "observed": "drains while walking", "documented": "only while sprinting"}],
        "options": [{"label": "code", "effect": "fix the code", "price": "1 item"},
            {"label": "spec", "effect": "allow walking", "price": "a rebalance"}],
        "recommendation": 0
    })
    .to_string();
    let run = run_product(
        &spec_bin(),
        &home,
        &a,
        &[
            "--root",
            a.to_str().unwrap(),
            "propose",
            "discrepancy",
            "--input",
            "-",
        ],
        input.as_bytes(),
    );
    run.code(0);
    let report = run.stdout.lines().next().unwrap().to_owned();

    let server = Server::serve(&home, &cwd, &[&a]);
    let p = "/api/projects/lantern-keep";
    let projects = server.get("/api/projects").status(200).text().to_owned();
    let inbox = server
        .get(&format!("{p}/inbox"))
        .status(200)
        .text()
        .to_owned();
    let reviews: Vec<String> = [&question, &update, &report]
        .iter()
        .map(|id| {
            server
                .get(&format!("{p}/proposals/{id}"))
                .status(200)
                .text()
                .to_owned()
        })
        .collect();
    let nodes: Vec<String> = ["MEC-STAMINA", "NOPE-1"]
        .iter()
        .map(|reference| {
            server
                .get(&format!("{p}/nodes/{reference}"))
                .text()
                .to_owned()
        })
        .collect();
    let search = server
        .get(&format!("{p}/search?query=stamina"))
        .status(200)
        .text()
        .to_owned();
    let bundles: Vec<String> = ["MEC-STAMINA", "NOPE-1"]
        .iter()
        .map(|reference| {
            server
                .get(&format!("{p}/bundle?node_ids={reference}"))
                .text()
                .to_owned()
        })
        .collect();

    vec![
        (
            "Project",
            one_key_set(
                "Project",
                &raw_items(&format!("{{\"all\":{projects}}}"), "all"),
                keys_of,
            ),
        ),
        (
            "InboxEntry",
            one_key_set("InboxEntry", &raw_items(&inbox, "proposals"), keys_of),
        ),
        ("Proposal", one_key_set("Proposal", &reviews, keys_of)),
        ("NodeView", one_key_set("NodeView", &nodes, keys_of)),
        (
            "SearchHit",
            one_key_set("SearchHit", &raw_items(&search, "hits"), keys_of),
        ),
        ("BundleView", one_key_set("BundleView", &bundles, keys_of)),
    ]
}

/// The file's text: one line per type, in the order above.
fn render(sets: &[(&str, Vec<String>)]) -> String {
    let mut out = String::from("{\n");
    for (index, (name, keys)) in sets.iter().enumerate() {
        let list: Vec<String> = keys.iter().map(|key| format!("\"{key}\"")).collect();
        out.push_str(&format!("  \"{name}\": [{}]", list.join(", ")));
        out.push_str(if index + 1 < sets.len() { ",\n" } else { "\n" });
    }
    out.push_str("}\n");
    out
}

#[test]
fn ac09_the_daemons_key_sets_are_fixtures_daemon_keys_json() {
    let sets = daemon_key_sets();
    let text = render(&sets);
    // The counts the spec names: 4 project keys, 11 inbox-entry keys.
    assert_eq!(sets[0].1, ["slug", "name", "root", "branch"]);
    assert_eq!(sets[1].1.len(), 11, "{:?}", sets[1].1);
    assert!(!sets[1].1.contains(&"task_id".to_owned()));
    assert!(!sets[2].1.contains(&"task_id".to_owned()));
    let path = repository_root().join("fixtures").join("daemon-keys.json");
    if std::env::var_os("SPECENGINE_WRITE_DAEMON_KEYS").is_some_and(|value| value == "1") {
        fs::write(&path, &text).expect("write fixtures/daemon-keys.json");
        return;
    }
    let written = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}; regenerate it with SPECENGINE_WRITE_DAEMON_KEYS=1 \
             cargo nextest run -p specengine-http --test daemon_keys",
            path.display()
        )
    });
    assert_eq!(
        written, text,
        "fixtures/daemon-keys.json is the daemon's key sets on A (regenerate with \
         SPECENGINE_WRITE_DAEMON_KEYS=1 only when the documents change on purpose)"
    );
}
