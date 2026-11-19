//! docs/features/doc-library.md (RM-0127) "Acceptance criteria", the
//! daemon's half: `GET /api/projects/lantern-keep/library` over the scene
//! (the shared scene of `specengine-cli`'s `tests/library_scene`, a scratch
//! git copy of `fixtures/spec-a` with the spec's changes), a daemon of its
//! own under a scratch `HOME`.
//!
//! - DL2: the route and `?archive=true` (and `false`) are byte-equal to
//!   `spec --root R library [--archive] --json` less its LF; past the CLI's
//!   cut the route holds every row; `?kinds=x` and `?archive=maybe` are 400s
//!   naming the name, nothing run; an unknown route's 404 names `library`.
//!   The key sets are `daemon_keys.rs`'s.
//! - DL4, the daemon's half: at C1, C2, with stamina.md edited and not
//!   committed, and at C3, the route is the CLI's answer at that state.
//! - DL12: this repository through the route (the CLI's JSON is cut there):
//!   each ADR whose `canon:` resolves listed once under its canon page.
//! - DL10: a hub at `--slow-ms 0`: the first `GET …/library` line
//!   `index=walked`, the second `index=trusted`, both bodies the CLI's. M:
//!   the route served on `run` (the second line reads `walked`).
//! - DL11: a local daemon at `--slow-ms 0`: the second read at one `HEAD`
//!   spawns the catalog's warm count (the `rev-parse` calls alone); after
//!   reads at six commits C1-C6, `HEAD` back at C1 walks, back at C6 does
//!   not; the daemon constructs one
//!   `LastChangeCache` (`App.catalogs`) and no other. M: no cache (the
//!   second read walks); the eviction removed (C1's read has `git=4`).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

#[path = "../../specengine-cli/tests/library_scene/mod.rs"]
mod library_scene;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use common::hub::{ORIGIN, Proxied, hub_args, through};
use common::slow::{Request, requests_of, slow_ms};
use common::{
    Scratch, Server, beyond_daemon, files_ending, read_text, repository_root, root_args, spec,
};
use library_scene::Scene;
use serde_json::Value;

const ROUTE: &str = "/api/projects/lantern-keep/library";
/// A catalog read over the same files, for the warm count's yardstick.
const CATALOG: &str = "/api/projects/lantern-keep/catalog?kinds=mechanic";
/// The route as a slow line names it.
const TEMPLATE: &str = "/api/projects/{p}/library";

/// `spec --root R library [--archive] --json` under `home`, exit 0: its
/// document less its LF.
fn cli(scene: &Scene, home: &Path, archive: bool) -> String {
    let root = scene.root.to_str().expect("a UTF-8 root");
    let mut args = vec!["--root", root, "library", "--json"];
    if archive {
        args.push("--archive");
    }
    let run = spec(home, &scene.root, &args);
    run.code(0);
    run.document().to_owned()
}

/// `GET path` on `server`: 200, its body.
fn body(server: &Server, path: &str) -> String {
    let reply = server.get(path);
    assert_eq!(reply.status, 200, "{path}: {}", reply.text());
    reply.text().to_owned()
}

/// The route and its `archive` forms against the CLI under the scene's
/// `HOME`.
fn assert_route_is_the_cli(server: &Server, scene: &Scene, context: &str) {
    for (query, archive) in [
        ("", false),
        ("?archive=true", true),
        ("?archive=false", false),
    ] {
        assert_eq!(
            body(server, &format!("{ROUTE}{query}")),
            cli(scene, &scene.home, archive),
            "{context}: {query:?}"
        );
    }
}

/// A local daemon over the scene under its `HOME`.
fn serve(scene: &Scene) -> Server {
    let cwd = scene.dir("cwd");
    Server::serve(&scene.home, &cwd, &[&scene.root])
}

// ------------------------------------------------------------------ DL2

