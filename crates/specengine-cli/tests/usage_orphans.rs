//! docs/features/usage-orphans.md (RM-0111) "Acceptance criteria", the CLI
//! half: the `spec` binary over scratch copies of `fixtures/spec-a` ("the
//! copy") and `fixtures/spec-b` (both only read), each under its own
//! `HOME`, with invented files and cards written here.
//!
//! - UO1: `show REF --links --json` serves the spec's table, node by node;
//!   `catalog --kind mechanic` MEC-SPRINT `{1, 1, false}` and MEC-STAMINA
//!   `{2, 6, false}`, `--kind rule` RULE-STAM-REGEN `{2, 3, null}`; core
//!   `usage_of` over the copy's whole graph equals `show`'s usage (and the
//!   catalog's) for every node of `spec-a` and `spec-b`, cold and warm.
//! - UO2: DEC-0023 keeps `used_by` 1 under `--archive` while its In list
//!   gains DEC-0007's rows; a `class: generated` file citing A-102 leaves it
//!   at 0 and an orphan. M: Tier 3 rows counted under `--archive`; a
//!   generated source counted.
//! - UO3: the four files: MEC-LANTERN an orphan, DOM-LONE not (a child),
//!   MEC-LONE-CHILD an orphan, MEC-OLD not (`abandoned`); A-102 an orphan,
//!   Q-032 not. M: one's own `parent:` a link; children ignored;
//!   `abandoned` ignored; links out ignored.
//! - UO4: `health --json`'s `orphans` `{live: 12, rows: [A-102's], notes:
//!   []}` and Data's three lines; a card on A-102 keeps the row, `tasks 1,
//!   open 1`, a cancelled one counts for nothing; no queue yet, and a queue
//!   at a newer schema, read `tasks: null` with the note, exit 0; each note
//!   also a stderr `note:` line (iteration 2), a card whose `targets` cannot
//!   be read one more, its row still counted from the cards that read. M: a
//!   document with an open card left out; Tier 3 documents judged (`live`
//!   13); a queue error failing the read.
//! - UO5: Data's two catalog text rows, a section row with no `orphan`
//!   word, the JSON's 16 keys. M: `orphan` set on a section.
//! - UO6, the CLI half: MEC-STAMINA's block ends `links 5 out, 10 in; used
//!   by 2, mentioned by 6`, `--archive` keeps the counts, plain `show`
//!   serves `links: null`. M: the counts tallied over admitted rows; the
//!   summary line without the counts.
//! - UO7: warm `catalog`, `health` and `show --links` reads leave the index
//!   database's `sqlite_master` and every table's row count as they were;
//!   the queue schema, the backup format and `INDEX_FORMAT` unchanged, no
//!   check code of uses. M: a cache table of counts.
//! - UO8: the copy at two roots and two `HOME`s, its files written in
//!   opposite orders, gives byte-identical `catalog`, `health` and `show
//!   --links` text and JSON. M: rows taken in input order.
//!
//! Every expected value is worked out from the spec and the files here,
//! never read back from a run of the read under test.

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::{Run, Scratch, data_dir, fixture, read, spec, write};
use serde_json::{Value, json};
use specengine_core::ProjectConfig;
use specengine_core::check::{CHECK_CODES, NodeAt, SpecGraph, Standing};
use specengine_core::usage::usage_of;
use specengine_store::{WorkingTree, check_input};

/// `(used_by, mentioned_by, orphan)` of one node.
type Counts = (u32, u32, Option<bool>);

/// The spec's table on `fixtures/spec-a` ("Data", "On `fixtures/spec-a`"):
/// each REF with its counts.
const SPEC_A: [(&str, Counts); 13] = [
    ("MEC-STAMINA", (2, 6, Some(false))),
    ("RULE-STAM-REGEN", (2, 3, None)),
    ("MEC-SPRINT", (1, 1, Some(false))),
    ("DOM-MOVEMENT", (0, 1, Some(false))),
    ("DOM-GAME", (0, 0, Some(false))),
    ("A-101", (2, 3, Some(false))),
    ("A-102", (0, 0, Some(true))),
    ("DEC-0023", (1, 1, Some(false))),
    ("Q-031", (1, 3, Some(false))),
    ("Q-032", (0, 0, Some(false))),
    ("R-12", (1, 6, Some(false))),
    ("TERM-exhausted", (1, 1, Some(false))),
    ("docs/features/stamina-tuning.md", (0, 1, Some(false))),
];

const A102_TITLE: &str = "The lantern burns one unit of oil per minute";
/// Row 8's note with no queue yet, in JSON and on stderr.
const NO_QUEUE: &str = "orphans: no queue yet: no task read";
const A102_PATH: &str = "docs/records/A/A-102.md";

