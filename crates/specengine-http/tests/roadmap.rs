//! docs/features/roadmap-surfaces.md (Phase 6, slice 2b) RD-9, the
//! daemon's `GET /api/projects/:p/roadmap[?release=R | ?pool=true]`: `spec
//! roadmap`'s JSON document in the browser view (`View::Browser`: nothing
//! cut, `truncated` false, `omitted` 0), one daemon serving scratch git
//! repositories of `fixtures/roadmap-a` (a harbor game's plan),
//! `fixtures/roadmap-b` (a lending library's desk service: another
//! vocabulary, ADR-0008) and `fixtures/spec-a` (no `[roadmap]`), and a
//! generated 600-item plan past the CLI's cut.
//!
//! - Parity: exit 0 a 200 byte-equal to `spec --root R roadmap [--release
//!   R | --pool] --json` less its LF (the fixture's `expected.json` for the
//!   whole plan); exit 1 (a release not listed) a 404 with the exit-1
//!   document; no `[roadmap]` (exit 2) a 503, its message the CLI's line.
//! - M10, `roadmap_route_is_uncut`: past 40 000 characters (505 items,
//!   as CLI `roadmap_position.rs`' cut) the CLI cuts (`truncated` true,
//!   `omitted` k), the route lists every item: `truncated` false, `omitted`
//!   0, byte-equal to `render_json` of `roadmap_with_view(..,
//!   View::Browser)`, the CLI's items its first ones, every other key the
//!   CLI's.
//! - The query: `release` with `pool=true` a 400 with the CLI's line
//!   (`roadmap_flags`), nothing run; an unknown name, a repeated
//!   `release`, a `pool` not a boolean: 400s naming them.
//! - An unknown route's 404 lists `roadmap` after `health`; the reads
//!   leave every root as it was.

mod common;

use std::path::{Path, PathBuf};

use common::{
    Scratch, Server, files_ending, product_env, read_text, snapshot, spec, spec_json, write,
};
use serde_json::{Value, json};
use specengine_cli::{
    Env, GitEnv, Globals, Outcome, RoadmapRequest, View, render_json, roadmap_with_view,
};

/// The fixtures with a `[roadmap]`: name, slug, a release with open
/// items, a name its config does not list.
const PLANS: [(&str, &str, &str, &str); 2] = [
    ("roadmap-a", "tide-runner", "beta", "someday"),
    ("roadmap-b", "agora-lending", "next", "Next"),
];

/// A daemon's roots and `HOME`.
struct World {
    scratch: Scratch,
    home: PathBuf,
    cwd: PathBuf,
}

impl World {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self { scratch, home, cwd }
    }

    /// `spec --root root roadmap <args> --json` (exit 0 or 1).
    fn cli(&self, root: &Path, args: &[&str]) -> common::Run {
        let mut all = vec!["roadmap"];
        all.extend_from_slice(args);
        spec_json(&self.home, &self.cwd, root, &all)
    }
}

/// `path` answers as `run` byte for byte: exit 0 a 200, exit 1 a 404, the
/// JSON headers.
fn same(server: &Server, path: &str, run: &common::Run) -> Value {
    let reply = server.get(path);
    let want = if run.code == 0 { 200 } else { 404 };
    assert_eq!(
        reply.status,
        want,
        "{path} vs spec {:?} (exit {}): {}",
        run.args,
        run.code,
        reply.text()
    );
    assert_eq!(reply.text(), run.document(), "{path}: the CLI's document");
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8"),
        "{path}"
    );
    reply.json()
}

