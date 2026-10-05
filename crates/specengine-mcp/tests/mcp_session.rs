//! AC-06, AC-07 and AC-09 of docs/features/mcp-read.md.
//!
//! AC-06: started in an empty directory both eras start, `tools/list`
//! answers, every tool answers the CLI's exit-2 line naming `spec init`,
//! `resources/list` is empty and `resources/read` -32603; with `HOME` unset
//! every tool names `HOME`; `--root`, `--config` reach a project outside the
//! working directory. M: discovery at startup.
//!
//! AC-07: one session sees an edited file in the next `get_node` without
//! `spec index`, answers after its database is deleted, applies an edited
//! `[budgets] bundle_node` on the next bundle, and answers a request sent
//! after a bundle over 1 000 candidates first (legacy `ping`; `ping` is no
//! 2026-07-28 method, rmcp answers it -32601, so the stateless era sends
//! `resources/templates/list`); a cancelled bundle gets no response and the
//! next call answers. M: a kept outcome, project or DB handle; the call on
//! the runtime thread.
//!
//! AC-09: every spawn has a cleared environment and a new scratch `HOME`;
//! after one read `<HOME>/Library/Application Support/specengine/<slug>.db`
//! exists. M: the harness inheriting the env.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::Path;
use std::process::Stdio;

use common::read::{ERAS, Era, READ_TOOLS, Session, TOOLS, checked_call, cli_env, output_schemas};
use common::*;
use serde_json::{Value, json};
use specengine_cli::Globals;

/// Valid arguments of every tool, for a spec-a project (the queue tools'
/// of task spec `agent-intake` too).
fn valid_args(tool: &str) -> Value {
    match tool {
        "get_tree" => json!({}),
        "get_node" => json!({"id": "MEC-STAMINA"}),
        "search" => json!({"query": "stamina"}),
        "get_context_bundle" => json!({"node_ids": ["MEC-STAMINA"]}),
        "get_proposal" => json!({"proposal_id": "PR-0001"}),
        "propose_change" => json!({
            "kind": "update", "target": "EDGE-STAM-ZERO", "base": "b3:00", "text": "x",
            "rationale": "Why.", "author_role": "writer"
        }),
        "ask_question" => json!({
            "node_ids": ["EDGE-STAM-ZERO"], "text": "Why?", "working_answer": "Yes.",
            "price_of_other": "None.", "author_role": "writer"
        }),
        "report_discrepancy" => json!({
            "node_ids": ["EDGE-STAM-ZERO"], "summary": "It departs.", "gap_type": "partial",
            "severity": "low",
            "evidence": [{"file": "src/a.rs", "observed": "a", "documented": "b"}],
            "options": [
                {"label": "one", "effect": "e", "price": "p"},
                {"label": "two", "effect": "e", "price": "p"}
            ],
            "recommendation": 0, "author_role": "writer"
        }),
        other => panic!("no arguments for {other}"),
    }
}

// ------------------------------------------------------------------ AC-09

#[test]
fn ac09_the_harness_clears_the_environment_and_sets_only_home() {
    let scratch = Scratch::new("env");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let output = isolated_command("/usr/bin/env", &[], &cwd, Some(&home))
        .stdin(Stdio::null())
        .output()
        .expect("run env");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("HOME={}\n", home.display()),
        "the child's whole environment"
    );
    let output = isolated_command("/usr/bin/env", &[], &cwd, None)
        .stdin(Stdio::null())
        .output()
        .expect("run env");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "", "HOME unset");
}

#[test]
fn ac09_each_spawn_gets_a_new_scratch_home_holding_the_database() {
    let scratch = Scratch::new("homes");
    let root = scratch.copy("spec-a", "copy");
    let own_home = std::env::var_os("HOME");
    let mut seen = Vec::new();
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&root), Home::Fresh);
        let home = session.server.home().expect("a HOME").to_path_buf();
        assert!(
            home.starts_with(std::env::temp_dir().canonicalize().unwrap()),
            "{home:?} is not under the temp dir"
        );
        assert_ne!(Some(home.as_os_str().to_owned()), own_home, "{era:?}");
        assert!(!seen.contains(&home), "a HOME reused: {home:?}");
        assert_eq!(
            fs::read_dir(&home).unwrap().count(),
            0,
            "{era:?}: the HOME starts empty"
        );
        let reply = session.call("get_tree", json!({}));
        assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
        let db = data_dir(&home).join("lantern-keep.db");
        assert!(db.is_file(), "{era:?}: {db:?} after one read");
        let done = session.finish();
        assert!(done.status.success(), "{}", done.stderr);
        seen.push(home);
    }
    // The spawn's scratch is gone with its server.
    for home in &seen {
        assert!(!home.exists(), "{home:?} left behind");
    }
}