/// The copy's slug: the index's and the queue's file name.
const SLUG: &str = "lantern-keep";

/// One scratch copy and its `HOME`.
struct Copy {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
}

impl Copy {
    /// A copy of `fixtures/<name>` with `files` written in.
    fn new(label: &str, name: &str, files: &[(&str, &str)]) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.copy(name, "copy");
        for (path, text) in files {
            write(&root, path, text);
        }
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            root,
            home,
        }
    }

    fn run(&self, args: &[&str]) -> Run {
        spec(&self.home, &self.root, args)
    }

    /// `spec args… --json`, exit 0: the document.
    fn json(&self, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let run = self.run(&all);
        run.code(0);
        run.json()
    }

    /// The index and queue database under this copy's `HOME`.
    fn db(&self) -> PathBuf {
        data_dir(&self.home).join(format!("{SLUG}.db"))
    }
}

/// `sqlite3 db sql`, which must answer: its stdout, trimmed.
fn sql_at(db: &Path, sql: &str) -> String {
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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// `{used_by, mentioned_by}` as served.
fn usage_json(used_by: u32, mentioned_by: u32) -> Value {
    json!({"used_by": used_by, "mentioned_by": mentioned_by})
}

/// The one node of `show REF --links --json` (more holders: the first).
fn shown_links(copy: &Copy, reference: &str, archive: bool) -> Value {
    let mut args = vec!["show", reference, "--links"];
    if archive {
        args.push("--archive");
    }
    let document = copy.json(&args);
    document["nodes"][0]["links"].clone()
}

/// The last line of the links block of `show REF --links` text.
fn summary(stdout: &str) -> &str {
    stdout
        .lines()
        .find(|line| line.starts_with("  links "))
        .unwrap_or_else(|| panic!("no links summary:\n{stdout}"))
}

/// The catalog row of `id`.
fn catalog_row<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("{id}: {document}"))
}

/// `(used_by, mentioned_by, orphan)` of a catalog row.
fn row_counts(row: &Value) -> Counts {
    let count = |key: &str| {
        u32::try_from(row[key].as_u64().unwrap_or_else(|| panic!("{key}: {row}"))).expect("u32")
    };
    (
        count("used_by"),
        count("mentioned_by"),
        row["orphan"].as_bool(),
    )
}

// ------------------------------------------------------------------ UO1

/// UO1, UO6: `show REF --links --json` serves `links.usage` as the spec's
/// table, for every node in it, after `incoming` and before `left_out`;
/// the text summary carries the same counts. M: rows counted, not sources
/// (MEC-STAMINA and R-12 `mentioned_by` 7); same-file sources counted
/// (`stamina-tuning.md` 2); a document without its sections' uses
/// (MEC-STAMINA `used_by` 1); the summary line without the counts.
#[test]
fn uo1_show_links_serves_the_specs_table() {
    let copy = Copy::new("uo1-show", "spec-a", &[]);
    for (reference, (used_by, mentioned_by, _)) in SPEC_A {
        let run = copy.run(&["show", reference, "--links", "--json"]);
        run.code(0);
        let document = run.json();
        let links = &document["nodes"][0]["links"];
        assert_eq!(
            links["usage"],
            usage_json(used_by, mentioned_by),
            "{reference}"
        );
        let raw = format!(
            "],\"usage\":{{\"used_by\":{used_by},\"mentioned_by\":{mentioned_by}}},\"left_out\":{{"
        );
        assert!(
            run.stdout.contains(&raw),
            "{reference}: `usage` between `incoming` and `left_out`: {}",
            run.stdout
        );
        let mut keys: Vec<&str> = links
            .as_object()
            .expect("links")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["incoming", "left_out", "omitted", "outgoing", "usage"],
            "{reference}"
        );

        let text = copy.run(&["show", reference, "--links"]);
        text.code(0);
        let outgoing = links["outgoing"].as_array().expect("outgoing").len();
        let incoming = links["incoming"].as_array().expect("incoming").len();
        let line = summary(&text.stdout);
        let head = format!(
            "  links {outgoing} out, {incoming} in; used by {used_by}, mentioned by {mentioned_by}"
        );
        assert!(
            line == head || line.starts_with(&format!("{head}; left out: ")),
            "{reference}: {line:?}"
        );
    }
}

/// UO6, the CLI half: MEC-STAMINA's block ends Data's line; plain `show`
/// serves `links: null`. M: the summary line without the counts.
#[test]
fn uo6_mec_stamina_block_ends_with_the_counts() {
    let copy = Copy::new("uo6-block", "spec-a", &[]);
    let run = copy.run(&["show", "MEC-STAMINA", "--links"]);
    run.code(0);
    assert_eq!(
        summary(&run.stdout),
        "  links 5 out, 10 in; used by 2, mentioned by 6",
        "{}",
        run.show()
    );
    let plain = copy.json(&["show", "MEC-STAMINA"]);
    assert_eq!(plain["nodes"][0]["links"], Value::Null, "{plain}");
}