/// RD-9: the route is the CLI's document on both fixtures (see the module
/// docs); spec-a's is a 503 with the CLI's line. M: exit 1 a 200; the
/// query ignored.
#[test]
fn roadmap_route_answers_as_the_cli() {
    let world = World::new("rm-route");
    let a = world.scratch.repo("roadmap-a", "a", "main");
    let b = world.scratch.repo("roadmap-b", "b", "main");
    let c = world.scratch.repo("spec-a", "c", "main");
    let before: Vec<_> = [&a, &b, &c].iter().map(|root| snapshot(root)).collect();
    let server = Server::serve(&world.home, &world.cwd, &[&a, &b, &c]);
    for ((name, slug, release, unlisted), root) in PLANS.iter().zip([&a, &b]) {
        let p = format!("/api/projects/{slug}/roadmap");
        let whole = same(&server, &p, &world.cli(root, &[]));
        let oracle: Value =
            serde_json::from_str(&read_text(&common::fixture(name), "expected.json"))
                .expect("expected.json");
        assert_eq!(whole, oracle, "{name}: the hand-made oracle");
        assert_eq!(whole["truncated"], json!(false));
        assert_eq!(whole["omitted"], json!(0), "{name}: `omitted` 0, not null");
        let listed = same(
            &server,
            &format!("{p}?release={release}"),
            &world.cli(root, &["--release", release]),
        );
        assert_eq!(listed["release"], json!(release));
        assert_eq!(listed["groups"].as_array().map(Vec::len), Some(1));
        let pool = same(
            &server,
            &format!("{p}?pool=true"),
            &world.cli(root, &["--pool"]),
        );
        assert_eq!(pool["pool"], json!(true));
        assert_ne!(pool, whole, "{name}: `pool` read");
        same(&server, &format!("{p}?pool=false"), &world.cli(root, &[]));
        // Exit 1: the document, a 404.
        let run = world.cli(root, &["--release", unlisted]);
        run.code(1);
        let missing = same(&server, &format!("{p}?release={unlisted}"), &run);
        assert!(
            missing["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains(&format!("\"{unlisted}\""))),
            "{missing}"
        );
        assert_eq!(missing["groups"], json!([]));
        // Twice: byte-identical.
        assert_eq!(server.get(&p).body, server.get(&p).body, "{name}");
    }

    // No `[roadmap]`: the CLI's exit 2, a 503 with its line.
    let run = spec(
        &world.home,
        &world.cwd,
        &["--root", c.to_str().unwrap(), "roadmap", "--json"],
    );
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
    let line = run.stderr.trim_end_matches('\n');
    assert!(line.contains("has no [roadmap]"), "{line}");
    let reply = server.get("/api/projects/lantern-keep/roadmap");
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.error_message(), line, "the CLI's line");

    let after: Vec<_> = [&a, &b, &c].iter().map(|root| snapshot(root)).collect();
    assert!(before == after, "the reads change nothing under a root");
}

/// The query is checked before any call: `release` with `pool=true` is a
/// 400 naming both (the CLI's usage refusal); an unknown name lists the
/// two; a repeated `release`, a `pool` not a boolean: 400s naming them. A
/// fresh `HOME` holds no database after them.
#[test]
fn roadmap_query_is_checked_before_any_call() {
    let world = World::new("rm-query");
    let a = world.scratch.repo("roadmap-a", "a", "main");
    let server = Server::serve(&world.home, &world.cwd, &[&a]);
    let p = "/api/projects/tide-runner/roadmap";
    for query in ["release=beta&pool=true", "pool=true&release=beta"] {
        let reply = server.get(&format!("{p}?{query}"));
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        assert_eq!(
            reply.text(),
            "{\"status\":400,\"message\":\"spec: --release and --pool cannot be used together\"}",
            "{query}: the CLI's line"
        );
    }
    let reply = server.get(&format!("{p}?bogus=1"));
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert!(
        reply
            .error_message()
            .contains("`bogus` is no query name of roadmap: it takes release, pool"),
        "{}",
        reply.text()
    );
    for (path, named) in [
        (format!("{p}?x=1"), "release, pool"),
        (format!("{p}?releases=beta"), "release, pool"),
        (format!("{p}?release=beta&release=launch"), "release"),
        (format!("{p}?pool=yes"), "pool=yes"),
        (format!("{p}?pool=1"), "pool=1"),
    ] {
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains(named),
            "{path}: {message:?} names {named:?}"
        );
    }
    assert!(
        files_ending(&world.home, ".db").is_empty(),
        "a refused query ran no call"
    );
}

/// An unknown route under the project lists `roadmap` after `health`.
#[test]
fn unknown_route_lists_roadmap() {
    let world = World::new("rm-unknown");
    let a = world.scratch.repo("roadmap-a", "a", "main");
    let server = Server::serve(&world.home, &world.cwd, &[&a]);
    for path in [
        "/api/projects/tide-runner/roadmap/beta",
        "/api/projects/tide-runner/roadmaps",
        "/api/projects/tide-runner/roadmap/",
    ] {
        let reply = server.get(path);
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.starts_with(&format!("no route {path}: the routes are /api/projects ")),
            "{path}: {message}"
        );
        // docs/features/lock-diff.md "Route": `lock-diff` after `roadmap`.
        assert!(
            message.contains(" health, roadmap, lock-diff, inbox, "),
            "{path}: {message}"
        );
    }
}