/// DL2: the route is the CLI's JSON byte for byte, with and without the
/// archive.
#[test]
fn dl2_the_route_is_the_clis_document() {
    let scene = Scene::new("http-dl2");
    let server = serve(&scene);
    assert_route_is_the_cli(&server, &scene, "C1");
    let document: Value = serde_json::from_str(&body(&server, ROUTE)).expect("JSON");
    assert_eq!(document["counts"]["documents"], 13, "{document}");
    let archived: Value =
        serde_json::from_str(&body(&server, &format!("{ROUTE}?archive=true"))).expect("JSON");
    assert_eq!(archived["counts"]["documents"], 15, "{archived}");
    assert_eq!(archived["archive"], true);
}

/// DL2, "The route": the browser view is never cut: where the CLI's JSON
/// is cut, the route holds every row.
#[test]
fn dl2_the_route_never_cuts() {
    let scene = Scene::new("http-dl2-uncut");
    let many = 400;
    for n in 0..many {
        scene.put(
            &format!("docs/spec/many/m-{n:03}.md"),
            &format!(
                "---\nid: MEC-MANY-{n:03}\nclass: canon\nparent: DOM-MOVEMENT\n---\n\n# Lamp \
                 wick {n:03} {}\n\nSee MEC-STAMINA.\n",
                "of the long stair below the lamp room "
                    .repeat(2)
                    .trim_end()
            ),
        );
    }
    scene.commit("2026-01-02T10:01:00Z", "Many wicks\n");
    let cut: Value = serde_json::from_str(&cli(&scene, &scene.home, false)).expect("JSON");
    assert_eq!(cut["truncated"], true, "premise: the CLI's JSON is cut");
    let server = serve(&scene);
    let whole: Value = serde_json::from_str(&body(&server, ROUTE)).expect("JSON");
    assert_eq!(whole["truncated"], false);
    assert_eq!(whole["omitted"], Value::Null);
    assert_eq!(
        whole["rows"].as_array().expect("rows").len(),
        13 + many,
        "every row"
    );
}

/// DL2: an unknown query name and a non-boolean `archive` are 400s naming
/// the name, nothing run; an unknown route's 404 names `library`.
#[test]
fn dl2_the_library_query() {
    let scene = Scene::new("http-dl2-query");
    let fresh = scene.dir("fresh-home");
    let cwd = scene.dir("cwd");
    let server = Server::serve(&fresh, &cwd, &[&scene.root]);
    let reply = server.get(&format!("{ROUTE}?kinds=x"));
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert_eq!(
        reply.error_message(),
        "`kinds` is no query name of library: it takes archive"
    );
    let reply = server.get(&format!("{ROUTE}?archive=maybe"));
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert!(
        reply.error_message().contains("`archive"),
        "names `archive`: {}",
        reply.error_message()
    );
    assert!(
        beyond_daemon(&fresh).is_empty(),
        "a 400 runs no call: {:?}",
        beyond_daemon(&fresh)
    );
    for path in [
        "/api/projects/lantern-keep/libraries".to_owned(),
        format!("{ROUTE}/MEC-STAMINA"),
    ] {
        let reply = server.get(&path);
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
        assert!(
            reply.error_message().contains(", library")
                || reply.error_message().contains("{library"),
            "{path}: {}",
            reply.error_message()
        );
    }
}

// ------------------------------------------------------------------ DL4