/// UO1: the catalog's counts: MEC-SPRINT `{1, 1, false}`, MEC-STAMINA
/// `{2, 6, false}`, RULE-STAM-REGEN `{2, 3, null}`.
#[test]
fn uo1_the_catalog_serves_the_counts() {
    let copy = Copy::new("uo1-catalog", "spec-a", &[]);
    let mechanics = copy.json(&["catalog", "--kind", "mechanic"]);
    assert_eq!(
        row_counts(catalog_row(&mechanics, "MEC-SPRINT")),
        (1, 1, Some(false))
    );
    assert_eq!(
        row_counts(catalog_row(&mechanics, "MEC-STAMINA")),
        (2, 6, Some(false))
    );
    let rules = copy.json(&["catalog", "--kind", "rule"]);
    let regen = catalog_row(&rules, "RULE-STAM-REGEN");
    assert_eq!(row_counts(regen), (2, 3, None));
    assert!(regen["orphan"].is_null(), "{regen}");
}

/// The copy's whole graph through core: every node's `(path, line)` with
/// its name, standing and usage.
fn core_usage(root: &Path) -> Vec<(String, usize, String, Standing, Counts)> {
    let text = String::from_utf8(read(root, "specengine.toml")).expect("UTF-8 config");
    let config = ProjectConfig::from_toml(&text).expect("the config");
    let tree = WorkingTree::new(root, &config.paths).expect("the working tree");
    let input = check_input(&tree, &config.scheme);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let usage = usage_of(&graph);
    let mut out = Vec::new();
    for file in 0..graph.paths().len() {
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            let node = usage
                .get(&at)
                .unwrap_or_else(|| panic!("{}: no usage", graph.name(at)));
            out.push((
                graph.paths()[file].to_owned(),
                graph.line(at),
                graph.name(at),
                graph.standing(file),
                (node.used_by, node.mentioned_by, node.orphan),
            ));
        }
    }
    out
}

/// The node of a `show` document at `path:line`.
fn node_at<'a>(document: &'a Value, path: &str, line: usize) -> &'a Value {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["path"] == path && node["line"] == line)
        .unwrap_or_else(|| panic!("{path}:{line}: {document}"))
}

/// UO1: core `usage_of` over the whole graph equals `show`'s usage for
/// every node (a node of a file that is not live through `--archive`), the
/// index cold and then warm, and the catalog's counts and `orphan` for
/// every row of every kind, `--archive` too.
fn assert_core_is_served(name: &str) {
    let copy = Copy::new(&format!("uo1-core-{name}"), name, &[]);
    let nodes = core_usage(&copy.root);
    assert!(nodes.len() > 10, "{name}: {}", nodes.len());
    for pass in ["cold", "warm"] {
        for (path, line, reference, standing, (used_by, mentioned_by, _)) in &nodes {
            let archive = *standing != Standing::Live;
            let mut args = vec!["show", reference.as_str(), "--links"];
            if archive {
                args.push("--archive");
            }
            let document = copy.json(&args);
            let node = node_at(&document, path, *line);
            assert_eq!(
                node["links"]["usage"],
                usage_json(*used_by, *mentioned_by),
                "{name} {pass}: {reference} at {path}:{line}"
            );
        }
    }

    let text = String::from_utf8(read(&copy.root, "specengine.toml")).expect("UTF-8 config");
    let config = ProjectConfig::from_toml(&text).expect("the config");
    let mut kinds: Vec<String> = config
        .scheme
        .prefixes()
        .iter()
        .map(|prefix| prefix.kind.clone())
        .collect();
    kinds.sort();
    kinds.dedup();
    let by_place: BTreeMap<(String, usize), Counts> = nodes
        .iter()
        .map(|(path, line, _, _, counts)| ((path.clone(), *line), *counts))
        .collect();
    for archive in [false, true] {
        let mut args = vec!["catalog"];
        for kind in &kinds {
            args.push("--kind");
            args.push(kind);
        }
        if archive {
            args.push("--archive");
        }
        let document = copy.json(&args);
        let rows = document["rows"].as_array().expect("rows");
        assert!(!rows.is_empty(), "{name}: {document}");
        for row in rows {
            let place = (
                row["path"].as_str().expect("path").to_owned(),
                usize::try_from(row["line"].as_u64().expect("line")).expect("usize"),
            );
            let want = by_place
                .get(&place)
                .unwrap_or_else(|| panic!("{name}: {place:?} not in the graph"));
            assert_eq!(row_counts(row), *want, "{name} archive {archive}: {row}");
        }
    }
}

