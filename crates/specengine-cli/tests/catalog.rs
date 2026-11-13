//! docs/features/direction-catalog.md (RM-0106) "Acceptance criteria", the
//! CLI half: the `spec` binary over the copy (`tests/catalog_scene`, a
//! scratch git copy of `fixtures/studio-lamp` with its commits and queue
//! built here) and over scratch repositories built at run time, every
//! commit with a named author and committer and their dates.
//!
//! - DC-1: the rows are `spec tree --kind K… [--archive]`'s nodes, in its
//!   order, sections included, `--archive` matching. M: rows taken from the
//!   whole tree.
//! - DC-2: `state` is `spec trace <ID> --json`'s first node's `rollup`,
//!   `tasks` its card count; an accept makes `in work` `confirmed` and
//!   `tasks_open` 0; a cancelled card is in neither. M: `state` always `no
//!   card`; cancelled cards counted.
//! - DC-3: open items of this repository, staged ones too, an item once per
//!   node it names, a document never counting its sections'. M: applied
//!   items counted; the repository filter dropped.
//! - DC-4, the CLI half: each document row's `last_change` is `spec show
//!   <ID> --history --json`'s newest commit entry; mtime changes nothing; a
//!   section row carries its file's, `of_file`; an uncommitted file reads
//!   `null`, an uncommitted edit `HEAD`'s; on a merged branch the merge is
//!   the change. M: the mtime used; the oldest commit taken; `--first-parent`
//!   dropped.
//! - DC-7: the pinned text and JSON byte for byte; no `--kind` exit 2 with
//!   nothing read; `--kind nothing` `rows 0`; no git on `PATH` every
//!   `last_change` `null` and the note; the queue, shallow and scan notes;
//!   the text cut. M: `by_name` printed for `by`.
//! - DC-8, the CLI half: one call over 40 rows in 12 files runs one
//!   `ls-tree -r` (the spec's `cat-file --batch-check` until iteration 2),
//!   one walking `log`, one `--no-walk` `log`
//!   and `rev-parse` alone besides (a `git` shim logging its arguments). M:
//!   a `file_log` per path.
//!
//! Every expected value is worked out from the spec and the commits made
//! here, never read back from a run of the read under test.

#![cfg(unix)]

mod catalog_scene;
mod common;

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use catalog_scene::{
    ANN, ARRIVAL, BOB, CLEO, Ident, Lamp, PINNED_TEXT, SHORE, STORM, apply_message, pinned_json,
};
use common::git;
use common::proposal::Pair;
use common::{Run, spec_with};
use serde_json::{Value, json};
use specengine_cli::{Exit, Globals, StageBody, StageRequest, stage};
use specengine_core::proposal::{ApplyTrailers, Author, AuthorType, CommitFacts};

/// The read's character ceiling (`docs/canon/spec-cli-read.md` "The cap").
const CAP: usize = 40_000;

/// `spec args` in `cwd` with the copy's variables and `extra` over them.
fn spec_in(copy: &Lamp, cwd: &Path, args: &[&str], extra: &[(&str, &OsStr)]) -> Run {
    let vars = copy.vars(extra);
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

/// `spec args` in the copy's root.
fn spec(copy: &Lamp, args: &[&str]) -> Run {
    spec_in(copy, &copy.root, args, &[])
}

/// `spec catalog args… --json` in the copy's root, exit 0: the document.
fn catalog_json(copy: &Lamp, args: &[&str]) -> Value {
    let mut all = vec!["catalog"];
    all.extend_from_slice(args);
    all.push("--json");
    let run = spec(copy, &all);
    run.code(0);
    run.json()
}

/// `spec tree args… --json` in the copy's root, exit 0: the document.
fn tree_json(copy: &Lamp, args: &[&str]) -> Value {
    let mut all = vec!["tree"];
    all.extend_from_slice(args);
    all.push("--json");
    let run = spec(copy, &all);
    run.code(0);
    run.json()
}

/// The `id`s of a document's `key` array, in order.
fn ids(document: &Value, key: &str) -> Vec<String> {
    document[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key}: {document}"))
        .iter()
        .map(|row| row["id"].as_str().expect("an id").to_owned())
        .collect()
}

/// The row of `id`.
fn row<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no row {id}: {document}"))
}

/// A row's open items.
fn open_items(document: &Value, id: &str) -> u64 {
    row(document, id)["open_items"]
        .as_u64()
        .unwrap_or_else(|| panic!("{id}: open_items: {document}"))
}

/// The 16 keys of a row, in order (docs/features/usage-orphans.md
/// "Acceptance criteria", UO5: `used_by`, `mentioned_by`, `orphan` after
/// `tasks_open`), and the 6 of its last change.
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
const CHANGE_KEYS: [&str; 6] = ["at", "by", "by_name", "commit", "proposal", "of_file"];

/// An object's keys as written (serde_json keeps the order here only when
/// the text is read raw): the keys of `text`'s first object at `from`.
fn assert_row_shape(document: &Value) {
    for row in document["rows"].as_array().expect("rows") {
        let mut keys: Vec<&str> = row
            .as_object()
            .expect("a row")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        let mut expected = ROW_KEYS.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected, "{row}");
        if let Some(change) = row["last_change"].as_object() {
            let mut keys: Vec<&str> = change.keys().map(String::as_str).collect();
            keys.sort_unstable();
            let mut expected = CHANGE_KEYS.to_vec();
            expected.sort_unstable();
            assert_eq!(keys, expected, "{row}");
        }
    }
}

/// Whether `text` is a full OID (40 or 64 lower-case hex digits).
fn is_oid(text: &str) -> bool {
    (text.len() == 40 || text.len() == 64)
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

// ------------------------------------------------------------------ DC-7

/// DC-7: on the copy, `spec catalog --kind scene` prints the spec's text
/// byte for byte and `--json` its document (the OIDs the two commits made
/// here, each 40 or 64 hex), no note, exit 0; the apply message of the copy
/// is what core writes for an apply. M: `by_name` printed for `by`.
#[test]
fn dc7_the_copy_prints_the_spec_byte_for_byte() {
    let pinned = Lamp::pinned("dc7-pinned");
    let copy = &pinned.copy;
    let author = Author {
        author_type: AuthorType::Agent,
        role: Some("spec-writer".to_owned()),
        model: Some("m".to_owned()),
        run: Some("r".to_owned()),
        login: None,
    };
    let decided = ANN.value();
    assert_eq!(
        apply_message("PR-0001", "Name the shingle wet.", ANN, &copy.first),
        specengine_core::proposal::commit_message_raising(
            &CommitFacts {
                id: "PR-0001",
                rationale: "Name the shingle wet.",
                decided_by: &decided,
                author: &author,
                base_commit: &copy.first,
            },
            None,
            &ApplyTrailers::default(),
        ),
        "the scene's apply message is an apply's"
    );
    assert!(is_oid(&pinned.applied) && is_oid(&pinned.hand));

    let text = spec(copy, &["catalog", "--kind", "scene"]);
    text.code(0);
    assert_eq!(text.stdout, PINNED_TEXT, "{}", text.show());
    assert_eq!(text.stderr, "", "no note: {}", text.show());

    let json = spec(copy, &["catalog", "--kind", "scene", "--json"]);
    json.code(0);
    assert_eq!(
        json.stdout,
        pinned_json(&pinned.applied, &pinned.hand),
        "{}",
        json.show()
    );
    assert_eq!(json.stderr, "", "no note: {}", json.show());
    let document = json.json();
    assert_row_shape(&document);
    // The global flag before the command reads the same.
    let before = spec(copy, &["--json", "catalog", "--kind", "scene"]);
    before.code(0);
    assert_eq!(before.stdout, json.stdout);
}

/// DC-7: no `--kind`: exit 2, the spec's message, nothing on stdout, no git
/// run and nothing written under a fresh `HOME`.
#[test]
fn dc7_no_kind_is_exit_2_and_reads_nothing() {
    let pinned = Lamp::pinned("dc7-no-kind");
    let copy = &pinned.copy;
    let (bin, args_log, _) = shim(copy, "shim");
    let path = path_with(copy, &bin);
    let fresh = copy.scratch().join("fresh-home");
    fs::create_dir_all(&fresh).expect("a fresh home");
    for args in [
        &["catalog"][..],
        &["catalog", "--json"],
        &["--json", "catalog"],
    ] {
        let _ = fs::remove_file(&args_log);
        let run = spec_in(
            copy,
            &copy.root,
            args,
            &[("PATH", path.as_os_str()), ("HOME", fresh.as_os_str())],
        );
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}: {}", run.show());
        assert!(
            run.stderr
                .lines()
                .any(|line| line == "spec catalog: give at least one `--kind`"),
            "{args:?}: the spec's message: {}",
            run.show()
        );
        assert_eq!(logged(&args_log), Vec::<String>::new(), "{args:?}: no git");
        assert_eq!(
            files_under(&fresh),
            Vec::<String>::new(),
            "{args:?}: nothing written under HOME"
        );
    }
}