/// A generated plan of 505 items with long titles (a lighthouse keeper's
/// log: words of its own), as CLI `roadmap_position.rs`' cut: 500 in
/// `winter`, 3 in `spring`, 2 in the pool; committed once.
fn bulk_plan(world: &World) -> PathBuf {
    let root = world.scratch.dir("bulk");
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"beacon-log\"\n\n[paths]\nroots = [\"log\"]\n\n[ids]\nLAMP = { kind = \"lamp-entry\", width = 4 }\n\n[roadmap]\nprefix = \"LAMP\"\ndir = \"log\"\nreleases = [\"winter\", \"spring\"]\npriorities = [\"bright\", \"dim\"]\nunit = \"nights\"\n",
    );
    for n in 1..=BULK {
        let release = match n {
            1..=500 => "release: winter\n",
            501..=503 => "release: spring\n",
            _ => "",
        };
        write(
            &root,
            &format!("log/LAMP-{n:04}.md"),
            format!(
                "---\nclass: spec\nid: LAMP-{n:04}\nstatus: draft\nscope: [beacon]\n{release}priority: bright\nestimate: 1-2\n---\n\n# Lamp entry {n:04} with a long title that fills the line of the plan\n\nGoal.\n"
            ),
        );
    }
    let git = world.scratch.git();
    git.run(
        &root,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    git.quiet(&root);
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "the log"]);
    std::fs::canonicalize(&root).expect("the root")
}

/// The items of [`bulk_plan`].
const BULK: usize = 505;

/// The daemon's view of `root` through the library: `render_json` of
/// `roadmap_with_view(.., View::Browser)` (`--root root`, the world's
/// `HOME` and working directory), less its LF.
fn browser_view(world: &World, root: &Path, release: Option<&str>) -> String {
    let env = Env {
        cwd: world.cwd.clone(),
        home: Some(world.home.clone().into_os_string()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(root.to_path_buf()),
        config: None,
    };
    let request = RoadmapRequest {
        release: release.map(str::to_owned),
        pool: false,
        git: GitEnv::new(&world.cwd, product_env(&world.home)),
    };
    let outcome = roadmap_with_view(&env, &globals, &request, View::Browser)
        .unwrap_or_else(|error| panic!("roadmap_with_view: {}", error.message));
    let mut text = render_json(&Outcome::Roadmap(Box::new(outcome)));
    assert_eq!(text.pop(), Some('\n'));
    text
}

/// The IDs of a roadmap document's items, in order.
fn item_ids(document: &Value) -> Vec<String> {
    document["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .flat_map(|group| group["items"].as_array().expect("items").iter())
        .map(|item| item["id"].as_str().expect("an ID").to_owned())
        .collect()
}

/// M10: the route is uncut. 505 items pass the CLI's 40 000 characters:
/// `spec roadmap --json` is cut (`truncated` true, `omitted` k > 0); the
/// route lists all 505 in the CLI's order, `truncated` false, `omitted` 0,
/// byte-equal to the library's browser view, every other key the CLI's;
/// `?release=winter` (500) alike.
#[test]
fn roadmap_route_is_uncut() {
    let world = World::new("rm-uncut");
    let root = bulk_plan(&world);
    let server = Server::serve(&world.home, &world.cwd, &[&root]);
    for (query, release, count) in [("", None, BULK), ("?release=winter", Some("winter"), 500)] {
        let path = format!("/api/projects/beacon-log/roadmap{query}");
        let args: Vec<&str> = release.map_or_else(Vec::new, |r| vec!["--release", r]);
        let run = world.cli(&root, &args);
        run.code(0);
        let cli = run.json();
        assert_eq!(cli["truncated"], json!(true), "{query}: the CLI cuts");
        let omitted = cli["omitted"].as_u64().expect("omitted");
        assert!(omitted > 0, "{query}");
        let cut = item_ids(&cli);
        assert_eq!(cut.len() as u64 + omitted, count as u64, "{query}");

        let reply = server.get(&path);
        assert_eq!(reply.status, 200, "{path}: {}", reply.text());
        assert_eq!(
            reply.text(),
            browser_view(&world, &root, release),
            "{query}: the library's browser view, byte for byte"
        );
        let daemon = reply.json();
        assert_eq!(daemon["truncated"], json!(false), "{query}: uncut");
        assert_eq!(daemon["omitted"], json!(0), "{query}: `omitted` 0");
        let all = item_ids(&daemon);
        assert_eq!(all.len(), count, "{query}: every item");
        assert_eq!(all[..cut.len()], cut[..], "{query}: the CLI's first");
        let expected: Vec<String> = (1..=count).map(|n| format!("LAMP-{n:04}")).collect();
        assert_eq!(all, expected, "{query}: by key");
        let mut rest = daemon.clone();
        let mut cli_rest = cli.clone();
        for document in [&mut rest, &mut cli_rest] {
            for key in ["groups", "truncated", "omitted"] {
                document[key] = Value::Null;
            }
        }
        assert_eq!(rest, cli_rest, "{query}: every other key the CLI's");
        let keys = |value: &Value| -> Vec<String> {
            value
                .as_object()
                .expect("an object")
                .keys()
                .cloned()
                .collect()
        };
        assert_eq!(keys(&daemon), keys(&cli), "{query}: the same keys");
    }
}