#[test]
fn uo1_core_usage_of_is_served_on_spec_a() {
    assert_core_is_served("spec-a");
}

#[test]
fn uo1_core_usage_of_is_served_on_spec_b() {
    assert_core_is_served("spec-b");
}

// ------------------------------------------------------------------ UO2

/// A generated file citing A-102 strong and weak.
const GENERATED: (&str, &str) = (
    "docs/spec/oil-map.md",
    "---\nclass: generated\nlinks:\n  depends_on: [A-102]\n---\n\n# Oil map\n\nGenerated from A-102.\n",
);

/// UO2: DEC-0023 keeps `{1, 1}` with `--archive` while its In list gains
/// DEC-0007's row; a generated file citing A-102 leaves it at 0 and
/// an orphan in `show`, the catalog and Health. M: Tier 3 rows counted
/// under `--archive`; a generated source counted.
#[test]
fn uo2_live_sources_only() {
    let copy = Copy::new("uo2", "spec-a", &[GENERATED]);
    let live = shown_links(&copy, "DEC-0023", false);
    let archived = shown_links(&copy, "DEC-0023", true);
    assert_eq!(live["usage"], usage_json(1, 1), "{live}");
    assert_eq!(archived["usage"], usage_json(1, 1), "{archived}");
    let from_archive = |links: &Value| {
        links["incoming"]
            .as_array()
            .expect("incoming")
            .iter()
            .filter(|link| link["path"] == "docs/records/DEC/DEC-0007.md")
            .count()
    };
    // DEC-0007's mention lands in In; its `superseded-by` is an `out
    // supersedes` on DEC-0023 (both counted in `left_out` without
    // `--archive`).
    assert_eq!(from_archive(&live), 0, "{live}");
    assert_eq!(from_archive(&archived), 1, "{archived}");
    assert_eq!(live["left_out"]["tier3"], 2, "{live}");

    let a102 = shown_links(&copy, "A-102", false);
    assert_eq!(a102["usage"], usage_json(0, 0), "{a102}");
    assert_eq!(
        a102["left_out"]["generated"], 2,
        "premise: the generated file's two links resolve: {a102}"
    );
    let a102 = shown_links(&copy, "A-102", true);
    assert_eq!(a102["usage"], usage_json(0, 0), "{a102}");

    let health = copy.json(&["health"]);
    let rows = health["orphans"]["rows"].as_array().expect("rows");
    assert!(
        rows.iter().any(|row| row["id"] == "A-102"),
        "A-102 is an orphan: {health}"
    );
    assert!(
        rows.iter().all(|row| row["path"] != GENERATED.0),
        "a generated file is never judged: {health}"
    );
    assert_eq!(health["orphans"]["live"], 12, "{health}");

    // Through the catalog: the records in the tree.
    let copy = Copy::new("uo2-catalog", "spec-a", &[GENERATED]);
    wide(&copy.root);
    let requirements = copy.json(&["catalog", "--kind", "requirement"]);
    assert_eq!(
        row_counts(catalog_row(&requirements, "A-102")),
        (0, 0, Some(true))
    );
}

// ------------------------------------------------------------------ UO3

/// UO3's four files.
const FOUR: [(&str, &str); 4] = [
    (
        "docs/spec/movement/lantern.md",
        "---\nid: MEC-LANTERN\nclass: canon\ntitle: Lantern\nparent: DOM-MOVEMENT\n---\n\n# Lantern\n\nThe lantern lights the walls.\n",
    ),
    (
        "docs/spec/lone.md",
        "---\nid: DOM-LONE\nclass: canon\ntitle: Lone\n---\n\n# Lone\n\nA domain on its own.\n",
    ),
    (
        "docs/spec/lone-child.md",
        "---\nid: MEC-LONE-CHILD\nclass: canon\ntitle: Lone child\nparent: DOM-LONE\n---\n\n# Lone child\n\nUnder the lone domain.\n",
    ),
    (
        "docs/spec/old.md",
        "---\nid: MEC-OLD\nclass: canon\ntitle: Old\nstatus: abandoned\n---\n\n# Old\n\nSet aside.\n",
    ),
];

/// The copy's `specengine.toml` with `[paths] spec = "docs"` appended:
/// every document in the tree, so the catalog lists the records.
fn wide(root: &Path) {
    let mut text = String::from_utf8(read(root, "specengine.toml")).expect("UTF-8");
    text.push_str("\n[paths]\nspec = \"docs\"\n");
    write(root, "specengine.toml", text);
}

