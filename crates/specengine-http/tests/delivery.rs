//! docs/features/delivery-analytics.md (RM-0107 a) "Acceptance criteria",
//! DA-11, the daemon's half: `GET /api/projects/tide-almanac/delivery` over
//! the spec's scenario (the shared scene of `specengine-cli`'s
//! `tests/delivery_scene`), a daemon of its own under the scene's `HOME`.
//!
//! - The route with `as_of` is byte-equal to `spec delivery --as-of D
//!   --json` less its LF, at 2026-11-20 the spec's JSON, at two other days
//!   the CLI's of that day; without `as_of`, today's UTC date. M: `as_of`
//!   not passed through.
//! - `as_of=2026-13-01` is a 400 naming it; an unknown parameter a 400
//!   listing `as_of`; no queue yet a 200 with the CLI's note.
//! - An unknown route's 404 lists `delivery`.
//!
//! docs/features/delivery-timeline.md (RM-0107 b) "Acceptance criteria",
//! the daemon's half:
//!
//! - DT5: `timeline=true` without `as_of` is the 400 "`timeline` needs
//!   `as_of`: the daemon reads no clock for a timeline", nothing run;
//!   `timeline=false` without it is part a's read at today's UTC date. M:
//!   the daemon defaulting `as_of` for a timeline.
//! - DT6: `?as_of=D&timeline=true` is byte-equal to `spec delivery --as-of D
//!   --timeline --json` less its LF, at 2026-11-20 the scenario's document
//!   (`delivery_scene/data.rs`); `timeline=false` part a's; `timeline=yes`
//!   a 400 naming `timeline`; an unknown parameter's 400 lists `as_of` and
//!   `timeline`. M: `timeline` not passed through.
//!
//! The key sets are `daemon_keys.rs`'s.

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

#[path = "../../specengine-cli/tests/delivery_scene/mod.rs"]
mod delivery_scene;

use common::{Server, spec};
use delivery_scene::{AS_OF, DATA_JSON, Scene, compact, data_timeline_json};

const ROUTE: &str = "/api/projects/tide-almanac/delivery";

fn serve(scene: &Scene) -> Server {
    Server::serve(&scene.home, &scene.root, &[&scene.root])
}

/// `spec --root R delivery --as-of D --json`'s document, less its LF.
fn cli(scene: &Scene, as_of: &str) -> String {
    cli_with(scene, as_of, &[])
}

/// [`cli`] with `more` flags after `--json`.
fn cli_with(scene: &Scene, as_of: &str, more: &[&str]) -> String {
    let root = scene.root.to_str().expect("UTF-8 root");
    let mut args = vec!["--root", root, "delivery", "--as-of", as_of, "--json"];
    args.extend_from_slice(more);
    let run = spec(&scene.home, &scene.root, &args);
    run.code(0);
    run.document().to_owned()
}

/// Today's UTC date, `YYYY-MM-DD` (the civil calendar from the Unix day).
fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after 1970")
        .as_secs();
    let z = i64::try_from(secs / 86_400).expect("a day") + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// DA-11: `GET …/delivery?as_of=D` is byte-equal to the CLI's `--json` at
/// D, the spec's JSON at 2026-11-20; with no `as_of` the day is today's
/// UTC date. M: `as_of` not passed through.
#[test]
fn da11_the_route_is_the_clis_json() {
    let scene = Scene::scenario("http-da11");
    let server = serve(&scene);
    for as_of in [AS_OF, "2026-11-01", "2026-11-12", "2027-03-01"] {
        let reply = server.get(&format!("{ROUTE}?as_of={as_of}"));
        reply.status(200);
        assert_eq!(reply.text(), cli(&scene, as_of), "{as_of}");
        assert_eq!(reply.json()["as_of"], as_of);
    }
    let pinned = server.get(&format!("{ROUTE}?as_of={AS_OF}"));
    assert_eq!(pinned.text(), compact(DATA_JSON));
    let before = today_utc();
    let plain = server.get(ROUTE);
    let after = today_utc();
    plain.status(200);
    let as_of = plain.json()["as_of"].as_str().expect("as_of").to_owned();
    assert!(as_of == before || as_of == after, "{as_of}");
    assert_eq!(plain.text(), cli(&scene, &as_of));
}

