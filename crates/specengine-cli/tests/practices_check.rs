//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! the key, the check and the index: the `spec` binary over the scene
//! (`tests/practices_scene`, a scratch git copy of `fixtures/studio-lamp`
//! with the spec's practices) and over scratch copies of `fixtures/spec-a`
//! (no `[card]`: `PLAIN_FLOW`). The fixtures are only read.
//!
//! - DP1: `norm_hash` and `rev` of `PRA-ART-MODELS` equal with and without
//!   `disciplines:`; no `unknown-key`; a closed `[classes]` contract
//!   without it gives `key-extra`; `disciplines: art` (and a mapping, and a
//!   list holding a number) gives `frontmatter-type` and keeps none. M:
//!   counted in `norm_input`; out of `TYPED_KEYS`.
//! - DP2: `["Art", "art", "art", <65 bytes>, 16 more valid]` on spec-a gives
//!   exactly the warnings of entries 1, 3, 4 and 20 (the spec writes "the
//!   21st"; with 20 entries the first past the 16th kept is the 20th, see
//!   the report), the kept list `art` plus the first 15 valid, the exit
//!   unchanged. M: the repeat rule dropped.
//! - DP3: on the scene `ghost.md` gives one warning naming `puppetry`; on
//!   spec-a with the same file, none; with a broken `[card]`, `card-config`
//!   and no `practice-discipline`. M: judging under `PLAIN_FLOW`.
//! - DP4: `INDEX_FORMAT` 19 (21 today, since docs/features/release-tail.md),
//!   the `disciplines` table holds the kept
//!   entries; an index left as a format-18 build leaves it (no table, no
//!   `fields.disciplines`, stamp 18) serves them after this build's first
//!   read, no file touched. M: the format left at 18. The literal form (a
//!   base build writes the index) is `#[ignore]`d: `SPECENGINE_BASE_SPEC=<a
//!   spec built at 690fe65e> cargo nextest run -p specengine-cli --test
//!   practices_check --run-ignored only dp4_`. The history line (`19
//!   <hash>`) is the store's `tests/format.rs`.
//! - DP15, the check's half: `spec check`'s exit with `ghost.md` equals its
//!   exit without it. M: `practice-discipline` raised to an error.
//!
//! Every expected value is worked out from the spec and the scene's files.

#![cfg(unix)]

mod common;
mod practices_scene;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[allow(unused_imports)]
use common::git;
use common::{Run, program_with, spec_with};
use practices_scene::{
    ART, ART_DISCIPLINES, ART_ID, ART_TEXT, GHOST, GHOST_TEXT, PRA_LINE, SHARED_ID, SLUG, Scene,
    line_of, with_ids_line,
};
use serde_json::{Value, json};

/// "Data", the four messages.
fn no_name(entry: usize) -> String {
    format!("disciplines: entry {entry} is no discipline name ([a-z0-9_-], 1 to 64 bytes)")
}
fn repeats(entry: usize, earlier: usize) -> String {
    format!("disciplines: entry {entry} repeats entry {earlier}")
}
fn past(entry: usize) -> String {
    format!("disciplines: entry {entry} is past the 16th kept; a practice keeps 16")
}
fn no_flow(discipline: &str) -> String {
    format!(
        "disciplines: `{discipline}` is in no `[[card.flow]] disciplines`; no card receives this \
         practice"
    )
}

// ------------------------------------------------------------- the runs

