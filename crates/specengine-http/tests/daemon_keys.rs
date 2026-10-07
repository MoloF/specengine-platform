//! AC-09 of docs/features/daemon-read.md, its Rust half: the key sets of
//! the daemon's documents on fixture A, `Project`, `InboxEntry`,
//! `Proposal` (the review document of an update, a question and a
//! discrepancy alike), `NodeView`, `SearchHit`, `BundleView`, in the order
//! the daemon sends them, equal `fixtures/daemon-keys.json`, which
//! `ui/src/api/daemonKeys.test.ts` compares with the UI's types.
//! `SPECENGINE_WRITE_DAEMON_KEYS=1` regenerates the file from the daemon
//! instead of asserting it unchanged. M: a review key missing.
//!
//! AC-10 of docs/features/task-package.md: `Proposal` holds `task_id`
//! after `choice` (43 keys), `InboxEntry` none (11 keys).
//!
//! AC-09 of docs/features/ui-live.md, its Rust half: nine sets more, in
//! the order the daemon sends their keys. `GraphView`, `GraphNode`,
//! `GraphEdge`, `FollowedType` from graphs on A with nodes and edges (and
//! an exit-1 one); `CheckReport`, `CheckCounts`, `CheckFinding`,
//! `DebtEntry` (a `stale` entry), `CheckCause` (a `cannot_check` cause)
//! from checks on A (blocked), on a copy of A in `observe` mode with a
//! look-alike section ID in debt (a finding with both `fix` and `debt`),
//! a warning in debt and a stale entry, and on a copy whose
//! `.spec-debt.toml` is not TOML (cannot-check). `CheckFinding` is the
//! union of its instances: the order of the one instance holding every
//! key, every other instance's keys in that order. The debt dates are
//! centuries away: today's date never changes the answer.
//!
//! AC-07 of docs/features/ui-live-tasks.md, its Rust half: seventeen sets
//! more ("Key sets"), 32 in all, from the documents of A in state S
//! (`task_state`) served by a daemon of their own: `TaskList`,
//! `TaskListEntry` from `/tasks`; `TaskNotFound` from `/tasks/T-0099` and
//! `/tasks/foo`; `TaskPackage` and its parts (`TaskTarget`,
//! `TaskCriterion`, `TaskAssumption`, `TaskProposal`, `OwnerNote`,
//! `SpecSnapshot`, `SnapshotPlace`, `SnapshotNode`, `SnapshotDiff`,
//! `TaskClaim`, `TaskRun`, `TaskBundle`, `Author`) from T-0001 to T-0004,
//! each part from every package that holds one. M: a `TaskRun` key
//! dropped from the fixture.

mod common;
mod task_state;

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

/// The keys of the objects in `texts` as one list: the order of the
/// instance holding every key seen (asserted to exist), every instance's
/// keys a subsequence of it.
fn union_key_set(
    name: &str,
    texts: &[String],
    ordered: impl Fn(&str) -> Vec<String>,
) -> Vec<String> {
    let instances: Vec<Vec<String>> = texts.iter().map(|text| ordered(text)).collect();
    assert!(!instances.is_empty(), "{name}: no instance");
    let mut every: Vec<&String> = instances.iter().flatten().collect();
    every.sort();
    every.dedup();
    let whole = instances
        .iter()
        .find(|keys| every.iter().all(|key| keys.contains(key)))
        .unwrap_or_else(|| {
            panic!("{name}: no instance holds every key {every:?}: the corpus needs one")
        })
        .clone();
    for keys in &instances {
        let positions: Vec<usize> = keys
            .iter()
            .map(|key| whole.iter().position(|k| k == key).expect("in the union"))
            .collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{name}: {keys:?} is not in the order of {whole:?}"
        );
    }
    whole
}

