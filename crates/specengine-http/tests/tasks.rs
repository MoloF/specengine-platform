//! docs/features/ui-live-tasks.md, the daemon's two task routes, on A and
//! B in state S (`task_state`), one `HOME`, one `--port 0` daemon:
//!
//! - AC-01, list parity: `/tasks`, `?status=ready`,
//!   `?status=in_progress&status=draft` (and a state given twice) are 200,
//!   byte-equal to `spec --root R task list [--status S]... --json` less
//!   its final LF. M: `status` ignored.
//! - AC-02, show parity: T-0001 to T-0004 are 200, byte-equal to `spec
//!   --root R task show T --json` less its LF, the 25 keys in order; two
//!   reads byte-identical; T-0002 `stale` true with one `snapshot_diff`
//!   entry. M: the request built with `next: true`.
//! - AC-03, refusals: `/tasks/T-0099` 404 exactly `{id, reason}` of
//!   "Data"; `/tasks/foo` 404, `id` `null`; `/tasks/%D0%A2-0001` (a
//!   Cyrillic `T`) 503 naming `T-0001`, the CLI's line; `/tasks/` and
//!   `/tasks/T-0001/transition` 404 listing the routes; a copy of A with
//!   no `.git` and one whose `[ids]` claims `T`: 503 on both routes, the
//!   CLI's line; a corrupt row and another repository's task (same slug):
//!   503 on `show`, 200 with the CLI's note on `list`. M: exit 1 mapped to
//!   200.
//! - AC-04, query: `?status=triage`, `?status=`, `?status=Ready`,
//!   `?state=ready`, `?x=1`, `%FF`, `/tasks/T-0001?next=true` are a 400
//!   naming the parameter (a refused state lists the model's
//!   `TaskStatus::ALL`), a fresh `HOME` left empty.
//! - AC-05, read-only: after the reads `git status --porcelain --ignored`
//!   empty, `HEAD` unchanged, the highest event `seq` and `spec export
//!   state`'s bytes as before, on A and B.

mod common;
mod task_state;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use common::{
    Reply, Run, Scratch, Server, encode_component, root_args, snapshot, spec, spec_json, write,
};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals, TaskStatus, events_after};
use task_state::{A, B, Case, repo_in_s};

/// The package's 25 keys, in order (docs/canon/task-package.md
/// "Package").
const PACKAGE_KEYS: [&str; 25] = [
    "schema_version",
    "id",
    "project",
    "status",
    "title",
    "goal",
    "profile",
    "stale",
    "targets",
    "criteria",
    "affected_nodes",
    "plan",
    "assumptions",
    "open_proposals",
    "owner_notes",
    "bindings",
    "spec_snapshot",
    "snapshot_diff",
    "claim",
    "runs",
    "bundle",
    "author",
    "created_at",
    "updated_at",
    "notes",
];

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

fn keys_of(text: &str) -> Vec<String> {
    serde_json::from_str::<Keys>(text)
        .unwrap_or_else(|error| panic!("{error}: {text}"))
        .0
}

/// The model's ten states, as a refusal lists them.
fn states() -> String {
    TaskStatus::ALL.map(TaskStatus::as_str).to_vec().join(", ")
}

/// A and B in state S under one `HOME`, a cwd outside both.
struct World {
    scratch: Scratch,
    a: PathBuf,
    b: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let home = scratch.home("h");
        let a = repo_in_s(&scratch, &A, "a", &home);
        let b = repo_in_s(&scratch, &B, "b", &home);
        let cwd = scratch.dir("cwd");
        Self {
            scratch,
            a,
            b,
            home,
            cwd,
        }
    }

    fn roots(&self) -> [(&Case, &Path); 2] {
        [(&A, self.a.as_path()), (&B, self.b.as_path())]
    }

    /// `spec --root root args --json`, exit 0 or 1.
    fn cli(&self, root: &Path, args: &[&str]) -> Run {
        spec_json(&self.home, &self.cwd, root, args)
    }

    /// `spec --root root args --json` cannot run (exit 2): its stderr, no
    /// final LF.
    fn cannot(&self, root: &Path, args: &[&str]) -> String {
        cannot_run(&self.home, &self.cwd, root, args)
    }
}

/// `spec --root root args --json` under `home` that cannot run (exit 2,
/// nothing on stdout): its stderr without the final LF.
fn cannot_run(home: &Path, cwd: &Path, root: &Path, args: &[&str]) -> String {
    let mut all = vec!["--root", root.to_str().expect("UTF-8 root")];
    all.extend_from_slice(args);
    all.push("--json");
    let run = spec(home, cwd, &all);
    run.code(2);
    assert_eq!(run.stdout, "", "exit 2 prints no document: {}", run.show());
    run.stderr.trim_end_matches('\n').to_owned()
}

