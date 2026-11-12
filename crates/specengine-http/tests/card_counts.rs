//! docs/features/department-overview.md (RM-0086) "Acceptance criteria",
//! the daemon's half: `GET /api/projects/atlas-bindery/cards/counts` over
//! the spec's scenario (the shared scene of `specengine-cli`'s
//! `tests/counts_scene`), a daemon of its own under the scene's `HOME`.
//!
//! - DO10: the route is byte-equal to `spec card counts --json` less its LF
//!   (the spec's JSON, `agent_tasks` `counted`); a query parameter is a 400,
//!   nothing run; another method a 405 with `Allow: GET`; a bad `[card]`, a
//!   newer queue schema and a corrupt queue are each a 503, the CLI's line
//!   for the first; an unknown route's 404 lists the counts. M: `tasks_here`
//!   true on a hub.
//! - DO1 (the route's half): 700 cards with 120-character titles, past the
//!   CLI list's cut: the route sums them to 700, uncut.
//! - DO4: the hub's route, for a login no `[team]` lists and under a
//!   `[team]` no reader takes, answers 200 with `agent_tasks` `local_only`
//!   and every `review_agent_tasks` `null`, the rest the local document's.
//!   M: `[]` served on a hub.
//!
//! The key sets are `daemon_keys.rs`'s.

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

#[path = "../../specengine-cli/tests/counts_scene/mod.rs"]
mod counts_scene;

use std::path::Path;

use common::hub::{ORIGIN, Proxied, hub_args, through};
use common::{Server, spec};
use counts_scene::{Scene, new_card, with_flows};
use serde_json::{Value, json};

const ROUTE: &str = "/api/projects/atlas-bindery/cards/counts";

/// A `[team]` no reader takes: a role off the three (`hub_team.rs`'s).
const BAD_TEAM: &str = r#"
[team.mira]
name = "Mira K"
role = "editor"
email = "mira@example.org"
"#;

fn serve(scene: &Scene) -> Server {
    Server::serve(&scene.home, &scene.root, &[&scene.root])
}

/// `spec --root R card counts --json`'s run.
fn cli_run(scene: &Scene) -> common::Run {
    let root = scene.root.to_str().expect("UTF-8 root");
    spec(
        &scene.home,
        &scene.root,
        &["--root", root, "card", "counts", "--json"],
    )
}

/// The CLI's document, less its LF.
fn cli(scene: &Scene) -> String {
    let run = cli_run(scene);
    run.code(0);
    run.stdout.trim_end_matches('\n').to_owned()
}

/// The raw `flows` of `spec card list --json`.
fn listed_flows(scene: &Scene) -> String {
    let root = scene.root.to_str().expect("UTF-8 root");
    let run = spec(
        &scene.home,
        &scene.root,
        &["--root", root, "card", "list", "--json"],
    );
    run.code(0);
    let listed: Value = serde_json::from_str(run.stdout.trim_end()).expect("JSON");
    let flows = listed["flows"].clone();
    // The raw text: the list's stdout between `"flows":` and the value's end,
    // found as the compact serialization of each flow in order.
    let start = run.stdout.find("\"flows\":").expect("flows") + "\"flows\":".len();
    let raw = &run.stdout[start..];
    let mut depth = 0usize;
    let (mut in_string, mut escaped) = (false, false);
    for (at, letter) in raw.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if letter == '\\' {
                escaped = true;
            } else if letter == '"' {
                in_string = false;
            }
            continue;
        }
        match letter {
            '"' => in_string = true,
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    let text = raw[..=at].to_owned();
                    let parsed: Value = serde_json::from_str(&text).expect("the flows");
                    assert_eq!(parsed, flows);
                    return text;
                }
            }
            _ => {}
        }
    }
    panic!("no flows: {}", run.stdout);
}

/// The scene's queue file set to `sql` by the system's `sqlite3`.
fn sql(scene: &Scene, sql: &str) {
    counts_scene::sql_at(&scene.db(), sql);
}

/// The `specengine.toml` of `root` with `text` appended.
fn append_config(root: &Path, text: &str) {
    let path = root.join("specengine.toml");
    let mut config = std::fs::read_to_string(&path).expect("config");
    config.push_str(text);
    std::fs::write(&path, config).expect("config written");
}

// ------------------------------------------------------------------ DO10

/// DO10: `GET …/cards/counts` is byte-equal to `spec card counts --json`
/// less its LF, the spec's JSON with the list's `flows`; `agent_tasks`
/// `counted` locally, `T-0001` in review under `copy` and `plates`.
#[test]
fn do10_the_route_is_the_clis_json() {
    let scene = Scene::scenario("http-do10");
    let server = serve(&scene);
    let reply = server.get(ROUTE);
    reply.status(200);
    assert_eq!(reply.text(), cli(&scene));
    assert_eq!(reply.text(), with_flows(&listed_flows(&scene)));
    let document = reply.json();
    assert_eq!(document["agent_tasks"], "counted");
    assert_eq!(
        document["by_flow"][0]["review_agent_tasks"],
        json!(["T-0001"])
    );
    // Twice, one answer.
    assert_eq!(server.get(ROUTE).text(), reply.text());
}