// ------------------------------------------------------------------ AC-06

#[test]
fn ac06_no_project_starts_both_eras_and_every_tool_names_spec_init() {
    let scratch = Scratch::new("noproject");
    let home = scratch.home("h");
    let empty = scratch.dir("empty");
    let env = cli_env(&empty, Some(&home));
    let globals = Globals::default();
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&empty), Home::At(&home));
        let list = session.tools();
        let names = tool_names(&list);
        if cfg!(feature = "probes") {
            assert!(
                READ_TOOLS
                    .iter()
                    .all(|name| names.iter().any(|listed| listed == name)),
                "{era:?}: {names:?}"
            );
        } else {
            assert_eq!(names, TOOLS, "{era:?}");
        }
        let schemas = output_schemas(&list);
        // Every tool, the queue tools of task spec `agent-intake` too.
        for tool in TOOLS {
            let result = checked_call(
                &mut session,
                &schemas,
                tool,
                valid_args(tool),
                &env,
                &globals,
            );
            assert_eq!(result["isError"], json!(true), "{era:?} {tool}");
            let text = common::read::content_text(&result);
            assert!(
                text.contains("spec init") && text.starts_with("spec: "),
                "{era:?} {tool}: {text:?}"
            );
        }
        let reply = session.resources(None);
        assert_eq!(result(&reply)["resources"], json!([]), "{era:?}");
        let reply = session.read("spec://lantern-keep/node/R-12");
        assert_eq!(error_code(&reply), INTERNAL_ERROR, "{era:?}: {reply}");
        assert!(
            reply["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("spec init")),
            "{reply}"
        );
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "");
    }
    assert_eq!(
        snapshot(&empty).len(),
        0,
        "nothing written in the working directory"
    );
}

#[test]
fn ac06_without_home_every_tool_names_home() {
    let scratch = Scratch::new("nohome");
    let root = scratch.copy("spec-a", "copy");
    let env = cli_env(&root, None);
    let globals = Globals::default();
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&root), Home::Unset);
        let list = session.tools();
        let schemas = output_schemas(&list);
        for tool in READ_TOOLS {
            let result = checked_call(
                &mut session,
                &schemas,
                tool,
                valid_args(tool),
                &env,
                &globals,
            );
            assert_eq!(result["isError"], json!(true), "{era:?} {tool}");
            let text = common::read::content_text(&result);
            assert!(text.contains("HOME"), "{era:?} {tool}: {text:?}");
        }
        let reply = session.resources(None);
        assert_eq!(result(&reply)["resources"], json!([]), "{era:?}");
        let reply = session.read("spec://lantern-keep/tree");
        assert_eq!(error_code(&reply), INTERNAL_ERROR, "{era:?}: {reply}");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
    // An empty or relative HOME is unusable too.
    for bad in ["", "relative/home"] {
        let mut session = Session::open(Era::Legacy, &[], Some(&root), Home::At(Path::new(bad)));
        let reply = session.call("get_tree", json!({}));
        let result = result(&reply);
        assert_eq!(result["isError"], json!(true), "HOME={bad:?}");
        assert!(
            common::read::content_text(result).contains("HOME"),
            "HOME={bad:?}: {result}"
        );
    }
}

