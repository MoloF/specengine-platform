//! docs/features/roadmap-view.md RD-3, RD-4 and TE-1, the CLI half, on
//! scratch copies of `fixtures/roadmap-a` (a harbor game's plan, a unit, a
//! cut) and `fixtures/roadmap-b` (a lending desk's plan, Greek and Japanese
//! titles, no unit, no cut) whose configs (written into the copies, never
//! `fixtures/`) add `[paths] index` and an `index = true` entry with
//! `roadmap` in a subdirectory: `docs/views/roadmap.md` (a) and
//! `docs/generated/work/plan.md` (b).
//!
//! - TE-1: `spec export index` writes each fixture's `expected-view.md`
//!   byte for byte, under two roots of different length and creation
//!   order alike; `spec check` then reports no `index-*` finding.
//! - RD-3 (git only): export and check run with no `HOME` and no
//!   `[project]` slug, nothing written outside the root; under `--staged`
//!   the staged items and the staged config decide the view; an
//!   `in_progress` task executing an item leaves the view's bytes alone.
//! - RD-4 (Q-A): the view is `spec roadmap`'s lines as Markdown, derived
//!   here from `spec roadmap`'s own output (the summary a paragraph, each
//!   group a `##` heading, each item linked from the view's directory, no
//!   path column, `## Notes` with the JSON's notes).
//! - Q-B: with a view, a bad `[roadmap]` fails `check` and `export` at its
//!   line; without the key the same table fails `spec roadmap` alone.
//!
//! Named mutation: M4 (the view computed through `spec roadmap`'s path:
//! the index database, the slug, the data directory) turns
//! `export_writes_the_view_without_a_data_directory` red.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::fs;
use std::path::{Path, PathBuf};

use common::check::{quoted, set_paths_key, without_project};
use common::git::Sandbox;
use common::proposal::Pair;
use common::staged::Repo;
use common::{Run, Scratch, data_dir, fixture, read, read_text, snapshot, spec, spec_with, write};
use serde_json::Value;
use task_common::Tasks as _;

/// The registered command of every scratch config.
const COMMAND: &str = "gen-index";

/// The index root of every scratch config: inside both fixtures' walk.
const INDEX: &str = "docs/index.md";

/// `(fixture, the view's path)`: a subdirectory each, of different depth.
const CASES: [(&str, &str); 2] = [
    ("roadmap-a", "docs/views/roadmap.md"),
    ("roadmap-b", "docs/generated/work/plan.md"),
];

/// `base` plus `[paths] index` and the index entry writing the index and
/// the view at `view`.
fn view_config(base: &str, view: &str) -> String {
    let mut text = set_paths_key(base, "index", &quoted(INDEX));
    text.push_str(&format!(
        "\n[[generators]]\ncommand = {}\nwrites  = [{}, {}]\nindex   = true\nroadmap = {}\n",
        quoted(COMMAND),
        quoted(INDEX),
        quoted(view),
        quoted(view)
    ));
    text
}

/// A scratch copy of `name` at `<scratch>/<dir>` with the view registered.
fn viewed_copy(scratch: &Scratch, name: &str, dir: &str, view: &str) -> PathBuf {
    let root = scratch.copy(name, dir);
    let base = read_text(&root, "specengine.toml");
    write(&root, "specengine.toml", view_config(&base, view));
    root
}

/// The directories of every output of the view's config, created.
fn make_dirs(root: &Path, view: &str) {
    for path in [INDEX, view] {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
    }
}

fn expected_view(name: &str) -> String {
    fs::read_to_string(fixture(name).join("expected-view.md")).expect("expected-view.md")
}

