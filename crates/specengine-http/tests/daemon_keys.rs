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
//!
//! AC-14 of docs/features/decision-staging.md, its Rust half ("Data",
//! "Documents"): `Proposal` holds `staged`, `staged_at` between `task_id`
//! and `notes` (45 keys), `InboxEntry` `staged_at` after `record_id` (12
//! keys); two sets more, 34 in all, the stage's two shapes as the review
//! document serves them after a same-origin POST: `StagedApprove` (6 keys,
//! from a staged approve of the update, with its span hash, and of the
//! discrepancy, with an option and a note) and `StagedReject` (2 keys, from
//! the question's staged reject). M: the reject shape's `reason` dropped.
//!
//! T-04 of docs/features/ui-round.md, its Rust half ("Data"): six sets
//! more, 40 in all, last: `RoundSheet`, `RoundItem`, `RoundTarget`,
//! `RoundTask` from `GET round` on A (a question with a staged reject, a
//! discrepancy with a staged option) and on A in state S (a question
//! waited on by T-0003); `RoundAnswersOutcome` (`{project, answers,
//! notes}`) and `RoundAnswerEntry` (`{id, outcome, stage, staged_at,
//! replaced, reason}`) from a `POST round/answers` on A staging one answer
//! (replacing a stage) and refusing another. M: an item key dropped from
//! the fixture.
//!
//! AC-T4 of docs/features/kind-interpretation.md, its Rust half ("Data",
//! "Daemon keys"): `Proposal` holds `interpretation` between `staged_at`
//! and `notes` (46 keys, the update's, question's and discrepancy's
//! reviews alike: `null` there); one set more, 41 in all, then last:
//! `Interpretation` (`{decision, how, why}`) from the review of an
//! interpretation of `RULE-STAM-REGEN` carrying `DEC-0023`, raised through
//! `spec propose interpretation`. M: a key renamed in Rust.
//!
//! AC-T2 of docs/features/health-left.md, its Rust half ("Data"): three
//! sets more, 44 in all, last: `HealthReport` (`{documents, amendments,
//! notes}`, the CLI's `HealthOutcome` as `spec health --json` sends it),
//! `UnappliedAmendment` (`{path, id, title, class, status, links}`) and
//! `ShownLink` (the nine keys of a `spec show --links` link) from `GET
//! health` on A and on a copy of A holding amendments written in (a spec
//! draft amending `RULE-STAM-REGEN`, an accepted decision whose links are
//! resolved, an alias, dangling and in a Tier 3 file). M: a key dropped
//! from the CLI struct.
//!
//! TE-3 of docs/features/rev-discipline.md, its Rust half ("Review
//! document, stage, queue"): `Proposal` holds `revisions` after `conflict`
//! (47 keys), `StagedApprove` `editorial` after `canon` (7 keys); one set
//! more, 45 in all, last: `RevisionRow`, the five keys of a `revisions`
//! entry (`{id, rev, to, lock_entries, refused}`, Q15), from the reviews of
//! the update of `EDGE-STAM-ZERO` and the interpretation of
//! `RULE-STAM-REGEN`, each changing its node's meaning. M: `editorial`
//! dropped from the stage; `refused` dropped from the entry.
//!
//! TE-11 of docs/features/drift-queue.md, its Rust half ("Review, inbox,
//! round, stage"): `Proposal` holds `drift` after `revisions` (48 keys,
//! `null` on every other kind; the drift items' reviews served with the
//! same keys); one set more, 46 in all, last: `DriftEvidence`
//! (`{hash, rows, more}`) from the reviews of the drift items raised by
//! `spec propose drift` on `fixtures/code-a` with its golden lock and
//! `CAP` edited. M: `drift` dropped from the review document.

mod common;
mod task_state;

use std::fmt;
use std::fs;

