//! AC-08 of docs/features/mcp-read.md (08 AC-3, the single door), default
//! build: a temp git repository of spec-a, committed clean, the server's
//! working directory inside it; every tool `tools/list` names is called
//! with valid and invalid arguments (a listed tool without calls here fails
//! the test); every listed resource and two template URIs are read; then
//! `git status --porcelain --ignored` is empty, the tree (`.git` included)
//! is byte-identical, and every new path is under the scratch `HOME`.
//!
//! M: a tool writing under the root; a new tool without a call.
//!
//! AC-01 of docs/features/agent-intake.md: the queue tools join the door —
//! a change to `RULE-STAM-REGEN` against `get_node`'s `span_hash` (stored),
//! a stale one, questions (stored, then a hit), a discrepancy with a
//! proposed patch (stored with its linked update), `get_proposal` on them,
//! and invalid calls of each; the repository stays byte-identical (`.git`
//! included: no commit, no ref), `git status` empty, every new path under
//! `HOME`, and the queue holds what was proposed. M: `propose_change`
//! writes its target.
//!
//! Compiles to nothing with `--features probes` (the AC names the default
//! build; the measurement build lists the demo and probe tools).

#![cfg(all(unix, not(feature = "probes")))]

mod common;

/// Only its [`scratch_git::QUIET`]: the repository-local config that keeps
/// git's automatic maintenance out of the repository (see that module).
#[path = "../../specengine-cli/tests/common/git.rs"]
mod scratch_git;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

use common::read::{ERAS, Era, Session, TOOLS, assert_bad_arguments, content_text};
use common::*;
use serde_json::{Value, json};

/// `git args` in `dir`: a cleared environment, no global or system config,
/// a fixed identity. A repository it makes gets [`scratch_git::QUIET`] in
/// its own config right after `init`, so neither this git nor the server's
/// starts a background repack that changes `.git` under the snapshot.
fn git(dir: &Path, home: &Path, args: &[&str]) -> String {
    let program = if Path::new("/usr/bin/git").exists() {
        "/usr/bin/git"
    } else {
        "git"
    };
    let output = Command::new(program)
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CEILING_DIRECTORIES", dir.parent().expect("parent"))
        .env("GIT_AUTHOR_NAME", "Scratch Author")
        .env("GIT_AUTHOR_EMAIL", "author@example.invalid")
        .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00+0000")
        .env("GIT_COMMITTER_NAME", "Scratch Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@example.invalid")
        .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00+0000")
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 git output")
}