/// DA-11: a bad `as_of` is a 400 naming it; an unknown parameter a 400
/// listing `as_of`; the daemon serves the next good read.
#[test]
fn da11_a_bad_query_is_a_400() {
    let scene = Scene::scenario("http-da11-400");
    let server = serve(&scene);
    for bad in ["2026-13-01", "2026-11-31", "20261120", "tomorrow"] {
        let reply = server.get(&format!("{ROUTE}?as_of={bad}"));
        assert_eq!(reply.status, 400, "{bad}: {}", reply.text());
        assert!(
            reply.error_message().contains("as_of") && reply.error_message().contains(bad),
            "{bad}: {}",
            reply.error_message()
        );
    }
    for query in ["x=1", "asof=2026-11-20", "as_of=2026-11-20&release=r1"] {
        let reply = server.get(&format!("{ROUTE}?{query}"));
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        assert!(
            reply.error_message().contains("as_of"),
            "{query}: {}",
            reply.error_message()
        );
    }
    server.get(&format!("{ROUTE}?as_of={AS_OF}")).status(200);
}

/// DA-11: no queue yet: a 200, the CLI's document with its note; no queue
/// made.
#[test]
fn da11_no_queue_is_the_clis_note() {
    let scene = Scene::new("http-da11-empty");
    let server = serve(&scene);
    let reply = server.get(&format!("{ROUTE}?as_of={AS_OF}"));
    reply.status(200);
    assert_eq!(reply.text(), cli(&scene, AS_OF));
    assert_eq!(
        reply.json()["notes"],
        serde_json::json!(["delivery: no queue yet: no card read"])
    );
}

/// An unknown route under the project lists `delivery` among the routes.
#[test]
fn da11_an_unknown_route_lists_delivery() {
    let scene = Scene::new("http-da11-404");
    let server = serve(&scene);
    for path in [
        "/api/projects/tide-almanac/deliveries",
        "/api/projects/tide-almanac/delivery/r1",
    ] {
        let reply = server.get(path);
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.starts_with(&format!("no route {path}: the routes are /api/projects ")),
            "{path}: {message}"
        );
        let listed = message
            .split_once('{')
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(list, _)| list.split(", ").collect::<Vec<_>>())
            .unwrap_or_default();
        assert!(listed.contains(&"delivery"), "{path}: {message}");
    }
}

/// DA-11 (the browser view): the daemon never cuts: 200 late tasks
/// past the CLI's cut are all served, `truncated` false, `omitted` `null`;
/// the CLI's rows its first ones.
#[test]
fn da11_the_route_never_cuts() {
    let scene = Scene::new("http-da11-uncut");
    let title = |n: usize| {
        format!(
            "Chart {n:03} {}",
            "of the long harbor wall ".repeat(8).trim_end()
        )
    };
    for n in 1..=200 {
        scene.card(&specengine_cli::CardNewRequest {
            due: Some("2026-11-10".to_owned()),
            ..delivery_scene::new_card(&title(n), "copy", 1)
        });
    }
    let server = serve(&scene);
    let served = server
        .get(&format!("{ROUTE}?as_of={AS_OF}"))
        .status(200)
        .json();
    assert_eq!(served["truncated"], false);
    assert_eq!(served["omitted"], serde_json::Value::Null);
    let late = served["late"].as_array().expect("late");
    assert_eq!(late.len(), 200);
    let cli: serde_json::Value = serde_json::from_str(&cli(&scene, AS_OF)).expect("JSON");
    assert_eq!(cli["truncated"], true);
    let cut = cli["late"].as_array().expect("late");
    assert!(cut.len() < 200);
    assert_eq!(cut[..], late[..cut.len()]);
}

// ----------------------------------------------------- delivery-timeline