use common::{
    Scratch, Server, ask, propose_update, repository_root, run_product, spec_bin, spec_json,
};
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
    // kind-interpretation: an interpretation of RULE-STAM-REGEN carrying
    // DEC-0023 (its review's `interpretation` an object).
    let interpretation = interpret(&home, &cwd, &a);

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
    // health-left: A holding amendments written in.
    let amended = check_copy(&scratch, "amended", "lk-amended", |root| {
        for (path, text) in AMENDMENTS {
            fs::write(root.join(path), text).expect("an amendment");
        }
    });

    // drift-queue: fixtures/code-a with its golden lock committed, `CAP`
    // edited and committed, its drift raised (a drift item's review has a
    // `drift` object).
    let code = drift_repo(&scratch, &home);

    let server = Server::serve(&home, &cwd, &[&a, &observed, &cannot, &amended, &code.0]);
    let p = "/api/projects/lantern-keep";
    // decision-staging: each proposal staged, as a same-origin page stages,
    // before the inbox and the reviews are read.
    for (id, fields) in [
        (&update, "\"decision\":\"approve\""),
        (
            &report,
            "\"decision\":\"approve\",\"option\":1,\"note\":\"keep the cap\"",
        ),
        (
            &question,
            "\"decision\":\"reject\",\"reason\":\"asked before\"",
        ),
    ] {
        let read = server
            .get(&format!("{p}/proposals/{id}"))
            .status(200)
            .json();
        let updated_at = read["updated_at"].as_str().expect("updated_at");
        server
            .stage(
                &format!("{p}/proposals/{id}/decision"),
                &format!("{{{fields},\"updated_at\":\"{updated_at}\"}}"),
            )
            .status(200);
    }
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
    let interpretations = vec![
        server
            .get(&format!("{p}/proposals/{interpretation}"))
            .status(200)
            .text()
            .to_owned(),
    ];
    let drift_reviews: Vec<String> = code
        .1
        .iter()
        .map(|id| {
            server
                .get(&format!("/api/projects/code-a/proposals/{id}"))
                .status(200)
                .text()
                .to_owned()
        })
        .collect();
    // rev-discipline: the entries of the reviews whose `revisions` is an
    // array (the update's and the interpretation's; the question's and the
    // discrepancy's are `null`).
    let revision_rows: Vec<String> = [&reviews[1], &interpretations[0]]
        .iter()
        .flat_map(|text| raw_items(text, "revisions"))
        .collect();
    assert_eq!(revision_rows.len(), 2, "one entry each: {revision_rows:?}");
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
    // health-left: the amended copy's unapplied amendments, and A's none.
    let healths: Vec<String> = ["lk-amended", "lantern-keep"]
        .iter()
        .map(|slug| {
            server
                .get(&format!("/api/projects/{slug}/health"))
                .status(200)
                .text()
                .to_owned()
        })
        .collect();
    // ui-round: A's sheet, then its answers (after every other read of A).
    let round_a = server
        .get(&format!("{p}/round"))
        .status(200)
        .text()
        .to_owned();
    let shown = serde_json::from_str::<serde_json::Value>(&round_a).expect("the sheet")["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["id"] == serde_json::json!(question))
        .expect("the question on the sheet")["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned();
    let answers = vec![
        server
            .stage(
                &format!("{p}/round/answers"),
                &serde_json::json!({"project": "lantern-keep", "answers": [
                    {"id": question, "stage": {"decision": "approve", "option": null,
                        "answer": "Yes.", "canon": null, "note": null, "updated_at": shown}},
                    {"id": "PR-0099", "stage": {"decision": "reject", "reason": "gone",
                        "updated_at": shown}}]})
                .to_string(),
            )
            .status(200)
            .text()
            .to_owned(),
    ];
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
    let round_s = tasks_server
        .get(&format!("{p}/round"))
        .status(200)
        .text()
        .to_owned();
    drop(tasks_server);
    let sheets = vec![round_a, round_s];
    let round_items = items(&sheets, "items");
    // The packages whose `key` holds an object (or an array): not `null`.
    let holding = |key: &str, open: char| -> Vec<String> {
        packages
            .iter()
            .filter(|text| text.contains(&format!("\"{key}\":{open}")))
            .cloned()
            .collect()
    };
    let snapshots = objects(&holding("spec_snapshot", '{'), "spec_snapshot");
    let amendments = items(&healths, "amendments");
    assert_eq!(amendments.len(), 2, "the amended copy's two: {healths:?}");

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
        (
            "Proposal",
            one_key_set(
                "Proposal",
                &[
                    reviews.clone(),
                    interpretations.clone(),
                    drift_reviews.clone(),
                ]
                .concat(),
                keys_of,
            ),
        ),
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
        (
            "StagedApprove",
            one_key_set("StagedApprove", &objects(&reviews[1..], "staged"), keys_of),
        ),
        (
            "StagedReject",
            one_key_set("StagedReject", &objects(&reviews[..1], "staged"), keys_of),
        ),
        ("RoundSheet", one_key_set("RoundSheet", &sheets, keys_of)),
        ("RoundItem", one_key_set("RoundItem", &round_items, keys_of)),
        (
            "RoundTarget",
            one_key_set("RoundTarget", &items(&round_items, "targets"), keys_of),
        ),
        (
            "RoundTask",
            one_key_set("RoundTask", &items(&round_items, "tasks"), keys_of),
        ),
        (
            "RoundAnswersOutcome",
            one_key_set("RoundAnswersOutcome", &answers, keys_of),
        ),
        (
            "RoundAnswerEntry",
            one_key_set("RoundAnswerEntry", &items(&answers, "answers"), keys_of),
        ),
        (
            "Interpretation",
            one_key_set(
                "Interpretation",
                &objects(&interpretations, "interpretation"),
                keys_of,
            ),
        ),
        (
            "HealthReport",
            one_key_set("HealthReport", &healths, keys_of),
        ),
        (
            "UnappliedAmendment",
            one_key_set("UnappliedAmendment", &amendments, keys_of),
        ),
        (
            "ShownLink",
            one_key_set("ShownLink", &items(&amendments, "links"), keys_of),
        ),
        (
            "RevisionRow",
            one_key_set("RevisionRow", &revision_rows, keys_of),
        ),
        (
            "DriftEvidence",
            one_key_set("DriftEvidence", &objects(&drift_reviews, "drift"), keys_of),
        ),
    ]
}