/// Asserts the daemon's `path` is `run`'s document byte for byte (exit 0
/// → 200, exit 1 → 404), with the JSON headers.
fn same(server: &Server, path: &str, run: &Run) -> Reply {
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
    assert_eq!(
        reply.text(),
        run.document(),
        "{path} is byte-equal to `spec {}`",
        run.args.join(" ")
    );
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8"),
        "{path}"
    );
    assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
    reply
}

/// The project's stored events `(seq, type, payload)`, read through the
/// CLI library under `home` (not through the daemon).
fn stored_events(home: &Path, root: &Path) -> Vec<(i64, String, String)> {
    let env = Env {
        cwd: root.to_path_buf(),
        home: Some(home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(root.to_path_buf()),
        config: None,
    };
    events_after(&env, &globals, Some(0))
        .expect("the events read")
        .events
        .into_iter()
        .map(|event| (event.seq, event.event_type, event.payload))
        .collect()
}

/// What AC-05 compares on one root: its status with the ignored files,
/// `HEAD`, the highest event `seq` and every stored event, and the bytes
/// of `spec export state`.
#[derive(Debug, PartialEq, Eq)]
struct Kept {
    porcelain: String,
    head: String,
    highest: i64,
    events: Vec<(i64, String, String)>,
    export: Vec<u8>,
}

fn kept(world: &World, root: &Path, out: &str) -> Kept {
    let git = world.scratch.git();
    let events = stored_events(&world.home, root);
    let out = world.scratch.join(out);
    let run = spec(
        &world.home,
        &world.cwd,
        &[
            "--root",
            root.to_str().unwrap(),
            "export",
            "state",
            "--out",
            out.to_str().unwrap(),
        ],
    );
    run.code(0);
    let export = fs::read(&out).expect("the export");
    fs::remove_file(&out).expect("remove the export");
    Kept {
        porcelain: git.run(root, &["status", "--porcelain", "--ignored"]),
        head: git.run(root, &["rev-parse", "HEAD"]),
        highest: events.last().map_or(0, |event| event.0),
        events,
        export,
    }
}

#[test]
fn ac01_to_ac05_the_task_routes_are_the_clis_documents_and_change_nothing() {
    let world = World::new("tasks-parity");
    let before: Vec<Kept> = world
        .roots()
        .iter()
        .enumerate()
        .map(|(index, (_, root))| kept(&world, root, &format!("before-{index}.jsonl")))
        .collect();
    for (case, kept) in world.roots().iter().map(|(case, _)| case).zip(&before) {
        assert_eq!(kept.porcelain, "", "{}: S is committed", case.slug);
        assert_eq!(kept.highest, 13, "{}: S stores 13 events", case.slug);
        let export = String::from_utf8_lossy(&kept.export);
        assert!(
            export.contains("\"T-0004\"") && export.contains("task.run_reported"),
            "{}: the export holds S",
            case.slug
        );
    }
    let server = Server::serve(&world.home, &world.cwd, &[&world.a, &world.b]);

    for (case, root) in world.roots() {
        let p = format!("/api/projects/{}", case.slug);

        // AC-01: the list, its filter, a state given twice.
        for (query, args, ids) in [
            ("", vec![], vec!["T-0001", "T-0002", "T-0003", "T-0004"]),
            ("?status=ready", vec!["--status", "ready"], vec!["T-0002"]),
            (
                "?status=in_progress&status=draft",
                vec!["--status", "in_progress", "--status", "draft"],
                vec!["T-0001", "T-0003"],
            ),
            (
                "?status=done&status=done",
                vec!["--status", "done", "--status", "done"],
                vec![],
            ),
            (
                "?status=cancelled",
                vec!["--status", "cancelled"],
                vec!["T-0004"],
            ),
        ] {
            let mut all = vec!["task", "list"];
            all.extend_from_slice(&args);
            let run = world.cli(root, &all);
            run.code(0);
            let reply = same(&server, &format!("{p}/tasks{query}"), &run);
            let listed = reply.json();
            let got: Vec<&str> = listed["tasks"]
                .as_array()
                .expect("tasks")
                .iter()
                .map(|task| task["id"].as_str().expect("id"))
                .collect();
            assert_eq!(got, ids, "{}{query}", case.slug);
            assert_eq!(keys_of(reply.text()), ["tasks", "notes"]);
        }
        let listed = server.get(&format!("{p}/tasks")).json();
        let statuses: Vec<&str> = listed["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|task| task["status"].as_str().unwrap())
            .collect();
        assert_eq!(
            statuses,
            ["draft", "ready", "in_progress", "cancelled"],
            "{}: S",
            case.slug
        );
        assert_eq!(listed["tasks"][1]["stale"], json!(true), "{}", case.slug);

        // AC-02: every package, twice, the CLI's.
        for id in ["T-0001", "T-0002", "T-0003", "T-0004"] {
            let run = world.cli(root, &["task", "show", id]);
            run.code(0);
            let path = format!("{p}/tasks/{id}");
            let first = same(&server, &path, &run);
            let second = server.get(&path);
            assert_eq!(second.status, 200, "{path}");
            assert_eq!(
                second.body, first.body,
                "{path}: two reads are byte-identical"
            );
            assert_eq!(keys_of(first.text()), PACKAGE_KEYS, "{path}: the 25 keys");
            let package = first.json();
            assert_eq!(package["id"], json!(id), "{path}");
            assert_eq!(package["project"], json!(case.slug), "{path}");
        }
        let t2 = server.get(&format!("{p}/tasks/T-0002")).json();
        assert_eq!(t2["stale"], json!(true), "{}: T-0002 is stale", case.slug);
        let diffs = t2["snapshot_diff"].as_array().expect("a diff list");
        assert_eq!(diffs.len(), 1, "{}: one changed node: {t2}", case.slug);
        assert_eq!(diffs[0]["id"], json!(case.stale));
        assert!(
            diffs[0]["diff"]
                .as_str()
                .is_some_and(|diff| diff.contains(case.to)),
            "{}: the diff shows the edit: {}",
            case.slug,
            diffs[0]
        );
        let t3 = server.get(&format!("{p}/tasks/T-0003")).json();
        assert_eq!(t3["status"], json!("in_progress"));
        assert_eq!(t3["stale"], json!(false), "{}: {t3}", case.slug);
        assert_eq!(t3["criteria"].as_array().unwrap().len(), 2, "{t3}");
        assert!(t3["criteria"][0]["ref"].is_string(), "{t3}");
        assert_eq!(t3["criteria"][1]["ref"], Value::Null, "{t3}");
        assert_eq!(t3["owner_notes"].as_array().unwrap().len(), 1, "{t3}");
        assert_eq!(t3["assumptions"][0]["proposal"], json!("PR-0001"), "{t3}");
        assert_eq!(t3["open_proposals"][0]["task_id"], json!("T-0003"), "{t3}");
        assert!(t3["claim"].is_object(), "{t3}");
        assert_eq!(t3["runs"].as_array().unwrap().len(), 1, "{t3}");
        assert_eq!(t3["runs"][0]["outcome"], json!("completed"), "{t3}");
        assert_eq!(t3["author"]["type"], json!("agent"), "{t3}");
        let t1 = server.get(&format!("{p}/tasks/T-0001")).json();
        assert_eq!(t1["stale"], Value::Null, "a draft has no snapshot: {t1}");

        // AC-03: no such task, no task ID, a look-alike.
        let run = world.cli(root, &["task", "show", "T-0099"]);
        run.code(1);
        let reply = same(&server, &format!("{p}/tasks/T-0099"), &run);
        assert_eq!(
            reply.text(),
            "{\"id\":\"T-0099\",\"reason\":\"no task T-0099 in this repository\"}"
        );
        let run = world.cli(root, &["task", "show", "foo"]);
        run.code(1);
        let reply = same(&server, &format!("{p}/tasks/foo"), &run);
        assert_eq!(keys_of(reply.text()), ["id", "reason"]);
        assert_eq!(reply.json()["id"], Value::Null, "{}", reply.text());
        assert!(reply.json()["reason"].is_string(), "{}", reply.text());
        let lookalike = "\u{0422}-0001";
        let line = world.cannot(root, &["task", "show", lookalike]);
        assert!(
            line.contains("T-0001"),
            "the CLI names the Latin form: {line}"
        );
        let path = format!("{p}/tasks/{}", encode_component(lookalike));
        assert_eq!(path, format!("{p}/tasks/%D0%A2-0001"));
        let reply = server.get(&path);
        assert_eq!(reply.status, 503, "{path}: {}", reply.text());
        assert_eq!(reply.error_message(), line, "{path}: the CLI's line");

        // AC-03: no route, a bad path escape.
        for path in [
            format!("{p}/tasks/"),
            format!("{p}/tasks/T-0001/transition"),
        ] {
            for method in ["GET", "POST"] {
                let reply = server.request(method, &path, &[]);
                assert_eq!(reply.status, 404, "{method} {path}: {}", reply.text());
                let message = reply.error_message();
                assert!(
                    message.starts_with(&format!("no route {path}: the routes are ")),
                    "{method} {path}: {message}"
                );
                for route in [" tasks,", " tasks/<id>,"] {
                    assert!(message.contains(route), "{path}: `{route}`: {message}");
                }
            }
        }
        let reply = server.get(&format!("{p}/tasks/T%2"));
        assert_eq!(reply.status, 400, "a bad escape: {}", reply.text());
        reply.error_message();

        // AC-04: a bad query on the served project too.
        for (query, message) in bad_queries() {
            let path = format!("{p}/tasks{query}");
            let reply = server.get(&path);
            assert_eq!(reply.status, 400, "{path}: {}", reply.text());
            let got = reply.error_message();
            if let Some(message) = message {
                assert_eq!(got, message, "{path}");
            }
        }
    }

    // AC-05: the reads moved nothing.
    drop(server);
    for ((case, root), before) in world.roots().iter().zip(&before) {
        let after = kept(&world, root, "after.jsonl");
        assert_eq!(&after, before, "{}: nothing changed", case.slug);
    }
}

/// The 400s of AC-04 as `(path after .../tasks, the exact message when
/// it is fixed)`.
fn bad_queries() -> Vec<(String, Option<String>)> {
    let states = states();
    vec![
        (
            "?status=triage".to_owned(),
            Some(format!("`status=triage`: not a task state: {states}")),
        ),
        (
            "?status=".to_owned(),
            Some(format!("`status=`: not a task state: {states}")),
        ),
        (
            "?status=Ready".to_owned(),
            Some(format!("`status=Ready`: not a task state: {states}")),
        ),
        (
            "?status=ready&status=triage".to_owned(),
            Some(format!("`status=triage`: not a task state: {states}")),
        ),
        (
            "?state=ready".to_owned(),
            Some("`state` is no query name of tasks: it takes status".to_owned()),
        ),
        (
            "?x=1".to_owned(),
            Some("`x` is no query name of tasks: it takes status".to_owned()),
        ),
        ("?status=%FF".to_owned(), None),
        (
            "/T-0001?next=true".to_owned(),
            Some("`next` is no query name of tasks/:id: it takes no query name".to_owned()),
        ),
        (
            "/T-0001?x=1".to_owned(),
            Some("`x` is no query name of tasks/:id: it takes no query name".to_owned()),
        ),
    ]
}

#[test]
fn ac04_a_bad_query_is_a_400_naming_the_parameter_and_reads_nothing() {
    let scratch = Scratch::new("tasks-query");
    let a = scratch.repo("spec-a", "a", "main");
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[&a]);
    assert_eq!(states().split(", ").count(), 10, "{}", states());
    for (query, message) in bad_queries() {
        let path = format!("/api/projects/lantern-keep/tasks{query}");
        let reply = server.get(&path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
        assert_eq!(reply.header("cache-control"), Some("no-store"), "{path}");
        let got = reply.error_message();
        match message {
            Some(message) => assert_eq!(got, message, "{path}"),
            None => assert!(got.contains("`status`"), "{path}: names `status`: {got}"),
        }
    }
    assert!(
        snapshot(&home).is_empty(),
        "a refused query runs no call: the fresh HOME holds {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
    // The control: a good query reads (and may create the database).
    let reply = server.get("/api/projects/lantern-keep/tasks?status=ready");
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.text(), "{\"tasks\":[],\"notes\":[]}");
}

/// A git copy of A at `dir` served as `slug`, `change` applied before the
/// commit (`git` false: a byte copy, no repository).
fn copy_of_a(
    scratch: &Scratch,
    dir: &str,
    slug: &str,
    git: bool,
    change: impl FnOnce(&Path),
) -> PathBuf {
    let root = scratch.copy("spec-a", dir);
    let config = fs::read_to_string(root.join("specengine.toml")).expect("config");
    assert_eq!(config.matches("slug = \"lantern-keep\"").count(), 1);
    write(
        &root,
        "specengine.toml",
        config.replace("slug = \"lantern-keep\"", &format!("slug = \"{slug}\"")),
    );
    change(&root);
    if git {
        let sandbox = scratch.git();
        sandbox.run(
            &root,
            &["init", "-q", "--template=", "--initial-branch=main"],
        );
        sandbox.quiet(&root);
        sandbox.run(&root, &["add", "-A"]);
        sandbox.run(&root, &["commit", "-q", "-m", "fixture"]);
    }
    root
}

#[test]
fn ac03_no_worktree_and_t_in_ids_are_a_503_on_both_routes() {
    let scratch = Scratch::new("tasks-cannot");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let bare = copy_of_a(&scratch, "nogit", "lk-nogit", false, |_| {});
    let claimed = copy_of_a(&scratch, "tids", "lk-tids", true, |root| {
        let config = fs::read_to_string(root.join("specengine.toml")).expect("config");
        assert_eq!(config.matches("\n[ids]\n").count(), 1, "{config}");
        write(
            root,
            "specengine.toml",
            config.replace(
                "\n[ids]\n",
                "\n[ids]\nT    = { kind = \"ticket\", width = 4 }\n",
            ),
        );
    });
    let server = Server::start(&home, &cwd, &root_args(&[&bare, &claimed]));
    for (root, slug) in [(&bare, "lk-nogit"), (&claimed, "lk-tids")] {
        for (route, args) in [
            ("tasks", vec!["task", "list"]),
            (
                "tasks?status=ready",
                vec!["task", "list", "--status", "ready"],
            ),
            ("tasks/T-0001", vec!["task", "show", "T-0001"]),
        ] {
            let line = cannot_run(&home, &cwd, root, &args);
            let path = format!("/api/projects/{slug}/{route}");
            let reply = server.get(&path);
            assert_eq!(reply.status, 503, "{path}: {}", reply.text());
            assert_eq!(reply.error_message(), line, "{path}: the CLI's line");
        }
    }
    // The control: the other routes still read both copies.
    for slug in ["lk-nogit", "lk-tids"] {
        server
            .get(&format!("/api/projects/{slug}/nodes/MEC-STAMINA"))
            .status(200);
    }
}

#[test]
fn ac03_a_corrupt_row_or_another_repositorys_task_is_a_503_on_show_a_note_on_list() {
    let scratch = Scratch::new("tasks-rows");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let a = repo_in_s(&scratch, &A, "a", &home);
    // Another repository of the same project (slug), the same queue.
    let other = scratch.repo("spec-a", "other", "main");
    let db = common::data_dir(&home).join("lantern-keep.db");
    let output = std::process::Command::new("/usr/bin/sqlite3")
        .env_clear()
        .args(["-cmd", ".timeout 2000"])
        .arg(&db)
        .arg("UPDATE tasks SET targets = '[1,' WHERE id = 'T-0001';")
        .output()
        .expect("run sqlite3");
    assert!(output.status.success(), "{output:?}");

    let server = Server::serve(&home, &cwd, &[&a]);
    let p = "/api/projects/lantern-keep";
    // The corrupt row: show cannot run, list skips it with a note.
    let line = cannot_run(&home, &cwd, &a, &["task", "show", "T-0001"]);
    assert!(
        line.contains("task T-0001: the stored `targets` cannot be read"),
        "{line}"
    );
    let reply = server.get(&format!("{p}/tasks/T-0001"));
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.error_message(), line);
    let run = spec_json(&home, &cwd, &a, &["task", "list"]);
    run.code(0);
    let listed = same(&server, &format!("{p}/tasks"), &run).json();
    let notes = listed["notes"].as_array().expect("notes");
    assert!(
        notes.iter().any(|note| note
            .as_str()
            .is_some_and(|note| note.starts_with("task T-0001: the stored `"))),
        "{listed}"
    );
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 3, "{listed}");
    drop(server);

    // Another repository's task: served alone (the same slug).
    let server = Server::serve(&home, &cwd, &[&other]);
    let line = cannot_run(&home, &cwd, &other, &["task", "show", "T-0002"]);
    assert!(
        line.contains("`T-0002` belongs to another repository of the project `lantern-keep`"),
        "{line}"
    );
    let reply = server.get(&format!("{p}/tasks/T-0002"));
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.error_message(), line);
    let run = spec_json(&home, &cwd, &other, &["task", "list"]);
    run.code(0);
    let listed = same(&server, &format!("{p}/tasks"), &run).json();
    assert_eq!(listed["tasks"], json!([]), "{listed}");
    assert!(
        listed["notes"].as_array().unwrap().iter().any(|note| note
            .as_str()
            .is_some_and(|note| note.contains("of another repository of the project"))),
        "{listed}"
    );
}