/// DO10: a query parameter is a 400 (`?flow=copy`, any name), nothing run;
/// POST, PUT, DELETE and PATCH are 405 with `Allow: GET`; the daemon serves
/// the next good read.
#[test]
fn do10_a_query_is_a_400_and_another_method_a_405() {
    let scene = Scene::scenario("http-do10-400");
    let server = serve(&scene);
    for query in ["flow=copy", "x=1", "tasks_here=true", "flow="] {
        let reply = server.get(&format!("{ROUTE}?{query}"));
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        let name = query.split('=').next().unwrap_or_default();
        assert_eq!(
            reply.error_message(),
            format!("`{name}` is no query name of cards/counts: it takes no query name"),
            "{query}"
        );
    }
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        let reply = server.send(
            method,
            ROUTE,
            &[
                ("Content-Type", "application/json"),
                ("Sec-Fetch-Site", "same-origin"),
            ],
            b"{}",
        );
        assert_eq!(reply.status, 405, "{method}: {}", reply.text());
        assert_eq!(reply.header("allow"), Some("GET"), "{method}");
    }
    server.get(ROUTE).status(200);
}

/// DO10: a bad `[card]` is a 503 whose message is the CLI's `<config>:<line>:`
/// line; the config is read per call, so the next good one answers 200.
#[test]
fn do10_a_bad_card_table_is_a_503_with_the_clis_line() {
    let scene = Scene::scenario("http-do10-503");
    let server = serve(&scene);
    server.get(ROUTE).status(200);
    append_config(
        &scene.root,
        "\n[[card.flow]]\nname = \"bad\"\ncolumns = [\"todo\"]\n",
    );
    let run = cli_run(&scene);
    run.code(2);
    let line = run.stderr.trim_end_matches('\n').to_owned();
    assert!(line.contains("specengine.toml:"), "{}", run.show());
    let reply = server.get(ROUTE);
    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.error_message(), line);
    for read in ["tree", "inbox"] {
        let reply = server.get(&format!("/api/projects/atlas-bindery/{read}"));
        assert_eq!(reply.status, 200, "{read}: {}", reply.text());
    }
}

/// DO10: a queue of a newer schema is a 503, and so is a queue file that is
/// no database.
#[test]
fn do10_a_newer_or_corrupt_queue_is_a_503() {
    let scene = Scene::scenario("http-do10-schema");
    let server = serve(&scene);
    server.get(ROUTE).status(200);
    sql(&scene, "PRAGMA user_version = 99;");
    let reply = server.get(ROUTE);
    assert_eq!(reply.status, 503, "newer: {}", reply.text());
    drop(server);

    let scene = Scene::scenario("http-do10-corrupt");
    let db = scene.db();
    for suffix in ["-wal", "-shm"] {
        let mut name = db.clone().into_os_string();
        name.push(suffix);
        let _ = std::fs::remove_file(std::path::PathBuf::from(name));
    }
    std::fs::write(&db, b"this is no sqlite database, only words in a file").expect("written");
    let server = serve(&scene);
    let reply = server.get(ROUTE);
    assert_eq!(reply.status, 503, "corrupt: {}", reply.text());
}

/// DO10: an unknown route under the project lists the counts among the
/// routes.
#[test]
fn do10_an_unknown_route_lists_the_counts() {
    let scene = Scene::new("http-do10-404");
    let server = serve(&scene);
    let path = "/api/projects/atlas-bindery/cards/counts/copy";
    let reply = server.get(path);
    assert_eq!(reply.status, 404, "{path}: {}", reply.text());
    let message = reply.error_message();
    assert!(
        message.starts_with(&format!("no route {path}: the routes are /api/projects ")),
        "{message}"
    );
    let listed = message
        .split_once('{')
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(list, _)| list.split(", ").collect::<Vec<_>>())
        .unwrap_or_default();
    assert!(
        listed.iter().any(|route| route.contains("counts")),
        "{message}"
    );
}