/// `spec args` in `cwd` under `home` with the scene's variables.
fn spec_at(scene: &Scene, cwd: &Path, home: &Path, args: &[&str]) -> Run {
    let vars = scene.vars_with(home);
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

fn spec(scene: &Scene, args: &[&str]) -> Run {
    spec_at(scene, &scene.root, &scene.home, args)
}

/// `spec check --json` in `cwd`: exit 0 or 1.
fn check_at(scene: &Scene, cwd: &Path, home: &Path) -> Run {
    let run = spec_at(scene, cwd, home, &["check", "--json"]);
    assert!(run.code == 0 || run.code == 1, "{}", run.show());
    run
}

fn check(scene: &Scene) -> Run {
    check_at(scene, &scene.root, &scene.home)
}

/// The `check --json` findings of `code`.
fn findings(run: &Run, code: &str) -> Vec<Value> {
    run.json()["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| finding["code"] == code)
        .cloned()
        .collect()
}

/// The findings of `code` about `path`.
fn findings_at(run: &Run, code: &str, path: &str) -> Vec<Value> {
    findings(run, code)
        .into_iter()
        .filter(|finding| finding["path"] == path)
        .collect()
}

/// `spec show <id> --json`'s one node in `cwd`.
fn shown_at(scene: &Scene, cwd: &Path, home: &Path, id: &str) -> Value {
    let run = spec_at(scene, cwd, home, &["show", id, "--json"]);
    run.code(0);
    let document = run.json();
    assert_eq!(
        document["nodes"].as_array().map(Vec::len),
        Some(1),
        "{document}"
    );
    document["nodes"][0].clone()
}

fn shown(scene: &Scene, id: &str) -> Value {
    shown_at(scene, &scene.root, &scene.home, id)
}

/// `spec card show <card> --practices --json`'s practices' names.
fn practice_names(scene: &Scene, card: &str) -> Vec<String> {
    let run = spec(scene, &["card", "show", card, "--practices", "--json"]);
    run.code(0);
    run.json()["practices"]
        .as_array()
        .unwrap_or_else(|| panic!("practices: {}", run.show()))
        .iter()
        .map(|entry| entry["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// A scratch copy of spec-a in the scene's scratch, `PRA` declared, its
/// its practices' directory made; its root and a `HOME` of its own.
fn spec_a_copy(scene: &Scene, name: &str) -> (PathBuf, PathBuf) {
    let root = scene.dir(name);
    common::copy_dir(&common::fixture("spec-a"), &root);
    let config = fs::read_to_string(root.join("specengine.toml")).expect("spec-a's config");
    let config = with_ids_line(&config, PRA_LINE);
    fs::write(root.join("specengine.toml"), config).expect("the config");
    fs::create_dir_all(root.join("docs/spec/practices")).expect("the practices' directory");
    let home = scene.dir(&format!("{name}-home"));
    (root, home)
}

// ------------------------------------------------------------------ DP1

/// DP1: on the scene and on its twin whose `art.md` lacks the line,
/// `PRA-ART-MODELS`'s `norm_hash` and `rev` are equal, and so are each of
/// its rules'; the scene checks with no `unknown-key` for `disciplines`.
/// M: counted in `norm_input`; out of `TYPED_KEYS`.
#[test]
fn dp1_disciplines_is_typed_and_never_counted() {
    let scene = Scene::files("dp1", false);
    let plain = Scene::files("dp1-plain", false);
    plain.edit(ART, ART_DISCIPLINES, "");
    for id in [ART_ID, "PRA-ART-REALISM", "PRA-ART-LOD"] {
        let with = shown(&scene, id);
        let without = shown(&plain, id);
        assert!(with["norm_hash"].is_string(), "premise: hashed: {with}");
        assert_eq!(
            with["norm_hash"], without["norm_hash"],
            "{id}: M: disciplines counted"
        );
        assert_eq!(with["rev"], without["rev"], "{id}");
    }
    let run = check(&scene);
    let unknown: Vec<Value> = findings(&run, "unknown-key")
        .into_iter()
        .filter(|finding| finding["subject"] == "disciplines")
        .collect();
    assert_eq!(unknown, Vec::<Value>::new(), "M: disciplines untyped");
    assert_eq!(
        findings_at(&run, "practice-discipline", ART),
        Vec::<Value>::new(),
        "art is a flow's discipline: {}",
        run.show()
    );
}

/// DP1: a closed canon contract that does not list `disciplines` reads it
/// as any other key: `key-extra`, subject `disciplines`, at `art.md`'s line.
#[test]
fn dp1_a_closed_contract_reads_disciplines_as_a_key() {
    let scene = Scene::files("dp1-closed", false);
    scene.append(
        "specengine.toml",
        "\n[classes]\ncanon = { required = [\"class\"], optional = [\"id\", \"kind\", \"tier\", \
         \"title\", \"parent\", \"status\", \"owner\", \"reviewed\", \"links\", \"aliases\", \
         \"scope\", \"tags\"], closed = true }\n",
    );
    let run = check(&scene);
    let extra: Vec<Value> = findings_at(&run, "key-extra", ART)
        .into_iter()
        .filter(|finding| finding["subject"] == "disciplines")
        .collect();
    assert_eq!(extra.len(), 1, "{}", run.show());
    assert_eq!(extra[0]["line"], line_of(ART_TEXT, "disciplines: [art]"));
}

/// DP1: `disciplines: art` (a bare string), a mapping and a list holding a
/// number: `frontmatter-type` subject `disciplines` at the key's line, and
/// `art.md` keeps none, so `C-0001`'s practices are `shared.md` alone.
#[test]
fn dp1_a_mistyped_value_keeps_no_discipline() {
    let scene = Scene::new("dp1-mistyped");
    assert_eq!(
        practice_names(&scene, "C-0001"),
        [ART_ID, SHARED_ID],
        "premise"
    );
    let mut written = ART_DISCIPLINES.to_owned();
    for wrong in [
        "disciplines: art\n",
        "disciplines: {a: art}\n",
        "disciplines: [art, 3]\n",
    ] {
        scene.edit(ART, &written, wrong);
        written = wrong.to_owned();
        let run = check(&scene);
        let mistyped: Vec<Value> = findings_at(&run, "frontmatter-type", ART)
            .into_iter()
            .filter(|finding| finding["subject"] == "disciplines")
            .collect();
        assert_eq!(mistyped.len(), 1, "{wrong:?}: {}", run.show());
        assert_eq!(
            mistyped[0]["line"],
            line_of(ART_TEXT, "disciplines: [art]"),
            "{wrong:?}"
        );
        assert_eq!(
            practice_names(&scene, "C-0001"),
            [SHARED_ID],
            "{wrong:?}: a mistyped key keeps no discipline"
        );
    }
}

// ------------------------------------------------------------------ DP2

/// DP2: on spec-a (no `[card]`, so no entry is judged against a flow) a
/// practice whose entries are `["Art", "art", "art", <65 bytes>, d01 …
/// d16]`: exactly four `practice-discipline` warnings, at the key's line,
/// for entries 1, 3, 4 and 20; the library row keeps `art`, d01 … d15;
/// `spec check`'s exit as without the document. M: the repeat rule
/// dropped (entry 3 kept, so d15 is past the cap and the kept list moves).
#[test]
fn dp2_entry_warnings() {
    let scene = Scene::files("dp2", false);
    let (root, home) = spec_a_copy(&scene, "spec-a");
    let base_exit = check_at(&scene, &root, &home).code;
    let long = "a".repeat(65);
    let valid: Vec<String> = (1..=16).map(|n| format!("d{n:02}")).collect();
    let mut entries = vec![
        "\"Art\"".to_owned(),
        "art".to_owned(),
        "art".to_owned(),
        format!("\"{long}\""),
    ];
    entries.extend(valid.iter().cloned());
    assert_eq!(entries.len(), 20);
    let path = "docs/spec/practices/caps.md";
    let text = format!(
        "---\nid: PRA-CAPS\nclass: canon\ntier: 2\nowner: owner\nreviewed: 2026-11-20\n\
         disciplines: [{}]\n---\n\n# Caps\n\nOne practice with many entries.\n",
        entries.join(", ")
    );
    let key_line = 7;
    assert!(
        text.lines()
            .nth(key_line - 1)
            .is_some_and(|line| line.starts_with("disciplines: [")),
        "premise: the key's line"
    );
    fs::write(root.join(path), &text).expect("the practice");
    let run = check_at(&scene, &root, &home);
    assert_eq!(
        run.code,
        base_exit,
        "a warning changes no exit: {}",
        run.show()
    );
    let found = findings(&run, "practice-discipline");
    let mut messages: Vec<String> = found
        .iter()
        .map(|finding| {
            assert_eq!(finding["severity"], "warning", "{finding}");
            assert_eq!(finding["path"], path, "{finding}");
            assert_eq!(finding["line"], json!(key_line), "{finding}");
            assert_eq!(finding["subject"], "disciplines", "{finding}");
            finding["message"].as_str().expect("a message").to_owned()
        })
        .collect();
    messages.sort();
    let mut want = vec![no_name(1), repeats(3, 2), no_name(4), past(20)];
    want.sort();
    assert_eq!(messages, want, "M: the repeat rule dropped: {}", run.show());

    // The kept list, as the library row serves it.
    let library = spec_at(&scene, &root, &home, &["library", "--json"]);
    library.code(0);
    let document = library.json();
    let row = document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["path"] == path)
        .unwrap_or_else(|| panic!("no row {path}: {document}"))
        .clone();
    let mut kept = vec![json!("art")];
    kept.extend(valid[..15].iter().map(|name| json!(name)));
    assert_eq!(row["disciplines"], Value::Array(kept), "{row}");
}

// ------------------------------------------------------------------ DP3

/// DP3: on the scene, one `practice-discipline` warning, at `ghost.md`'s
/// key line, naming `puppetry`, and no other of the code. M: an unlisted
/// discipline dropped instead of kept (the library row).
#[test]
fn dp3_a_discipline_no_flow_lists_is_one_warning() {
    let scene = Scene::files("dp3", false);
    let run = check(&scene);
    let found = findings(&run, "practice-discipline");
    assert_eq!(found.len(), 1, "{}", run.show());
    assert_eq!(found[0]["path"], GHOST);
    assert_eq!(found[0]["severity"], "warning");
    assert_eq!(found[0]["subject"], "disciplines");
    assert_eq!(
        found[0]["line"],
        line_of(GHOST_TEXT, "disciplines: [puppetry]")
    );
    assert_eq!(found[0]["message"], no_flow("puppetry"));
    // An unlisted discipline is still kept.
    let library = spec(&scene, &["library", "--json"]);
    library.code(0);
    let document = library.json();
    let ghost = document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["path"] == GHOST)
        .unwrap_or_else(|| panic!("no ghost row: {document}"))
        .clone();
    assert_eq!(ghost["disciplines"], json!(["puppetry"]));
}

/// DP3: the same `ghost.md` on spec-a (`PLAIN_FLOW`): walked (it shows),
/// no `practice-discipline`. M: judging under `PLAIN_FLOW`.
#[test]
fn dp3_under_the_plain_flow_nothing_is_judged() {
    let scene = Scene::files("dp3-plain", false);
    let (root, home) = spec_a_copy(&scene, "spec-a");
    fs::create_dir_all(root.join(GHOST).parent().expect("a parent")).expect("a directory");
    fs::write(root.join(GHOST), GHOST_TEXT).expect("ghost.md");
    let node = shown_at(&scene, &root, &home, "PRA-GHOST");
    assert_eq!(node["path"], GHOST, "premise: walked");
    let run = check_at(&scene, &root, &home);
    assert_eq!(
        findings(&run, "practice-discipline"),
        Vec::<Value>::new(),
        "M: judged under PLAIN_FLOW: {}",
        run.show()
    );
}

/// DP3: the scene with a broken `[card]` (development's `return = 7`):
/// `card-config` is given, and no `practice-discipline` (ghost's entry is
/// no grammar, repeat or cap problem, so nothing).
#[test]
fn dp3_a_broken_card_table_judges_no_flow() {
    let scene = Scene::files("dp3-broken", false);
    scene.edit(
        "specengine.toml",
        "accept = { from = \"verify\", to = \"done\" }\nreturn = \"sent-back\"\n",
        "accept = { from = \"verify\", to = \"done\" }\nreturn = 7\n",
    );
    let run = check(&scene);
    assert!(
        !findings(&run, "card-config").is_empty(),
        "premise: the card table does not read: {}",
        run.show()
    );
    assert_eq!(
        findings(&run, "practice-discipline"),
        Vec::<Value>::new(),
        "{}",
        run.show()
    );
}

/// DP3, "Rules and edge cases": with `[card]` broken the grammar, repeat
/// and cap warnings are still given.
#[test]
fn dp3_a_broken_card_table_keeps_the_entry_warnings() {
    let scene = Scene::files("dp3-broken-entries", false);
    scene.edit(
        "specengine.toml",
        "accept = { from = \"verify\", to = \"done\" }\nreturn = \"sent-back\"\n",
        "accept = { from = \"verify\", to = \"done\" }\nreturn = 7\n",
    );
    scene.edit(
        GHOST,
        "disciplines: [puppetry]\n",
        "disciplines: [puppetry, Bad, puppetry]\n",
    );
    let run = check(&scene);
    let mut messages: Vec<String> = findings(&run, "practice-discipline")
        .iter()
        .map(|finding| finding["message"].as_str().expect("a message").to_owned())
        .collect();
    messages.sort();
    let mut want = vec![no_name(2), repeats(3, 1)];
    want.sort();
    assert_eq!(messages, want, "{}", run.show());
}

// ------------------------------------------------------------- DP15, check

/// DP15: `spec check`'s exit with `ghost.md` equals its exit without it,
/// on a scene whose only finding apart is ghost's. M: `practice-discipline`
/// raised to an error (exit 1 with it).
#[test]
fn dp15_the_warning_changes_no_exit() {
    let with = Scene::files("dp15-with", false);
    let without = Scene::files("dp15-without", false);
    without.remove(GHOST);
    let a = check(&with);
    let b = check(&without);
    assert_eq!(
        findings(&a, "practice-discipline").len(),
        1,
        "premise: {}",
        a.show()
    );
    assert_eq!(
        a.code,
        b.code,
        "M: raised to an error:\n{}\n{}",
        a.show(),
        b.show()
    );
    assert_eq!(
        a.json()["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .filter(|finding| finding["severity"] == "error")
            .count(),
        b.json()["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .filter(|finding| finding["severity"] == "error")
            .count(),
        "no error more"
    );
}

// ------------------------------------------------------------------ DP4

/// `sqlite3 db statements`; its stdout.
fn sql(db: &Path, statements: &str) -> String {
    let output = Command::new(common::proposal::sqlite3())
        .env_clear()
        .args(["-cmd", ".timeout 5000"])
        .arg(db)
        .arg(statements)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {statements}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 sqlite3 output")
}

fn index_db(home: &Path) -> PathBuf {
    common::data_dir(home).join(format!("{SLUG}.db"))
}

/// The `disciplines` table's rows `path|discipline`, by path then `ord`.
const ROWS: &str = "SELECT f.path, d.discipline FROM disciplines d JOIN files f ON f.file_id = \
                    d.file_id ORDER BY f.path, d.ord;";

/// What the scene's files keep: every file's entries (a Tier 3 file's too:
/// `update` writes every file's), by path, in written order.
const KEPT: &str = "docs/spec/practices/art/2025-photoreal.md|art\n\
                    docs/spec/practices/art/models.md|art\n\
                    docs/spec/practices/development/code.md|development\n\
                    docs/spec/practices/puppetry/ghost.md|puppetry\n\
                    docs/spec/practices/shared/naming.md|art\n\
                    docs/spec/practices/shared/naming.md|development\n";

/// DP4: `INDEX_FORMAT` 19 (20 since docs/features/specengine-guide.md, the
/// typed `summarises`; 21 since docs/features/release-tail.md, rule (7)'s
/// citation tail); after `spec index` the table holds [`KEPT`], each file's
/// `ord` distinct.
#[test]
fn dp4_the_table_holds_the_kept_entries() {
    assert_eq!(
        specengine_store::INDEX_FORMAT,
        21,
        "M: the format left at 18"
    );
    let scene = Scene::files("dp4-table", false);
    spec(&scene, &["index"]).code(0);
    let db = index_db(&scene.home);
    assert_eq!(sql(&db, ROWS), KEPT);
    assert_eq!(
        sql(
            &db,
            "SELECT COUNT(*) FROM (SELECT file_id, ord FROM disciplines GROUP BY file_id, ord);"
        ),
        "6\n",
        "the primary key holds"
    );
    assert_eq!(
        sql(&db, "SELECT value FROM index_meta WHERE key = 'format';"),
        "21\n",
        "the stored stamp is the build's (20 since specengine-guide, 21 since release-tail)"
    );
}

/// What a read and a check serve after the format step: `C-0001`'s
/// practices, the table's rows, the stamp this build's.
fn assert_read_again(scene: &Scene) {
    assert_eq!(
        practice_names(scene, "C-0001"),
        [ART_ID, SHARED_ID],
        "M: INDEX_FORMAT left at 18: the old index is trusted"
    );
    let db = index_db(&scene.home);
    assert_eq!(sql(&db, ROWS), KEPT, "the rows are written again");
    assert_eq!(
        sql(&db, "SELECT value FROM index_meta WHERE key = 'format';"),
        format!("{}\n", specengine_store::INDEX_FORMAT),
        "restamped"
    );
    let unknown: Vec<Value> = findings(&check(scene), "unknown-key")
        .into_iter()
        .filter(|finding| finding["subject"] == "disciplines")
        .collect();
    assert_eq!(unknown, Vec::<Value>::new());
}

/// DP4: an index of this build made what a format-18 build leaves over the
/// same tree (no `disciplines` table, no `fields.disciplines` in any node,
/// the stamp 18), no file touched: the next read serves the practices. M: the format left at 18
/// (the stamp matches, the missing table is trusted).
#[test]
fn dp4_a_format_18_index_is_read_again() {
    let scene = Scene::new("dp4-step");
    spec(&scene, &["index"]).code(0);
    let db = index_db(&scene.home);
    let before = common::snapshot(&scene.root);
    sql(
        &db,
        "DROP TABLE disciplines; \
         UPDATE nodes SET node = json_remove(node, '$.fields.disciplines') WHERE ord = 0; \
         UPDATE index_meta SET value = '18' WHERE key = 'format';",
    );
    assert_eq!(
        sql(
            &db,
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'disciplines';"
        ),
        "0\n",
        "premise: no table"
    );
    assert_read_again(&scene);
    assert_eq!(common::snapshot(&scene.root), before, "no file touched");
}

/// DP4, the literal form: the base commit's `spec index` writes a format-18
/// index over the scene; this build's read serves the practices. Needs the
/// base build: `SPECENGINE_BASE_SPEC=<a spec built at 690fe65e> cargo
/// nextest run -p specengine-cli --test practices_check --run-ignored only
/// dp4_`.
#[test]
#[ignore = "needs SPECENGINE_BASE_SPEC, a `spec` built at the base commit"]
fn dp4_the_base_builds_index_is_read_again() {
    let base = std::env::var_os("SPECENGINE_BASE_SPEC")
        .expect("SPECENGINE_BASE_SPEC names the base commit's `spec`");
    let scene = Scene::new("dp4-base");
    let vars = scene.vars();
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    let run = program_with(&base, &scene.root, &["index"], &env);
    run.code(0);
    assert_eq!(
        sql(
            &index_db(&scene.home),
            "SELECT value FROM index_meta WHERE key = 'format';"
        ),
        "18\n",
        "premise: the base build writes format 18"
    );
    assert_read_again(&scene);
}

/// "Data" (the orchestrator's ruling on the developer's iteration 2): a
/// flow that lists no discipline takes any, so with one such flow declared
/// no `practice-discipline` "no flow" warning is given; `card-config` reads
/// (no error); the grammar warnings still stand. M: `declared_disciplines`
/// ignoring the open flow (ghost's `puppetry` warned).
#[test]
fn an_open_flow_judges_no_discipline() {
    let scene = Scene::files("dp3-open", false);
    scene.append(
        "specengine.toml",
        "\n[[card.flow]]\nname = \"open\"\ncolumns = [\"todo\", \"doing\", \"done\"]\naccept = \
         { from = \"doing\", to = \"done\" }\n",
    );
    let run = check(&scene);
    assert_eq!(
        findings(&run, "card-config"),
        Vec::<Value>::new(),
        "premise: the open flow reads: {}",
        run.show()
    );
    assert_eq!(
        findings(&run, "practice-discipline"),
        Vec::<Value>::new(),
        "M: the open flow ignored: {}",
        run.show()
    );
    scene.edit(
        GHOST,
        "disciplines: [puppetry]\n",
        "disciplines: [puppetry, Bad]\n",
    );
    let messages: Vec<String> = findings(&check(&scene), "practice-discipline")
        .iter()
        .map(|finding| finding["message"].as_str().expect("a message").to_owned())
        .collect();
    assert_eq!(messages, [no_name(2)], "the grammar still judged");
}