/// DL4, the daemon's half: at each new `HEAD` (and with an uncommitted
/// edit) the route is the CLI's answer.
#[test]
fn dl4_the_route_follows_each_head() {
    let scene = Scene::new("http-dl4");
    let server = serve(&scene);
    assert_route_is_the_cli(&server, &scene, "C1");
    let behind = |server: &Server| -> Value {
        let document: Value = serde_json::from_str(&body(server, ROUTE)).expect("JSON");
        document["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row["name"] == "MEC-STAMINA")
            .expect("stamina")["behind"]
            .clone()
    };
    assert_eq!(behind(&server), false);
    scene.c2();
    assert_route_is_the_cli(&server, &scene, "C2");
    assert_eq!(behind(&server), true);
    scene.stamina_edit();
    assert_route_is_the_cli(&server, &scene, "an uncommitted edit");
    assert_eq!(behind(&server), true);
    scene.c3();
    assert_route_is_the_cli(&server, &scene, "C3");
    assert_eq!(behind(&server), false);
}

// ------------------------------------------------------------------ DL10

/// DL10: on a hub at `--slow-ms 0`, the first library read walks, the
/// second trusts, and both bodies are the CLI's. M: the route on `run`.
#[test]
fn dl10_a_hub_trusts_the_library_read() {
    let scene = Scene::new("http-dl10");
    let cwd = scene.dir("cwd");
    let hub_home = scene.dir("hub-home");
    let server = Server::start(
        &hub_home,
        &cwd,
        &slow_ms(hub_args(&[&scene.root], ORIGIN), "0"),
    );
    let mut bodies = Vec::new();
    for _ in 0..2 {
        let reply = through(&server, "GET", ROUTE, &Proxied::fetch("mira"), b"");
        assert_eq!(reply.status, 200, "{}", reply.text());
        bodies.push(reply.text().to_owned());
    }
    let lines = requests_of(&server, TEMPLATE, 2);
    assert_eq!(
        lines
            .iter()
            .map(|line| line.index.as_str())
            .collect::<Vec<_>>(),
        ["walked", "trusted"],
        "{lines:#?}"
    );
    let want = cli(&scene, &scene.home, false);
    for body in &bodies {
        assert_eq!(body, &want);
    }
}

// ------------------------------------------------------------------ DL12

/// The front-matter value of `key` in `text`, as written.
fn front_value(text: &str, key: &str) -> Option<String> {
    let body = text.strip_prefix("---\n")?;
    let end = body.find("\n---")?;
    body[..end].lines().find_map(|line| {
        line.strip_prefix(&format!("{key}:"))
            .map(|value| value.trim().to_owned())
    })
}

/// The group "Data" gives a status.
fn group_of(status: Option<&str>) -> &'static str {
    match status {
        Some("accepted") => "accepted",
        Some(status) if status.starts_with("superseded-by ") => "superseded",
        Some("rejected") => "rejected",
        _ => "other",
    }
}

/// The map's node at the index `at`.
fn node_at<'a>(nodes: &'a [Value], at: &Value) -> &'a Value {
    &nodes[usize::try_from(at.as_u64().expect("an index")).expect("an index")]
}