/// DC-7: `--kind nothing`: `rows 0`, exit 0, text and JSON.
#[test]
fn dc7_a_kind_with_no_node_is_rows_0() {
    let pinned = Lamp::pinned("dc7-nothing");
    let copy = &pinned.copy;
    let text = spec(copy, &["catalog", "--kind", "nothing"]);
    text.code(0);
    assert_eq!(text.stdout, "catalog nothing | rows 0\n", "{}", text.show());
    let document = catalog_json(copy, &["--kind", "nothing"]);
    assert_eq!(
        document,
        json!({"kinds": ["nothing"], "archive": false, "notes": [], "rows": [],
               "truncated": false, "omitted": null})
    );
    // Two kinds join with ", " in the header, `[archive]` after it.
    let two = spec(
        copy,
        &[
            "catalog",
            "--kind",
            "nothing",
            "--kind",
            "scene",
            "--archive",
        ],
    );
    two.code(0);
    assert!(
        two.stdout
            .starts_with("catalog nothing, scene | rows 2 [archive]\n"),
        "{}",
        two.show()
    );
}

/// DC-7: git absent from `PATH`: exit 0, every `last_change` `null`, one
/// `catalog: last change not read: …` note on stderr and in `notes`; the
/// rest of each row as pinned; the text prints `- | - | -`.
#[test]
fn dc7_no_git_on_path_reads_no_last_change() {
    let pinned = Lamp::pinned("dc7-no-git");
    let copy = &pinned.copy;
    let empty = copy.scratch().join("no-git");
    fs::create_dir_all(&empty).expect("an empty directory");
    let extra: [(&str, &OsStr); 1] = [("PATH", empty.as_os_str())];
    let json = spec_in(
        copy,
        &copy.root,
        &["catalog", "--kind", "scene", "--json"],
        &extra,
    );
    json.code(0);
    let notes: Vec<&str> = json
        .stderr
        .lines()
        .filter_map(|line| line.strip_prefix("note: "))
        .collect();
    assert_eq!(notes.len(), 1, "{}", json.show());
    assert!(
        notes[0].starts_with("catalog: last change not read: "),
        "{}",
        json.show()
    );
    let document = json.json();
    assert_eq!(document["notes"], json!([notes[0]]));
    let mut expected: Value =
        serde_json::from_str(&pinned_json(&pinned.applied, &pinned.hand)).expect("JSON");
    expected["notes"] = json!([notes[0]]);
    for row in expected["rows"].as_array_mut().expect("rows") {
        row["last_change"] = Value::Null;
    }
    assert_eq!(document, expected, "only last_change and the note differ");
    let text = spec_in(copy, &copy.root, &["catalog", "--kind", "scene"], &extra);
    text.code(0);
    assert_eq!(
        text.stdout,
        "catalog scene | rows 2\nSCN-ARRIVAL | version 1 | in work | open items 1 | tasks 1, \
         open 1 | used by 1, mentioned by 0 | - | - | - | Arrival\nSCN-STORM | version 1 | no \
         card | open items 0 | tasks 0, open 0 | used by 0, mentioned by 0 | - | - | - | The \
         storm\n",
        "{}",
        text.show()
    );
}

/// DC-7 (the notes): no queue yet: the note, every count 0, exit 0, no
/// queue made.
#[test]
fn dc7_no_queue_yet_is_a_note() {
    let pinned = Lamp::committed("dc7-no-queue");
    let copy = &pinned.copy;
    let json = spec(copy, &["catalog", "--kind", "scene", "--json"]);
    json.code(0);
    let note = "catalog: no queue yet: no open item or task read";
    assert_eq!(json.stderr, format!("note: {note}\n"), "{}", json.show());
    let document = json.json();
    assert_eq!(document["notes"], json!([note]));
    for id in ["SCN-ARRIVAL", "SCN-STORM"] {
        let row = row(&document, id);
        assert_eq!(row["state"], "no card", "{row}");
        assert_eq!(
            [&row["open_items"], &row["tasks"], &row["tasks_open"]],
            [&json!(0), &json!(0), &json!(0)],
            "{row}"
        );
        assert!(row["last_change"].is_object(), "{row}");
    }
    // The index shares the file; the queue's tables are its own.
    assert_eq!(
        copy.sql("SELECT count(*) FROM sqlite_master WHERE name IN ('proposals', 'cards');"),
        "0",
        "no queue made"
    );
}

/// DC-7 (the notes): a `--depth 2` clone of the copy's two commits on top:
/// the storm's change is the tip, read; the arrival's commit is the clone's
/// boundary, whose parent the clone lacks: `null` and the shallow note
/// naming one file. A `--depth 1` clone names both.
#[test]
fn dc7_a_shallow_clone_is_a_note() {
    let pinned = Lamp::committed("dc7-shallow");
    let copy = &pinned.copy;
    for (depth, reached) in [("2", true), ("1", false)] {
        let clone = copy.scratch().join(format!("shallow-{depth}"));
        let url = format!("file://{}", copy.root.display());
        copy.git.git(
            copy.scratch(),
            &[
                "clone",
                "-q",
                "--depth",
                depth,
                &url,
                clone.to_str().expect("UTF-8"),
            ],
        );
        let run = spec_in(copy, &clone, &["catalog", "--kind", "scene", "--json"], &[]);
        run.code(0);
        let document = run.json();
        let k = if reached { 1 } else { 2 };
        let shallow = format!(
            "catalog: a shallow clone: {k} files its history does not reach read no last change"
        );
        let queue = "catalog: no queue yet: no open item or task read".to_owned();
        assert_eq!(
            document["notes"],
            json!([shallow, queue]),
            "depth {depth}: sorted, each once: {}",
            run.show()
        );
        assert_eq!(
            run.stderr,
            format!("note: {shallow}\nnote: {queue}\n"),
            "depth {depth}"
        );
        assert_eq!(
            row(&document, "SCN-ARRIVAL")["last_change"],
            Value::Null,
            "depth {depth}"
        );
        let storm = &row(&document, "SCN-STORM")["last_change"];
        if reached {
            assert_eq!(storm["commit"], pinned.hand.as_str(), "depth {depth}");
            assert_eq!(storm["by"], BOB.value(), "depth {depth}");
        } else {
            assert_eq!(storm, &Value::Null, "depth {depth}: {document}");
        }
    }
}