/// Valid and invalid calls of each tool (`base`: `RULE-STAM-REGEN`'s
/// `span_hash`); a tool missing here fails the test when it is listed.
fn calls_of(tool: &str, base: &str) -> Option<(Vec<Value>, Vec<Value>)> {
    let regen = "## Regeneration {#RULE-STAM-REGEN}\n\nStamina regenerates only at rest.\n";
    let calls = match tool {
        "get_tree" => (
            vec![
                json!({}),
                json!({"root": "DOM-MOVEMENT", "depth": 1, "kinds": ["mechanic"], "archive": true}),
                json!({"root": "DOM-NOPE"}),
                json!({"depth": -1}),
            ],
            vec![json!({"depth": "x"}), json!({"bogus": true})],
        ),
        "get_node" => (
            vec![
                json!({"id": "MEC-STAMINA"}),
                json!({"id": "MEC-STAMINA", "with": ["links"], "archive": true}),
                json!({"id": "docs/spec/game.md"}),
                json!({"id": "MEC-NOPE"}),
                json!({"id": "M\u{0415}C-STAMINA"}),
            ],
            vec![json!({}), json!({"id": "R-12", "with": ["bindings"]})],
        ),
        "search" => (
            vec![
                json!({"query": "stamina", "kinds": ["requirement"], "limit": 5, "archive": true}),
                json!({"query": "ab"}),
            ],
            vec![json!({"query": ["stamina"]}), json!({"limit": 3})],
        ),
        "get_context_bundle" => (
            vec![
                json!({"node_ids": ["MEC-STAMINA"], "budget": 10000}),
                json!({"node_ids": ["MEC-STAMINA"]}),
                json!({"node_ids": ["MEC-NOPE"]}),
                json!({"node_ids": ["MEC-STAMINA"], "budget": 0}),
            ],
            vec![json!({"node_ids": "MEC-STAMINA"}), json!({"budget": 100})],
        ),
        "propose_change" => (
            vec![
                json!({"kind": "update", "target": "RULE-STAM-REGEN", "base": base,
                    "text": regen, "rationale": "Rest only.", "author_role": "writer"}),
                json!({"kind": "update", "target": "RULE-STAM-REGEN", "base": "b3:00",
                    "text": regen, "rationale": "Stale.", "author_role": "writer"}),
                json!({"kind": "update", "target": "RULE-NOPE", "base": base,
                    "text": regen, "rationale": "None.", "author_role": "writer",
                    "author_model": "m-1", "run": "r-1"}),
            ],
            vec![
                json!({"kind": "update", "target": "RULE-STAM-REGEN", "base": base,
                    "text": regen, "rationale": "No author."}),
                json!({"kind": "delete", "target": "RULE-STAM-REGEN", "base": base,
                    "text": regen, "rationale": "Kind.", "author_role": "writer"}),
                json!({"kind": "update", "target": "RULE-STAM-REGEN", "base": base,
                    "text_file": "x.md", "rationale": "Path.", "author_role": "writer"}),
            ],
        ),
        "ask_question" => (
            vec![
                json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "Does the sprint stop at zero?",
                    "working_answer": "Yes.", "price_of_other": "A new case.",
                    "author_role": "writer"}),
                json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "does the sprint  stop at zero?",
                    "working_answer": "Yes.", "price_of_other": "A new case.",
                    "severity": "high", "author_role": "writer"}),
                json!({"node_ids": ["RULE-STAM-REGEN"], "text": "Rest only?",
                    "working_answer": "Yes.", "price_of_other": "A new case.",
                    "author_role": "writer"}),
                json!({"node_ids": [], "text": "None?", "working_answer": "Yes.",
                    "price_of_other": "A new case.", "author_role": "writer"}),
            ],
            vec![
                json!({"node_ids": "EDGE-STAM-ZERO", "text": "T", "working_answer": "W",
                    "price_of_other": "P", "author_role": "writer"}),
                json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "T", "working_answer": "W",
                    "price_of_other": "P", "author_role": "writer", "bogus": true}),
                json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "T", "working_answer": "W",
                    "price_of_other": "P", "author_role": "writer", "severity": "urgent"}),
            ],
        ),
        "report_discrepancy" => {
            let discrepancy = |patch: Option<Value>| {
                let mut args = json!({
                    "node_ids": ["RULE-STAM-REGEN", "EDGE-STAM-ZERO"],
                    "summary": "Regeneration starts while walking.",
                    "gap_type": "contradicts", "severity": "normal",
                    "evidence": [{"file": "src/stamina.rs", "qpath": "regen", "lines": "3-9",
                        "observed": "regenerates while walking", "documented": "only at rest"}],
                    "options": [{"label": "code", "effect": "fix the code", "price": "1 item"},
                        {"label": "spec", "effect": "allow walking", "price": "a rebalance"}],
                    "recommendation": 0, "distinct_from": ["DEC-0023"], "author_role": "writer"
                });
                if let Some(patch) = patch {
                    args["proposed_patch"] = patch;
                }
                args
            };
            (
                vec![
                    discrepancy(Some(json!({"target": "RULE-STAM-REGEN", "base": base,
                        "text": regen, "rationale": "Rest only."}))),
                    discrepancy(None),
                    discrepancy(Some(json!({"target": "RULE-STAM-REGEN", "base": "b3:00",
                        "text": regen, "rationale": "Stale."}))),
                ],
                vec![
                    {
                        let mut args = discrepancy(None);
                        args["gap_type"] = json!("bogus");
                        args
                    },
                    {
                        let mut args = discrepancy(None);
                        args["options"] = json!([{"label": "only"}]);
                        args
                    },
                    {
                        let mut args = discrepancy(None);
                        args.as_object_mut().unwrap().remove("author_role");
                        args
                    },
                ],
            )
        }
        "get_proposal" => (
            vec![
                json!({"proposal_id": "PR-0001"}),
                json!({"proposal_id": "PR-0003"}),
                json!({"proposal_id": "PR-9999"}),
                json!({"proposal_id": "\u{0420}R-0001"}),
            ],
            vec![json!({}), json!({"proposal_id": 1})],
        ),
        _ => return None,
    };
    Some(calls)
}