/// UO3: the four files, A-102 and Q-032 judged by the catalog and Health.
/// M: one's own `parent:` taken as a link (MEC-LANTERN, MEC-LONE-CHILD not
/// orphans); children ignored (DOM-LONE an orphan); `abandoned` ignored
/// (MEC-OLD an orphan); links out ignored (Q-032 an orphan).
#[test]
fn uo3_the_four_files() {
    let copy = Copy::new("uo3", "spec-a", &FOUR);
    let mechanics = copy.json(&["catalog", "--kind", "mechanic", "--kind", "domain"]);
    for (id, orphan) in [
        ("MEC-LANTERN", true),
        ("DOM-LONE", false),
        ("MEC-LONE-CHILD", true),
        ("MEC-OLD", false),
        ("DOM-MOVEMENT", false),
    ] {
        let row = catalog_row(&mechanics, id);
        assert_eq!(row["orphan"], json!(orphan), "{id}: {row}");
    }

    let health = copy.json(&["health"]);
    let orphans = &health["orphans"];
    assert_eq!(orphans["live"], 16, "{orphans}");
    let ids: Vec<&str> = orphans["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID"))
        .collect();
    // `mechanic` before `requirement`, then by path.
    assert_eq!(ids, ["MEC-LONE-CHILD", "MEC-LANTERN", "A-102"], "{orphans}");

    // A-102 and Q-032 through the catalog: the records in the tree.
    let wide_copy = Copy::new("uo3-wide", "spec-a", &FOUR);
    wide(&wide_copy.root);
    let records = wide_copy.json(&["catalog", "--kind", "requirement", "--kind", "question"]);
    assert_eq!(catalog_row(&records, "A-102")["orphan"], json!(true));
    assert_eq!(catalog_row(&records, "Q-032")["orphan"], json!(false));
    assert_eq!(
        row_counts(catalog_row(&records, "Q-032")),
        (0, 0, Some(false))
    );
}

// ------------------------------------------------------------------ UO4

/// A-102's Health row, its tasks as given.
fn a102_row(tasks: Value, open: Value) -> Value {
    json!({
        "name": "A-102",
        "id": "A-102",
        "kind": "requirement",
        "title": A102_TITLE,
        "path": A102_PATH,
        "line": 1,
        "status": "open",
        "tasks": tasks,
        "tasks_open": open,
    })
}

/// The last three lines of `health`'s text.
fn last_three(stdout: &str) -> Vec<&str> {
    let lines: Vec<&str> = stdout.lines().collect();
    lines[lines.len().saturating_sub(3)..].to_vec()
}

/// A card `title` on `target` through the CLI: its ID.
fn card(copy: &Copy, title: &str, target: &str) -> String {
    let run = copy.run(&[
        "card", "new", "--title", title, "--target", target, "--json",
    ]);
    run.code(0);
    let document = run.json();
    document["id"]
        .as_str()
        .unwrap_or_else(|| panic!("a card ID: {document}"))
        .to_owned()
}

