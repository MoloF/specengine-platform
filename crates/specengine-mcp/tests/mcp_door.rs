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
//! Compiles to nothing with `--features probes` (the AC names the default
//! build; the measurement build lists the demo and probe tools).

#![cfg(all(unix, not(feature = "probes")))]

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

use common::read::{ERAS, Era, Session, assert_bad_arguments};
use common::*;
use serde_json::{Value, json};

/// `git args` in `dir`: a cleared environment, no global or system config,
/// a fixed identity.
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

/// Valid and invalid calls of each read tool; a tool missing here fails
/// the test when it is listed.
fn calls_of(tool: &str) -> Option<(Vec<Value>, Vec<Value>)> {
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

    let mut called = BTreeSet::new();
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&cwd), Home::At(&home));
        let list = session.tools();
        for name in tool_names(&list) {
            let (valid, invalid) = calls_of(&name)
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
    assert_eq!(
        called.into_iter().collect::<Vec<_>>(),
        ["get_context_bundle", "get_node", "get_tree", "search"]
    );

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