#[test]
fn ac06_root_and_config_reach_a_project_outside_the_working_directory() {
    let scratch = Scratch::new("rootflag");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let empty = scratch.dir("elsewhere");
    let root_arg = root.to_str().expect("UTF-8 path").to_owned();
    // A config outside the root naming another slug.
    let config = scratch.join("cfg/alt.toml");
    let text = read_text(&root, "specengine.toml")
        .replace("slug = \"lantern-keep\"", "slug = \"lantern-alt\"");
    write(scratch.path(), "cfg/alt.toml", text);
    let config_arg = config.to_str().expect("UTF-8 path").to_owned();
    for (args, slug) in [
        (vec!["--root", root_arg.as_str()], "lantern-keep"),
        (
            vec!["--root", root_arg.as_str(), "--config", config_arg.as_str()],
            "lantern-alt",
        ),
    ] {
        let globals = Globals {
            root: Some(root.clone()),
            config: (args.len() == 4).then(|| config.clone()),
        };
        let env = cli_env(&empty, Some(&home));
        for era in ERAS {
            let mut session = Session::open(era, &args, Some(&empty), Home::At(&home));
            let schemas = output_schemas(&session.tools());
            for tool in READ_TOOLS {
                let result = checked_call(
                    &mut session,
                    &schemas,
                    tool,
                    valid_args(tool),
                    &env,
                    &globals,
                );
                assert_eq!(result["isError"], json!(false), "{era:?} {args:?} {tool}");
            }
            let reply = session.resources(None);
            assert_eq!(
                result(&reply)["resources"][0]["uri"],
                json!(format!("spec://{slug}/tree")),
                "{era:?} {args:?}"
            );
            let reply = session.read(&format!("spec://{slug}/node/R-12"));
            assert!(
                result(&reply)["contents"][0]["text"]
                    .as_str()
                    .is_some_and(|text| text.starts_with("R-12 | ")),
                "{reply}"
            );
            assert!(
                data_dir(&home).join(format!("{slug}.db")).is_file(),
                "{slug}.db"
            );
        }
    }
    // `--config` alone: the root is the working directory.
    let globals = Globals {
        root: None,
        config: Some(config.clone()),
    };
    let env = cli_env(&root, Some(&home));
    let args = ["--config", config_arg.as_str()];
    let mut session = Session::open(Era::Legacy, &args, Some(&root), Home::At(&home));
    let schemas = output_schemas(&session.tools());
    checked_call(
        &mut session,
        &schemas,
        "get_tree",
        json!({}),
        &env,
        &globals,
    );
    assert_eq!(
        snapshot(&empty).len(),
        0,
        "the working directory stays empty"
    );
}

// ------------------------------------------------------------------ AC-07

#[test]
fn ac07_one_session_reads_the_files_as_they_are_now() {
    let scratch = Scratch::new("fresh");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let env = cli_env(&root, Some(&home));
    let globals = Globals::default();
    for era in ERAS {
        replace(
            &root,
            "docs/records/R/R-12.md",
            "short delay;",
            "brief delay;",
        );
        let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
        let schemas = output_schemas(&session.tools());
        let before = checked_call(
            &mut session,
            &schemas,
            "get_node",
            json!({"id": "R-12"}),
            &env,
            &globals,
        );
        assert!(common::read::content_text(&before).contains("brief delay;"));
        // An edit shows in the next call, no `spec index`.
        replace(
            &root,
            "docs/records/R/R-12.md",
            "brief delay;",
            "short delay;",
        );
        let after = checked_call(
            &mut session,
            &schemas,
            "get_node",
            json!({"id": "R-12"}),
            &env,
            &globals,
        );
        assert!(
            common::read::content_text(&after).contains("short delay;"),
            "{era:?}: the edit is not shown"
        );
        // A new record shows in the next search and resource list.
        let new_path = format!(
            "docs/records/A/A-{}.md",
            if era == Era::Legacy { 103 } else { 104 }
        );
        let new_id = new_path
            .rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".md")
            .to_owned();
        write(
            &root,
            &new_path,
            format!(
                "---\nid: {new_id}\nclass: canon\nstatus: open\n---\n\n# Zephyrine note\n\nZephyrine.\n"
            ),
        );
        let found = checked_call(
            &mut session,
            &schemas,
            "search",
            json!({"query": "zephyrine"}),
            &env,
            &globals,
        );
        assert!(
            common::read::content_text(&found).contains(&new_id),
            "{era:?}: the new record is not found"
        );
        let reply = session.resources(None);
        assert!(
            result(&reply)["resources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|resource| resource["name"] == json!(new_id)),
            "{era:?}: the new record is not listed"
        );
        // The database deleted between calls: the next call answers.
        fs::remove_dir_all(data_dir(&home)).expect("delete the data directory");
        let again = checked_call(
            &mut session,
            &schemas,
            "get_node",
            json!({"id": "R-12"}),
            &env,
            &globals,
        );
        assert_eq!(again["isError"], json!(false), "{era:?}");
        assert!(data_dir(&home).join("lantern-keep.db").is_file());
        // An edited `bundle_node` applies on the next bundle.
        let bundle = checked_call(
            &mut session,
            &schemas,
            "get_context_bundle",
            json!({"node_ids": ["MEC-STAMINA"]}),
            &env,
            &globals,
        );
        assert_eq!(
            bundle["structuredContent"]["budget"],
            json!(2000),
            "{era:?}"
        );
        let config = read_text(&root, "specengine.toml");
        write(
            &root,
            "specengine.toml",
            format!("{config}\n[budgets]\nbundle_node = 4321\n"),
        );
        let bundle = checked_call(
            &mut session,
            &schemas,
            "get_context_bundle",
            json!({"node_ids": ["MEC-STAMINA"]}),
            &env,
            &globals,
        );
        assert_eq!(
            bundle["structuredContent"]["budget"],
            json!(4321),
            "{era:?}"
        );
        write(&root, "specengine.toml", config);
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "");
    }
}

