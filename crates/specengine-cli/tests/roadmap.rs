//! docs/features/roadmap-items.md RD-5, RD-7 and TE-1, the CLI half:
//! `spec roadmap [--release R | --pool] [--json]` over scratch copies of
//! `fixtures/roadmap-a` (a harbor game's plan, English titles, a unit, a
//! cut) and `fixtures/roadmap-b` (a lending desk's plan, Greek and Japanese
//! titles, no unit, no cut) and over a generated 500-item corpus — the
//! groups, the slices, every exit, the 40 000-character cut and its tail,
//! byte identity across roots and creation order, and nothing written
//! under the root (`git status`, `HEAD`). The fixtures are only read.
//!
//! The oracle is each fixture's hand-made `expected.txt` (stdout) and
//! `expected.json` (the `--json` value); a slice is cut from them.
//!
//! Named mutations: M7 (output following walk or insertion order) turns
//! `roadmap_is_byte_identical_across_roots_and_order` red; M8 (the cut
//! without its tail) `roadmap_cut_names_its_narrowing`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use common::git::Sandbox;
use common::{Run, Scratch, fixture, read_text, snapshot, spec, spec_with, write};
use serde_json::Value;

/// The character cap of a read's output (`cap::OUTPUT_CAP_CHARS`).
const CAP: usize = 40_000;

/// The two fixtures: (name, releases in config order).
const FIXTURES: [(&str, [&str; 3]); 2] = [
    ("roadmap-a", ["alpha", "beta", "launch"]),
    ("roadmap-b", ["now", "next", "later"]),
];

// ------------------------------------------------------------------ oracle

fn expected_text(name: &str) -> String {
    fs::read_to_string(fixture(name).join("expected.txt")).expect("expected.txt")
}

fn expected_json(name: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(fixture(name).join("expected.json")).unwrap())
        .expect("expected.json is JSON")
}

/// The first line of a text output and its group blocks, each keyed by
/// its group (`release <name>`, `pool`, `unplaced`) with its lines.
fn blocks(text: &str) -> (String, Vec<(String, String)>) {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next().expect("a first line").to_owned();
    let mut out: Vec<(String, String)> = Vec::new();
    for line in lines {
        if line.starts_with("  ") {
            out.last_mut()
                .expect("an item line under a header")
                .1
                .push_str(line);
        } else {
            let key = line
                .split(" | ")
                .next()
                .unwrap_or(line)
                .trim_end()
                .to_owned();
            out.push((key, line.to_owned()));
        }
    }
    (first, out)
}

/// `expected.txt` sliced to `keys` (the first line kept).
fn text_slice(name: &str, keys: &[&str]) -> String {
    let (first, groups) = blocks(&expected_text(name));
    let mut out = first;
    for key in keys {
        let block = groups
            .iter()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("{name}: no block {key}"));
        out.push_str(&block.1);
    }
    out
}

/// `expected.json` sliced: the request keys set, the groups filtered; every
/// other top-level key is the whole plan's.
fn json_slice(name: &str, release: Option<&str>, pool: bool) -> Value {
    let mut value = expected_json(name);
    value["release"] = release.map_or(Value::Null, |r| Value::String(r.to_owned()));
    value["pool"] = Value::Bool(pool);
    let groups: Vec<Value> = value["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|group| match release {
            Some(r) => group["group"] == "release" && group["name"] == r,
            None => group["group"] == "pool" || group["group"] == "unplaced",
        })
        .cloned()
        .collect();
    value["groups"] = Value::Array(groups);
    value
}

/// The stderr `note:` lines of a run, without the prefix.
fn notes(run: &Run) -> Vec<&str> {
    run.stderr
        .lines()
        .filter_map(|line| line.strip_prefix("note: "))
        .collect()
}