/// DL12: over this repository (read only; a daemon under a scratch `HOME`,
/// the whole answer, since the CLI's JSON is cut at the output cap),
/// `GET …/library?archive=true` lists each ADR whose `canon:` resolves (the
/// map's `canon` edge out of it) exactly once, under the row of its canon
/// page, with its status's group; ADR-0083 under
/// `docs/canon/spec-check-write.md`, accepted.
#[test]
fn dl12_this_repository_lists_each_adr_under_its_canon_page() {
    let repository = repository_root();
    let config: toml::Table =
        toml::from_str(&read_text(&repository, "specengine.toml")).expect("the config");
    let slug = config["project"]["slug"]
        .as_str()
        .expect("a slug")
        .to_owned();
    let scratch = Scratch::new("dl12");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&repository]);
    let document: Value = serde_json::from_str(&body(
        &server,
        &format!("/api/projects/{slug}/library?archive=true"),
    ))
    .expect("JSON");
    assert_eq!(document["truncated"], false);
    let map: Value = serde_json::from_str(&body(
        &server,
        &format!("/api/projects/{slug}/map?archive=true"),
    ))
    .expect("JSON");
    drop(server);
    let rows = document["rows"].as_array().expect("rows");
    let nodes = map["nodes"].as_array().expect("nodes");
    let mut expected: BTreeMap<String, String> = BTreeMap::new();
    for edge in map["edges"].as_array().expect("edges") {
        let source = node_at(nodes, &edge["src"]);
        let path = source["path"].as_str().unwrap_or_default();
        if edge["type"] == "canon"
            && path.starts_with("docs/decisions/ADR-")
            && source["of"].is_null()
        {
            let name = source["name"].as_str().expect("a name").to_owned();
            let target = node_at(nodes, &edge["dst"])["path"]
                .as_str()
                .expect("a path")
                .to_owned();
            assert!(
                expected.insert(name.clone(), target).is_none(),
                "{name}: one canon edge"
            );
        }
    }
    let mut listed: Vec<(String, String, String)> = Vec::new();
    for row in rows {
        let path = row["path"].as_str().expect("a path");
        for decision in row["decisions"].as_array().expect("decisions") {
            listed.push((
                decision["name"].as_str().expect("a name").to_owned(),
                path.to_owned(),
                decision["group"].as_str().expect("a group").to_owned(),
            ));
        }
    }
    let paths: BTreeSet<&str> = rows
        .iter()
        .map(|row| row["path"].as_str().expect("a path"))
        .collect();
    let decisions_dir = repository.join("docs/decisions");
    let mut adrs: Vec<PathBuf> = fs::read_dir(&decisions_dir)
        .expect("docs/decisions")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.starts_with("ADR-") && name.ends_with(".md"))
        })
        .collect();
    adrs.sort();
    let mut checked = 0;
    for adr in &adrs {
        let text = fs::read_to_string(adr).expect("an ADR");
        let id = front_value(&text, "id").expect("an ADR's id");
        // The oracle's own sanity: a `canon:` naming a listed file with no
        // anchor resolves.
        if let Some(canon) = front_value(&text, "canon")
            && !canon.contains('#')
            && paths.contains(canon.as_str())
        {
            assert!(
                expected.contains_key(&id),
                "{id}: `canon: {canon}` resolves"
            );
        }
        let Some(target) = expected.get(&id) else {
            continue;
        };
        let entries: Vec<&(String, String, String)> =
            listed.iter().filter(|(name, _, _)| *name == id).collect();
        assert_eq!(entries.len(), 1, "{id}: listed once: {entries:?}");
        let status = front_value(&text, "status");
        assert_eq!(
            (&entries[0].1, entries[0].2.as_str()),
            (target, group_of(status.as_deref())),
            "{id}"
        );
        checked += 1;
    }
    assert!(checked > 60, "premise: most ADRs resolve ({checked})");
    let page = rows
        .iter()
        .find(|row| row["path"] == "docs/canon/spec-check-write.md")
        .expect("spec-check-write.md's row");
    let adr_0083: Vec<&Value> = page["decisions"]
        .as_array()
        .expect("decisions")
        .iter()
        .filter(|decision| decision["name"] == "ADR-0083")
        .collect();
    assert_eq!(adr_0083.len(), 1, "{page}");
    assert_eq!(adr_0083[0]["group"], "accepted");
    println!(
        "DL12: {} rows, {checked} ADRs checked, {} decisions listed",
        rows.len(),
        listed.len()
    );
}

// ------------------------------------------------------------------ DL11

/// A request line's `git` count.
fn git_of(line: &Request) -> u64 {
    line.cost
        .as_ref()
        .unwrap_or_else(|| panic!("write-cost's five fields: {}", line.line))
        .git
        .unwrap_or_else(|| panic!("a git count: {}", line.line))
}

/// DL11: one bounded cache. A warm read at one `HEAD` spawns what a warm
/// `GET …/catalog` spawns over the same cache (the `rev-parse` calls alone;
/// the spec's "`git=4`" is the catalog's count, which the daemon's lines
/// give as measured here); after reads at six commits C1-C6, `HEAD` back at
/// C1 walks, back at C6 does not. M: no cache (the second read walks); the
/// eviction removed (C1's read is warm).
#[test]
fn dl11_one_bounded_cache() {
    let scene = Scene::new("http-dl11");
    let cwd = scene.dir("cwd");
    let server = Server::start(&scene.home, &cwd, &slow_ms(root_args(&[&scene.root]), "0"));
    let mut count = 0;
    let mut read = || -> Request {
        body(&server, ROUTE);
        count += 1;
        let lines = requests_of(&server, TEMPLATE, count);
        assert_eq!(lines.len(), count, "{lines:#?}");
        lines[count - 1].clone()
    };
    let first = read();
    let second = read();
    // The catalog's warm read over the same cache at the same HEAD: the
    // library walked every file, so both catalog reads are warm.
    for _ in 0..2 {
        body(&server, CATALOG);
    }
    let catalog = requests_of(&server, "/api/projects/{p}/catalog", 2);
    let warm = git_of(&catalog[1]);
    assert_eq!(git_of(&catalog[0]), warm, "{catalog:#?}");
    assert!(warm <= 4, "rev-parse alone: {}", catalog[1].line);
    assert!(git_of(&first) > warm, "a first read walks: {}", first.line);
    assert_eq!(
        git_of(&second),
        warm,
        "the catalog's warm count (M: no cache): {}",
        second.line
    );
    let mut heads = vec![scene.c1.clone()];
    for n in 2..=6 {
        scene.append(
            "docs/records/A/A-101.md",
            &format!("\nWalking was timed again, round {n}.\n"),
        );
        heads.push(scene.commit(
            &format!("2026-01-02T10:0{}:00Z", n - 1),
            &format!("Round {n}\n"),
        ));
        let at = read();
        assert!(git_of(&at) > warm, "a new commit walks: {}", at.line);
    }
    assert_eq!(heads.len(), 6);
    scene.checkout(&heads[0]);
    let back = read();
    assert!(
        git_of(&back) > warm,
        "C1 left the four kept heads (M: no eviction): {}",
        back.line
    );
    scene.checkout(&heads[5]);
    let kept = read();
    assert_eq!(git_of(&kept), warm, "C6 is kept: {}", kept.line);
    println!(
        "DL11: git first {}, warm {warm} (catalog {warm}), C1 again {}, C6 again {}",
        git_of(&first),
        git_of(&back),
        git_of(&kept)
    );
}