/// spec-a plus 1 000 open questions on `MEC-STAMINA`: a bundle over 1 007
/// candidates.
fn thousand_candidates(root: &Path) {
    replace(
        root,
        "specengine.toml",
        "TERM = { kind = \"term\",        shape = \"name\" }\n",
        "TERM = { kind = \"term\",        shape = \"name\" }\nOQ   = { kind = \"question\",    width = 4 }\n",
    );
    let filler: String = (0..150)
        .map(|_| "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9} ")
        .collect();
    for n in 0..1_000 {
        write(
            root,
            &format!("docs/records/OQ/OQ-{n:04}.md"),
            format!(
                "---\nid: OQ-{n:04}\nclass: canon\nstatus: open\nworking_answer: A-101\n\
                 refs: [MEC-STAMINA]\nowner: owner\nreviewed: 2026-09-20\n---\n\n# Question {n}\n\n{filler}\n"
            ),
        );
    }
}

/// The quick request of the era: `ping` (legacy) or
/// `resources/templates/list` (2026-07-28 has no `ping`).
fn quick(session: &Session) -> (&'static str, Value) {
    match session.era {
        Era::Legacy => ("ping", json!({})),
        Era::Stateless => ("resources/templates/list", json!({})),
    }
}

#[test]
fn ac07_a_long_bundle_does_not_block_the_session() {
    let scratch = Scratch::new("pingfirst");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    thousand_candidates(&root);
    let bundle = json!({"name": "get_context_bundle", "arguments": {"node_ids": ["MEC-STAMINA"], "budget": 1000}});
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
        // Warm: the index is built, the candidates counted.
        let warm = session.request("tools/call", bundle.clone());
        let warm = result(&warm);
        assert_eq!(warm["isError"], json!(false), "{era:?}");
        let document = &warm["structuredContent"];
        let candidates =
            document["more"].as_u64().unwrap() + document["tail"].as_array().unwrap().len() as u64;
        assert!(candidates >= 900, "{era:?}: {candidates} left out");
        if era == Era::Stateless {
            let reply = session.request("ping", json!({}));
            assert_eq!(error_code(&reply), -32601, "stateless ping: {reply}");
        }
        // The bundle, then at once the quick request: it is answered first.
        let call_id = session.next_id();
        let params = session.params(bundle.clone());
        session.server.send(
            &json!({"jsonrpc": "2.0", "id": call_id, "method": "tools/call", "params": params}),
        );
        let quick_id = session.next_id();
        let (method, quick_params) = quick(&session);
        let params = session.params(quick_params);
        session
            .server
            .send(&json!({"jsonrpc": "2.0", "id": quick_id, "method": method, "params": params}));
        let first = session.server.recv();
        let second = session.server.recv();
        assert_eq!(
            first["id"],
            json!(quick_id),
            "{era:?}: {method} was not answered first: {}",
            clip(&first.to_string())
        );
        assert_eq!(second["id"], json!(call_id), "{era:?}");
        assert_eq!(result(&second)["isError"], json!(false));
        assert_eq!(
            result(&second)["structuredContent"],
            warm["structuredContent"],
            "{era:?}: the same bundle"
        );

        // A cancelled bundle: no response; the next call answers.
        let cancelled = session.next_id();
        let params = session.params(bundle.clone());
        session.server.send(
            &json!({"jsonrpc": "2.0", "id": cancelled, "method": "tools/call", "params": params}),
        );
        session.server.notify(
            "notifications/cancelled",
            Some(session.params(json!({"requestId": cancelled, "reason": "test"}))),
        );
        let next = session.call("get_node", json!({"id": "R-12"}));
        assert_eq!(result(&next)["isError"], json!(false), "{era:?}");
        let (method, quick_params) = quick(&session);
        result(&session.request(method, quick_params));
        std::thread::sleep(std::time::Duration::from_millis(1_500));
        let (method, quick_params) = quick(&session);
        result(&session.request(method, quick_params));
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert!(
            done.messages
                .iter()
                .all(|message| message["id"] != json!(cancelled)),
            "{era:?}: the cancelled bundle was answered: {:?}",
            done.messages
                .iter()
                .map(|m| clip(&m.to_string()))
                .collect::<Vec<_>>()
        );
    }
}