/// DT6: `?as_of=D&timeline=true`, in either order, is byte-equal to the
/// CLI's `--as-of D --timeline --json` less its LF; at 2026-11-20 the
/// scenario's document; `timeline=false` part a's, `"timeline": null`. M:
/// `timeline` not passed through.
#[test]
fn dt6_the_route_with_a_timeline_is_the_clis_json() {
    let scene = Scene::scenario("http-dt6");
    let server = serve(&scene);
    for as_of in [AS_OF, "2026-11-01", "2027-03-01"] {
        let reply = server.get(&format!("{ROUTE}?as_of={as_of}&timeline=true"));
        reply.status(200);
        assert_eq!(
            reply.text(),
            cli_with(&scene, as_of, &["--timeline"]),
            "{as_of}"
        );
        assert!(reply.json()["timeline"].is_object(), "{as_of}");
        let swapped = server.get(&format!("{ROUTE}?timeline=true&as_of={as_of}"));
        assert_eq!(swapped.status(200).text(), reply.text(), "{as_of}");
        let off = server.get(&format!("{ROUTE}?as_of={as_of}&timeline=false"));
        off.status(200);
        assert_eq!(off.text(), cli(&scene, as_of), "{as_of}");
        assert_eq!(off.json()["timeline"], serde_json::Value::Null);
    }
    let pinned = server.get(&format!("{ROUTE}?as_of={AS_OF}&timeline=true"));
    assert_eq!(pinned.text(), compact(&data_timeline_json()));
    let at_first = server
        .get(&format!("{ROUTE}?as_of=2026-11-01&timeline=true"))
        .status(200)
        .json();
    assert_eq!(at_first["timeline"]["from"], "2026-11-01");
    assert_eq!(at_first["timeline"]["to"], "2026-11-15");
}

/// DT5: `timeline=true` without `as_of` is the 400 naming both, the
/// daemon's clock never read for a timeline; nothing run (no queue made on
/// a project with none); `timeline=false` without it is today's part a
/// document. M: the daemon defaulting `as_of` for a timeline.
#[test]
fn dt5_a_timeline_without_as_of_is_a_400() {
    let scene = Scene::scenario("http-dt5");
    let server = serve(&scene);
    for query in ["timeline=true", "timeline=true&timeline=true"] {
        let reply = server.get(&format!("{ROUTE}?{query}"));
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        let message = reply.error_message();
        if query == "timeline=true" {
            assert_eq!(
                message,
                "`timeline` needs `as_of`: the daemon reads no clock for a timeline"
            );
        } else {
            assert!(message.contains("timeline"), "{query}: {message}");
        }
    }
    let before = today_utc();
    let plain = server.get(&format!("{ROUTE}?timeline=false"));
    let after = today_utc();
    plain.status(200);
    let as_of = plain.json()["as_of"].as_str().expect("as_of").to_owned();
    assert!(as_of == before || as_of == after, "{as_of}");
    assert_eq!(plain.text(), cli(&scene, &as_of));
    server
        .get(&format!("{ROUTE}?as_of={AS_OF}&timeline=true"))
        .status(200);
    let empty = Scene::new("http-dt5-empty");
    let server = serve(&empty);
    let reply = server.get(&format!("{ROUTE}?timeline=true"));
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert!(!empty.db().exists(), "nothing run");
}

/// DT6: a `timeline` other than `true` or `false` is a 400 naming it, the
/// value too; an unknown parameter's 400 lists `as_of` and `timeline`; a
/// bad `as_of` beside a good `timeline` names `as_of`; the daemon serves
/// the next good read.
#[test]
fn dt6_a_bad_timeline_is_a_400_naming_it() {
    let scene = Scene::scenario("http-dt6-400");
    let server = serve(&scene);
    for bad in ["yes", "1", "TRUE", "on", ""] {
        let reply = server.get(&format!("{ROUTE}?as_of={AS_OF}&timeline={bad}"));
        assert_eq!(reply.status, 400, "{bad:?}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains(&format!("`timeline={bad}`")),
            "{bad:?}: {message}"
        );
    }
    for query in ["timelines=true", "as_of=2026-11-20&tl=true"] {
        let reply = server.get(&format!("{ROUTE}?{query}"));
        assert_eq!(reply.status, 400, "{query}: {}", reply.text());
        let message = reply.error_message();
        assert!(
            message.contains("as_of") && message.contains("timeline"),
            "{query}: {message}"
        );
    }
    let reply = server.get(&format!("{ROUTE}?as_of=2026-13-01&timeline=true"));
    assert_eq!(reply.status, 400, "{}", reply.text());
    assert!(
        reply.error_message().contains("as_of"),
        "{}",
        reply.error_message()
    );
    server
        .get(&format!("{ROUTE}?as_of={AS_OF}&timeline=true"))
        .status(200);
}