/// drift-queue, TE-11
/// (`docs/features/drift-queue.md` "Review, inbox, round, stage"):
/// `fixtures/code-a` as a repository, the golden lock
/// (`fixtures/spec-lock/code-a.lock`) committed, `CAP` edited and committed;
/// `spec propose drift` raised there with `HOME` `home`: the root and the
/// IDs it created (the fixture's own `PIECE-COUNTER` and `LAW-BOUND`).
fn drift_repo(scratch: &Scratch, home: &std::path::Path) -> (std::path::PathBuf, Vec<String>) {
    let root = scratch.repo("code-a", "code", "main");
    let git = scratch.git();
    fs::copy(
        common::fixture("spec-lock").join("code-a.lock"),
        root.join("spec.lock"),
    )
    .expect("the golden lock");
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "the lock"]);
    common::replace(
        &root,
        "crates/tally/src/counter.rs",
        "CAP: u32 = 10;",
        "CAP: u32 = 12;",
    );
    git.run(&root, &["commit", "-q", "-a", "-m", "the cap"]);
    let run = run_product(
        &spec_bin(),
        home,
        &root,
        &["propose", "drift", "--json"],
        b"",
    );
    run.code(0);
    let document = run.json();
    let ids: Vec<String> = document["created"]
        .as_array()
        .expect("created")
        .iter()
        .map(|created| created["id"].as_str().expect("an ID").to_owned())
        .collect();
    assert_eq!(ids, ["PR-0001", "PR-0002"], "{document}");
    (root, ids)
}