/// DC-7 (the text cut): 420 scenes with long titles: the text holds whole
/// rows while they fit, then `[truncated: <k> of <n> rows not shown]`; the
/// JSON lists the same first rows, `truncated` true, `omitted` k.
#[test]
fn dc7_the_text_is_cut_after_whole_rows() {
    let copy = Lamp::new("dc7-cut");
    let n = 420;
    let long = "of the long stair below the lamp room ".repeat(3);
    for i in 0..n {
        copy.put(
            &format!("docs/spec/many/scn-many-{i:03}.md"),
            &format!(
                "---\nid: SCN-MANY-{i:03}\nclass: canon\ntitle: Step {i:03} {}\nowner: \
                 owner\nreviewed: 2026-11-08\n---\n\n# Step {i:03}\n\nA step.\n",
                long.trim_end()
            ),
        );
    }
    copy.commit(
        BOB,
        BOB,
        "2026-02-01T10:00:00Z",
        "2026-02-01T10:00:00Z",
        "Many steps\n",
    );
    let text = spec(&copy, &["catalog", "--kind", "scene"]);
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    let tail = lines.last().expect("a tail");
    let shown = lines.len() - 2;
    let k = n + 2 - shown;
    assert_eq!(
        *tail,
        format!("[truncated: {k} of {} rows not shown]", n + 2),
        "{}",
        &text.stdout[text.stdout.len().saturating_sub(400)..]
    );
    assert!(k > 0 && shown > 0);
    assert!(
        text.stdout.chars().count() <= CAP,
        "{} chars",
        text.stdout.chars().count()
    );
    for line in &lines[1..lines.len() - 1] {
        assert!(
            // Ten fields since usage-orphans' uses (docs/features/usage-orphans.md
            // "Acceptance criteria", UO5).
            line.starts_with("SCN-") && line.split(" | ").count() == 10,
            "a whole row: {line}"
        );
    }
    let document = catalog_json(&copy, &["--kind", "scene"]);
    assert_eq!(document["truncated"], true);
    let listed = ids(&document, "rows");
    assert_eq!(
        document["omitted"],
        json!(n + 2 - listed.len()),
        "omitted counts the rows cut"
    );
    let printed: Vec<String> = lines[1..lines.len() - 1]
        .iter()
        .map(|line| line.split(" | ").next().expect("an id").to_owned())
        .collect();
    let shorter = listed.len().min(printed.len());
    assert_eq!(
        listed[..shorter],
        printed[..shorter],
        "the first rows, in order"
    );
    let all = ids(&tree_json(&copy, &["--kind", "scene"]), "nodes");
    assert_eq!(listed[..], all[..listed.len()], "the tree's first rows");
}

/// DC-7 (exits): a database a newer build wrote (the queue's and the
/// index's file) cannot be read: exit 2, nothing on stdout, a message.
#[test]
fn dc7_a_newer_queue_is_exit_2() {
    let pinned = Lamp::pinned("dc7-newer");
    let copy = &pinned.copy;
    copy.sql("PRAGMA user_version = 99;");
    let run = spec(copy, &["catalog", "--kind", "scene", "--json"]);
    run.code(2);
    assert_eq!(run.stdout, "");
    assert!(!run.stderr.is_empty(), "{}", run.show());
}

/// DC-4 (paths): the project in a directory below the worktree's top: the
/// rows' paths are root-relative, each last change its file's.
#[test]
fn dc4_a_project_below_the_worktrees_top() {
    let copy = Lamp::new("dc4-below");
    let top = copy.scratch().join("top");
    let game = top.join("game");
    catalog_scene::copy_tree(&catalog_scene::fixture("studio-lamp"), &game);
    copy.git.init(&top);
    let top = fs::canonicalize(&top).expect("canonical");
    let game = top.join("game");
    copy.git.add_all(&top);
    copy.git.commit(&top, "the game below the top");
    let storm = game.join(STORM);
    let text = fs::read_to_string(&storm).expect("the storm");
    fs::write(&storm, text.replace("Rain", "Sleet")).expect("an edit");
    copy.git.add_all(&top);
    let stamp = copy.stamp(BOB, BOB, "2026-02-01T10:00:00Z", "2026-02-01T10:00:00Z");
    let pairs: Vec<(&str, &OsStr)> = stamp
        .iter()
        .map(|(name, value)| (*name, value.as_os_str()))
        .collect();
    copy.git.git_env(
        &top,
        &["commit", "-q", "--no-verify", "-m", "Sleet"],
        &pairs,
    );
    let sleet = copy.git.git_text(&top, &["rev-parse", "HEAD"]);
    let first = copy.git.git_text(&top, &["rev-parse", "HEAD~1"]);
    let run = spec_in(&copy, &game, &["catalog", "--kind", "scene", "--json"], &[]);
    run.code(0);
    let document = run.json();
    assert_eq!(row(&document, "SCN-STORM")["path"], STORM);
    assert_eq!(
        row(&document, "SCN-STORM")["last_change"]["commit"],
        sleet.as_str()
    );
    assert_eq!(
        row(&document, "SCN-STORM")["last_change"]["by"],
        BOB.value()
    );
    assert_eq!(
        row(&document, "SCN-ARRIVAL")["last_change"]["commit"],
        first.as_str()
    );
}

// ------------------------------------------------------------------ DC-1

/// DC-1: on the copy, the rows' IDs are `spec tree --kind scene`'s nodes',
/// in order; each row keeps the tree node's `id`, `kind`, `title`, `path`,
/// `line`, `status`, `archived`, and `rev` (1 where none is written). M:
/// rows taken from the whole tree.
#[test]
fn dc1_the_rows_are_the_trees_in_order() {
    let pinned = Lamp::pinned("dc1-tree");
    let copy = &pinned.copy;
    let catalog = catalog_json(copy, &["--kind", "scene"]);
    let tree = tree_json(copy, &["--kind", "scene"]);
    assert_eq!(ids(&catalog, "rows"), ["SCN-ARRIVAL", "SCN-STORM"]);
    assert_eq!(ids(&catalog, "rows"), ids(&tree, "nodes"));
    assert_same_as_tree(&catalog, &tree);
    assert_eq!(catalog["kinds"], json!(["scene"]));
    assert_eq!(catalog["archive"], false);
}

/// DC-1: with a section of kind `scene` (`rev=2` written) in a scene's file
/// and another in the chapter's, an archived scene document and an island
/// listed by a second kind: every listing equals the tree's, in order;
/// `--archive` matches `tree --archive`, and the archived document shows
/// only there.
#[test]
fn dc1_sections_archive_and_kinds_are_the_trees() {
    let copy = Lamp::new("dc1-more");
    let storm = copy.read(STORM);
    copy.put(
        STORM,
        &format!("{storm}\n## Low tide {{#SCN-LOW-TIDE rev=2}}\n\nThe sand bar shows.\n"),
    );
    let shore = copy.read(SHORE);
    copy.put(
        SHORE,
        &format!("{shore}\n## The landing {{#SCN-LANDING}}\n\nThe keeper wades ashore.\n"),
    );
    copy.put(
        "docs/spec/scn-wreck.md",
        "---\nid: SCN-WRECK\nclass: spec\nstatus: shipped\ntitle: The wreck\nscope: \
         [scenes]\n---\n\n# The wreck\n\nA hull on the reef.\n",
    );
    copy.commit(
        BOB,
        BOB,
        "2026-02-01T10:00:00Z",
        "2026-02-01T10:00:00Z",
        "More scenes\n",
    );
    for args in [
        &["--kind", "scene"][..],
        &["--kind", "scene", "--archive"],
        &["--kind", "scene", "--kind", "island"],
        &["--kind", "island", "--kind", "scene", "--archive"],
    ] {
        let catalog = catalog_json(&copy, args);
        let tree = tree_json(&copy, args);
        assert_eq!(ids(&catalog, "rows"), ids(&tree, "nodes"), "{args:?}");
        assert_same_as_tree(&catalog, &tree);
        assert_eq!(catalog["archive"], json!(args.contains(&"--archive")));
        let listed = ids(&catalog, "rows");
        assert!(
            listed.contains(&"SCN-LOW-TIDE".to_owned())
                && listed.contains(&"SCN-LANDING".to_owned()),
            "{args:?}: sections listed: {listed:?}"
        );
        assert_eq!(
            listed.contains(&"SCN-WRECK".to_owned()),
            args.contains(&"--archive"),
            "{args:?}: {listed:?}"
        );
        assert!(
            !listed.contains(&"CHP-SHORE".to_owned()),
            "{args:?}: {listed:?}"
        );
    }
    let all = catalog_json(&copy, &["--kind", "scene", "--archive"]);
    assert_eq!(row(&all, "SCN-LOW-TIDE")["rev"], 2);
    assert_eq!(row(&all, "SCN-WRECK")["status"], "shipped");
    assert_eq!(row(&all, "SCN-WRECK")["archived"], true);
    assert_eq!(row(&all, "SCN-LOW-TIDE")["path"], STORM);
}

