//! docs/features/scope-rings.md (RM-0113) "Acceptance criteria", the
//! daemon's half of SR4 (and SR1's "every command"), through the real
//! daemon over git repositories of `fixtures/spec-a` with "Data"'s config
//! (the fixture only read; the copies are scratch):
//!
//! - `GET …/rings` is byte-equal to `spec --root R rings --json` under one
//!   `HOME`: "Data"'s two rings, with a card on MEC-STAMINA (`in work`),
//!   with a root of `DOM-NONE` (the note), with a bad table (the note, no
//!   ring), without `[map]` (`{"notes":[],"rings":[]}`), and with a queue at
//!   a newer schema (exit 2 a 503, else the same document).
//! - The daemon never cuts: past the CLI's cut its sectors are all there.
//! - Any query name is a 400 naming it; the 404 list names `rings` after
//!   `map`.
//! - A config holding `[map]`, good or bad, leaves the other reads as they
//!   were (`tree`, `catalog`, `graph` byte-equal to the CLI). M: `map`
//!   missing from `RawFile`.

#![cfg(unix)]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::{Scratch, Server, read_text, spec, spec_json, write};
use serde_json::{Value, json};

const KEEP: &str = "lantern-keep";

/// "Data"'s config.
const RINGS: &str = "\n[[map.ring]]\ntitle = \"Movement\"\nroot  = \"DOM-MOVEMENT\"\nkinds = [\"mechanic\"]\n\n[[map.ring]]\ntitle = \"Rules\"\nkinds = [\"rule\"]\n";

/// A git copy of `fixtures/spec-a` with `config` appended to its
/// `specengine.toml` (left uncommitted: the daemon reads the working tree).
struct Keep {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    own: String,
}

impl Keep {
    fn new(label: &str, config: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.repo("spec-a", "keep", "main");
        let own = read_text(&root, "specengine.toml");
        write(&root, "specengine.toml", format!("{own}{config}"));
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self {
            _scratch: scratch,
            root,
            home,
            cwd,
            own,
        }
    }

    fn set(&self, config: &str) {
        write(
            &self.root,
            "specengine.toml",
            format!("{}{config}", self.own),
        );
    }

    fn serve(&self) -> Server {
        Server::serve(&self.home, &self.cwd, &[&self.root])
    }