/// DL11: the library reads through `App.catalogs`: the daemon's sources
/// construct one `LastChangeCache` and hold it in one owned place.
#[test]
fn dl11_the_daemon_holds_one_last_change_cache() {
    let src = repository_root().join("crates/specengine-http/src");
    let (mut made, mut owned, mut wrapped) = (0, 0, 0);
    for file in files_ending(&src, ".rs") {
        let code: String = read_text(&src, &file)
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .map(|line| format!("{line}\n"))
            .collect();
        made += code.matches("LastChangeCache::default()").count()
            + code.matches("LastChangeCache::new(").count();
        // A type, never the struct literal's `field: LastChangeCache::…`.
        owned +=
            code.matches(": LastChangeCache").count() - code.matches(": LastChangeCache::").count();
        wrapped += code.matches("<LastChangeCache>").count();
    }
    assert_eq!(made, 1, "one cache made (App.catalogs)");
    assert_eq!(owned, 1, "one owned place: App.catalogs");
    assert_eq!(wrapped, 0, "no cache held behind another type");
}

// ------------------------------------------- DL14's walks, diagnosed

/// What happens between the two baseline reads and the two after.
#[derive(Clone, Copy, Debug)]
enum Between {
    /// A hand commit appending to `docs/records/A/A-101.md`.
    Commit,
    /// A hand commit that changes no file (`--allow-empty`): `HEAD` alone
    /// moves.
    EmptyCommit,
    /// One `ask_question` over `/mcp/lantern-keep`, no commit.
    Ask,
    /// The hand commit, then the `ask_question`.
    CommitThenAsk,
    /// The hand edit, never committed: the bytes move, `HEAD` does not.
    EditOnly,
}