/// UO4: Health's row 8 on the copy, through the queue's states. M: a
/// document with an open card left out; Tier 3 documents judged (`live`
/// 13); a queue error failing the read.
#[test]
fn uo4_healths_orphans_row() {
    let copy = Copy::new("uo4", "spec-a", &[]);

    // No queue yet: the rows read `tasks: null`, with the note.
    let run = copy.run(&["health", "--json"]);
    run.code(0);
    let document = run.json();
    assert_eq!(
        document["orphans"],
        json!({
            "live": 12,
            "rows": [a102_row(Value::Null, Value::Null)],
            "notes": ["orphans: no queue yet: no task read"],
        }),
        "{}",
        run.show()
    );
    let raw = format!(
        "\"orphans\":{{\"live\":12,\"rows\":[{{\"name\":\"A-102\",\"id\":\"A-102\",\"kind\":\
         \"requirement\",\"title\":\"{A102_TITLE}\",\"path\":\"{A102_PATH}\",\"line\":1,\
         \"status\":\"open\",\"tasks\":null,\"tasks_open\":null}}],\"notes\":["
    );
    assert!(run.stdout.contains(&raw), "the keys' order: {}", run.stdout);
    assert_eq!(run.stderr, format!("note: {NO_QUEUE}\n"), "{}", run.show());
    let text = copy.run(&["health"]);
    text.code(0);
    assert!(
        text.stdout.contains(&format!(
            "orphans: 1 of 12 live documents link to nothing\n  A-102 | requirement | \
             {A102_TITLE} | {A102_PATH}:1\n"
        )),
        "{}",
        text.show()
    );
    assert_eq!(
        text.stderr,
        format!("note: {NO_QUEUE}\n"),
        "{}",
        text.show()
    );
    assert!(
        !copy.db().exists()
            || sql_at(
                &copy.db(),
                "SELECT count(*) FROM sqlite_master WHERE name IN ('proposals', 'cards');"
            ) == "0",
        "Health makes no queue"
    );

    // A queue, no card on A-102: notes [], tasks 0; Data's three lines,
    // nothing on stderr.
    card(&copy, "Tune the stamina", "MEC-STAMINA");
    let run = copy.run(&["health", "--json"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    let document = run.json();
    assert_eq!(
        document["orphans"],
        json!({"live": 12, "rows": [a102_row(json!(0), json!(0))], "notes": []}),
        "{document}"
    );
    let text = copy.run(&["health"]);
    text.code(0);
    assert_eq!(
        last_three(&text.stdout),
        [
            "orphans: 1 of 12 live documents link to nothing",
            &format!("  A-102 | requirement | {A102_TITLE} | {A102_PATH}:1"),
            "spec health: 13 documents, 0 unapplied amendments",
        ],
        "{}",
        text.show()
    );
    assert_eq!(text.stderr, "", "{}", text.show());

    // A card on A-102: the row stays, `tasks 1, open 1`.
    let planned = card(&copy, "Bring the lantern in", "A-102");
    let document = copy.json(&["health"]);
    assert_eq!(
        document["orphans"],
        json!({"live": 12, "rows": [a102_row(json!(1), json!(1))], "notes": []}),
        "{document}"
    );
    let text = copy.run(&["health"]);
    text.code(0);
    assert_eq!(
        last_three(&text.stdout),
        [
            "orphans: 1 of 12 live documents link to nothing",
            &format!("  A-102 | requirement | {A102_TITLE} | {A102_PATH}:1 | tasks 1, open 1"),
            "spec health: 13 documents, 0 unapplied amendments",
        ],
        "{}",
        text.show()
    );

    // Cancelled: counted nowhere.
    let cancel = copy.run(&["card", "cancel", &planned, "--reason", "Not this release"]);
    cancel.code(0);
    let document = copy.json(&["health"]);
    assert_eq!(
        document["orphans"]["rows"],
        json!([a102_row(json!(0), json!(0))]),
        "{document}"
    );

    // A queue at a newer schema: `tasks: null`, the note in JSON and on
    // stderr, exit 0.
    sql_at(&copy.db(), "PRAGMA user_version = 999;");
    let newer = "orphans: tasks not read: the proposal queue's tables are of a newer SpecEngine \
                 (schema version 999, this build knows 14): upgrade SpecEngine (nothing changed)";
    let run = copy.run(&["health", "--json"]);
    run.code(0);
    let document = run.json();
    assert_eq!(
        document["orphans"],
        json!({"live": 12, "rows": [a102_row(Value::Null, Value::Null)], "notes": [newer]}),
        "{}",
        run.show()
    );
    assert_eq!(run.stderr, format!("note: {newer}\n"), "{}", run.show());
    let text = copy.run(&["health"]);
    text.code(0);
    assert_eq!(text.stderr, format!("note: {newer}\n"), "{}", text.show());
    assert_eq!(
        last_three(&text.stdout),
        [
            "orphans: 1 of 12 live documents link to nothing",
            &format!("  A-102 | requirement | {A102_TITLE} | {A102_PATH}:1"),
            "spec health: 13 documents, 0 unapplied amendments",
        ],
        "{}",
        text.show()
    );
}

/// UO4 (iteration 2, the card read's notes): a card whose stored `targets`
/// cannot be read is a note, prefixed `orphans: `, in JSON and on stderr,
/// exit 0; the card that reads still counts (A-102 `tasks 1, open 1`). The
/// card read is the store's, its column `targets` written by hand here.
#[test]
fn uo4_a_card_that_cannot_be_read_is_a_note() {
    let copy = Copy::new("uo4-card-note", "spec-a", &[]);
    let planned = card(&copy, "Bring the lantern in", "A-102");
    assert_eq!(planned, "C-0001");
    let broken = card(&copy, "Tune the stamina", "MEC-STAMINA");
    assert_eq!(broken, "C-0002");
    sql_at(
        &copy.db(),
        "UPDATE cards SET targets = 'nope' WHERE id = 'C-0002';",
    );
    let note = "orphans: card C-0002: the stored `targets` cannot be read: expected ident at \
                line 1 column 2; not read";
    let run = copy.run(&["health", "--json"]);
    run.code(0);
    let document = run.json();
    assert_eq!(
        document["orphans"],
        json!({"live": 12, "rows": [a102_row(json!(1), json!(1))], "notes": [note]}),
        "{}",
        run.show()
    );
    assert_eq!(run.stderr, format!("note: {note}\n"), "{}", run.show());
    let text = copy.run(&["health"]);
    text.code(0);
    assert_eq!(text.stderr, format!("note: {note}\n"), "{}", text.show());
    assert_eq!(
        last_three(&text.stdout),
        [
            "orphans: 1 of 12 live documents link to nothing",
            &format!("  A-102 | requirement | {A102_TITLE} | {A102_PATH}:1 | tasks 1, open 1"),
            "spec health: 13 documents, 0 unapplied amendments",
        ],
        "{}",
        text.show()
    );
}

// ------------------------------------------------------------------ UO5

/// The 16 keys of a catalog row, as the CLI's `tests/catalog.rs` lists the
/// 13 before this slice, the three new ones last of the counts.
const ROW_KEYS: [&str; 16] = [
    "id",
    "kind",
    "title",
    "path",
    "line",
    "rev",
    "status",
    "archived",
    "state",
    "open_items",
    "tasks",
    "tasks_open",
    "used_by",
    "mentioned_by",
    "orphan",
    "last_change",
];

/// UO5: Data's two text rows (A-102's with the records in the tree), a
/// section row with no `orphan` word, the JSON's 16 keys. M: `orphan` set
/// on a section.
#[test]
fn uo5_the_catalog_row() {
    let copy = Copy::new("uo5", "spec-a", &[]);
    let run = copy.run(&["catalog", "--kind", "mechanic"]);
    run.code(0);
    assert!(
        run.stdout.lines().any(|line| line
            == "MEC-STAMINA | version 1 | no card | open items 0 | tasks 0, open 0 | used by 2, \
                mentioned by 6 | - | - | - | Stamina"),
        "{}",
        run.show()
    );
    let rules = copy.run(&["catalog", "--kind", "rule"]);
    rules.code(0);
    assert!(
        rules.stdout.lines().any(|line| line
            == "RULE-STAM-REGEN | version 1 | no card | open items 0 | tasks 0, open 0 | used by \
                2, mentioned by 3 | - | - | - | Regeneration"),
        "{}",
        rules.show()
    );
    assert!(!rules.stdout.contains("orphan"), "{}", rules.show());

    let run = copy.run(&["catalog", "--kind", "mechanic", "--kind", "rule", "--json"]);
    run.code(0);
    let document = run.json();
    for row in document["rows"].as_array().expect("rows") {
        let mut keys: Vec<&str> = row
            .as_object()
            .expect("a row")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        let mut want = ROW_KEYS.to_vec();
        want.sort_unstable();
        assert_eq!(keys, want, "{row}");
    }
    // The order as written: the counts after `tasks_open`, before
    // `last_change`.
    assert!(
        run.stdout.contains(
            "\"tasks_open\":0,\"used_by\":2,\"mentioned_by\":6,\"orphan\":false,\"last_change\":"
        ),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains(
            "\"tasks_open\":0,\"used_by\":2,\"mentioned_by\":3,\"orphan\":null,\"last_change\":"
        ),
        "{}",
        run.stdout
    );

    let wide_copy = Copy::new("uo5-wide", "spec-a", &[]);
    wide(&wide_copy.root);
    let run = wide_copy.run(&["catalog", "--kind", "requirement"]);
    run.code(0);
    assert!(
        run.stdout.lines().any(|line| line
            == format!(
                "A-102 | version 1 | no card | open items 0 | tasks 0, open 0 | used by 0, \
                 mentioned by 0, orphan | - | - | - | {A102_TITLE}"
            )),
        "{}",
        run.show()
    );
}

// ------------------------------------------------------------------ UO6

/// UO6: `--archive` widens In and never the counts, on every node of the
/// table. M: the counts tallied over admitted rows.
#[test]
fn uo6_archive_widens_in_never_the_counts() {
    let copy = Copy::new("uo6-archive", "spec-a", &[]);
    let mut widened = 0;
    for (reference, (used_by, mentioned_by, _)) in SPEC_A {
        let live = shown_links(&copy, reference, false);
        let archived = shown_links(&copy, reference, true);
        assert_eq!(
            live["usage"],
            usage_json(used_by, mentioned_by),
            "{reference}"
        );
        assert_eq!(archived["usage"], live["usage"], "{reference}");
        let count = |links: &Value| links["incoming"].as_array().expect("incoming").len();
        if count(&archived) > count(&live) {
            widened += 1;
        }
    }
    assert!(widened >= 1, "premise: `--archive` widens some In list");
    let text = copy.run(&["show", "DEC-0023", "--links", "--archive"]);
    text.code(0);
    assert_eq!(
        summary(&text.stdout),
        "  links 5 out, 3 in; used by 1, mentioned by 1",
        "{}",
        text.show()
    );
    let text = copy.run(&["show", "DEC-0023", "--links"]);
    text.code(0);
    assert_eq!(
        summary(&text.stdout),
        "  links 4 out, 2 in; used by 1, mentioned by 1; left out: 2 archived (--archive)",
        "{}",
        text.show()
    );
}

// ------------------------------------------------------------------ UO7

/// `sqlite_master` and every table's row count.
fn database_state(db: &Path) -> (String, Vec<(String, String)>) {
    let master = sql_at(
        db,
        "SELECT type || '|' || name || '|' || tbl_name || '|' || ifnull(sql, '') FROM \
         sqlite_master ORDER BY type, name;",
    );
    let tables = sql_at(
        db,
        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name;",
    );
    let counts = tables
        .lines()
        .map(|table| {
            (
                table.to_owned(),
                sql_at(db, &format!("SELECT count(*) FROM \"{table}\";")),
            )
        })
        .collect();
    (master, counts)
}

/// UO7: warm reads store nothing; no format moves. M: a cache table of
/// counts.
#[test]
fn uo7_nothing_is_stored() {
    let copy = Copy::new("uo7", "spec-a", &FOUR);
    card(&copy, "Bring the lantern in", "A-102");
    copy.run(&["index"]).code(0);
    let reads: [&[&str]; 6] = [
        &["catalog", "--kind", "mechanic", "--kind", "rule"],
        &["catalog", "--kind", "mechanic", "--json"],
        &["health"],
        &["health", "--json"],
        &["show", "MEC-STAMINA", "--links"],
        &["show", "A-102", "--links", "--json"],
    ];
    // Warm: one round first, then the state, then the reads again.
    for args in reads {
        copy.run(args).code(0);
    }
    let before = database_state(&copy.db());
    assert!(!before.1.is_empty(), "premise: tables");
    for args in reads {
        copy.run(args).code(0);
    }
    assert_eq!(database_state(&copy.db()), before);

    assert_eq!(specengine_store::INDEX_FORMAT, 17, "the index format");
    assert_eq!(
        specengine_store::QUEUE_SCHEMA_VERSION,
        14,
        "the queue schema"
    );
    assert_eq!(specengine_cli::STATE_FORMAT, 7, "the backup format");
    let words: Vec<&&str> = CHECK_CODES
        .iter()
        .filter(|code| code.contains("orphan") || code.contains("usage") || code.contains("used"))
        .collect();
    assert!(words.is_empty(), "an orphan is no check finding: {words:?}");
}

// ------------------------------------------------------------------ UO8

/// Every regular file under `from`, relative, sorted.
fn files_under(from: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("a readable directory") {
            let entry = entry.expect("an entry");
            let path = entry.path();
            if entry.file_type().expect("a type").is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).expect("under the root");
                out.push(relative.to_str().expect("UTF-8").to_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(from, from, &mut out);
    out.sort();
    out
}

/// The copy at `<scratch>/<dir>`: the fixture's files and FOUR, written in
/// the order `files` gives.
fn written(scratch: &Scratch, dir: &str, files: &[(String, Vec<u8>)]) -> PathBuf {
    let root = scratch.dir(dir);
    for (path, bytes) in files {
        write(&root, path, bytes);
    }
    root
}

/// UO8: two roots, two `HOME`s, files written in opposite orders: the
/// reads byte for byte alike. M: rows taken in input order.
#[test]
fn uo8_one_corpus_one_answer() {
    let scratch = Scratch::new("uo8");
    let source = fixture("spec-a");
    let mut files: Vec<(String, Vec<u8>)> = files_under(&source)
        .into_iter()
        .map(|path| {
            let bytes = read(&source, &path);
            (path, bytes)
        })
        .collect();
    files.extend(
        FOUR.iter()
            .map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec())),
    );
    files.sort();
    let first = written(&scratch, "one/deeper/copy", &files);
    files.reverse();
    let second = written(&scratch, "two", &files);
    let homes = [scratch.home("first"), scratch.home("second")];
    for (root, home) in [(&first, &homes[0]), (&second, &homes[1])] {
        let run = spec(
            home,
            root,
            &[
                "card",
                "new",
                "--title",
                "Bring the lantern in",
                "--target",
                "A-102",
            ],
        );
        run.code(0);
    }
    let reads: [&[&str]; 8] = [
        &[
            "catalog", "--kind", "mechanic", "--kind", "rule", "--kind", "domain",
        ],
        &[
            "catalog", "--kind", "mechanic", "--kind", "rule", "--kind", "domain", "--json",
        ],
        &["health"],
        &["health", "--json"],
        &["show", "MEC-STAMINA", "--links"],
        &["show", "MEC-STAMINA", "--links", "--json"],
        &["show", "DOM-MOVEMENT", "--links", "--archive"],
        &["show", "DOM-MOVEMENT", "--links", "--archive", "--json"],
    ];
    for args in reads {
        let one = spec(&homes[0], &first, args);
        let two = spec(&homes[1], &second, args);
        one.code(0);
        assert_eq!(
            (two.code, &two.stdout, &two.stderr),
            (one.code, &one.stdout, &one.stderr),
            "{args:?}"
        );
        assert!(!one.stdout.is_empty(), "{args:?}");
    }
}
