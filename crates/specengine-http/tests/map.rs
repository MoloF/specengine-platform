//! docs/features/corpus-map.md (RM-0112, ADR-0080) "Acceptance criteria",
//! CM3, the daemon's half, through the real daemon over git repositories of
//! `fixtures/spec-a` (only read; the copies are scratch):
//!
//! - `GET …/map` and `?archive=true` (and `archive=false`) are byte-equal to
//!   `spec --root R map [--archive] --json` under one `HOME`, on the copy
//!   and on CM2's copy with its ghost.
//! - The JSON is never cut: past the CLI's text cut the route and the CLI
//!   JSON hold every node, byte for byte alike.
//! - `?x=1`, `archive=yes` and `archive` twice are 400s naming the name;
//!   nothing runs. The 404 list names `map` after `catalog`.

#![cfg(unix)]

mod common;

use std::path::PathBuf;

use common::{Scratch, Server, beyond_daemon, replace, spec_json, write};
use serde_json::Value;

const KEEP: &str = "lantern-keep";

/// A git copy of `fixtures/spec-a` and a daemon over it.
struct Keep {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
}

impl Keep {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.repo("spec-a", "keep", "main");
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self {
            _scratch: scratch,
            root,
            home,
            cwd,
        }
    }

    /// `GET …/map<query>` against `spec map <args> --json`: 200 and the
    /// same bytes; the document.
    fn same(&self, server: &Server, query: &str, args: &[&str]) -> Value {
        let mut all = vec!["map"];
        all.extend_from_slice(args);
        let run = spec_json(&self.home, &self.cwd, &self.root, &all);
        run.code(0);
        let path = format!("/api/projects/{KEEP}/map{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 200, "{path}: {}", reply.text());
        assert_eq!(reply.text(), run.document(), "{path} vs spec {all:?}");
        reply.json()
    }
}

/// CM3: the route is the CLI's JSON byte for byte, with and without the
/// archive, before and after CM2's ghost is written in.
#[test]
fn cm3_get_map_is_the_clis_document() {
    let keep = Keep::new("cm3-route");
    let server = Server::serve(&keep.home, &keep.cwd, &[&keep.root]);
    let document = keep.same(&server, "", &[]);
    assert_eq!(document["counts"]["edges"], 35, "{document}");
    assert_eq!(document["archive"], false);
    let document = keep.same(&server, "?archive=true", &["--archive"]);
    assert_eq!(document["counts"]["documents"], 13, "{document}");
    keep.same(&server, "?archive=false", &[]);
    replace(
        &keep.root,
        "docs/spec/movement/sprint.md",
        "depends_on: [MEC-STAMINA]",
        "depends_on: [MEC-STAMINA, MEC-GONE]",
    );
    replace(
        &keep.root,
        "docs/spec/movement/stamina.md",
        "\n## Regeneration {#RULE-STAM-REGEN}",
        "\nOnce paired with [[MEC-GONE]].\n\n## Regeneration {#RULE-STAM-REGEN}",
    );
    let document = keep.same(&server, "", &[]);
    assert_eq!(document["ghosts"][0]["written"], "MEC-GONE", "{document}");
    assert_eq!(document["ghosts"][0]["places"], 2);
    keep.same(&server, "?archive=true", &["--archive"]);
}

/// CM3: where the CLI's text is cut, the JSON (the CLI's and the route's,
/// byte-equal) holds every node.
#[test]
fn cm3_the_route_never_cuts() {
    let keep = Keep::new("cm3-uncut");
    let many = 400;
    for n in 0..many {
        write(
            &keep.root,
            &format!("docs/spec/many/m-{n:03}.md"),
            format!(
                "---\nid: MEC-MANY-{n:03}\nclass: canon\nparent: DOM-MOVEMENT\n---\n\n# Lamp wick {n:03} {}\n\nSee MEC-STAMINA.\n",
                "of the long stair below the lamp room".repeat(2)
            ),
        );
    }
    let text = common::spec(
        &keep.home,
        &keep.cwd,
        &["--root", keep.root.to_str().expect("UTF-8"), "map"],
    );
    text.code(0);
    assert!(
        text.stdout
            .lines()
            .last()
            .is_some_and(|line| line.starts_with("[truncated: ")),
        "the premise: the text is cut"
    );
    let server = Server::serve(&keep.home, &keep.cwd, &[&keep.root]);
    let document = keep.same(&server, "", &[]);
    assert_eq!(
        document["nodes"].as_array().expect("nodes").len(),
        19 + many
    );
}

/// CM3: the query's 400s, each naming the name, and nothing run; the 404
/// list names `map` after `catalog`.
#[test]
fn cm3_the_map_query() {
    let keep = Keep::new("cm3-query");
    let fresh = keep._scratch.home("fresh");
    let server = Server::serve(&fresh, &keep.cwd, &[&keep.root]);
    for (query, message) in [
        ("?x=1", "`x` is no query name of map: it takes archive"),
        ("?archive=yes", "`archive=yes`: not `true` or `false`"),
        (
            "?archive=true&archive=true",
            "`archive` is given 2 times: it takes one value",
        ),
        (
            "?ref=MEC-STAMINA",
            "`ref` is no query name of map: it takes archive",
        ),
    ] {
        let path = format!("/api/projects/{KEEP}/map{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        assert_eq!(reply.error_message(), message, "{path}");
    }
    assert!(
        beyond_daemon(&fresh).is_empty(),
        "a 400 runs no call: {:?}",
        beyond_daemon(&fresh)
    );
    for path in [
        format!("/api/projects/{KEEP}/maps"),
        format!("/api/projects/{KEEP}/map/MEC-STAMINA"),
    ] {
        let reply = server.get(&path);
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
        assert!(
            reply
                .error_message()
                .contains(", delivery, catalog, map, rings, workspaces}"),
            "{path}: {}",
            reply.error_message()
        );
    }
}