/// health-left's amendments, written into a copy of A: a spec draft and
/// an accepted decision whose links are resolved, an alias written raw,
/// dangling and a node in a Tier 3 file.
const AMENDMENTS: [(&str, &str); 2] = [
    (
        "docs/features/regen-delay.md",
        "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
         [RULE-STAM-REGEN]\n---\n\n# Regeneration waits after a sprint\n\n## Delay \
         {#AC-01}\n\nRegeneration starts 1.5 s after the last sprint.\n",
    ),
    (
        "docs/records/DEC/DEC-0031.md",
        "---\nid: DEC-0031\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n\
         links:\n  amends: [EDGE-STAM-ZERO, QST-031, RULE-NOPE, DEC-0007]\n---\n\n\
         # Amber light at zero stamina\n",
    ),
];

/// `spec --root <root> propose interpretation RULE-STAM-REGEN --decision
/// DEC-0023 …` against its current span hash, stored (exit 0): its ID.
fn interpret(home: &std::path::Path, cwd: &std::path::Path, root: &std::path::Path) -> String {
    let shown = spec_json(home, cwd, root, &["show", "RULE-STAM-REGEN"]).json();
    let node = &shown["nodes"][0];
    let base = node["span_hash"].as_str().expect("span_hash").to_owned();
    let text = node["text"].as_str().expect("text").replacen(
        "Base rate 10 units/s,",
        "Base rate 10 units/s at rest,",
        1,
    );
    let root = root.to_str().expect("UTF-8 root");
    let run = run_product(
        &spec_bin(),
        home,
        cwd,
        &[
            "--root",
            root,
            "propose",
            "interpretation",
            "RULE-STAM-REGEN",
            "--base",
            &base,
            "--decision",
            "DEC-0023",
            "--how",
            "Rest is any speed under 40 % of walking.",
            "--why",
            "DEC-0023 says rest; the rule names no speed.",
            "--text-file",
            "-",
            "--rationale",
            "Carry DEC-0023 into the rule.",
            "--author-role",
            "spec-author",
        ],
        text.as_bytes(),
    );
    run.code(0);
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert!(id.starts_with("PR-"), "not stored: {}", run.show());
    id
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
    // The counts the spec names: 4 project keys, 12 inbox-entry keys
    // (decision-staging: `staged_at` after `record_id`, never `staged`).
    assert_eq!(sets[0].1, ["slug", "name", "root", "branch"]);
    assert_eq!(sets[1].1.len(), 12, "{:?}", sets[1].1);
    assert!(!sets[1].1.contains(&"task_id".to_owned()));
    assert!(!sets[1].1.contains(&"staged".to_owned()));
    assert_eq!(sets[1].1[10..], ["record_id", "staged_at"]);
    // docs/features/task-package.md "Data", AC-10: the review document
    // gains `task_id` after `choice`; decision-staging `staged`,
    // `staged_at` after it, before `notes`; docs/features/kind-interpretation.md
    // `interpretation` after `staged_at`: 46 keys; docs/features/rev-discipline.md
    // `revisions` after `conflict`: 47; docs/features/drift-queue.md `drift`
    // after `revisions`: 48.
    assert_eq!(sets[2].0, "Proposal");
    assert_eq!(sets[2].1.len(), 48, "{:?}", sets[2].1);
    let conflict = sets[2].1.iter().position(|key| key == "conflict");
    assert_eq!(
        sets[2].1[conflict.expect("conflict") + 1..conflict.expect("conflict") + 3],
        ["revisions", "drift"],
        "{:?}",
        sets[2].1
    );
    assert_eq!(
        sets[2].1[42..],
        [
            "choice",
            "task_id",
            "staged",
            "staged_at",
            "interpretation",
            "notes"
        ]
    );
    // ui-live's nine: 11, 7, 8, 2; 6, 7, 8, 6, 2 keys; a finding's `fix`
    // and `debt` both there; a plain check never sends a base's keys.
    let names: Vec<&str> = sets.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names.len(),
        46,
        "{names:?} (health-left: 41 -> 44; rev-discipline: 45; drift-queue: 46)"
    );
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
    let counts: Vec<(&str, usize)> = sets[15..32]
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
    // decision-staging's two, last: the stage's shapes, keys in the canon's
    // order ("The stage").
    assert_eq!(sets[32].0, "StagedApprove");
    // docs/features/rev-discipline.md: `editorial` after `canon`, 7 keys.
    assert_eq!(
        sets[32].1,
        [
            "decision",
            "option",
            "answer",
            "canon",
            "editorial",
            "note",
            "span_hash"
        ]
    );
    assert_eq!(sets[33].0, "StagedReject");
    assert_eq!(sets[33].1, ["decision", "reason"]);
    // ui-round's six, last (docs/features/ui-round.md "Data", T-04): the
    // sheet and its parts as `docs/canon/round.md` "JSON" sends them, the
    // answers document and its entry.
    let round: Vec<&str> = sets[34..40].iter().map(|(name, _)| *name).collect();
    assert_eq!(
        round,
        [
            "RoundSheet",
            "RoundItem",
            "RoundTarget",
            "RoundTask",
            "RoundAnswersOutcome",
            "RoundAnswerEntry"
        ]
    );
    assert_eq!(sets[35].1.len(), 19, "{:?}", sets[35].1);
    assert_eq!(sets[37].1, ["id", "status", "title"]);
    assert_eq!(sets[38].1, ["project", "answers", "notes"]);
    assert_eq!(
        sets[39].1,
        ["id", "outcome", "stage", "staged_at", "replaced", "reason"]
    );
    // kind-interpretation's one ("Data", "Daemon keys").
    assert_eq!(sets[40].0, "Interpretation");
    assert_eq!(sets[40].1, ["decision", "how", "why"]);
    // health-left's three, last (docs/features/health-left.md "Data").
    assert_eq!(sets[41].0, "HealthReport");
    assert_eq!(sets[41].1, ["documents", "amendments", "notes"]);
    assert_eq!(sets[42].0, "UnappliedAmendment");
    assert_eq!(
        sets[42].1,
        ["path", "id", "title", "class", "status", "links"]
    );
    assert_eq!(sets[43].0, "ShownLink");
    assert_eq!(
        sets[43].1,
        [
            "type", "origin", "at", "name", "written", "path", "line", "state", "reason"
        ]
    );
    // rev-discipline's one, last: docs/features/rev-discipline.md
    // "Review document, stage, queue" ("5 keys", `refused` last).
    assert_eq!(sets[44].0, "RevisionRow");
    assert_eq!(sets[44].1, ["id", "rev", "to", "lock_entries", "refused"]);
    // drift-queue's one, last:
    // `docs/features/drift-queue.md` "Review, inbox, round, stage"
    // (`{hash, rows, more}` as stored, rows untyped).
    assert_eq!(sets[45].0, "DriftEvidence");
    assert_eq!(sets[45].1, ["hash", "rows", "more"]);
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

/// docs/features/kind-decision.md AC-T4 ("Data": `fixtures/daemon-keys.json`
/// unchanged, no new key): the fixture holds 46 sets (41 until
/// docs/features/health-left.md added its three, AC-T2,
/// docs/features/rev-discipline.md its `RevisionRow`, TE-3, and
/// docs/features/drift-queue.md its `DriftEvidence`, TE-11); a decision
/// raised through `spec propose decision` (with its patch's linked update,
/// a staged answer) is served with the fixture's `Proposal` keys in order
/// (its update's review too), its stage with `StagedApprove`'s, its inbox
/// entries with `InboxEntry`'s, its round item with `RoundItem`'s and
/// `RoundTarget`'s. M: a review key added.
#[test]
fn kind_decision_ac_t4_a_decision_is_served_with_the_fixture_key_sets() {
    let text = fs::read_to_string(repository_root().join("fixtures/daemon-keys.json"))
        .expect("fixtures/daemon-keys.json");
    let fixture: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let sets = fixture.as_object().expect("an object of sets");
    assert_eq!(
        sets.len(),
        46,
        "46 sets: 41, health-left's three (docs/features/health-left.md AC-T2), \
         rev-discipline's `RevisionRow` (TE-3) and drift-queue's `DriftEvidence`"
    );
    let set = |name: &str| -> Vec<String> {
        sets[name]
            .as_array()
            .unwrap_or_else(|| panic!("no set {name}"))
            .iter()
            .map(|key| key.as_str().expect("a key").to_owned())
            .collect()
    };
    // 48 since docs/features/drift-queue.md added `drift`.
    assert_eq!(set("Proposal").len(), 48);

    let scratch = Scratch::new("daemon-keys-kd");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let shown = spec_json(&home, &cwd, &a, &["show", "RULE-STAM-REGEN"]).json();
    let node = &shown["nodes"][0];
    let rest = node["text"].as_str().expect("text").replacen(
        "Base rate 10 units/s,",
        "Base rate 10 units/s at rest,",
        1,
    );
    let input = serde_json::json!({
        "node_ids": ["RULE-STAM-REGEN"], "text": "Does regeneration wait for rest?",
        "working_answer": "Yes: it starts 1.5 s after the last stamina use.",
        "price_of_other": "Without the wait R-28 is rebalanced.",
        "proposed_patch": {"target": "RULE-STAM-REGEN", "base": node["span_hash"],
            "text": rest, "rationale": "Rest only."},
        "distinct_from": ["DEC-0023"]
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
            "decision",
            "--input",
            "-",
            "--author-role",
            "developer",
        ],
        input.as_bytes(),
    );
    run.code(0);
    assert!(
        run.stdout.starts_with("PR-0001\nlinked: PR-0002\n"),
        "{}",
        run.show()
    );

    let server = Server::serve(&home, &cwd, &[&a]);
    let p = "/api/projects/lantern-keep";
    let read = server
        .get(&format!("{p}/proposals/PR-0001"))
        .status(200)
        .json();
    let updated_at = read["updated_at"].as_str().expect("updated_at");
    server
        .stage(
            &format!("{p}/proposals/PR-0001/decision"),
            &format!(
                "{{\"decision\":\"approve\",\"answer\":\"No.\",\"updated_at\":\"{updated_at}\"}}"
            ),
        )
        .status(200);
    let decision = server
        .get(&format!("{p}/proposals/PR-0001"))
        .status(200)
        .text()
        .to_owned();
    let update = server
        .get(&format!("{p}/proposals/PR-0002"))
        .status(200)
        .text()
        .to_owned();
    assert!(decision.contains("\"kind\":\"decision\""), "{decision}");
    assert_eq!(keys_of(&decision), set("Proposal"), "the decision's review");
    assert_eq!(keys_of(&update), set("Proposal"), "its update's review");
    assert_eq!(
        keys_of(&raw_object(&decision, "staged")),
        set("StagedApprove")
    );
    let inbox = server
        .get(&format!("{p}/inbox"))
        .status(200)
        .text()
        .to_owned();
    let entries = raw_items(&inbox, "proposals");
    assert_eq!(entries.len(), 2, "{inbox}");
    for entry in &entries {
        assert_eq!(keys_of(entry), set("InboxEntry"), "{entry}");
    }
    let round = server
        .get(&format!("{p}/round"))
        .status(200)
        .text()
        .to_owned();
    let items = raw_items(&round, "items");
    assert_eq!(items.len(), 1, "the decision alone: {round}");
    assert!(items[0].contains("\"kind\":\"decision\""), "{}", items[0]);
    assert_eq!(keys_of(&items[0]), set("RoundItem"));
    for target in raw_items(&items[0], "targets") {
        assert_eq!(keys_of(&target), set("RoundTarget"), "{target}");
    }
    drop(server);
}

/// docs/features/kind-amendment.md AC-T4 ("Row": `fixtures/daemon-keys.json`
/// unchanged, no new key) and AC-R8's HTTP half: the fixture holds 46 sets
/// (41 until docs/features/health-left.md added its three, AC-T2,
/// docs/features/rev-discipline.md its `RevisionRow`, TE-3, and
/// docs/features/drift-queue.md its `DriftEvidence`, TE-11); "Data"'s
/// amendment raised through `spec propose amendment` is
/// served with the fixture's `Proposal` keys in order (`kind`
/// `amendment`, `base_hash` null, `target_ids` the file and its own new
/// ID), its inbox entry with `InboxEntry`'s; a same-origin `POST
/// .../decision` with `option`, `answer` or `canon` is 400 naming the
/// amendment, nothing staged; an approve stages with `StagedApprove`'s
/// keys, `span_hash` null; the round lists no item. M: a key renamed in
/// Rust.
#[test]
fn kind_amendment_ac_t4_an_amendment_is_served_with_the_fixture_key_sets() {
    let text = fs::read_to_string(repository_root().join("fixtures/daemon-keys.json"))
        .expect("fixtures/daemon-keys.json");
    let fixture: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let sets = fixture.as_object().expect("an object of sets");
    assert_eq!(
        sets.len(),
        46,
        "46 sets: 41, health-left's three (docs/features/health-left.md AC-T2), \
         rev-discipline's `RevisionRow` (TE-3) and drift-queue's `DriftEvidence`"
    );
    let set = |name: &str| -> Vec<String> {
        sets[name]
            .as_array()
            .unwrap_or_else(|| panic!("no set {name}"))
            .iter()
            .map(|key| key.as_str().expect("a key").to_owned())
            .collect()
    };
    // 48 since docs/features/drift-queue.md added `drift`.
    assert_eq!(set("Proposal").len(), 48);

    let scratch = Scratch::new("daemon-keys-ka");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let path = "docs/features/regen-delay.md";
    let amendment = "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
                     [RULE-STAM-REGEN]\n---\n\n# Regeneration waits after a sprint\n\n## Delay \
                     {#AC-01}\n\nRegeneration starts 1.5 s after the last sprint, not after any \
                     stamina use.\n";
    let run = run_product(
        &spec_bin(),
        &home,
        &a,
        &[
            "--root",
            a.to_str().unwrap(),
            "propose",
            "amendment",
            path,
            "--text-file",
            "-",
            "--rationale",
            "Regeneration waits after a sprint.",
            "--author-role",
            "spec-author",
        ],
        amendment.as_bytes(),
    );
    run.code(0);
    assert_eq!(run.stdout, "PR-0001\nintroduced: 0\n", "{}", run.show());

    let server = Server::serve(&home, &cwd, &[&a]);
    let p = "/api/projects/lantern-keep";
    let review = server
        .get(&format!("{p}/proposals/PR-0001"))
        .status(200)
        .text()
        .to_owned();
    assert_eq!(keys_of(&review), set("Proposal"), "the amendment's review");
    let document: serde_json::Value = serde_json::from_str(&review).expect("JSON");
    assert_eq!(document["kind"], serde_json::json!("amendment"));
    assert_eq!(document["base_hash"], serde_json::Value::Null);
    assert_eq!(
        document["target_ids"],
        serde_json::json!([path, "regen-delay/AC-01"])
    );
    let updated_at = document["updated_at"].as_str().expect("updated_at");
    let inbox = server
        .get(&format!("{p}/inbox"))
        .status(200)
        .text()
        .to_owned();
    let entries = raw_items(&inbox, "proposals");
    assert_eq!(entries.len(), 1, "{inbox}");
    assert_eq!(keys_of(&entries[0]), set("InboxEntry"));
    assert!(
        entries[0].contains("\"kind\":\"amendment\""),
        "{}",
        entries[0]
    );

    for field in [
        "\"option\":0",
        "\"answer\":\"yes\"",
        "\"canon\":\"MEC-STAMINA\"",
    ] {
        let reply = server.stage(
            &format!("{p}/proposals/PR-0001/decision"),
            &format!("{{\"decision\":\"approve\",{field},\"updated_at\":\"{updated_at}\"}}"),
        );
        assert_eq!(reply.status, 400, "{field}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains("`PR-0001` is an amendment"),
            "{field}: {message}"
        );
    }
    let unstaged = server
        .get(&format!("{p}/proposals/PR-0001"))
        .status(200)
        .json();
    assert_eq!(
        unstaged["staged"],
        serde_json::Value::Null,
        "nothing staged"
    );
    server
        .stage(
            &format!("{p}/proposals/PR-0001/decision"),
            &format!("{{\"decision\":\"approve\",\"updated_at\":\"{updated_at}\"}}"),
        )
        .status(200);
    let staged = server
        .get(&format!("{p}/proposals/PR-0001"))
        .status(200)
        .text()
        .to_owned();
    assert_eq!(keys_of(&staged), set("Proposal"));
    let stage = raw_object(&staged, "staged");
    assert_eq!(keys_of(&stage), set("StagedApprove"));
    let stage: serde_json::Value = serde_json::from_str(&stage).expect("JSON");
    assert_eq!(stage["span_hash"], serde_json::Value::Null, "a new file");
    let round = server
        .get(&format!("{p}/round"))
        .status(200)
        .text()
        .to_owned();
    assert!(
        raw_items(&round, "items").is_empty(),
        "never on the sheet: {round}"
    );
    drop(server);
}