/// The raw text of `key`'s object in `json` (its first occurrence).
fn raw_object(json: &str, key: &str) -> String {
    let at = json
        .find(&format!("\"{key}\":{{"))
        .unwrap_or_else(|| panic!("no `{key}` object: {json}"));
    let start = at + key.len() + 3;
    let bytes = json.as_bytes();
    let (mut depth, mut in_string, mut index) = (0usize, false, start);
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
                b'{' | b'[' => depth += 1,
                b'}' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return json[start..=index].to_owned();
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    panic!("`{key}`'s object does not end: {json}")
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

    // The check corpora (ui-live): A observed, with debt and a stale
    // entry; A that cannot be checked.
    let observed = check_copy(&scratch, "observed", "lk-observed", |root| {
        let config = fs::read_to_string(root.join("specengine.toml")).expect("config");
        fs::write(
            root.join("specengine.toml"),
            format!("{config}\n[check]\nmode = \"observe\"\n"),
        )
        .expect("config");
        let game = fs::read_to_string(root.join("docs/spec/game.md")).expect("game.md");
        fs::write(
            root.join("docs/spec/game.md"),
            format!("{game}\n## Look {{#{LOOKALIKE}}}\n\nA look-alike section ID.\n"),
        )
        .expect("game.md");
        fs::write(
            root.join(".spec-debt.toml"),
            format!(
                "[[debt]]\ncode = \"homoglyph\"\npath = \"docs/spec/game.md\"\n\
                 subject = \"{LOOKALIKE}\"\nreason = \"kept for the importer\"\n\
                 expires = \"2999-01-01\"\n\n\
                 [[debt]]\ncode = \"unknown-key\"\npath = \"docs/features/stamina-tuning.md\"\n\
                 subject = \"priority\"\nreason = \"kept for the playtest\"\n\
                 expires = \"2998-06-30\"\n\n\
                 [[debt]]\ncode = \"ref-dangling\"\npath = \"docs/none.md\"\n\
                 subject = \"X-9\"\nreason = \"the file is gone\"\nexpires = \"2997-03-15\"\n"
            ),
        )
        .expect("baseline");
    });
    let cannot = check_copy(&scratch, "cannot", "lk-cannot", |root| {
        fs::write(root.join(".spec-debt.toml"), "this is [ not toml\n").expect("baseline");
    });

    let server = Server::serve(&home, &cwd, &[&a, &observed, &cannot]);
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
    let graphs: Vec<String> = [
        "ref=MEC-SPRINT",
        "ref=MEC-STAMINA&impact=true&depth=2",
        "ref=NOPE-1",
    ]
    .iter()
    .map(|query| server.get(&format!("{p}/graph?{query}")).text().to_owned())
    .collect();
    let checks: Vec<String> = ["lantern-keep", "lk-observed", "lk-cannot"]
        .iter()
        .map(|slug| {
            server
                .get(&format!("/api/projects/{slug}/check"))
                .status(200)
                .text()
                .to_owned()
        })
        .collect();
    let items = |texts: &[String], key: &str| -> Vec<String> {
        texts.iter().flat_map(|text| raw_items(text, key)).collect()
    };
    let objects = |texts: &[String], key: &str| -> Vec<String> {
        texts.iter().map(|text| raw_object(text, key)).collect()
    };

    // ui-live-tasks: A in state S, its own HOME and daemon.
    let home_s = scratch.home("s");
    let s = task_state::repo_in_s(&scratch, &task_state::A, "s", &home_s);
    let tasks_server = Server::serve(&home_s, &cwd, &[&s]);
    let list = vec![
        tasks_server
            .get(&format!("{p}/tasks"))
            .status(200)
            .text()
            .to_owned(),
    ];
    let not_found: Vec<String> = ["T-0099", "foo"]
        .iter()
        .map(|id| {
            tasks_server
                .get(&format!("{p}/tasks/{id}"))
                .status(404)
                .text()
                .to_owned()
        })
        .collect();
    let packages: Vec<String> = ["T-0001", "T-0002", "T-0003", "T-0004"]
        .iter()
        .map(|id| {
            tasks_server
                .get(&format!("{p}/tasks/{id}"))
                .status(200)
                .text()
                .to_owned()
        })
        .collect();
    drop(tasks_server);
    // The packages whose `key` holds an object (or an array): not `null`.
    let holding = |key: &str, open: char| -> Vec<String> {
        packages
            .iter()
            .filter(|text| text.contains(&format!("\"{key}\":{open}")))
            .cloned()
            .collect()
    };
    let snapshots = objects(&holding("spec_snapshot", '{'), "spec_snapshot");

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
        ("GraphView", one_key_set("GraphView", &graphs, keys_of)),
        (
            "GraphNode",
            one_key_set("GraphNode", &items(&graphs, "nodes"), keys_of),
        ),
        (
            "GraphEdge",
            one_key_set("GraphEdge", &items(&graphs, "edges"), keys_of),
        ),
        (
            "FollowedType",
            one_key_set("FollowedType", &items(&graphs, "types"), keys_of),
        ),
        ("CheckReport", one_key_set("CheckReport", &checks, keys_of)),
        (
            "CheckCounts",
            one_key_set("CheckCounts", &objects(&checks, "counts"), keys_of),
        ),
        (
            "CheckFinding",
            union_key_set("CheckFinding", &items(&checks, "findings"), keys_of),
        ),
        (
            "DebtEntry",
            one_key_set("DebtEntry", &items(&checks, "stale"), keys_of),
        ),
        (
            "CheckCause",
            one_key_set("CheckCause", &items(&checks, "cannot_check"), keys_of),
        ),
        ("TaskList", one_key_set("TaskList", &list, keys_of)),
        (
            "TaskListEntry",
            one_key_set("TaskListEntry", &items(&list, "tasks"), keys_of),
        ),
        (
            "TaskNotFound",
            one_key_set("TaskNotFound", &not_found, keys_of),
        ),
        (
            "TaskPackage",
            one_key_set("TaskPackage", &packages, keys_of),
        ),
        (
            "TaskTarget",
            one_key_set("TaskTarget", &items(&packages, "targets"), keys_of),
        ),
        (
            "TaskCriterion",
            one_key_set("TaskCriterion", &items(&packages, "criteria"), keys_of),
        ),
        (
            "TaskAssumption",
            one_key_set("TaskAssumption", &items(&packages, "assumptions"), keys_of),
        ),
        (
            "TaskProposal",
            one_key_set("TaskProposal", &items(&packages, "open_proposals"), keys_of),
        ),
        (
            "OwnerNote",
            one_key_set("OwnerNote", &items(&packages, "owner_notes"), keys_of),
        ),
        (
            "SpecSnapshot",
            one_key_set("SpecSnapshot", &snapshots, keys_of),
        ),
        (
            "SnapshotPlace",
            one_key_set("SnapshotPlace", &objects(&snapshots, "place"), keys_of),
        ),
        (
            "SnapshotNode",
            one_key_set("SnapshotNode", &items(&snapshots, "nodes"), keys_of),
        ),
        (
            "SnapshotDiff",
            one_key_set(
                "SnapshotDiff",
                &items(&holding("snapshot_diff", '['), "snapshot_diff"),
                keys_of,
            ),
        ),
        (
            "TaskClaim",
            one_key_set(
                "TaskClaim",
                &objects(&holding("claim", '{'), "claim"),
                keys_of,
            ),
        ),
        (
            "TaskRun",
            one_key_set("TaskRun", &items(&packages, "runs"), keys_of),
        ),
        (
            "TaskBundle",
            one_key_set(
                "TaskBundle",
                &objects(&holding("bundle", '{'), "bundle"),
                keys_of,
            ),
        ),
        (
            "Author",
            one_key_set("Author", &objects(&packages, "author"), keys_of),
        ),
    ]
}