    /// `GET …/<route>` against `spec --root R <args> --json` (exit 0): 200
    /// and the same bytes; the document.
    fn same(&self, server: &Server, route: &str, args: &[&str]) -> Value {
        let run = spec_json(&self.home, &self.cwd, &self.root, args);
        run.code(0);
        let path = format!("/api/projects/{KEEP}/{route}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 200, "{path}: {}", reply.text());
        assert_eq!(reply.text(), run.document(), "{path} vs spec {args:?}");
        reply.json()
    }
}

/// The sector names of every ring.
fn sector_names(document: &Value) -> Vec<Vec<String>> {
    document["rings"]
        .as_array()
        .expect("rings")
        .iter()
        .map(|ring| {
            ring["sectors"]
                .as_array()
                .expect("sectors")
                .iter()
                .map(|sector| sector["name"].as_str().expect("a name").to_owned())
                .collect()
        })
        .collect()
}

/// `sqlite3 db sql`, which must answer.
fn sql_at(db: &Path, sql: &str) {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(client)
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// SR4: the route is the CLI's document byte for byte in each state of
/// the copy, one daemon throughout.
#[test]
fn sr4_get_rings_is_the_clis_document() {
    let keep = Keep::new("sr4-route", RINGS);
    let server = keep.serve();
    let document = keep.same(&server, "rings", &["rings"]);
    assert_eq!(
        sector_names(&document),
        [
            vec!["MEC-SPRINT", "MEC-STAMINA"],
            vec![
                "RULE-CORE-LOOP",
                "RULE-MOVE-SPEEDS",
                "RULE-SPRINT-COST",
                "RULE-STAM-REGEN"
            ],
        ]
    );
    let root = keep.root.to_str().expect("UTF-8 root");
    spec(
        &keep.home,
        &keep.cwd,
        &[
            "--root",
            root,
            "card",
            "new",
            "--title",
            "Tune the stamina",
            "--target",
            "MEC-STAMINA",
        ],
    )
    .code(0);
    let document = keep.same(&server, "rings", &["rings"]);
    assert_eq!(document["rings"][0]["sectors"][1]["state"], "in work");

    keep.set(&RINGS.replace("DOM-MOVEMENT", "DOM-NONE"));
    let document = keep.same(&server, "rings", &["rings"]);
    assert_eq!(
        document["notes"],
        json!(["rings: ring 1 `Movement`: `DOM-NONE` resolves to no ID and no alias"])
    );

    keep.set(&format!("{RINGS}\n[map]\nx = 1\n"));
    let document = keep.same(&server, "rings", &["rings"]);
    assert_eq!(document["rings"], json!([]), "{document}");
    assert_eq!(document["notes"].as_array().expect("notes").len(), 1);

    keep.set("");
    let path = format!("/api/projects/{KEEP}/rings");
    let reply = server.get(&path);
    assert_eq!(reply.status, 200);
    assert_eq!(reply.text(), "{\"notes\":[],\"rings\":[]}");
}

/// SR4: a queue at a newer schema: the CLI's exit 2 is a 503, an answer the
/// same document.
#[test]
fn sr4_a_queue_the_read_cannot_open() {
    let keep = Keep::new("sr4-queue", RINGS);
    let root = keep.root.to_str().expect("UTF-8 root");
    spec(
        &keep.home,
        &keep.cwd,
        &[
            "--root",
            root,
            "card",
            "new",
            "--title",
            "Tune the stamina",
            "--target",
            "MEC-STAMINA",
        ],
    )
    .code(0);
    let server = keep.serve();
    let data = common::data_dir(&keep.home).join(format!("{KEEP}.db"));
    sql_at(&data, "PRAGMA user_version = 99;");
    let run = spec(&keep.home, &keep.cwd, &["--root", root, "rings", "--json"]);
    let reply = server.get(&format!("/api/projects/{KEEP}/rings"));
    match run.code {
        2 => assert_eq!(reply.status, 503, "exit 2 is a 503: {}", reply.text()),
        0 => {
            assert_eq!(reply.status, 200, "{}", reply.text());
            assert_eq!(reply.text(), run.document());
        }
        other => panic!("exit {other}: {}", run.show()),
    }
}

/// SR4: any query name is a 400 naming it, and nothing runs.
#[test]
fn sr4_any_query_name_is_a_400() {
    let keep = Keep::new("sr4-query", RINGS);
    let server = keep.serve();
    for (query, name) in [
        ("?x=1", "`x`"),
        ("?archive=true", "`archive`"),
        ("?kinds=rule", "`kinds`"),
        ("?ref=DOM-GAME", "`ref`"),
    ] {
        let path = format!("/api/projects/{KEEP}/rings{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(message.contains(name), "{path}: {message}");
    }
    let reply = server.get(&format!("/api/projects/{KEEP}/rings?x=1"));
    assert_eq!(
        reply.error_message(),
        "`x` is no query name of rings: it takes no query name"
    );
    for path in [
        format!("/api/projects/{KEEP}/ring"),
        format!("/api/projects/{KEEP}/rings/1"),
    ] {
        let reply = server.get(&path);
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains(", catalog, map, rings, workspaces}"),
            "{path}: `rings` after `map`: {message}"
        );
    }
}

/// SR4 ("Data"): the daemon's document is whole where the CLI cuts.
#[test]
fn sr4_the_daemon_never_cuts() {
    let keep = Keep::new("sr4-uncut", RINGS);
    let many = 450;
    for n in 0..many {
        write(
            &keep.root,
            &format!("docs/spec/wicks/w-{n:03}.md"),
            format!(
                "---\nid: MEC-WICK-{n:03}\nclass: canon\nparent: DOM-MOVEMENT\n---\n\n# Wick {n:03} {}\n",
                "of the long stair below the lamp room "
                    .repeat(2)
                    .trim_end()
            ),
        );
    }
    let server = keep.serve();
    let run = spec_json(&keep.home, &keep.cwd, &keep.root, &["rings"]);
    run.code(0);
    let cut: usize = sector_names(&run.json()).iter().map(Vec::len).sum();
    assert!(cut < 6 + many, "the premise: the CLI cuts ({cut})");
    let reply = server.get(&format!("/api/projects/{KEEP}/rings"));
    assert_eq!(reply.status, 200);
    let whole = sector_names(&reply.json());
    assert_eq!(
        whole.iter().map(Vec::len).collect::<Vec<_>>(),
        [2 + many, 4]
    );
}

/// SR1, the daemon's half: with "Data"'s `[map]`, and with a bad one, the
/// other reads are the CLI's, as they are without it.
#[test]
fn sr1_other_routes_load_a_config_holding_map() {
    let keep = Keep::new("sr1-routes", "");
    let server = keep.serve();
    let reads: [(&str, &[&str]); 3] = [
        ("tree", &["tree"]),
        ("graph?ref=MEC-STAMINA", &["graph", "MEC-STAMINA"]),
        ("catalog?kinds=mechanic", &["catalog", "--kind", "mechanic"]),
    ];
    let before: Vec<Value> = reads
        .iter()
        .map(|(route, args)| keep.same(&server, route, args))
        .collect();
    for config in [RINGS.to_owned(), format!("{RINGS}\n[map]\nx = 1\n")] {
        keep.set(&config);
        for ((route, args), want) in reads.iter().zip(&before) {
            let document = keep.same(&server, route, args);
            assert_eq!(&document, want, "{route} with {config:?}");
        }
        keep.same(&server, "rings", &["rings"]);
    }
}
