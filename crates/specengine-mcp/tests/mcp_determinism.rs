//! AC-11 of docs/features/mcp-read.md: two copies of a fixture created in
//! opposite file orders, at other roots and with other `HOME`s, give
//! byte-identical reply lines (the same request ids) for every tool and
//! resource in both eras, and no reply holds the root, the `HOME` or
//! today's date; a repeated session gives the same bytes again. Both
//! fixtures (`spec-a` game design, `spec-b` command-line tooling).
//!
//! M: the absolute root in a header.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use common::read::{ERAS, Era, Session};
use common::*;
use serde_json::{Value, json};

/// `YYYY-MM-DD` of `days` since 1970-01-01 (the civil-from-days algorithm).
fn civil(days: i64) -> String {
    let z = days + 719_468;
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

/// Today's date and its neighbours (any time zone).
fn dates_around_now() -> Vec<String> {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after 1970")
        .as_secs() as i64
        / 86_400;
    (days - 1..=days + 1).map(civil).collect()
}

/// The requests of one session over a project of `slug`.
fn requests(slug: &str, references: &[&str], query: &str) -> Vec<(String, Value)> {
    let mut list: Vec<(String, Value)> = vec![("tools/list".into(), json!({}))];
    let call = |name: &str, arguments: Value| {
        (
            "tools/call".to_owned(),
            json!({"name": name, "arguments": arguments}),
        )
    };
    list.push(call("get_tree", json!({})));
    list.push(call("get_tree", json!({"depth": 1, "archive": true})));
    list.push(call(
        "search",
        json!({"query": query, "limit": 200, "archive": true}),
    ));
    list.push(call("search", json!({"query": query})));
    for reference in references {
        list.push(call("get_node", json!({"id": reference})));
        list.push(call(
            "get_node",
            json!({"id": reference, "with": ["links"], "archive": true}),
        ));
        list.push(call(
            "get_context_bundle",
            json!({"node_ids": [reference], "budget": 10000}),
        ));
        list.push(call("get_context_bundle", json!({"node_ids": [reference]})));
    }
    list.push(call("get_node", json!({"id": "NOPE-1"})));
    list.push(call("get_tree", json!({"depth": -1})));
    list.push(("resources/list".into(), json!({})));
    list.push(("resources/templates/list".into(), json!({})));
    list.push((
        "resources/read".into(),
        json!({"uri": format!("spec://{slug}/tree")}),
    ));
    list
}

/// Every reply line of one session, the resource reads of every listed
/// document appended.
fn session_lines(
    era: Era,
    root: &Path,
    home: &Path,
    slug: &str,
    references: &[&str],
    query: &str,
) -> Vec<String> {
    let mut session = Session::open(era, &[], Some(root), Home::At(home));
    let mut lines = Vec::new();
    for (method, params) in requests(slug, references, query) {
        lines.push(session.request_line(&method, params));
    }
    let list = parse_message(&lines[lines.len() - 3]);
    for resource in list["result"]["resources"].as_array().expect("resources") {
        let uri = resource["uri"].as_str().expect("uri").to_owned();
        lines.push(session.request_line("resources/read", json!({"uri": uri})));
    }
    let done = session.finish();
    assert!(done.status.success(), "{era:?}: {}", done.stderr);
    assert_eq!(done.stderr, "");
    lines
}

#[test]
fn ac11_one_state_gives_byte_identical_results() {
    let dates = dates_around_now();
    for (fixture, slug, references, query) in [
        (
            "spec-a",
            "lantern-keep",
            &[
                "MEC-STAMINA",
                "QST-031",
                "stamina-tuning/AC-07",
                "docs/spec/game.md",
            ][..],
            "stamina",
        ),
        (
            "spec-b",
            "zerkalo",
            &[
                "REQ-001",
                "\u{0422}\u{0420}\u{0411}-001",
                "MOD-CLI#CMD-SYNC",
            ][..],
            "worktree",
        ),
    ] {
        let one = Scratch::new("det-one");
        let two = Scratch::new("det-two-with-a-longer-name");
        let root_one = one.join("first");
        copy_dir(&fixture_dir(fixture), &root_one, false);
        let root_two = two.join("nested/second-root");
        copy_dir(&fixture_dir(fixture), &root_two, true);
        let root_one = root_one.canonicalize().unwrap();
        let root_two = root_two.canonicalize().unwrap();
        let home_one = one.home("home-one");
        let home_two = two.home("another-home");
        for era in ERAS {
            let first = session_lines(era, &root_one, &home_one, slug, references, query);
            let second = session_lines(era, &root_two, &home_two, slug, references, query);
            assert_eq!(first.len(), second.len(), "{era:?} {fixture}");
            for (index, (left, right)) in first.iter().zip(&second).enumerate() {
                assert_eq!(left, right, "{era:?} {fixture}: reply {index} differs");
            }
            let again = session_lines(era, &root_one, &home_one, slug, references, query);
            assert_eq!(first, again, "{era:?} {fixture}: a second session differs");
            for line in first.iter().chain(&second) {
                for forbidden in [&root_one, &root_two, &home_one, &home_two] {
                    let text = forbidden.to_str().unwrap();
                    assert!(
                        !line.contains(text),
                        "{era:?} {fixture}: a reply holds {text}: {}",
                        clip(line)
                    );
                }
                for name in [
                    "det-one",
                    "det-two",
                    "second-root",
                    "home-one",
                    "another-home",
                ] {
                    assert!(!line.contains(name), "{era:?}: {name} in {}", clip(line));
                }
                for date in &dates {
                    assert!(
                        !line.contains(date.as_str()),
                        "{era:?} {fixture}: today's date {date} in {}",
                        clip(line)
                    );
                }
            }
        }
    }
}

fn fixture_dir(name: &str) -> std::path::PathBuf {
    common::fixture(name)
}
