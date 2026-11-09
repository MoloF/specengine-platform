//! docs/features/workspaces.md (RM-0081, v4 step 1a) "Acceptance criteria",
//! WS-8, the MCP server's half: on a scratch copy of `fixtures/card-press`, its
//! `[[workspace]]` and `[[module]]` tables in place, `initialize` and
//! `tools/list` answer, the list is the 29 tools of before (D5: no tool is
//! added) and a read and a card tool answer with no error. With the first
//! `[[workspace]]` header misspelt `[[workspaces]]`, the session still opens
//! (neither call reads the config) and every tool that reads the project is an
//! error result naming `specengine.toml:<line>: ` and the TOML reader's
//! unknown-field message: the server's form of the CLI's exit 2. M18:
//! `workspace` or `module` missing from `RawFile`.
//!
//! The CLI's commands and the daemon's start are `specengine-cli`'s
//! `tests/workspace_cli.rs` and `specengine-http`'s `tests/workspaces.rs`.
//!
//! Every server has a cleared environment and a scratch `HOME`.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;

use common::read::{ERAS, Session, TOOLS, content_text};
use common::*;
use serde_json::{Value, json};

/// The project's tools the test calls: a read, the flow read and a card read.
const CALLS: [&str; 3] = ["get_tree", "get_flow", "list_cards"];

/// A scratch copy of card-press with its own `HOME`.
struct Press {
    _scratch: Scratch,
    home: std::path::PathBuf,
    root: std::path::PathBuf,
}

impl Press {
    fn new() -> Self {
        let scratch = Scratch::new("mcp-workspaces");
        let home = scratch.home("h");
        let root = scratch.copy("card-press", "press");
        Self {
            _scratch: scratch,
            home,
            root,
        }
    }

    fn session(&self, era: common::read::Era) -> Session {
        Session::open(era, &[], Some(&self.root), Home::At(&self.home))
    }
}

/// The names of a `tools/list` result.
fn tool_names(list: &Value) -> BTreeSet<String> {
    list["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// WS-8 on a session of each era: with the tables, every call answers; with
/// `[[workspaces]]`, the session opens and lists, and each project tool is an
/// error result carrying the config's line and the unknown-field message.
#[test]
fn ws8_the_server_loads_the_tables_and_names_a_misspelt_one() {
    let press = Press::new();
    let want: BTreeSet<String> = TOOLS.iter().map(|name| (*name).to_owned()).collect();
    for era in ERAS {
        let mut session = press.session(era);
        let names = tool_names(&session.tools());
        assert_eq!(names, want, "{era:?}: D5, no tool is added");
        assert!(
            names.iter().all(|name| !name.contains("workspace")),
            "{era:?}: {names:?}"
        );
        for tool in CALLS {
            let reply = session.call(tool, json!({}));
            let reply = result(&reply);
            assert_eq!(reply["isError"], json!(false), "{era:?} {tool}: {reply}");
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }

    let config = press.root.join("specengine.toml");
    let text = std::fs::read_to_string(&config).expect("config");
    let misspelt = text.replacen("\n[[workspace]]\n", "\n[[workspaces]]\n", 1);
    assert_ne!(misspelt, text, "card-press holds a `[[workspace]]` table");
    std::fs::write(&config, &misspelt).expect("config");
    let line = misspelt
        .lines()
        .position(|line| line == "[[workspaces]]")
        .expect("the misspelt header")
        + 1;
    let opening = format!("specengine.toml:{line}: unknown field `workspaces`, expected one of ");
    for era in ERAS {
        let mut session = press.session(era);
        assert_eq!(tool_names(&session.tools()), want, "{era:?}");
        for tool in CALLS {
            let reply = session.call(tool, json!({}));
            let reply = result(&reply);
            assert_eq!(reply["isError"], json!(true), "{era:?} {tool}: {reply}");
            assert!(
                content_text(reply).contains(&opening),
                "{era:?} {tool}: `{opening}...`: {reply}"
            );
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}