#[test]
fn ac08_reads_write_nothing_under_the_project_root() {
    let scratch = Scratch::new("door");
    let home = scratch.home("h");
    let git_home = scratch.home("git");
    let root = scratch.copy("spec-a", "repo");
    git(
        &root,
        &git_home,
        &["init", "-q", "--template=", "-b", "main"],
    );
    for (key, value) in scratch_git::QUIET {
        git(&root, &git_home, &["config", "--local", key, value]);
    }
    git(&root, &git_home, &["add", "-A"]);
    git(&root, &git_home, &["commit", "-q", "-m", "spec-a"]);
    assert_eq!(
        git(
            &root,
            &git_home,
            &["--no-optional-locks", "status", "--porcelain", "--ignored"]
        ),
        "",
        "committed clean"
    );
    let tree_before = snapshot(&root);
    let paths_before: BTreeSet<String> = snapshot(scratch.path()).into_keys().collect();
    let cwd = root.join("docs").join("spec");
    let mut base = None;

    let mut called = BTreeSet::new();
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&cwd), Home::At(&home));
        let list = session.tools();
        let base = base
            .get_or_insert_with(|| {
                let reply = session.call("get_node", json!({"id": "RULE-STAM-REGEN"}));
                result(&reply)["structuredContent"]["nodes"][0]["span_hash"]
                    .as_str()
                    .expect("a span_hash")
                    .to_owned()
            })
            .clone();
        for name in tool_names(&list) {
            let (valid, invalid) = calls_of(&name, &base)
                .unwrap_or_else(|| panic!("{name} is listed but this test has no call for it"));
            for args in valid {
                let reply = session.call(&name, args.clone());
                assert!(
                    reply.get("result").is_some(),
                    "{era:?} {name} {args}: {reply}"
                );
            }
            for args in invalid {
                // AC-05: an error result, no JSON-RPC error.
                let reply = session.call(&name, args.clone());
                assert_bad_arguments(&reply, "", &format!("{era:?} {name} {args}"));
            }
            called.insert(name);
        }
        let mut uris: Vec<String> = Vec::new();
        for page in session.all_resources() {
            for resource in page["resources"].as_array().expect("resources") {
                uris.push(resource["uri"].as_str().expect("uri").to_owned());
            }
        }
        assert!(uris.len() > 10, "{era:?}: {uris:?}");
        uris.push("spec://lantern-keep/node/MEC-STAMINA%23RULE-STAM-REGEN".to_owned());
        uris.push("spec://lantern-keep/node/QST-031".to_owned());
        for uri in &uris {
            let reply = session.read(uri);
            assert!(reply.get("result").is_some(), "{era:?} {uri}: {reply}");
        }
        for uri in [
            "spec://lantern-keep/node/MEC-NOPE",
            "spec://other/tree",
            "spec://lantern-keep/node/%ZZ",
        ] {
            assert!(session.read(uri).get("error").is_some(), "{era:?} {uri}");
        }
        result(&session.request("resources/templates/list", json!({})));
        if era == Era::Stateless {
            result(&session.request("server/discover", json!({})));
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}");
    }
    assert_eq!(called.into_iter().collect::<Vec<_>>(), TOOLS);

    // The queue holds what the valid calls proposed, in call order: the
    // first era's question (its repeat a hit), change, and discrepancy with
    // its linked update (its repeat a hit); the second era's change only.
    let mut session = Session::open(Era::Legacy, &[], Some(&cwd), Home::At(&home));
    let mut stored = Vec::new();
    for number in 1..=6 {
        let id = format!("PR-{number:04}");
        let reply = session.call("get_proposal", json!({"proposal_id": id}));
        let result = result(&reply);
        let document = &result["structuredContent"];
        if result["isError"] == json!(true) {
            assert!(
                content_text(result).contains(&format!("`{id}`")),
                "{id}: {result}"
            );
            break;
        }
        stored.push((
            document["kind"].as_str().expect("kind").to_owned(),
            document["status"].as_str().expect("status").to_owned(),
            document["linked"].as_str().map(str::to_owned),
        ));
    }
    assert_eq!(
        stored,
        [
            ("question".to_owned(), "open".to_owned(), None),
            ("update".to_owned(), "open".to_owned(), None),
            (
                "discrepancy".to_owned(),
                "open".to_owned(),
                Some("PR-0004".to_owned())
            ),
            (
                "update".to_owned(),
                "open".to_owned(),
                Some("PR-0003".to_owned())
            ),
            ("update".to_owned(), "open".to_owned(), None),
        ]
    );
    drop(session.finish());

    assert_eq!(snapshot(&root), tree_before, "the repository changed");
    assert_eq!(
        git(
            &root,
            &git_home,
            &["--no-optional-locks", "status", "--porcelain", "--ignored"]
        ),
        "",
        "git status after the session"
    );
    let home_prefix = format!("{}/", home.strip_prefix(scratch.path()).unwrap().display());
    let mut new_paths = Vec::new();
    for path in snapshot(scratch.path()).into_keys() {
        if !paths_before.contains(&path) {
            assert!(
                path.starts_with(&home_prefix),
                "a new path outside HOME: {path}"
            );
            new_paths.push(path);
        }
    }
    assert!(
        new_paths
            .iter()
            .any(|path| path.ends_with("specengine/lantern-keep.db")),
        "the index lives under HOME: {new_paths:?}"
    );
}