/// Each row's tree keys equal the tree node's (`rev` 1 where it is `null`).
fn assert_same_as_tree(catalog: &Value, tree: &Value) {
    let rows = catalog["rows"].as_array().expect("rows");
    let nodes = tree["nodes"].as_array().expect("nodes");
    assert_eq!(rows.len(), nodes.len());
    for (row, node) in rows.iter().zip(nodes) {
        for key in ["id", "kind", "title", "path", "line", "status", "archived"] {
            assert_eq!(row[key], node[key], "{key}: {row} / {node}");
        }
        let rev = if node["rev"].is_null() {
            json!(1)
        } else {
            node["rev"].clone()
        };
        assert_eq!(row["rev"], rev, "rev: {row} / {node}");
    }
    assert_row_shape(catalog);
}

// ------------------------------------------------------------------ DC-2

/// `spec trace <id> --json`'s first node.
fn traced(copy: &Lamp, id: &str) -> Value {
    let run = spec(copy, &["trace", id, "--json"]);
    run.code(0);
    run.json()["nodes"][0].clone()
}

/// Each row's `state` and `tasks` against the trace's first node.
fn assert_trace_agrees(copy: &Lamp, catalog: &Value) {
    for row in catalog["rows"].as_array().expect("rows") {
        let id = row["id"].as_str().expect("an id");
        let node = traced(copy, id);
        assert_eq!(node["id"], id, "{node}");
        assert_eq!(row["state"], node["rollup"], "{id}: {row} / {node}");
        assert_eq!(
            row["tasks"],
            json!(node["cards"].as_array().expect("cards").len()),
            "{id}: {row} / {node}"
        );
    }
}

/// DC-2: on the copy, each row's `state` and `tasks` are the trace's; C-0002
/// on the storm cancelled is in neither; C-0003 on the storm's section
/// counts as the trace counts it; C-0001 accepted makes the arrival
/// `confirmed`, `tasks` 1, `tasks_open` 0. M: `state` always `no card`;
/// cancelled cards counted.
#[test]
fn dc2_state_and_tasks_are_the_traces() {
    let pinned = Lamp::pinned("dc2-trace");
    let copy = &pinned.copy;
    let start = catalog_json(copy, &["--kind", "scene"]);
    assert_trace_agrees(copy, &start);
    assert_eq!(row(&start, "SCN-ARRIVAL")["state"], "in work");
    assert_eq!(row(&start, "SCN-ARRIVAL")["tasks_open"], 1);

    let cancelled = copy.card("Light the storm", &["SCN-STORM"], "2026-01-05T10:00:00Z");
    assert_eq!(cancelled, "C-0002");
    copy.cancel(&cancelled, "The storm is cut", "2026-01-05T11:00:00Z");
    let after_cancel = catalog_json(copy, &["--kind", "scene"]);
    assert_trace_agrees(copy, &after_cancel);
    let storm = row(&after_cancel, "SCN-STORM");
    assert_eq!(storm["state"], "no card", "{storm}");
    assert_eq!(
        [&storm["tasks"], &storm["tasks_open"]],
        [&json!(0), &json!(0)],
        "a cancelled card is in neither: {storm}"
    );

    copy.accept("C-0001", "2026-01-06T10:00:00Z");
    let accepted = catalog_json(copy, &["--kind", "scene"]);
    assert_trace_agrees(copy, &accepted);
    let arrival = row(&accepted, "SCN-ARRIVAL");
    assert_eq!(arrival["state"], "confirmed", "{arrival}");
    assert_eq!(
        [&arrival["tasks"], &arrival["tasks_open"]],
        [&json!(1), &json!(0)],
        "{arrival}"
    );

    // A section's card, as the trace counts it on its document.
    let storm_text = copy.read(STORM);
    copy.put(
        STORM,
        &format!("{storm_text}\n## Low tide {{#SCN-LOW-TIDE}}\n\nThe sand bar shows.\n"),
    );
    let section_card = copy.card("Draw the bar", &["SCN-LOW-TIDE"], "2026-01-07T10:00:00Z");
    assert_eq!(section_card, "C-0003");
    let sectioned = catalog_json(copy, &["--kind", "scene"]);
    assert_trace_agrees(copy, &sectioned);
    let low = row(&sectioned, "SCN-LOW-TIDE");
    assert_eq!(low["state"], "in work", "{low}");
    assert_eq!(
        [&low["tasks"], &low["tasks_open"]],
        [&json!(1), &json!(1)],
        "{low}"
    );
}

// ------------------------------------------------------------------ DC-3