fn expected_notes(name: &str) -> Vec<String> {
    expected_json(name)["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|note| note.as_str().unwrap().to_owned())
        .collect()
}

/// Every item ID of a group's text block.
fn block_ids(block: &str) -> Vec<String> {
    block
        .lines()
        .filter(|line| line.starts_with("  "))
        .map(|line| {
            let line = line.trim_start();
            let line = line.strip_prefix("- ").unwrap_or(line);
            let line = line.split_once(". ").map_or(line, |(_, rest)| rest);
            line.split(" | ").next().unwrap().to_owned()
        })
        .collect()
}

#[track_caller]
fn assert_text(run: &Run, want: &str, context: &str) {
    run.code(0);
    assert!(
        run.stdout == want,
        "{context}: stdout differs\n--- got\n{}--- want\n{want}--- stderr\n{}",
        run.stdout,
        run.stderr
    );
}

#[track_caller]
fn assert_json(run: &Run, want: &Value, context: &str) {
    run.code(0);
    let got = run.json();
    if &got != want {
        panic!(
            "{context}: JSON differs\n--- got\n{}\n--- want\n{}",
            serde_json::to_string_pretty(&got).unwrap(),
            serde_json::to_string_pretty(want).unwrap()
        );
    }
}

// ------------------------------------------------------------------- TE-1

/// TE-1: the whole output, then `--release` of each release and `--pool`
/// as its slices, text and JSON; the notes on stderr, one `note:` line
/// each, in the JSON's order.
fn matches_expected(name: &str, releases: [&str; 3]) {
    let scratch = Scratch::new(&format!("roadmap-te1-{name}"));
    let home = scratch.home("h");
    let root = scratch.copy(name, "copy");

    let run = spec(&home, &root, &["roadmap"]);
    assert_text(&run, &expected_text(name), name);
    let want_notes = expected_notes(name);
    assert_eq!(notes(&run), want_notes, "{name}: stderr\n{}", run.stderr);
    common::assert_plain(&run.stdout, name);

    let run = spec(&home, &root, &["roadmap", "--json"]);
    assert_json(&run, &expected_json(name), name);
    // `--json` before the command reads the same.
    let leading = spec(&home, &root, &["--json", "roadmap"]);
    assert_eq!(leading.stdout, run.stdout, "{name}: --json placement");

    for release in releases {
        let context = format!("{name} --release {release}");
        let run = spec(&home, &root, &["roadmap", "--release", release]);
        assert_text(
            &run,
            &text_slice(name, &[&format!("release {release}")]),
            &context,
        );
        let run = spec(&home, &root, &["--json", "roadmap", "--release", release]);
        assert_json(&run, &json_slice(name, Some(release), false), &context);
    }
    let run = spec(&home, &root, &["roadmap", "--pool"]);
    assert_text(&run, &text_slice(name, &["pool", "unplaced"]), name);
    let run = spec(&home, &root, &["--json", "roadmap", "--pool"]);
    assert_json(
        &run,
        &json_slice(name, None, true),
        &format!("{name} --pool"),
    );
}

#[test]
fn roadmap_a_matches_expected() {
    let (name, releases) = FIXTURES[0];
    matches_expected(name, releases);
}

#[test]
fn roadmap_b_matches_expected() {
    let (name, releases) = FIXTURES[1];
    matches_expected(name, releases);
}

// ------------------------------------------------------------------- RD-5

/// An item without `release` is in the pool and nowhere else: not under a
/// release, not unplaced, in no `--release` slice; one added later too.
#[test]
fn item_without_release_is_pool_only() {
    let scratch = Scratch::new("roadmap-pool-only");
    let home = scratch.home("h");
    let root = scratch.copy("roadmap-a", "copy");
    write(
        &root,
        "plan/RM-0043.md",
        "---\nclass: spec\nid: RM-0043\nstatus: draft\nscope: [harbor]\npriority: must\nestimate: 4\n---\n\n# Night fishing\n\nGoal: fish at night.\n",
    );
    let run = spec(&home, &root, &["roadmap"]);
    run.code(0);
    let (_, groups) = blocks(&run.stdout);
    for (key, block) in &groups {
        let ids = block_ids(block);
        for id in ["RM-0040", "RM-0041", "RM-0043"] {
            assert_eq!(
                ids.contains(&id.to_owned()),
                key == "pool",
                "{id} in {key}:\n{}",
                run.stdout
            );
        }
    }
    let (_, pool) = groups.iter().find(|(key, _)| key == "pool").expect("pool");
    assert_eq!(block_ids(pool), ["RM-0043", "RM-0040", "RM-0041"], "by key");
    assert!(
        pool.starts_with("pool | 3 open, 1 done | 9 days, 1 unestimated\n"),
        "{pool}"
    );
    let json = spec(&home, &root, &["--json", "roadmap"]).json();
    for group in json["groups"].as_array().unwrap() {
        for item in group["items"].as_array().unwrap() {
            if ["RM-0040", "RM-0041", "RM-0043"].contains(&item["id"].as_str().unwrap()) {
                assert_eq!(group["group"], "pool", "{item}");
                assert_eq!(item["position"], Value::Null);
                assert_eq!(item["release"], Value::Null);
            }
        }
    }
    for release in ["alpha", "beta", "launch"] {
        let run = spec(&home, &root, &["roadmap", "--release", release]);
        run.code(0);
        assert!(
            !run.stdout.contains("RM-0043"),
            "{release}:\n{}",
            run.stdout
        );
    }
}

/// An unknown release is unplaced, never the pool: listed with its
/// release as written, its note on stderr; a mistyped one is unplaced as
/// `mistyped`, with `release` null and its own note.
#[test]
fn unknown_release_is_unplaced() {
    let scratch = Scratch::new("roadmap-unplaced");
    let home = scratch.home("h");
    let root = scratch.copy("roadmap-a", "copy");
    write(
        &root,
        "plan/RM-0051.md",
        "---\nclass: spec\nid: RM-0051\nstatus: draft\nscope: [harbor]\nrelease: [beta]\npriority: could\n---\n\n# Regatta\n\nGoal: a race.\n",
    );
    write(
        &root,
        "plan/RM-0052.md",
        "---\nclass: spec\nid: RM-0052\nstatus: draft\nscope: [harbor]\nrelease: Beta\n---\n\n# Mixed case\n\nGoal: none.\n",
    );
    let run = spec(&home, &root, &["roadmap"]);
    run.code(0);
    let (_, groups) = blocks(&run.stdout);
    let (_, unplaced) = groups
        .iter()
        .find(|(key, _)| key == "unplaced")
        .expect("unplaced");
    assert_eq!(
        unplaced,
        "unplaced | 3 open, 0 done | 2-6 days, 2 unestimated\n\
         \x20 - RM-0050 | Co-op mode | draft | must | 2-6 | plan/RM-0050.md | release someday\n\
         \x20 - RM-0051 | Regatta | draft | could | - | plan/RM-0051.md | release mistyped\n\
         \x20 - RM-0052 | Mixed case | draft | - | - | plan/RM-0052.md | release Beta\n"
    );
    let (_, pool) = groups.iter().find(|(key, _)| key == "pool").expect("pool");
    for id in ["RM-0050", "RM-0051", "RM-0052"] {
        assert!(!pool.contains(id), "{id} not in the pool");
    }
    let notes = notes(&run);
    for note in [
        "RM-0050: release \"someday\" is not in [roadmap] releases",
        "RM-0051: release is not a string",
        "RM-0052: release \"Beta\" is not in [roadmap] releases",
    ] {
        assert!(notes.contains(&note), "{note} in {notes:?}");
    }
    let json = spec(&home, &root, &["--json", "roadmap", "--pool"]).json();
    let unplaced = &json["groups"][1];
    assert_eq!(unplaced["group"], "unplaced");
    let releases: Vec<&Value> = unplaced["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| &item["release"])
        .collect();
    assert_eq!(
        releases,
        [&Value::from("someday"), &Value::Null, &Value::from("Beta")]
    );
}

/// `--release R`: the whole plan's first line, then that release alone;
/// JSON `release` set, one group, the top-level keys the whole plan's; a
/// release of done items prints its header alone.
#[test]
fn release_flag_lists_one_group() {
    let scratch = Scratch::new("roadmap-release");
    let home = scratch.home("h");
    let root = scratch.copy("roadmap-a", "copy");
    let run = spec(&home, &root, &["roadmap", "--release", "launch"]);
    run.code(0);
    let (first, groups) = blocks(&run.stdout);
    assert_eq!(
        first,
        blocks(&expected_text("roadmap-a")).0,
        "the whole plan's"
    );
    let keys: Vec<&str> = groups.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, ["release launch"]);
    assert_eq!(
        block_ids(&groups[0].1),
        ["RM-0030", "RM-0021", "RM-0020", "RM-0022", "RM-0060"]
    );
    let run = spec(&home, &root, &["roadmap", "--release", "alpha"]);
    run.code(0);
    assert_eq!(
        run.stdout.lines().skip(1).collect::<Vec<_>>(),
        ["release alpha | 0 open, 2 done | 0 days"]
    );
    let json = spec(&home, &root, &["--json", "roadmap", "--release", "launch"]).json();
    assert_eq!(json["release"], "launch");
    assert_eq!(json["pool"], false);
    assert_eq!(json["reason"], Value::Null);
    assert_eq!(json["groups"].as_array().unwrap().len(), 1);
    let whole = expected_json("roadmap-a");
    for key in [
        "current", "counts", "cut", "total", "notes", "prefix", "dir", "unit",
    ] {
        assert_eq!(json[key], whole[key], "{key} is the whole plan's");
    }
}

/// `--pool`: the pool and the unplaced groups, in that order, after the
/// whole plan's first line.
#[test]
fn pool_flag_lists_pool_and_unplaced() {
    for (name, _) in FIXTURES {
        let scratch = Scratch::new(&format!("roadmap-pool-{name}"));
        let home = scratch.home("h");
        let root = scratch.copy(name, "copy");
        let run = spec(&home, &root, &["roadmap", "--pool"]);
        run.code(0);
        let (first, groups) = blocks(&run.stdout);
        assert_eq!(first, blocks(&expected_text(name)).0, "{name}");
        let keys: Vec<&str> = groups.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, ["pool", "unplaced"], "{name}");
        assert!(
            groups
                .iter()
                .all(|(_, block)| block.lines().skip(1).all(|line| line.starts_with("  - "))),
            "{name}: no positions:\n{}",
            run.stdout
        );
        let json = spec(&home, &root, &["--json", "roadmap", "--pool"]).json();
        assert_eq!(json["pool"], true);
        assert_eq!(json["release"], Value::Null);
        let kinds: Vec<&Value> = json["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| &group["group"])
            .collect();
        assert_eq!(kinds, [&Value::from("pool"), &Value::from("unplaced")]);
    }
}

/// Exit 1: `--release R` naming no release (case-sensitive): stderr `spec:
/// no release "R" in [roadmap] releases (<list>)`, no stdout; with
/// `--json` the document, `reason` set, `groups` empty, the rest the
/// whole plan's.
#[test]
fn release_flag_naming_nothing_exits_1() {
    for (name, releases) in FIXTURES {
        let scratch = Scratch::new(&format!("roadmap-exit1-{name}"));
        let home = scratch.home("h");
        let root = scratch.copy(name, "copy");
        for missing in ["gamma", "Beta", "pool"] {
            let context = format!("{name} --release {missing}");
            let run = spec(&home, &root, &["roadmap", "--release", missing]);
            run.code(1);
            assert_eq!(run.stdout, "", "{context}");
            let last = run.stderr.lines().last().unwrap_or_default();
            let head = format!("spec: no release \"{missing}\" in [roadmap] releases (");
            assert!(
                last.starts_with(&head) && last.ends_with(')'),
                "{context}: {last:?}"
            );
            let list = &last[head.len()..last.len() - 1];
            let mut at = 0;
            for release in releases {
                let found = list[at..]
                    .find(release)
                    .unwrap_or_else(|| panic!("{context}: {release} in order in {list:?}"));
                at += found + release.len();
            }

            let run = spec(&home, &root, &["--json", "roadmap", "--release", missing]);
            run.code(1);
            let json = run.json();
            assert_eq!(json["release"], missing, "{context}");
            assert_eq!(json["groups"], Value::Array(Vec::new()), "{context}");
            let reason = json["reason"].as_str().expect("reason is set");
            assert!(
                reason.contains(&format!("no release \"{missing}\"")),
                "{context}: {reason}"
            );
            let whole = expected_json(name);
            for key in ["current", "counts", "cut", "total", "prefix", "dir", "unit"] {
                assert_eq!(json[key], whole[key], "{context}: {key}");
            }
            assert_eq!(json["truncated"], false);
            assert_eq!(json["omitted"], 0);
        }
    }
}

/// Exit 2, nothing on stdout (no JSON document either): no
/// `specengine.toml`; a project without `[roadmap]`; another config error.
#[test]
fn no_roadmap_table_exits_2() {
    let scratch = Scratch::new("roadmap-exit2");
    let home = scratch.home("h");
    for (fixture_name, json) in [("spec-a", false), ("spec-b", true), ("spec-c", false)] {
        let root = scratch.copy(fixture_name, fixture_name);
        let args: &[&str] = if json {
            &["--json", "roadmap"]
        } else {
            &["roadmap"]
        };
        let run = spec(&home, &root, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{fixture_name}");
        assert!(
            run.stderr.contains("specengine.toml has no [roadmap]"),
            "{fixture_name}: {}",
            run.stderr
        );
    }
    let empty = scratch.dir("empty");
    for args in [&["roadmap"][..], &["--json", "roadmap"][..]] {
        let run = spec(&home, &empty, args);
        run.code(2);
        assert_eq!(run.stdout, "", "no specengine.toml");
        assert!(!run.stderr.is_empty());
    }
    // A config error outside `[roadmap]`.
    let root = scratch.copy("roadmap-a", "bad-ids");
    let config = read_text(&root, "specengine.toml");
    write(
        &root,
        "specengine.toml",
        config.replace("RM  = { kind = \"plan-item\",   width = 4 }", "RM  = 3"),
    );
    let run = spec(&home, &root, &["roadmap"]);
    run.code(2);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.contains("specengine.toml:"), "{}", run.stderr);
}

/// Exit 2 naming the line: a bad `[roadmap]` (`specengine.toml:<line>:
/// …`), text and JSON, nothing on stdout.
#[test]
fn bad_roadmap_table_exits_2_naming_its_line() {
    let scratch = Scratch::new("roadmap-bad-table");
    let home = scratch.home("h");
    let root = scratch.copy("roadmap-a", "copy");
    let config = read_text(&root, "specengine.toml");
    let line_of = |text: &str, needle: &str| {
        text.lines()
            .position(|line| line.contains(needle))
            .expect(needle)
            + 1
    };
    for (from, to, fragment) in [
        (
            "releases   = [\"alpha\", \"beta\", \"launch\"]",
            "releases   = [\"alpha\", \"beta\", \"alpha\"]",
            "`roadmap.releases`: \"alpha\" is repeated",
        ),
        (
            "unit       = \"days\"",
            "unit = \"days\"\ncolour = 1",
            "unknown field `colour`",
        ),
        ("prefix     = \"RM\"", "prefix = \"BAY\"", "roadmap.prefix"),
        (
            "mvp_cut    = \"beta\"",
            "mvp_cut = \"gamma\"",
            "roadmap.mvp_cut",
        ),
    ] {
        assert!(config.contains(from), "{from}");
        let bad = config.replace(from, to);
        write(&root, "specengine.toml", &bad);
        let needle = to.lines().last().unwrap();
        let line = line_of(&bad, needle);
        for json in [false, true] {
            let args: &[&str] = if json {
                &["--json", "roadmap"]
            } else {
                &["roadmap"]
            };
            let run = spec(&home, &root, args);
            run.code(2);
            assert_eq!(run.stdout, "", "{to}");
            assert!(
                run.stderr
                    .lines()
                    .any(|l| l.contains(&format!("specengine.toml:{line}: "))
                        && l.contains(fragment)),
                "{to}: line {line}, {fragment:?} in\n{}",
                run.stderr
            );
        }
        // Every other command still reads the project (only `spec
        // roadmap` reads the table).
        spec(&home, &root, &["tree"]).code(0);
    }
}

/// `--release` with `--pool` is a usage error: exit 2, nothing on stdout.
#[test]
fn release_with_pool_is_usage() {
    let scratch = Scratch::new("roadmap-usage");
    let home = scratch.home("h");
    let root = scratch.copy("roadmap-a", "copy");
    for args in [
        &["roadmap", "--release", "beta", "--pool"][..],
        &["--json", "roadmap", "--pool", "--release", "beta"][..],
    ] {
        let run = spec(&home, &root, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}");
        assert!(run.stderr.contains("Usage:"), "{args:?}: {}", run.stderr);
    }
}

/// RD-1, the CLI half: `spec check` and `spec tree` run on both fixtures
/// (the table name-checked, no `cannot`); a bad `[roadmap]` fails `spec
/// roadmap` alone; `[roadmapp]` is an unknown key every command refuses.
#[test]
fn check_and_tree_run_on_the_fixtures() {
    for (name, _) in FIXTURES {
        let scratch = Scratch::new(&format!("roadmap-check-{name}"));
        let home = scratch.home("h");
        let root = scratch.copy(name, "copy");
        let check = spec(&home, &root, &["check"]);
        check.code(0);
        assert!(
            !check.stdout.contains("cannot"),
            "{name}:\n{}",
            check.stdout
        );
        spec(&home, &root, &["tree"]).code(0);

        let config = read_text(&root, "specengine.toml");
        write(
            &root,
            "specengine.toml",
            config.replace("[roadmap]", "[roadmapp]"),
        );
        let check = spec(&home, &root, &["check"]);
        check.code(2);
        assert!(
            check.stdout.contains("roadmapp"),
            "{name}:\n{}",
            check.stdout
        );
        spec(&home, &root, &["tree"]).code(2);
        spec(&home, &root, &["roadmap"]).code(2);
    }
}

// ------------------------------------------------------------------- RD-7

/// Every file of `fixtures/<name>`, root-relative, sorted.
fn fixture_files(name: &str) -> Vec<(String, Vec<u8>)> {
    snapshot(&fixture(name))
        .into_iter()
        .filter_map(|(path, bytes)| bytes.map(|bytes| (path, bytes)))
        .collect()
}

/// M7: the same bytes under two roots of different length and two `HOME`s,
/// the second one's files created in reverse order and indexed in two
/// steps (its later half first, so the index holds them in another
/// order): text and JSON byte-identical, and the oracle's.
#[test]
fn roadmap_is_byte_identical_across_roots_and_order() {
    for (name, releases) in FIXTURES {
        let scratch = Scratch::new(&format!("roadmap-bytes-{name}"));
        let first = scratch.copy(name, "one");
        let first_home = scratch.home("first");
        let second = scratch.dir("a/much/longer/second/root");
        let second_home = scratch.home("second-home-of-another-length");

        let mut files = fixture_files(name);
        files.reverse();
        let (config, rest): (Vec<_>, Vec<_>) = files
            .into_iter()
            .partition(|(path, _)| path == "specengine.toml");
        for (path, bytes) in &config {
            write(&second, path, bytes);
        }
        let half = rest.len() / 2;
        // The later half by path, created in reverse order, indexed first.
        for (path, bytes) in &rest[..half] {
            write(&second, path, bytes);
        }
        spec(&second_home, &second, &["roadmap"]);
        for (path, bytes) in &rest[half..] {
            write(&second, path, bytes);
        }

        let mut requests: Vec<Vec<&str>> = vec![vec!["roadmap"], vec!["roadmap", "--pool"]];
        for release in releases {
            requests.push(vec!["roadmap", "--release", release]);
        }
        for request in requests {
            for json in [false, true] {
                let mut args = request.clone();
                if json {
                    args.insert(0, "--json");
                }
                let one = spec(&first_home, &first, &args);
                let two = spec(&second_home, &second, &args);
                one.code(0);
                two.code(0);
                assert!(
                    one.stdout.as_bytes() == two.stdout.as_bytes(),
                    "{name} {args:?}: byte-identical across roots and order\n--- one\n{}--- two\n{}",
                    one.stdout,
                    two.stdout
                );
                assert_eq!(one.stderr, two.stderr, "{name} {args:?}: the notes alike");
                if request.len() == 1 {
                    if json {
                        assert_eq!(one.json(), expected_json(name), "{name}");
                    } else {
                        assert_eq!(one.stdout, expected_text(name), "{name}");
                    }
                }
            }
        }
        // A repeat run is byte-identical too.
        let again = spec(&second_home, &second, &["--json", "roadmap"]);
        assert_eq!(
            again.stdout,
            spec(&first_home, &first, &["--json", "roadmap"]).stdout
        );
    }
}

/// The generated corpus of the cut: releases `bulk` (500 open items, long
/// titles) and `spare` (3), and 2 pool items; every item `high`, `1-2`.
fn bulk_corpus(root: &Path) {
    write(
        root,
        "specengine.toml",
        "[project]\nslug = \"bulk-plan\"\n\n[paths]\nroots = [\"plan\"]\n\n[ids]\nBK = { kind = \"bulk-item\", width = 4 }\n\n[roadmap]\nprefix = \"BK\"\ndir = \"plan\"\nreleases = [\"bulk\", \"spare\"]\npriorities = [\"high\", \"low\"]\nunit = \"hours\"\n",
    );
    for n in 1..=505 {
        let release = match n {
            1..=500 => "release: bulk\n",
            501..=503 => "release: spare\n",
            _ => "",
        };
        write(
            root,
            &format!("plan/BK-{n:04}.md"),
            format!(
                "---\nclass: spec\nid: BK-{n:04}\nstatus: draft\nscope: [bulk]\n{release}priority: high\nestimate: 1-2\n---\n\n# {}\n\nGoal.\n",
                bulk_title(n)
            ),
        );
    }
}

fn bulk_title(n: usize) -> String {
    format!("Bulk item {n:04} with a long title that fills the line of the plan view")
}

/// The text line item `n` of `bulk` prints as (position `n`, ID order).
fn bulk_line(n: usize) -> String {
    format!(
        "  {n}. BK-{n:04} | {} | draft | high | 1-2 | plan/BK-{n:04}.md",
        bulk_title(n)
    )
}

/// The printed part of a cut output and its tail `(k, n)`.
#[track_caller]
fn cut_parts(stdout: &str) -> (&str, usize, usize) {
    let body = stdout.strip_suffix('\n').expect("a line end");
    let (before, tail) = body.rsplit_once('\n').expect("a tail line");
    // The tail's brackets are literal, as every read's (`cap.rs`).
    let rest = tail
        .strip_prefix("[truncated: ")
        .unwrap_or_else(|| panic!("the tail: {tail:?}"));
    let (k, rest) = rest.split_once(" of ").expect("`k of n`");
    let (n, rest) = rest.split_once(' ').expect("`n items`");
    assert_eq!(
        rest, "items not shown; add --release R or --pool]",
        "{tail}"
    );
    let before = &stdout[..before.len() + 1];
    (before, k.parse().unwrap(), n.parse().unwrap())
}

/// M8: 500 items in one release. The whole plan and `--release bulk`
/// print whole lines up to at most 40 000 characters, the next line would
/// pass the cap, and the tail counts the rest (`k` + shown = the items the
/// request lists); JSON holds what the text prints, `omitted` = `k`;
/// `--release spare` and `--pool` are uncut.
#[test]
fn roadmap_cut_names_its_narrowing() {
    let scratch = Scratch::new("roadmap-cut");
    let home = scratch.home("h");
    let root = scratch.dir("bulk");
    bulk_corpus(&root);

    for (request, listed) in [
        (&["roadmap"][..], 505),
        (&["roadmap", "--release", "bulk"][..], 500),
    ] {
        let run = spec(&home, &root, request);
        run.code(0);
        let (before, k, n) = cut_parts(&run.stdout);
        assert_eq!(n, listed, "{request:?}: n");
        assert!(
            before.chars().count() <= CAP,
            "{request:?}: {} characters before the tail",
            before.chars().count()
        );
        let shown: Vec<&str> = before
            .lines()
            .filter(|line| line.starts_with("  "))
            .collect();
        assert_eq!(k + shown.len(), listed, "{request:?}: k + shown");
        assert!(k > 0, "{request:?}: cut");
        for (at, line) in shown.iter().enumerate() {
            assert_eq!(
                *line,
                bulk_line(at + 1),
                "{request:?}: whole lines, in order"
            );
        }
        let next = bulk_line(shown.len() + 1);
        assert!(
            before.chars().count() + next.chars().count() + 1 > CAP,
            "{request:?}: the next line would fit ({} + {})",
            before.chars().count(),
            next.chars().count()
        );
        assert_eq!(
            before
                .lines()
                .filter(|line| !line.starts_with("  "))
                .count(),
            2,
            "{request:?}: the first line and bulk's header only"
        );

        let mut args = vec!["--json"];
        args.extend_from_slice(request);
        let json = spec(&home, &root, &args).json();
        assert_eq!(json["truncated"], true, "{request:?}");
        assert_eq!(json["omitted"], k as u64, "{request:?}");
        let groups = json["groups"].as_array().unwrap();
        assert_eq!(
            groups.len(),
            1,
            "{request:?}: the groups whose header printed"
        );
        assert_eq!(groups[0]["name"], "bulk");
        let ids: Vec<&str> = groups[0]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect();
        let want: Vec<String> = (1..=shown.len()).map(|n| format!("BK-{n:04}")).collect();
        assert_eq!(ids, want, "{request:?}: the printed items");
        assert_eq!(groups[0]["open"], 500, "the counts stay whole");
        assert_eq!(json["counts"]["open"], 505);
    }

    for request in [
        &["roadmap", "--release", "spare"][..],
        &["roadmap", "--pool"][..],
    ] {
        let run = spec(&home, &root, request);
        run.code(0);
        assert!(
            !run.stdout.contains("[truncated: "),
            "{request:?}:\n{}",
            run.stdout
        );
        let mut args = vec!["--json"];
        args.extend_from_slice(request);
        let json = spec(&home, &root, &args).json();
        assert_eq!(json["truncated"], false, "{request:?}");
        assert_eq!(json["omitted"], 0, "{request:?}");
    }
    let spare = spec(&home, &root, &["roadmap", "--release", "spare"]);
    assert_eq!(
        spare.stdout.lines().filter(|l| l.starts_with("  ")).count(),
        3
    );
}

/// A corpus of releases `bulk` (`count` open items) and `empty` (none),
/// no pool, nothing unplaced; the last item's title padded by `pad`
/// characters.
fn edge_corpus(root: &Path, count: usize, pad: usize) {
    write(
        root,
        "specengine.toml",
        "[project]\nslug = \"edge-plan\"\n\n[paths]\nroots = [\"plan\"]\n\n[ids]\nBK = { kind = \"bulk-item\", width = 4 }\n\n[roadmap]\nprefix = \"BK\"\ndir = \"plan\"\nreleases = [\"bulk\", \"empty\"]\npriorities = [\"high\", \"low\"]\nunit = \"hours\"\n",
    );
    for n in 1..=count {
        let mut title = bulk_title(n);
        if n == count {
            title.push_str(&"x".repeat(pad));
        }
        write(
            root,
            &format!("plan/BK-{n:04}.md"),
            format!(
                "---\nclass: spec\nid: BK-{n:04}\nstatus: draft\nscope: [bulk]\nrelease: bulk\npriority: high\nestimate: 1-2\n---\n\n# {title}\n\nGoal.\n"
            ),
        );
    }
}

/// Canon `spec-roadmap` "Cut" (1a iteration 2, b): the tail and
/// `truncated: true` only when k > 0. Every item printed with the output
/// at exactly 40 000 characters, so the empty release's, the pool's and
/// the unplaced headers do not fit: no tail, `truncated` false, `omitted`
/// 0, JSON the groups whose header printed (`bulk`). One character more in
/// the last title: that item is omitted, the tail `1 of <n>`, `truncated`
/// true.
#[test]
fn an_empty_group_past_the_cap_is_no_truncation() {
    let scratch = Scratch::new("roadmap-cut-edge");
    let home = scratch.home("h");
    let count = 300;

    // The bulk release alone, unpadded: its length sets the padding.
    let probe = scratch.dir("probe");
    edge_corpus(&probe, count, 0);
    let run = spec(&home, &probe, &["roadmap", "--release", "bulk"]);
    run.code(0);
    assert!(!run.stdout.contains("[truncated: "), "{}", run.stdout);
    let length = run.stdout.chars().count();
    assert!(length < CAP, "{length}: the probe fits");
    let pad = CAP - length;

    // Exactly at the cap: every item printed, the empty headers cut.
    let exact = scratch.dir("exact");
    edge_corpus(&exact, count, pad);
    let bulk = spec(&home, &exact, &["roadmap", "--release", "bulk"]);
    bulk.code(0);
    assert_eq!(
        bulk.stdout.chars().count(),
        CAP,
        "the bulk release fills the cap"
    );
    let whole = spec(&home, &exact, &["roadmap"]);
    whole.code(0);
    assert_eq!(
        whole.stdout, bulk.stdout,
        "the first line and bulk's lines alone; no header after them, no tail"
    );
    assert_eq!(
        whole
            .stdout
            .lines()
            .filter(|line| line.starts_with("  "))
            .count(),
        count
    );
    let json = spec(&home, &exact, &["--json", "roadmap"]).json();
    assert_eq!(json["truncated"], false);
    assert_eq!(json["omitted"], 0);
    let groups = json["groups"].as_array().unwrap();
    assert_eq!(
        groups.len(),
        1,
        "the groups whose header printed: {groups:?}"
    );
    assert_eq!(groups[0]["name"], "bulk");
    assert_eq!(groups[0]["items"].as_array().unwrap().len(), count);
    // The empty release on its own prints whole.
    let empty = spec(&home, &exact, &["roadmap", "--release", "empty"]);
    empty.code(0);
    assert_eq!(
        empty.stdout.lines().skip(1).collect::<Vec<_>>(),
        ["release empty | 0 open, 0 done | 0 hours"]
    );

    // One character more: the last item no longer fits.
    let over = scratch.dir("over");
    edge_corpus(&over, count, pad + 1);
    let run = spec(&home, &over, &["roadmap"]);
    run.code(0);
    let (before, k, n) = cut_parts(&run.stdout);
    assert_eq!(
        (k, n),
        (1, count),
        "{}",
        run.stdout.lines().last().unwrap_or_default()
    );
    assert!(before.chars().count() <= CAP);
    assert_eq!(
        before.lines().filter(|line| line.starts_with("  ")).count(),
        count - 1
    );
    let json = spec(&home, &over, &["--json", "roadmap"]).json();
    assert_eq!(json["truncated"], true);
    assert_eq!(json["omitted"], 1);
}

/// `spec args` in `root` with the git sandbox's variables and `HOME`.
fn spec_in_git(sandbox: &Sandbox, home: &Path, root: &Path, args: &[&str]) -> Run {
    let mut vars: Vec<(OsString, OsString)> = sandbox
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .collect();
    vars.push(("HOME".into(), home.as_os_str().to_owned()));
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.to_str().expect("UTF-8 name"), value.as_os_str()))
        .collect();
    spec_with(root, args, &env)
}