/// `(code, path)` of every `index-*` finding of a `--json` check.
fn index_findings(run: &Run) -> Vec<(String, String)> {
    let json: Value = serde_json::from_str(&run.stdout)
        .unwrap_or_else(|e| panic!("a JSON report: {e}\n{}", run.show()));
    json["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["code"].as_str().is_some_and(|c| c.starts_with("index-")))
        .map(|f| {
            (
                f["code"].as_str().unwrap().to_owned(),
                f["path"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// `target` relative to the directory of the file `from` (both
/// root-relative): the common leading directories dropped, one `..` per
/// directory of `from` left.
fn relative(from: &str, target: &str) -> String {
    let from: Vec<&str> = from.split('/').collect();
    let from = &from[..from.len() - 1];
    let to: Vec<&str> = target.split('/').collect();
    let common = from
        .iter()
        .zip(&to[..to.len() - 1])
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts = vec![".."; from.len() - common];
    parts.extend_from_slice(&to[common..]);
    parts.join("/")
}

// ------------------------------------------------------------------- TE-1

/// TE-1: what `export index` writes is `expected-view.md`, and the copy
/// then checks with no `index-*` finding; a rerun leaves it `unchanged`.
#[test]
fn expected_views_are_what_export_writes() {
    for (name, view) in CASES {
        let scratch = Scratch::new(&format!("view-te1-{name}"));
        let home = scratch.home("h");
        let root = viewed_copy(&scratch, name, "copy", view);
        make_dirs(&root, view);
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        let written = read_text(&root, view);
        assert!(
            written == expected_view(name),
            "{name}: export wrote\n{written}"
        );
        assert_eq!(
            run.stdout.lines().last(),
            Some(format!("wrote {view}: {} bytes", written.len()).as_str()),
            "{name}: the view last\n{}",
            run.stdout
        );
        let check = spec(&home, &root, &["--json", "check"]);
        assert!(check.code == 0, "{name}: {}", check.show());
        assert_eq!(index_findings(&check), [], "{name}");
        let again = spec(&home, &root, &["export", "index"]);
        again.code(0);
        assert!(
            again
                .stdout
                .lines()
                .all(|line| line.starts_with("unchanged ")),
            "{name}: {}",
            again.stdout
        );
        assert!(read_text(&root, view) == written, "{name}: rerun");
    }
}

/// TE-1 and "Rules": the same view under two roots of different length,
/// the second's files created in reverse order and indexed in between, two
/// `HOME`s: byte-identical, and the fixture's.
#[test]
fn the_view_is_byte_identical_across_roots_and_order() {
    for (name, view) in CASES {
        let scratch = Scratch::new(&format!("view-roots-{name}"));
        let first = viewed_copy(&scratch, name, "one", view);
        make_dirs(&first, view);
        let second = scratch.dir("a/much/longer/second/root/of/another/length");
        let mut files: Vec<(String, Vec<u8>)> = snapshot(&first)
            .into_iter()
            .filter_map(|(path, bytes)| bytes.map(|bytes| (path, bytes)))
            .collect();
        files.reverse();
        let (config, rest): (Vec<_>, Vec<_>) = files
            .into_iter()
            .partition(|(path, _)| path == "specengine.toml");
        for (path, bytes) in &config {
            write(&second, path, bytes);
        }
        let second_home = scratch.home("second-home-of-another-length");
        let half = rest.len() / 2;
        for (path, bytes) in &rest[..half] {
            write(&second, path, bytes);
        }
        // An index database filled in another order meanwhile: never read.
        spec(&second_home, &second, &["roadmap"]);
        for (path, bytes) in &rest[half..] {
            write(&second, path, bytes);
        }
        make_dirs(&second, view);

        spec(&scratch.home("first"), &first, &["export", "index"]).code(0);
        spec(&second_home, &second, &["export", "index"]).code(0);
        let one = read(&first, view);
        let two = read(&second, view);
        assert!(one == two, "{name}: across roots and order");
        assert!(one == expected_view(name).into_bytes(), "{name}");
        // `--stdout`, twice: the same bytes as written.
        let printed = spec(&second_home, &second, &["export", "index", "--stdout"]);
        printed.code(0);
        assert_eq!(
            printed.stdout,
            spec(&second_home, &second, &["export", "index", "--stdout"]).stdout
        );
        let marker = format!("==> {view} <==\n");
        let at = printed
            .stdout
            .find(&marker)
            .unwrap_or_else(|| panic!("{name}: no {marker:?}\n{}", printed.stdout));
        assert!(
            &printed.stdout.as_bytes()[at + marker.len()..] == one.as_slice(),
            "{name}: --stdout ends with the view"
        );
    }
}

// ------------------------------------------------------------------- RD-3

/// M4: export and check need no `HOME`, no slug, no data directory; the
/// view is the fixture's, and nothing is written outside the root.
#[test]
fn export_writes_the_view_without_a_data_directory() {
    for (name, view) in CASES {
        let scratch = Scratch::new(&format!("view-nohome-{name}"));
        let root = viewed_copy(&scratch, name, "copy", view);
        make_dirs(&root, view);
        // No `[project]` table: no slug.
        let config = read_text(&root, "specengine.toml");
        write(&root, "specengine.toml", without_project(&config));
        assert!(!read_text(&root, "specengine.toml").contains("slug"));
        let outside = || {
            snapshot(scratch.path())
                .into_keys()
                .filter(|path| !path.starts_with("copy"))
                .collect::<Vec<_>>()
        };
        let before = outside();

        // No environment at all: no `HOME`, no `XDG_DATA_HOME`.
        let run = spec_with(&root, &["export", "index"], &[]);
        assert_eq!(run.code, 0, "{name}: export without HOME\n{}", run.show());
        assert!(
            read_text(&root, view) == expected_view(name),
            "{name}: the view without HOME"
        );
        let check = spec_with(&root, &["--json", "check"], &[]);
        assert_eq!(
            check.code,
            0,
            "{name}: check without HOME\n{}",
            check.show()
        );
        assert_eq!(index_findings(&check), [], "{name}");
        assert_eq!(outside(), before, "{name}: written outside the root");

        // With a `HOME`: still no data directory.
        let home = scratch.home("h");
        fs::remove_file(root.join(view)).unwrap();
        let run = spec(&home, &root, &["export", "index"]);
        assert_eq!(run.code, 0, "{name}: {}", run.show());
        spec(&home, &root, &["check"]).code(0);
        assert!(
            !data_dir(&home).exists(),
            "{name}: export or check made a data directory"
        );
        assert!(read_text(&root, view) == expected_view(name), "{name}");
    }
}

/// `(code, path)` of the `index-*` findings of `spec --json check
/// [--staged]` in the repository.
fn staged_index(repo: &Repo, staged: bool) -> Vec<(String, String)> {
    let run = if staged {
        repo.staged_check(&["--json"])
    } else {
        repo.plain_check(&["--json"])
    };
    assert!(run.code < 2, "{}", run.show());
    index_findings(&run)
}

/// Under `--staged` the staged item files and the staged config decide the
/// view; the working tree's never do.
#[test]
fn staged_view_follows_the_staged_items_and_config() {
    for (name, view) in CASES {
        let repo = Repo::of(&format!("view-staged-{name}"), name);
        let top = repo.top.clone();
        let base = read_text(&top, "specengine.toml");
        write(&top, "specengine.toml", view_config(&base, view));
        make_dirs(&top, view);
        repo.spec(&["export", "index"]).code(0);
        repo.add_all();
        repo.git.commit(&top, "the plan and its view");
        assert_eq!(staged_index(&repo, true), [], "{name}: committed");
        let drift = vec![("index-drift".to_owned(), view.to_owned())];

        // An item's priority, a field the index never shows: on disk only,
        // then staged, then the view regenerated and staged.
        let (item, from, to) = if name == "roadmap-a" {
            ("plan/RM-0016.md", "priority: could\n", "priority: must\n")
        } else {
            ("work/WORK-040.md", "priority: p0\n", "priority: p2\n")
        };
        let text = read_text(&top, item);
        assert_eq!(text.matches(from).count(), 1, "{name}: {item}");
        write(&top, item, text.replacen(from, to, 1));
        assert_eq!(staged_index(&repo, true), [], "{name}: item on disk only");
        assert_eq!(staged_index(&repo, false), drift, "{name}: plain sees it");
        repo.git.git(&top, &["add", "--", item]);
        assert_eq!(staged_index(&repo, true), drift, "{name}: item staged");
        repo.spec(&["export", "index"]).code(0);
        assert_eq!(staged_index(&repo, true), drift, "{name}: view not staged");
        repo.git.git(&top, &["add", "--", view]);
        assert_eq!(staged_index(&repo, true), [], "{name}: view staged");

        // `[roadmap]`: the priorities reversed, on disk only, then staged.
        let config = read_text(&top, "specengine.toml");
        let priorities = config
            .lines()
            .find(|line| line.starts_with("priorities"))
            .expect("a priorities line")
            .to_owned();
        let (key, list) = priorities.split_once('=').unwrap();
        let mut values: Vec<&str> = list
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(str::trim)
            .collect();
        values.reverse();
        let reversed = format!("{key}= [{}]", values.join(", "));
        write(
            &top,
            "specengine.toml",
            config.replacen(&priorities, &reversed, 1),
        );
        assert_eq!(staged_index(&repo, true), [], "{name}: config on disk only");
        assert_eq!(staged_index(&repo, false), drift, "{name}: plain sees it");
        repo.git.git(&top, &["add", "--", "specengine.toml"]);
        assert_eq!(staged_index(&repo, true), drift, "{name}: config staged");
        repo.spec(&["export", "index"]).code(0);
        repo.git.git(&top, &["add", "--", view]);
        assert_eq!(staged_index(&repo, true), [], "{name}: regenerated");
        assert_eq!(staged_index(&repo, false), [], "{name}: plain too");
    }
}

/// Slice 2's guard ("Rules": the queue changes, no file does): an
/// `in_progress` task executing a plan item leaves the view's bytes, and
/// export reports every output `unchanged`.
#[test]
fn view_ignores_the_queue() {
    let (name, view) = CASES[0];
    let scratch = Scratch::new("view-queue");
    let git = Sandbox::new(scratch.path());
    let main = viewed_copy(&scratch, name, "main", view);
    make_dirs(&main, view);
    let home = scratch.home("h");
    spec(&home, &main, &["export", "index"]).code(0);
    let before = read(&main, view);
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "the plan and its view");
    let pair = Pair {
        scratch,
        git,
        main: main.clone(),
        linked: main.clone(),
        home: home.clone(),
        slug: "tide-runner".to_owned(),
    };

    let id = pair.new_ok(&main, &["RM-0010"]);
    pair.plan_ok(&main, &id, &["Holds."], &[]);
    pair.approve_task(&main, &id);
    pair.claim_ok(&main, &id, &main);
    let package = pair.package(&main, &id);
    assert_eq!(package["status"], "in_progress", "{package}");
    assert!(
        package["targets"].to_string().contains("RM-0010"),
        "{}",
        package["targets"]
    );
    assert!(
        data_dir(&home).exists(),
        "the queue is in the data directory"
    );

    let run = spec(&home, &main, &["export", "index"]);
    run.code(0);
    assert!(
        run.stdout
            .lines()
            .all(|line| line.starts_with("unchanged ")),
        "{}",
        run.stdout
    );
    assert!(read(&main, view) == before, "the queue entered the view");
    let printed = spec(&home, &main, &["export", "index", "--stdout"]);
    printed.code(0);
    assert!(
        printed
            .stdout
            .ends_with(std::str::from_utf8(&before).unwrap()),
        "--stdout"
    );
    let check = spec(&home, &main, &["--json", "check"]);
    assert_eq!(index_findings(&check), [], "{}", check.show());
}

// ------------------------------------------------------------------- RD-4

/// The view's header for [`COMMAND`] and the default gate.
fn header() -> String {
    format!(
        "---\nclass: generated\ngenerator: {COMMAND}\nsource: the plan items of [roadmap]\n---\n\n# Roadmap\n\n<!-- Built by `{COMMAND}`. Manual edits are overwritten on rebuild, and `spec check` rejects them. -->\n\n"
    )
}

/// The view `spec roadmap`'s output implies (Q-A, "Data"): its text and its
/// JSON (the items' paths, the plan's notes) in Markdown, links from the
/// directory of `view`.
fn view_from_roadmap(text: &str, json: &Value, view: &str) -> String {
    let paths: Vec<(String, String)> = json["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["items"].as_array().unwrap())
        .map(|item| {
            (
                item["id"].as_str().unwrap().to_owned(),
                item["path"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let mut lines = text.lines();
    let mut out = header();
    out.push_str(lines.next().expect("the summary"));
    out.push('\n');
    let mut first_item = false;
    for line in lines {
        if let Some(entry) = line.strip_prefix("  ") {
            if first_item {
                out.push('\n');
                first_item = false;
            }
            let (lead, rest) = entry.split_once(' ').expect("a lead");
            let (id, rest) = rest.split_once(" | ").expect("an ID");
            let path = &paths
                .iter()
                .find(|(known, _)| known == id)
                .unwrap_or_else(|| panic!("no path of {id}"))
                .1;
            let column = format!(" | {path}");
            assert_eq!(rest.matches(&column).count(), 1, "{line}");
            let rest = rest.replacen(&column, "", 1);
            out.push_str(&format!(
                "{lead} [{id}]({}) | {rest}\n",
                relative(view, path)
            ));
        } else {
            out.push_str(&format!("\n## {line}\n"));
            first_item = true;
        }
    }
    let notes = json["notes"].as_array().unwrap();
    if !notes.is_empty() {
        out.push_str("\n## Notes\n\n");
        for note in notes {
            out.push_str(&format!("- {}\n", note.as_str().unwrap()));
        }
    }
    out
}

/// Q-A on both fixtures: the view written is `spec roadmap`'s text in
/// Markdown, every item linked from the view's directory to a file that
/// exists, the notes those of `spec roadmap --json`.
#[test]
fn view_lines_are_the_roadmap_lines() {
    for (name, view) in CASES {
        let scratch = Scratch::new(&format!("view-lines-{name}"));
        let home = scratch.home("h");
        let root = viewed_copy(&scratch, name, "copy", view);
        make_dirs(&root, view);
        let text = spec(&home, &root, &["roadmap"]);
        text.code(0);
        let json = spec(&home, &root, &["--json", "roadmap"]);
        json.code(0);
        let json = json.json();
        assert_eq!(json["truncated"], false, "{name}: uncut");
        assert!(
            !json["notes"].as_array().unwrap().is_empty(),
            "{name}: the fixture has notes"
        );
        spec(&home, &root, &["export", "index"]).code(0);
        let written = read_text(&root, view);
        let want = view_from_roadmap(&text.stdout, &json, view);
        assert!(
            written == want,
            "{name}: the view is not spec roadmap's lines\n--- written\n{written}--- derived\n{want}"
        );
        // Every link names a file of the copy, read from the view's
        // directory, and none is root-relative.
        let dir = root.join(view).parent().unwrap().to_path_buf();
        let mut count = 0;
        for line in written.lines() {
            let Some((_, rest)) = line.split_once("](") else {
                continue;
            };
            let link = rest.split_once(')').unwrap().0;
            assert!(link.starts_with("../"), "{name}: {line}");
            assert!(dir.join(link).is_file(), "{name}: {link} names no file");
            count += 1;
        }
        assert_eq!(
            count,
            json["counts"]["open"].as_u64().unwrap() as usize,
            "{name}: one link per open item"
        );
    }
}

// -------------------------------------------------------------------- Q-B

/// With a view, a bad `[roadmap]` fails `check` (exit 2) and `export`
/// (exit 2, nothing written) at the table's own line; without the key the
/// same table fails `spec roadmap` alone.
#[test]
fn bad_roadmap_with_a_view_fails_check_and_export_at_its_line() {
    for (name, view) in CASES {
        let scratch = Scratch::new(&format!("view-bad-{name}"));
        let home = scratch.home("h");
        let root = viewed_copy(&scratch, name, "copy", view);
        make_dirs(&root, view);
        let config = read_text(&root, "specengine.toml");
        let releases = config
            .lines()
            .position(|line| line.starts_with("releases"))
            .expect("a releases line");
        let line = releases + 1;
        let bad: String = config
            .lines()
            .enumerate()
            .map(|(at, text)| {
                if at == releases {
                    "releases   = []\n".to_owned()
                } else {
                    format!("{text}\n")
                }
            })
            .collect();
        write(&root, "specengine.toml", &bad);
        let at = format!("specengine.toml:{line}: ");

        let check = spec(&home, &root, &["check"]);
        assert_eq!(check.code, 2, "{name}: {}", check.show());
        assert!(
            check.stdout.contains(&at) || check.stderr.contains(&at),
            "{name}: {at:?}\n{}",
            check.show()
        );
        let before = snapshot(&root);
        let export = spec(&home, &root, &["export", "index"]);
        assert_eq!(export.code, 2, "{name}: {}", export.show());
        assert_eq!(export.stdout, "", "{name}");
        assert!(export.stderr.contains(&at), "{name}: {}", export.stderr);
        assert_eq!(snapshot(&root), before, "{name}: export wrote");
        assert_eq!(spec(&home, &root, &["roadmap"]).code, 2, "{name}");
        // Reads do not load the check tables.
        spec(&home, &root, &["tree"]).code(0);

        // Without the key (and its `writes` entry), the same table fails
        // `spec roadmap` alone.
        let plain = bad
            .replace(&format!("roadmap = {}\n", quoted(view)), "")
            .replace(&format!(", {}]", quoted(view)), "]");
        assert!(!plain.contains(view), "{plain}");
        write(&root, "specengine.toml", &plain);
        let run = spec(&home, &root, &["export", "index"]);
        assert_eq!(run.code, 0, "{name}: {}", run.show());
        let check = spec(&home, &root, &["check"]);
        assert!(check.code < 2, "{name}: {}", check.show());
        assert_eq!(spec(&home, &root, &["roadmap"]).code, 2, "{name}");
    }
}