/// A question naming `nodes`, raised in `cwd` under the pair's `HOME`.
fn ask(pair: &Pair, cwd: &Path, nodes: &[&str], text: &str) -> String {
    let request = specengine_cli::QuestionRequest {
        node_ids: nodes.iter().map(|id| (*id).to_owned()).collect(),
        text: text.to_owned(),
        working_answer: "Yes.".to_owned(),
        price_of_other: "A scene rewritten.".to_owned(),
        severity: None,
        distinct_from: Vec::new(),
        card: None,
        author_role: Some("developer".to_owned()),
        author_model: Some("m".to_owned()),
        run: None,
        login: None,
        now: common::proposal::NOW.to_owned(),
        git: pair.git_env(cwd),
        gist: None,
        matters: None,
        options: None,
        recommendation: None,
    };
    let outcome = specengine_cli::propose_question(&pair.env(cwd), &Globals::default(), &request)
        .unwrap_or_else(|error| panic!("question {text}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

/// `spec catalog --kind scene --json` in the pair's main worktree.
fn pair_catalog(pair: &Pair, cwd: &Path) -> Value {
    let home = pair.home.clone();
    let vars: Vec<(OsString, OsString)> = pair
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .chain([(OsString::from("HOME"), home.into_os_string())])
        .collect();
    let env: Vec<(String, OsString)> = vars
        .into_iter()
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
        .collect();
    let env: Vec<(&str, &OsStr)> = env
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    let run = spec_with(cwd, &["catalog", "--kind", "scene", "--json"], &env);
    run.code(0);
    run.json()
}

/// DC-3: on the arrival, an open question naming it and an open update
/// targeting it count 2; a staged one counts; an applied update, a rejected
/// question, a question naming the storm and one raised in another
/// repository (same slug, same `HOME`) do not; one raised in the linked
/// worktree (the same repository) does; an item naming both scenes counts
/// once on each; a section's item is its own row's, never its document's.
/// M: applied items counted; the repository filter dropped.
#[test]
fn dc3_open_items_of_this_repository() {
    let pair = Pair::new("dc3-items", "studio-lamp");
    let main = pair.main.clone();
    let counts = |cwd: &Path| {
        let document = pair_catalog(&pair, cwd);
        (
            open_items(&document, "SCN-ARRIVAL"),
            open_items(&document, "SCN-STORM"),
        )
    };
    assert_eq!(counts(&main), (0, 0));

    // An applied update first: its commit moves the base of the next.
    let applied = pair.propose_edit(&main, "SCN-ARRIVAL", "shingle", "wet shingle");
    pair.approve_ok(&main, &applied);
    assert_eq!(pair.proposal(&applied).status.as_str(), "applied");
    assert_eq!(counts(&main), (0, 0), "an applied update does not count");

    ask(&pair, &main, &["SCN-ARRIVAL"], "Is the key on the boat?");
    pair.propose_edit(&main, "SCN-ARRIVAL", "locked", "bolted");
    assert_eq!(counts(&main), (2, 0), "a question and an update");

    let staged = ask(&pair, &main, &["SCN-ARRIVAL"], "Is the door oak?");
    let updated_at = pair.proposal(&staged).updated_at;
    let outcome = stage(
        &pair.env(&main),
        &Globals::default(),
        &StageRequest {
            id: staged.clone(),
            body: StageBody::Reject {
                reason: "Not now.".to_owned(),
                updated_at,
                actor: None,
            },
            now: common::proposal::LATER.to_owned(),
            git: pair.git_env(&main),
            actor: None,
        },
    )
    .unwrap_or_else(|error| panic!("stage: {error}"));
    assert_eq!(outcome.cause, None, "{outcome:?}");
    assert_eq!(counts(&main), (3, 0), "a staged item is open");

    let rejected = ask(&pair, &main, &["SCN-ARRIVAL"], "Is the hook brass?");
    let (result, _) = pair.reject_answer(&main, &rejected, "No.", true);
    let result = result.unwrap_or_else(|error| panic!("reject: {error}"));
    assert_eq!(result.exit(), Exit::Answered, "{result:?}");
    assert_eq!(pair.proposal(&rejected).status.as_str(), "rejected");
    assert_eq!(counts(&main), (3, 0), "a rejected item does not count");

    ask(&pair, &main, &["SCN-STORM"], "Does the rain stop?");
    assert_eq!(counts(&main), (3, 1), "the storm's is the storm's");

    // Another repository of the same project under the same HOME.
    let other = pair.scratch.copy("studio-lamp", "other");
    pair.git.init(&other);
    pair.git.add_all(&other);
    pair.git.commit(&other, "another copy");
    ask(&pair, &other, &["SCN-ARRIVAL"], "Is the boat red?");
    assert_eq!(
        counts(&main),
        (3, 1),
        "another repository's item does not count"
    );
    assert_eq!(
        counts(&other),
        (1, 0),
        "the other repository counts its own alone"
    );

    // The linked worktree is this repository.
    ask(&pair, &pair.linked, &["SCN-ARRIVAL"], "Is the tide high?");
    assert_eq!(counts(&main), (4, 1), "the linked worktree's item counts");

    ask(
        &pair,
        &main,
        &["SCN-ARRIVAL", "SCN-STORM"],
        "Do both scenes share the night?",
    );
    assert_eq!(counts(&main), (5, 2), "once on each");
}

/// DC-3 (D5): a question naming a section counts on the section's row and
/// not on its document's.
#[test]
fn dc3_a_document_does_not_count_its_sections_items() {
    let pair = Pair::new("dc3-sections", "studio-lamp");
    let main = pair.main.clone();
    let storm = fs::read_to_string(main.join(STORM)).expect("the storm");
    fs::write(
        main.join(STORM),
        format!("{storm}\n## Low tide {{#SCN-LOW-TIDE}}\n\nThe sand bar shows.\n"),
    )
    .expect("a section");
    pair.git.add_all(&main);
    pair.git.commit(&main, "a section");
    ask(&pair, &main, &["SCN-LOW-TIDE"], "Is the bar dry at noon?");
    let document = pair_catalog(&pair, &main);
    assert_eq!(open_items(&document, "SCN-LOW-TIDE"), 1);
    assert_eq!(open_items(&document, "SCN-STORM"), 0);
}

// ------------------------------------------------------------------ DC-4

/// `spec show <id> --history --json`'s newest `source: commit` entry.
fn newest_commit_entry(copy: &Lamp, id: &str) -> Value {
    let run = spec(copy, &["show", id, "--history", "--json"]);
    run.code(0);
    let document = run.json();
    document["nodes"][0]["history"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("{id}: no history: {document}"))
        .iter()
        .find(|entry| entry["source"] == "commit")
        .cloned()
        .unwrap_or_else(|| panic!("{id}: no commit entry: {document}"))
}

/// The row's `last_change` against a history entry (`id` as `commit`).
fn assert_change_is_entry(change: &Value, entry: &Value, of_file: bool, context: &str) {
    assert_eq!(
        change,
        &json!({
            "at": entry["at"], "by": entry["by"], "by_name": entry["by_name"],
            "commit": entry["id"], "proposal": entry["proposal"], "of_file": of_file,
        }),
        "{context}: {entry}"
    );
}

/// The expected change of one commit.
fn change(at: &str, by: Ident, commit: &str, proposal: Option<&str>, of_file: bool) -> Value {
    json!({"at": at, "by": by.value(), "by_name": by.name, "commit": commit,
           "proposal": proposal, "of_file": of_file})
}

const DANA: Ident = Ident {
    name: "Dana Desk",
    email: "dana@example.org",
};

/// DC-4's linear repository on the fixture's commit: (1) Bob by hand on the
/// arrival, committed by Cleo; (2) Cleo committing PR-0007 decided by Ann on
/// the storm, adding its section `SCN-LOW-TIDE`; (3) Bob by hand on the
/// arrival again; (4) Dana on the chapter alone; (5) an apply with no
/// `Decided-by:` creating `SCN-SIGNAL`, authored by Dana. Committer times
/// one day apart; author times earlier, never the `at`.
struct Linear {
    copy: Lamp,
    c1: String,
    c2: String,
    c3: String,
    c5: String,
}

fn linear(label: &str) -> Linear {
    let copy = Lamp::new(label);
    copy.edit(ARRIVAL, "locked", "shut");
    let c1 = copy.commit(
        BOB,
        CLEO,
        "2026-01-20T08:00:00Z",
        "2026-02-01T10:00:00Z",
        "Shut the door\n",
    );
    let storm = copy.read(STORM);
    copy.put(
        STORM,
        &format!("{storm}\n## Low tide {{#SCN-LOW-TIDE}}\n\nThe sand bar shows.\n"),
    );
    let c2 = copy.commit(
        CLEO,
        CLEO,
        "2026-01-21T08:00:00Z",
        "2026-02-02T10:00:00Z",
        &apply_message("PR-0007", "Add the low tide.", ANN, &c1),
    );
    copy.edit(ARRIVAL, "brass key", "iron key");
    let c3 = copy.commit(
        BOB,
        BOB,
        "2026-01-22T08:00:00Z",
        "2026-02-03T10:00:00Z",
        "Iron, not brass\n",
    );
    copy.edit(SHORE, "dark lighthouse", "dark old lighthouse");
    copy.commit(
        DANA,
        DANA,
        "2026-01-23T08:00:00Z",
        "2026-02-04T10:00:00Z",
        "Age the lighthouse\n",
    );
    copy.put(
        "docs/spec/scn-signal.md",
        "---\nid: SCN-SIGNAL\nclass: canon\ntitle: The signal\nowner: owner\nreviewed: \
         2026-11-08\n---\n\n# The signal\n\nThe lamp answers a ship.\n",
    );
    let c5 = copy.commit(
        DANA,
        CLEO,
        "2026-01-24T08:00:00Z",
        "2026-02-05T10:00:00Z",
        "spec: apply PR-0008\n\nAdd the signal.\n\nProposal: PR-0008\nBase-commit: 0\n",
    );
    Linear {
        copy,
        c1,
        c2,
        c3,
        c5,
    }
}

/// DC-4: on the linear repository, each document row's `last_change` is
/// its history's newest commit entry and the commit worked out here (a hand
/// commit's author, never its committer; an apply's `Decided-by:`; an apply
/// with none its author; the committer time); the section row carries its
/// file's with `of_file` true. M: the oldest commit taken.
#[test]
fn dc4_last_change_is_the_newest_commit_of_the_file() {
    let line = linear("dc4-linear");
    let copy = &line.copy;
    let document = catalog_json(copy, &["--kind", "scene"]);
    let listed: BTreeSet<String> = ids(&document, "rows").into_iter().collect();
    assert_eq!(
        listed,
        ["SCN-ARRIVAL", "SCN-LOW-TIDE", "SCN-SIGNAL", "SCN-STORM"]
            .map(str::to_owned)
            .into(),
        "{document}"
    );
    assert_eq!(
        ids(&document, "rows"),
        ids(&tree_json(copy, &["--kind", "scene"]), "nodes")
    );
    let arrival = &row(&document, "SCN-ARRIVAL")["last_change"];
    let storm = &row(&document, "SCN-STORM")["last_change"];
    let signal = &row(&document, "SCN-SIGNAL")["last_change"];
    let low = &row(&document, "SCN-LOW-TIDE")["last_change"];
    assert_eq!(
        arrival,
        &change("2026-02-03T10:00:00Z", BOB, &line.c3, None, false)
    );
    assert_eq!(
        storm,
        &change(
            "2026-02-02T10:00:00Z",
            ANN,
            &line.c2,
            Some("PR-0007"),
            false
        )
    );
    assert_eq!(
        signal,
        &change(
            "2026-02-05T10:00:00Z",
            DANA,
            &line.c5,
            Some("PR-0008"),
            false
        )
    );
    assert_eq!(
        low,
        &change("2026-02-02T10:00:00Z", ANN, &line.c2, Some("PR-0007"), true),
        "a section row carries its file's"
    );
    for id in ["SCN-ARRIVAL", "SCN-STORM", "SCN-SIGNAL"] {
        assert_change_is_entry(
            &row(&document, id)["last_change"],
            &newest_commit_entry(copy, id),
            false,
            id,
        );
    }
    assert_ne!(line.c1, line.c3);
}

/// DC-4: the files' mtimes moved far into the past and the future change
/// no byte; an uncommitted edit leaves `HEAD`'s change; an uncommitted new
/// scene reads `null`. M: the file's mtime used.
#[test]
fn dc4_mtime_and_the_working_tree_change_nothing() {
    let line = linear("dc4-mtime");
    let copy = &line.copy;
    let before = spec(copy, &["catalog", "--kind", "scene", "--json"]);
    before.code(0);
    for (relative, when) in [
        (
            ARRIVAL,
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000),
        ),
        (
            STORM,
            SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000),
        ),
    ] {
        let file = fs::File::options()
            .write(true)
            .open(copy.path(relative))
            .expect("open");
        file.set_modified(when).expect("set mtime");
    }
    let touched = spec(copy, &["catalog", "--kind", "scene", "--json"]);
    touched.code(0);
    assert_eq!(touched.stdout, before.stdout, "an mtime changes nothing");

    copy.edit(STORM, "sand bar", "sandbar");
    copy.put(
        "docs/spec/scn-fog.md",
        "---\nid: SCN-FOG\nclass: canon\ntitle: The fog\nowner: owner\nreviewed: \
         2026-11-08\n---\n\n# The fog\n\nThe lamp cannot be seen.\n",
    );
    let document = catalog_json(copy, &["--kind", "scene"]);
    assert_eq!(
        row(&document, "SCN-STORM")["last_change"],
        change(
            "2026-02-02T10:00:00Z",
            ANN,
            &line.c2,
            Some("PR-0007"),
            false
        ),
        "an uncommitted edit: HEAD's change"
    );
    assert_eq!(
        row(&document, "SCN-FOG")["last_change"],
        Value::Null,
        "an uncommitted file"
    );
    assert!(
        document["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .all(|note| !note.as_str().unwrap_or("").contains("not reached")),
        "an uncommitted file is no scan note: {document}"
    );
    // Staged but not committed is still not at HEAD.
    copy.git.add_all(&copy.root);
    let staged = catalog_json(copy, &["--kind", "scene"]);
    assert_eq!(row(&staged, "SCN-FOG")["last_change"], Value::Null);
}

/// DC-4: a branch `side` from the fixture's commit edits the storm and
/// edits then reverts the arrival; `main` edits the arrival by hand, then
/// merges `side` (authored by Mia, committed by Cleo). The storm's change is
/// the merge (its committer time, its author, no proposal); the arrival's
/// is `main`'s own edit, which the side's later edit and revert leave as
/// the first-parent line's. M: `--first-parent` dropped.
#[test]
fn dc4_on_a_merged_branch_the_merge_is_the_change() {
    const SID: Ident = Ident {
        name: "Sid Side",
        email: "sid@example.org",
    };
    const MIA: Ident = Ident {
        name: "Mia Merge",
        email: "mia@example.org",
    };
    let copy = Lamp::new("dc4-merge");
    copy.git.git(&copy.root, &["branch", "side"]);
    copy.edit(ARRIVAL, "shingle", "pebbles");
    let main_edit = copy.commit(
        BOB,
        BOB,
        "2026-03-01T10:00:00Z",
        "2026-03-01T10:00:00Z",
        "Pebbles\n",
    );
    copy.git.git(&copy.root, &["checkout", "-q", "side"]);
    copy.edit(STORM, "Rain", "Hard rain");
    let side_storm = copy.commit(
        SID,
        SID,
        "2026-03-02T10:00:00Z",
        "2026-03-02T10:00:00Z",
        "Hard rain\n",
    );
    copy.edit(ARRIVAL, "aground", "hard aground");
    copy.commit(
        SID,
        SID,
        "2026-03-03T10:00:00Z",
        "2026-03-03T10:00:00Z",
        "Hard aground\n",
    );
    copy.edit(ARRIVAL, "hard aground", "aground");
    let side_revert = copy.commit(
        SID,
        SID,
        "2026-03-04T10:00:00Z",
        "2026-03-04T10:00:00Z",
        "Back to aground\n",
    );
    copy.git.git(&copy.root, &["checkout", "-q", "main"]);
    let stamp = copy.stamp(MIA, CLEO, "2026-03-05T08:00:00Z", "2026-03-05T10:00:00Z");
    copy.git_as(
        &stamp,
        &[
            "merge",
            "-q",
            "--no-ff",
            "--no-edit",
            "-m",
            "Merge side",
            "side",
        ],
    );
    let merge = copy.git.git_text(&copy.root, &["rev-parse", "HEAD"]);
    assert_eq!(
        copy.read(ARRIVAL).matches("pebbles").count(),
        1,
        "main's edit stands"
    );
    let document = catalog_json(&copy, &["--kind", "scene"]);
    assert_eq!(
        row(&document, "SCN-STORM")["last_change"],
        change("2026-03-05T10:00:00Z", MIA, &merge, None, false),
        "the merge, not {side_storm}"
    );
    assert_eq!(
        row(&document, "SCN-ARRIVAL")["last_change"],
        change("2026-03-01T10:00:00Z", BOB, &main_edit, None, false),
        "main's own edit, not the side's revert {side_revert}"
    );
}

/// `ticks` commits on top of `HEAD`, each touching `notes/tick.txt` alone
/// (a `git fast-import` stream), the worktree reset to them.
fn ticks(copy: &Lamp, ticks: u32) {
    let mut stream = String::new();
    let parent = copy.git.git_text(&copy.root, &["rev-parse", "HEAD"]);
    for n in 0..ticks {
        let data = format!("tick {n}\n");
        let message = format!("tick {n}\n");
        stream.push_str(&format!(
            "commit refs/heads/main\ncommitter Tick Tock <tick@example.org> {} +0000\ndata {}\n{}",
            1_770_000_000 + u64::from(n),
            message.len(),
            message
        ));
        if n == 0 {
            stream.push_str(&format!("from {parent}\n"));
        }
        stream.push_str(&format!(
            "M 100644 inline notes/tick.txt\ndata {}\n{}\n",
            data.len(),
            data
        ));
    }
    copy.git
        .git_stdin(&copy.root, &["fast-import", "--quiet"], stream.as_bytes());
    copy.git
        .git(&copy.root, &["reset", "-q", "--hard", "refs/heads/main"]);
}

const NO_QUEUE: &str = "catalog: no queue yet: no open item or task read";

/// DC-7 (the scan note): 20 001 commits on top of the scenes' only commit,
/// each touching another file: the walk stops at 20 000, both scene files
/// unreached: `null` and the note naming 2 files.
#[test]
fn dc7_files_beyond_the_scan_are_a_note() {
    let copy = Lamp::new("dc7-scan");
    ticks(&copy, 20_001);
    let run = spec(&copy, &["catalog", "--kind", "scene", "--json"]);
    run.code(0);
    let document = run.json();
    let note = "catalog: 2 files not reached in the last 20000 commits read no last change";
    assert_eq!(document["notes"], json!([note, NO_QUEUE]), "{}", run.show());
    for id in ["SCN-ARRIVAL", "SCN-STORM"] {
        assert_eq!(row(&document, id)["last_change"], Value::Null, "{id}");
    }
}

/// DC-7 (the scan note): a history of exactly the cap, 20 000 commits (the
/// fixture's and 19 999 on top), ends naturally: the scenes' change is the
/// fixture's commit, no scan note; a `--depth 20000` clone of one commit
/// more ends at its boundary within the cap: the shallow note, never the
/// scan note.
#[test]
fn dc7_a_history_of_exactly_the_cap_is_no_scan_note() {
    let copy = Lamp::new("dc7-exact");
    ticks(&copy, 19_999);
    let run = spec(&copy, &["catalog", "--kind", "scene", "--json"]);
    run.code(0);
    let document = run.json();
    assert_eq!(document["notes"], json!([NO_QUEUE]), "{}", run.show());
    for id in ["SCN-ARRIVAL", "SCN-STORM"] {
        assert_eq!(
            row(&document, id)["last_change"]["commit"],
            copy.first.as_str(),
            "{id}"
        );
    }

    ticks(&copy, 1);
    let clone = copy.scratch().join("deep-shallow");
    let url = format!("file://{}", copy.root.display());
    copy.git.git(
        copy.scratch(),
        &[
            "clone",
            "-q",
            "--depth",
            "20000",
            &url,
            clone.to_str().expect("UTF-8"),
        ],
    );
    let run = spec_in(
        &copy,
        &clone,
        &["catalog", "--kind", "scene", "--json"],
        &[],
    );
    run.code(0);
    let shallow =
        "catalog: a shallow clone: 2 files its history does not reach read no last change";
    assert_eq!(
        run.json()["notes"],
        json!([shallow, NO_QUEUE]),
        "{}",
        run.show()
    );
}

/// DC-7 (the shallow note): a full clone of the copy reads every last
/// change as the copy does and says nothing of shallowness.
#[test]
fn dc7_a_full_clone_is_no_shallow_note() {
    let pinned = Lamp::committed("dc7-full-clone");
    let copy = &pinned.copy;
    let clone = copy.scratch().join("full");
    let url = format!("file://{}", copy.root.display());
    copy.git.git(
        copy.scratch(),
        &["clone", "-q", &url, clone.to_str().expect("UTF-8")],
    );
    let here = catalog_json(copy, &["--kind", "scene"]);
    let run = spec_in(copy, &clone, &["catalog", "--kind", "scene", "--json"], &[]);
    run.code(0);
    let there = run.json();
    assert_eq!(there["notes"], json!([NO_QUEUE]), "{}", run.show());
    assert_eq!(there["rows"], here["rows"]);
}

/// DC-7 (a partial clone): a blobless clone (`--filter=blob:none`) of the
/// copy shows each committed file's last change as the copy does, or says
/// in a note why not: never a silent `null`.
#[test]
fn dc7_a_blobless_clone_is_never_a_silent_null() {
    let pinned = Lamp::committed("dc7-blobless");
    let copy = &pinned.copy;
    copy.git
        .git(&copy.root, &["config", "uploadpack.allowFilter", "true"]);
    let clone = copy.scratch().join("blobless");
    let url = format!("file://{}", copy.root.display());
    copy.git.git(
        copy.scratch(),
        &[
            "clone",
            "-q",
            "--filter=blob:none",
            &url,
            clone.to_str().expect("UTF-8"),
        ],
    );
    let here = catalog_json(copy, &["--kind", "scene"]);
    let run = spec_in(copy, &clone, &["catalog", "--kind", "scene", "--json"], &[]);
    run.code(0);
    let there = run.json();
    let notes: Vec<&str> = there["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(Value::as_str)
        .filter(|note| *note != NO_QUEUE)
        .collect();
    for id in ["SCN-ARRIVAL", "SCN-STORM"] {
        let change = &row(&there, id)["last_change"];
        if change.is_null() {
            assert!(
                !notes.is_empty(),
                "{id}: a null last change says why: {}",
                run.show()
            );
        } else {
            assert_eq!(change, &row(&here, id)["last_change"], "{id}");
        }
    }
    eprintln!(
        "blobless: arrival {}, storm {}, notes {notes:?}",
        row(&there, "SCN-ARRIVAL")["last_change"]["commit"],
        row(&there, "SCN-STORM")["last_change"]["commit"]
    );
}

/// The pack files of a repository's object store.
fn packs(git_dir: &Path) -> Vec<String> {
    let mut packs: Vec<String> = fs::read_dir(git_dir.join("objects/pack"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    packs.sort();
    packs
}

/// DC-7 (a partial clone, iteration 2): a blobless clone made with no
/// checkout holds no blob at all, `HEAD`'s neither; the scenes' files are
/// written into its worktree by hand. The catalog reads each real last
/// change from trees and commits alone, says nothing, and fetches nothing:
/// the pack files are as they were and `HEAD`'s blob is still missing.
#[test]
fn dc7_a_blobless_clone_missing_head_blobs_fetches_nothing() {
    let pinned = Lamp::committed("dc7-blobless-bare");
    let copy = &pinned.copy;
    copy.git
        .git(&copy.root, &["config", "uploadpack.allowFilter", "true"]);
    let clone = copy.scratch().join("blobless-bare");
    let url = format!("file://{}", copy.root.display());
    copy.git.git(
        copy.scratch(),
        &[
            "clone",
            "-q",
            "--no-checkout",
            "--filter=blob:none",
            &url,
            clone.to_str().expect("UTF-8"),
        ],
    );
    let clone = fs::canonicalize(&clone).expect("canonical");
    catalog_scene::copy_tree(&copy.root.join("docs"), &clone.join("docs"));
    fs::copy(
        copy.root.join("specengine.toml"),
        clone.join("specengine.toml"),
    )
    .expect("the config");
    let missing = |relative: &str| {
        !copy
            .git
            .git_output(
                &clone,
                &["cat-file", "-e", &format!("HEAD:{relative}")],
                &[("GIT_NO_LAZY_FETCH", OsStr::new("1"))],
            )
            .status
            .success()
    };
    assert!(missing(ARRIVAL), "premise: HEAD's blob is not here");
    let before = packs(&clone.join(".git"));
    let here = catalog_json(copy, &["--kind", "scene"]);
    let run = spec_in(copy, &clone, &["catalog", "--kind", "scene", "--json"], &[]);
    run.code(0);
    let there = run.json();
    assert_eq!(there["notes"], json!([NO_QUEUE]), "{}", run.show());
    assert_eq!(there["rows"], here["rows"], "the real last changes");
    assert_eq!(
        row(&there, "SCN-ARRIVAL")["last_change"]["commit"],
        pinned.applied.as_str()
    );
    assert_eq!(packs(&clone.join(".git")), before, "nothing fetched");
    assert!(missing(ARRIVAL), "HEAD's blob still not fetched");
}

/// DC-7 (the shallow note, iteration 2): `side` (four commits on the
/// fixture's) merged into `main` after `main`'s own edit; a `--depth 3`
/// clone cuts `side` short (a shallow clone) while `main`'s first-parent
/// line reaches the true root: every change read, the storm's the root's,
/// no shallow note.
#[test]
fn dc7_a_depth_clone_reaching_the_true_root_is_no_shallow_note() {
    let copy = Lamp::new("dc7-depth-root");
    copy.git.git(&copy.root, &["branch", "side"]);
    copy.edit(ARRIVAL, "shingle", "pebbles");
    let main_edit = copy.commit(
        BOB,
        BOB,
        "2026-03-01T10:00:00Z",
        "2026-03-01T10:00:00Z",
        "Pebbles\n",
    );
    copy.git.git(&copy.root, &["checkout", "-q", "side"]);
    for n in 1..=4 {
        copy.put("notes/side.txt", &format!("side {n}\n"));
        copy.commit(
            CLEO,
            CLEO,
            &format!("2026-03-0{}T10:00:00Z", n + 1),
            &format!("2026-03-0{}T10:00:00Z", n + 1),
            &format!("Side {n}\n"),
        );
    }
    copy.git.git(&copy.root, &["checkout", "-q", "main"]);
    let stamp = copy.stamp(CLEO, CLEO, "2026-03-07T10:00:00Z", "2026-03-07T10:00:00Z");
    copy.git_as(
        &stamp,
        &[
            "merge",
            "-q",
            "--no-ff",
            "--no-edit",
            "-m",
            "Merge side",
            "side",
        ],
    );
    let clone = copy.scratch().join("depth-3");
    let url = format!("file://{}", copy.root.display());
    copy.git.git(
        copy.scratch(),
        &[
            "clone",
            "-q",
            "--depth",
            "3",
            &url,
            clone.to_str().expect("UTF-8"),
        ],
    );
    assert_eq!(
        copy.git
            .git_text(&clone, &["rev-parse", "--is-shallow-repository"]),
        "true",
        "premise: the clone is shallow"
    );
    let run = spec_in(
        &copy,
        &clone,
        &["catalog", "--kind", "scene", "--json"],
        &[],
    );
    run.code(0);
    let document = run.json();
    assert_eq!(document["notes"], json!([NO_QUEUE]), "{}", run.show());
    assert_eq!(
        row(&document, "SCN-ARRIVAL")["last_change"]["commit"],
        main_edit.as_str()
    );
    assert_eq!(
        row(&document, "SCN-STORM")["last_change"]["commit"],
        copy.first.as_str(),
        "the true root's"
    );
}

// ------------------------------------------------------------------ DC-8

/// A `git` shim first on `PATH` (in `<scratch>/<dir>`): it logs
/// `$GIT_NO_LAZY_FETCH|<args>` to `args.log`, tees a `cat-file`'s stdin to
/// `stdin.log`, and runs the real git. Its directory and the two logs.
fn shim(copy: &Lamp, dir: &str) -> (PathBuf, PathBuf, PathBuf) {
    let bin = copy.scratch().join(dir);
    fs::create_dir_all(&bin).expect("the shim's directory");
    let args = bin.join("args.log");
    let stdin = bin.join("stdin.log");
    let git = copy.git.git_program().to_path_buf();
    let script = format!(
        "#!/bin/sh\nprintf '%s|%s\\n' \"$GIT_NO_LAZY_FETCH\" \"$*\" >> '{args}'\ncase \" $* \" \
         in\n  *\" cat-file \"*) /usr/bin/tee -a '{stdin}' | '{git}' \"$@\" ;;\n  *) exec \
         '{git}' \"$@\" ;;\nesac\n",
        args = args.display(),
        stdin = stdin.display(),
        git = git.display(),
    );
    let path = bin.join("git");
    fs::write(&path, script).expect("the shim");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    (bin, args, stdin)
}

/// `PATH` with `first` before the sandbox's.
fn path_with(copy: &Lamp, first: &Path) -> OsString {
    let mut dirs = vec![first.to_path_buf()];
    dirs.extend(std::env::split_paths(
        copy.git.var("PATH").expect("the sandbox's PATH"),
    ));
    std::env::join_paths(dirs).expect("a PATH")
}

/// Every logged line.
fn logged(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A logged line's git subcommand: its first word that is no option and
/// no `-C` directory.
fn subcommand(line: &str) -> String {
    let args = line.split_once('|').map_or(line, |(_, args)| args);
    let mut words = args.split_whitespace();
    while let Some(word) = words.next() {
        if word == "-C" || word == "-c" {
            words.next();
            continue;
        }
        if !word.starts_with('-') {
            return word.to_owned();
        }
    }
    String::new()
}

/// Every file under `dir`, relative.
fn files_under(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.push(
                    path.strip_prefix(root)
                        .expect("under")
                        .display()
                        .to_string(),
                );
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// The fixture's two scenes removed, then 12 scene files (four with three
/// sections, eight with two: 40 nodes), each committed in its own commit by
/// Bob; a queue made by one card.
fn forty_rows(label: &str) -> (Lamp, BTreeSet<String>) {
    let copy = Lamp::new(label);
    fs::remove_file(copy.path(ARRIVAL)).expect("the arrival removed");
    fs::remove_file(copy.path(STORM)).expect("the storm removed");
    let mut paths = BTreeSet::new();
    for file in 0..12_u32 {
        let sections = if file < 4 { 3 } else { 2 };
        let mut text = format!(
            "---\nid: SCN-ROOM-{file:02}\nclass: canon\ntitle: Room {file:02}\nowner: \
             owner\nreviewed: 2026-11-08\n---\n\n# Room {file:02}\n\nA room of the tower.\n"
        );
        for section in 0..sections {
            text.push_str(&format!(
                "\n## Corner {section} {{#SCN-ROOM-{file:02}-C{section}}}\n\nA corner.\n"
            ));
        }
        let relative = format!("docs/spec/rooms/scn-room-{file:02}.md");
        copy.put(&relative, &text);
        copy.commit(
            BOB,
            BOB,
            "2026-02-01T10:00:00Z",
            &format!("2026-02-{:02}T10:00:00Z", file + 1),
            &format!("Room {file:02}\n"),
        );
        paths.insert(relative);
    }
    copy.card("Furnish a room", &["SCN-ROOM-00"], "2026-02-20T10:00:00Z");
    (copy, paths)
}

/// DC-8, the CLI half: one `spec catalog` over 40 rows in 12 files runs
/// one presence read of `HEAD`'s tree (iteration 2: one `ls-tree -r
/// --name-only`, where the spec names a `cat-file --batch-check` session;
/// no `cat-file` at all here, no boundary being judged), one walking `log
/// --first-parent`, one `log --no-walk`, each with `GIT_NO_LAZY_FETCH=1`,
/// and `rev-parse` alone besides. M: a `file_log` per path.
#[test]
fn dc8_one_call_runs_one_walk() {
    let (copy, rooms) = forty_rows("dc8-cli");
    assert_eq!(rooms.len(), 12);
    let (bin, args_log, _) = shim(&copy, "shim");
    let path = path_with(&copy, &bin);
    let run = spec_in(
        &copy,
        &copy.root,
        &["catalog", "--kind", "scene", "--json"],
        &[("PATH", path.as_os_str())],
    );
    run.code(0);
    let document = run.json();
    let rows = document["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 40, "40 rows");
    assert!(
        rows.iter().all(|row| row["last_change"].is_object()),
        "{document}"
    );
    let lines = logged(&args_log);
    let by = |sub: &str| -> Vec<&String> {
        lines
            .iter()
            .filter(|line| subcommand(line) == sub)
            .collect()
    };
    let others: Vec<&String> = lines
        .iter()
        .filter(|line| !["rev-parse", "log", "ls-tree"].contains(&subcommand(line).as_str()))
        .collect();
    assert_eq!(others, Vec::<&String>::new(), "rev-parse alone besides");
    let trees = by("ls-tree");
    assert_eq!(trees.len(), 1, "{lines:#?}");
    assert!(
        trees[0].contains(" -r ") && trees[0].contains("--name-only"),
        "{lines:#?}"
    );
    let logs = by("log");
    assert_eq!(logs.len(), 2, "{lines:#?}");
    let walking: Vec<&&String> = logs
        .iter()
        .filter(|line| line.contains("--first-parent"))
        .collect();
    let no_walk: Vec<&&String> = logs
        .iter()
        .filter(|line| line.contains("--no-walk"))
        .collect();
    assert_eq!((walking.len(), no_walk.len()), (1, 1), "{lines:#?}");
    for line in trees.iter().chain(logs.iter()) {
        assert!(line.starts_with("1|"), "GIT_NO_LAZY_FETCH=1: {line}");
    }
}