/// A hub over the scene at `--slow-ms 0`: library, library (the baseline:
/// walked, trusted), then `between`, then library ×3, tree, library, each request's
/// line as `route index=… git=…` in the order the hub answered them.
fn walks_around(label: &str, between: Between) -> Vec<String> {
    use common::mcp_http::{Stateless, result_of};
    let scene = Scene::new(label);
    let cwd = scene.dir("cwd");
    let hub_home = scene.dir("hub-home");
    let server = Server::start(
        &hub_home,
        &cwd,
        &slow_ms(hub_args(&[&scene.root], ORIGIN), "0"),
    );
    let read = |path: &str| {
        let reply = through(&server, "GET", path, &Proxied::fetch("mira"), b"");
        assert_eq!(reply.status, 200, "{path}: {}", reply.text());
    };
    read(ROUTE);
    read(ROUTE);
    let mut expected = 2;
    if matches!(between, Between::Commit | Between::CommitThenAsk) {
        scene.append("docs/records/A/A-101.md", "\nWalking was timed again.\n");
        scene.commit("2026-01-02T10:01:00Z", "A hand edit\n");
    }
    if matches!(between, Between::EditOnly) {
        scene.append("docs/records/A/A-101.md", "\nWalking was timed again.\n");
    }
    if matches!(between, Between::EmptyCommit) {
        scene.commit("2026-01-02T10:01:00Z", "Nothing changes\n");
    }
    if matches!(between, Between::Ask | Between::CommitThenAsk) {
        let asked = Stateless::new(server.port, "lantern-keep", "mira").call(
            "ask_question",
            serde_json::json!({"node_ids": ["MEC-STAMINA"], "text": "Does the lantern dim at dusk?",
                "working_answer": "No.", "price_of_other": "One rule rewritten.",
                "author_role": "drafter", "author_model": "m-1", "run": "r-1"}),
        );
        result_of(&asked, "ask_question");
        expected += 1;
    }
    // Three library reads with no other route between, then a tree read
    // and a library read.
    read(ROUTE);
    read(ROUTE);
    read(ROUTE);
    read("/api/projects/lantern-keep/tree");
    read(ROUTE);
    expected += 5;
    let lines = common::slow::wait_for(&server, common::slow::LINE_WAIT, "every line", |lines| {
        let requests = common::slow::requests(lines);
        (requests.len() >= expected).then_some(requests)
    });
    lines
        .iter()
        .map(|line| {
            let git = line
                .cost
                .as_ref()
                .and_then(|cost| cost.git)
                .map_or_else(|| "-".to_owned(), |git| git.to_string());
            let tool = if line.tool == "-" {
                String::new()
            } else {
                format!(" tool={}", line.tool)
            };
            format!("{}{tool} index={} git={git}", line.route, line.index)
        })
        .collect()
}

/// What makes a hub's read walk: the library read never re-reads a file's
/// bytes (`indexed(…, false)`), so the library reads after a hand commit,
/// with or without a changed file, trust (a new `HEAD` costs its git walk
/// alone); an MCP write walks on its own line, the reads after it trusted;
/// a `tree` read after a changed file (committed or not) re-reads it and
/// doubts the trust, so the library read after it walks. The fast
/// guard on the library's no-bytes read; each run's lines are printed when
/// an assertion fails.
#[test]
fn dl14_which_step_makes_a_hub_read_walk() {
    for (label, between) in [
        ("walk-commit", Between::Commit),
        ("walk-empty", Between::EmptyCommit),
        ("walk-ask", Between::Ask),
        ("walk-commit-ask", Between::CommitThenAsk),
        ("walk-edit", Between::EditOnly),
    ] {
        let lines = walks_around(label, between);
        let shown = format!("{between:?}:\n  {}", lines.join("\n  "));
        let after = &lines[2..];
        let library: Vec<&String> = after
            .iter()
            .filter(|line| line.starts_with(TEMPLATE))
            .collect();
        let index = |line: &str| -> String {
            line.split(" index=")
                .nth(1)
                .and_then(|rest| rest.split(' ').next())
                .expect("an index")
                .to_owned()
        };
        assert!(
            lines[..2].iter().all(|line| line.starts_with(TEMPLATE)),
            "{shown}"
        );
        assert_eq!(index(&lines[0]), "walked", "the first read walks: {shown}");
        assert_eq!(index(&lines[1]), "trusted", "the second trusts: {shown}");
        match between {
            Between::EmptyCommit => {
                for line in &library {
                    assert_eq!(index(line), "trusted", "after an empty commit: {shown}");
                }
            }
            // The commit changed a file: the library reads trust; the tree
            // read re-reads that file and doubts, so the read after it walks.
            Between::Commit | Between::EditOnly => {
                let (last, before) = library.split_last().expect("library reads");
                for line in before {
                    assert_eq!(index(line), "trusted", "before the tree read: {shown}");
                }
                assert_eq!(index(last), "walked", "after the tree read: {shown}");
            }
            Between::Ask | Between::CommitThenAsk => {
                let tool = after
                    .iter()
                    .find(|line| line.contains(" tool=ask_question "))
                    .unwrap_or_else(|| panic!("the write's line: {shown}"));
                assert_eq!(index(tool), "walked", "an MCP write walks: {shown}");
                for line in &library {
                    assert_eq!(index(line), "trusted", "after the write: {shown}");
                }
            }
        }
    }
}