/// A section ID with a Cyrillic capital A (U+0410) among Latin letters.
const LOOKALIKE: &str = "RULE-LOOK\u{0410}LIKE";

/// A git copy of A at `dir` served as `slug`, `change` applied before the
/// commit.
fn check_copy(
    scratch: &Scratch,
    dir: &str,
    slug: &str,
    change: impl FnOnce(&std::path::Path),
) -> std::path::PathBuf {
    let root = scratch.copy("spec-a", dir);
    let config = fs::read_to_string(root.join("specengine.toml")).expect("config");
    assert_eq!(config.matches("slug = \"lantern-keep\"").count(), 1);
    fs::write(
        root.join("specengine.toml"),
        config.replace("slug = \"lantern-keep\"", &format!("slug = \"{slug}\"")),
    )
    .expect("config");
    change(&root);
    let git = scratch.git();
    git.run(
        &root,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    git.quiet(&root);
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "fixture"]);
    root
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
    // docs/features/task-package.md "Data", AC-10: the review document
    // gains `task_id` after `choice`, 43 keys; inbox entries unchanged.
    assert_eq!(sets[2].0, "Proposal");
    assert_eq!(sets[2].1.len(), 43, "{:?}", sets[2].1);
    assert_eq!(sets[2].1[40..], ["choice", "task_id", "notes"]);
    // ui-live's nine: 11, 7, 8, 2; 6, 7, 8, 6, 2 keys; a finding's `fix`
    // and `debt` both there; a plain check never sends a base's keys.
    let names: Vec<&str> = sets.iter().map(|(name, _)| *name).collect();
    assert_eq!(names.len(), 32, "{names:?}");
    let counts: Vec<(&str, usize)> = sets[6..15]
        .iter()
        .map(|(name, keys)| (*name, keys.len()))
        .collect();
    assert_eq!(
        counts,
        [
            ("GraphView", 11),
            ("GraphNode", 7),
            ("GraphEdge", 8),
            ("FollowedType", 2),
            ("CheckReport", 6),
            ("CheckCounts", 7),
            ("CheckFinding", 8),
            ("DebtEntry", 6),
            ("CheckCause", 2),
        ]
    );
    let finding = &sets[12].1;
    assert!(finding.contains(&"fix".to_owned()) && finding.contains(&"debt".to_owned()));
    for (name, keys) in &sets[10..13] {
        assert!(
            !keys.contains(&"new_debt".to_owned()) && !keys.contains(&"introduced".to_owned()),
            "{name}: {keys:?}"
        );
    }
    // ui-live-tasks' seventeen (docs/features/ui-live-tasks.md "Key
    // sets"), in that order, with their counts; the package's 25 keys in
    // the canon's order, its `runs` items the seven of a run.
    let counts: Vec<(&str, usize)> = sets[15..]
        .iter()
        .map(|(name, keys)| (*name, keys.len()))
        .collect();
    assert_eq!(
        counts,
        [
            ("TaskList", 2),
            ("TaskListEntry", 6),
            ("TaskNotFound", 2),
            ("TaskPackage", 25),
            ("TaskTarget", 4),
            ("TaskCriterion", 2),
            ("TaskAssumption", 2),
            ("TaskProposal", 6),
            ("OwnerNote", 2),
            ("SpecSnapshot", 3),
            ("SnapshotPlace", 4),
            ("SnapshotNode", 3),
            ("SnapshotDiff", 5),
            ("TaskClaim", 4),
            ("TaskRun", 7),
            ("TaskBundle", 3),
            ("Author", 4),
        ]
    );
    assert_eq!(sets[18].1[..2], ["schema_version", "id"]);
    assert_eq!(sets[18].1[24], "notes");
    assert_eq!(
        sets[29].1,
        [
            "run",
            "role",
            "started_at",
            "ended_at",
            "outcome",
            "summary",
            "changed_files"
        ]
    );
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