/// Every file outside `.git`.
fn worktree(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    snapshot(root)
        .into_iter()
        .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
        .collect()
}

/// RD-7: `spec roadmap` in every form (a fresh index, an update, every
/// exit) writes nothing under the root: `git status --porcelain=v1 -z
/// --untracked-files=all`, `HEAD` and every file outside `.git` unchanged.
#[test]
fn roadmap_writes_nothing_under_the_root() {
    for (name, releases) in FIXTURES {
        let scratch = Scratch::new(&format!("roadmap-ro-{name}"));
        let sandbox = Sandbox::new(scratch.path());
        let home = scratch.home("h");
        let root: PathBuf = scratch.copy(name, "repo");
        sandbox.init(&root);
        sandbox.add_all(&root);
        sandbox.commit(&root, "the plan");
        let porcelain = |root: &Path| {
            sandbox.git(
                root,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            )
        };
        let head = sandbox.head(&root);
        assert!(head.is_some());
        let status = porcelain(&root);
        assert!(status.is_empty(), "{name}: a clean start");
        let files = worktree(&root);

        let mut requests: Vec<(Vec<&str>, i32)> = vec![
            (vec!["roadmap"], 0),
            (vec!["--json", "roadmap"], 0),
            (vec!["roadmap", "--pool"], 0),
            (vec!["roadmap", "--release", "nowhere"], 1),
            (vec!["--json", "roadmap", "--release", "nowhere"], 1),
            (vec!["roadmap", "--release", releases[1], "--pool"], 2),
        ];
        for release in releases {
            requests.push((vec!["roadmap", "--release", release], 0));
        }
        for (args, code) in &requests {
            spec_in_git(&sandbox, &home, &root, args).code(*code);
        }
        // An edited item (an index update), then the reads again.
        let item = files
            .keys()
            .find(|path| path.ends_with(".md") && !path.starts_with("docs/"))
            .expect("an item")
            .clone();
        let edited = format!("{}\nOne more line of the goal.\n", read_text(&root, &item));
        write(&root, &item, &edited);
        let status_edited = porcelain(&root);
        let files_edited = worktree(&root);
        for (args, code) in &requests {
            spec_in_git(&sandbox, &home, &root, args).code(*code);
        }
        assert_eq!(porcelain(&root), status_edited, "{name}: git status");
        assert_eq!(sandbox.head(&root), head, "{name}: HEAD");
        assert!(
            worktree(&root) == files_edited,
            "{name}: the files outside .git"
        );
        assert_ne!(files_edited, files, "the edit is visible");
    }
}