/// DO1, the route's half: the daemon never cuts: 700 cards with
/// 120-character titles, past the CLI list's cut, sum to 700 at the route,
/// `truncated` false, `omitted` null.
#[test]
fn do1_the_route_is_uncut() {
    let scene = Scene::new("http-do1");
    let disciplines = ["copy", "plates", "sound"];
    for n in 0..700 {
        let title: String = format!("Atlas sheet {n:04} {}", "y".repeat(120))
            .chars()
            .take(120)
            .collect();
        scene.card(&new_card(&title, disciplines[n % 3], 1));
    }
    let server = serve(&scene);
    let cards = server
        .get("/api/projects/atlas-bindery/cards")
        .status(200)
        .json();
    assert_eq!(
        cards["cards"].as_array().map(Vec::len),
        Some(700),
        "the browser's list"
    );
    let reply = server.get(ROUTE);
    reply.status(200);
    let document = reply.json();
    assert_eq!(document["truncated"], false);
    assert_eq!(document["omitted"], Value::Null);
    let sum: u64 = document["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .map(|row| {
            row["open"].as_u64().expect("open")
                + row["accepted"].as_u64().expect("accepted")
                + row["cancelled"].as_u64().expect("cancelled")
        })
        .sum();
    assert_eq!(sum, 700);
    assert_eq!(reply.text(), cli(&scene));
}

// ------------------------------------------------------------------ DO4

/// DO4: on a hub, `GET …/cards/counts` by a login no `[team]` names, under
/// a `[team]` no reader takes (so the route never reads it), is a 200:
/// `agent_tasks` `local_only`, every `review_agent_tasks` `null`, every
/// other value the local document's. M: `[]` served on a hub; `tasks_here`
/// true on a hub.
#[test]
fn do4_the_hub_serves_local_only_and_null() {
    let scene = Scene::scenario("hub-do4");
    let local = {
        let server = serve(&scene);
        let reply = server.get(ROUTE);
        reply.status(200);
        reply.json()
    };
    append_config(&scene.root, BAD_TEAM);
    let cwd = scene.home.join("cwd");
    std::fs::create_dir_all(&cwd).expect("a cwd");
    let hub = Server::start(&scene.home, &cwd, &hub_args(&[&scene.root], ORIGIN));
    for login in ["eve", "mira"] {
        let reply = through(&hub, "GET", ROUTE, &Proxied::fetch(login), b"");
        assert_eq!(reply.status, 200, "{login}: {}", reply.text());
        let served = reply.json();
        assert_eq!(served["agent_tasks"], "local_only", "{login}");
        let rows = served["by_flow"].as_array().expect("by_flow");
        assert_eq!(rows.len(), 3);
        for row in rows {
            assert_eq!(row["review_agent_tasks"], Value::Null, "{login}: {row}");
        }
        let mut want = local.clone();
        want["agent_tasks"] = json!("local_only");
        for row in want["by_flow"].as_array_mut().expect("by_flow") {
            row["review_agent_tasks"] = Value::Null;
        }
        assert_eq!(served, want, "{login}: the rest as the local daemon's");
    }
    // A query is a 400 on the hub too.
    let reply = through(
        &hub,
        "GET",
        &format!("{ROUTE}?flow=copy"),
        &Proxied::fetch("eve"),
        b"",
    );
    assert_eq!(reply.status, 400, "{}", reply.text());
}

/// DO4 with the iteration-2 notes: a stored `col` that is `NULL` (C-0006)
/// is named on a hub too, still counted (the columns sum to the seven
/// cards); a `tasks` that does not read (C-0007) is named locally alone,
/// since a hub reads no link. Locally the notes run by card ID.
#[test]
fn do4_the_hub_names_an_unread_name_but_no_link() {
    let scene = Scene::scenario("hub-do4-notes");
    sql(&scene, "UPDATE cards SET col = NULL WHERE id = 'C-0006';");
    sql(
        &scene,
        "UPDATE cards SET tasks = '\"x\"' WHERE id = 'C-0007';",
    );
    let named = "C-0006: the stored `col` cannot be read: it is NULL; counted under `NULL`";
    let unread = "C-0007: its `tasks` does not read; its agent tasks are not counted";
    let sum = |document: &Value| -> u64 {
        document["columns"]
            .as_array()
            .expect("columns")
            .iter()
            .map(|row| {
                row["open"].as_u64().expect("open")
                    + row["accepted"].as_u64().expect("accepted")
                    + row["cancelled"].as_u64().expect("cancelled")
            })
            .sum()
    };
    let local = {
        let server = serve(&scene);
        let reply = server.get(ROUTE);
        reply.status(200);
        reply.json()
    };
    assert_eq!(local["notes"], json!([named, unread]));
    assert_eq!(sum(&local), 7);
    let cwd = scene.home.join("cwd");
    std::fs::create_dir_all(&cwd).expect("a cwd");
    let hub = Server::start(&scene.home, &cwd, &hub_args(&[&scene.root], ORIGIN));
    let reply = through(&hub, "GET", ROUTE, &Proxied::fetch("eve"), b"");
    assert_eq!(reply.status, 200, "{}", reply.text());
    let served = reply.json();
    assert_eq!(served["notes"], json!([named]));
    assert_eq!(sum(&served), 7);
}
