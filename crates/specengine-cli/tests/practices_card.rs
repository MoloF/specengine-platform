//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! the reads that carry practices and citations, through the `spec`
//! binary over the scene (`tests/practices_scene`: a scratch git copy of
//! `fixtures/studio-lamp` with the practices, its queue made through the
//! library at fixed clocks). The fixture is only read.
//!
//! - DP9: `card show C-0001 --practices --json`: `practices` art.md then
//!   shared.md (`rules` 3, 1), `cited` `PRA-ART-LOD` alone with its title
//!   and path, the 38 card keys in order, each entry's keys in order; the
//!   text's `practices:` and `cited:` lines; plain `show` serves both
//!   `null`, prints neither line and runs with every `.md` and the
//!   practices' directory unreadable; C-0003 `practices` `[]`. M:
//!   unresolved `PRA-NOPE-X` served; practices filled on plain `show`.
//!   The scan's reach (title, goal, criteria, an item's text, a history
//!   note only with `--history`), first-seen order, and the first holder
//!   of an ID held twice.
//! - DP10: `--context`: `context.bundle` equals `spec bundle SCN-ARRIVAL
//!   --budget N --discipline art --json` as a value (default and 3000);
//!   `--context` implies the practices; the two lines print right before
//!   `context.parent:` and count in `U` (a card whose lines and citations
//!   leave a room the practices' bundle overfills stays within 40 000). M:
//!   the bundle asked without the discipline; the lines not counted.
//! - DP11, the card's caps: 40 practices of `art` list 32 by path and one
//!   note naming the 33rd's path; a goal citing 80 distinct resolvable IDs
//!   gives 64 `cited`, the first seen. M: no `PRACTICES_MAX`.
//! - DP12: `task show T-0001 --json`: 28 keys (29 since
//!   docs/features/comments.md, `comments`; 30 since
//!   docs/features/work-references.md, `references`), `practices` art.md, dev.md,
//!   shared.md once each by path; `bundle.disciplines` `["art",
//!   "development"]` and its hash `spec bundle`'s with both; the brief's
//!   Practices section before Bundle; T-0002 `[]` and `[]`;
//!   `schema_version` 1. M: a task no card lists given every practice.
//! - DP13: a proposal whose rationale cites `PRA-ART-NORMALS` and
//!   `PRA-NOPE-X`: `review --json`'s last key `cited` holds the first
//!   alone. (MCP's `get_proposal` and the daemon's route: their crates'
//!   `tests/practices.rs`.)
//!
//! Every expected value is worked out from the spec and the scene's files;
//! a bundle compared is `spec bundle`'s own (not the read under test).
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod practices_scene;
mod task_common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

#[allow(unused_imports)]
use common::git;
use common::{Run, md_files, spec_with};
use practices_scene::{
    ART, ART_ID, ART_TITLE, C1_CRITERION, C1_GOAL, C1_TITLE, CLOCK, DEV, DEV_ID, DEV_STATE,
    DEV_TITLE, LOD, LOD_TITLE, NAMING, NORMALS, RAISED, REALISM, SHARED, SHARED_ID, SHARED_TITLE,
    Scene, practice_text, prose,
};
use serde_json::{Value, json};
use specengine_cli::{
    CardMoveRequest, Exit, Globals, IntakeOutcome, QuestionRequest, card_move, propose_question,
};
use task_common::{Ordered, PACKAGE_KEYS};

/// The card document's 38 keys in order (docs/canon/cards.md with
/// `practices` and `cited` after `context`, "The card").
const CARD_KEYS: [&str; 38] = [
    "id",
    "flow",
    "column",
    "position",
    "title",
    "goal",
    "discipline",
    "targets",
    "mark",
    "on",
    "discrepancy",
    "correction",
    "upstream",
    "snapshot",
    "criteria",
    "assignee",
    "reviewer",
    "mandate",
    "estimate",
    "due",
    "parent",
    "corrects",
    "evidence",
    "tasks",
    "items",
    "subtask_items",
    "acceptance",
    "cancellation",
    "created_by",
    "created_at",
    "updated_at",
    "revision",
    "allowed",
    "history",
    "context",
    "practices",
    "cited",
    "notes",
];
/// A practice entry's keys ("The card").
const PRACTICE_KEYS: [&str; 5] = ["name", "title", "path", "disciplines", "rules"];
/// A citation's keys ("The card").
const CITED_KEYS: [&str; 4] = ["written", "name", "title", "path"];
/// The ceiling of a read's text (`OUTPUT_CAP_CHARS`).
const CEILING: usize = 40_000;

// ------------------------------------------------------------- the runs

fn spec_at(scene: &Scene, cwd: &Path, args: &[&str]) -> Run {
    let vars = scene.vars();
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

fn spec(scene: &Scene, args: &[&str]) -> Run {
    spec_at(scene, &scene.root, args)
}

/// `spec --json args`, exit 0: the document and its raw line.
fn doc(scene: &Scene, args: &[&str]) -> (Value, String) {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let run = spec(scene, &all);
    run.code(0);
    (run.json(), run.stdout)
}

/// `card show <card> <flags>`: the text run (exit 0), the document, the raw
/// JSON line.
fn card(scene: &Scene, card: &str, flags: &[&str]) -> (Run, Value, String) {
    let mut args = vec!["card", "show", card];
    args.extend_from_slice(flags);
    let text = spec(scene, &args);
    text.code(0);
    let (document, raw) = doc(scene, &args);
    (text, document, raw)
}

fn practice(name: &str, title: &str, path: &str, disciplines: &[&str], rules: u64) -> Value {
    json!({"name": name, "title": title, "path": path, "disciplines": disciplines, "rules": rules})
}

fn cited(id: &str, title: &str, path: &str) -> Value {
    json!({"written": id, "name": id, "title": title, "path": path})
}

/// The scene's two practices of `art`.
fn art_practices() -> Value {
    json!([
        practice(ART_ID, ART_TITLE, ART, &["art"], 3),
        practice(SHARED_ID, SHARED_TITLE, SHARED, &["art", "development"], 1),
    ])
}

/// The key orders of a card document and of each practice and citation.
fn assert_card_keys(raw: &str) {
    let ordered = Ordered::of(raw);
    assert_eq!(ordered.keys(), CARD_KEYS, "{raw}");
    for (key, want) in [
        ("practices", &PRACTICE_KEYS[..]),
        ("cited", &CITED_KEYS[..]),
    ] {
        if let Ordered::Array(items) = ordered.get(key) {
            for item in items {
                assert_eq!(item.keys(), want, "{key}: {raw}");
            }
        }
    }
}

/// The `practices:` line and the `cited:` lines of a text, and the line
/// right after the last of them.
fn practice_lines(text: &str) -> (Vec<String>, Option<String>) {
    let lines: Vec<&str> = text.lines().collect();
    let Some(first) = lines
        .iter()
        .position(|line| line.starts_with("practices: "))
    else {
        return (Vec::new(), None);
    };
    let mut end = first + 1;
    while end < lines.len() && lines[end].starts_with("cited: ") {
        end += 1;
    }
    (
        lines[first..end]
            .iter()
            .map(|line| (*line).to_owned())
            .collect(),
        lines.get(end).map(|line| (*line).to_owned()),
    )
}

/// The stored item's ID of an intake raise.
fn stored(outcome: &IntakeOutcome) -> String {
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

/// A question on `SCN-ARRIVAL` for `card`, its text `text`, at [`RAISED`],
/// declared distinct from whatever it first hits: its ID.
fn question(scene: &Scene, card: &str, text: &str) -> String {
    let mut request = QuestionRequest {
        node_ids: vec!["SCN-ARRIVAL".to_owned()],
        text: text.to_owned(),
        working_answer: "Yes, as drawn.".to_owned(),
        price_of_other: "A model redone.".to_owned(),
        severity: None,
        distinct_from: Vec::new(),
        card: Some(card.to_owned()),
        author_role: Some("modeller".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        login: None,
        now: RAISED.to_owned(),
        git: scene.git_env(),
        gist: None,
        matters: None,
        options: None,
        recommendation: None,
    };
    let ask = |request: &QuestionRequest| {
        propose_question(&scene.env(), &Globals::default(), request).expect("a question")
    };
    let first = ask(&request);
    if first.document.created {
        return stored(&first);
    }
    request.distinct_from = first
        .document
        .hits
        .iter()
        .map(|hit| hit.name().to_owned())
        .collect();
    stored(&ask(&request))
}

/// `card move <card> --to <column> [--note <note>]` at [`CLOCK`].
fn move_card(scene: &Scene, card: &str, column: &str, note: Option<&str>) {
    let outcome = card_move(
        &scene.env(),
        &Globals::default(),
        &CardMoveRequest {
            id: card.to_owned(),
            to: Some(column.to_owned()),
            before: None,
            note: note.map(str::to_owned),
            revision: None,
            actor: None,
            now: CLOCK.to_owned(),
        },
    )
    .unwrap_or_else(|error| panic!("card move {card}: {error}"));
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "card move {card}: {outcome:?}"
    );
}

// ------------------------------------------------------------------ DP9

/// DP9: C-0001's practices and citations, key orders, the text's lines
/// right before the context's line. M: unresolved `PRA-NOPE-X` served.
#[test]
fn dp9_the_cards_practices_and_cited() {
    let scene = Scene::new("dp9");
    let (text, document, raw) = card(&scene, "C-0001", &["--practices"]);
    assert_card_keys(&raw);
    assert_eq!(document["practices"], art_practices(), "{raw}");
    assert_eq!(
        document["cited"],
        json!([cited(LOD, LOD_TITLE, ART)]),
        "M: an unresolved ID served: {raw}"
    );
    assert_eq!(
        document["context"],
        Value::Null,
        "--practices asks no context"
    );
    let (lines, after) = practice_lines(&text.stdout);
    assert_eq!(
        lines,
        [
            format!("practices: {ART_ID}, {SHARED_ID}"),
            format!("cited: {LOD} | {LOD_TITLE} | {ART}"),
        ],
        "{}",
        text.stdout
    );
    assert!(
        after
            .as_deref()
            .is_some_and(|line| line.starts_with("context")),
        "the lines stand right before the context's: {}",
        text.stdout
    );

    // C-0003 (`sound`): no practice, nothing cited.
    let (text, document, raw) = card(&scene, "C-0003", &["--practices"]);
    assert_card_keys(&raw);
    assert_eq!(document["practices"], json!([]));
    assert_eq!(document["cited"], json!([]));
    let (lines, _) = practice_lines(&text.stdout);
    assert_eq!(lines, ["practices: -", "cited: -"], "{}", text.stdout);
}

/// DP9: plain `show` serves `practices` and `cited` `null` and prints
/// neither line, byte for byte as before the files went unreadable: with
/// every `.md` of the root and the practices' directory at mode 000 it
/// exits 0 with nothing on stderr. The card is an `art` card citing a
/// rule that links no task (a linked task's state is read through the
/// index by card-tasks' rule, which this slice leaves as it is). M:
/// practices filled on plain `show`.
#[test]
fn dp9_plain_show_reads_no_index() {
    let scene = Scene::new("dp9-plain");
    let id = scene.card(
        "Model the lamp",
        "art",
        &["SCN-ARRIVAL"],
        Some(C1_GOAL),
        &[],
        None,
    );
    let (asked, _, _) = card(&scene, &id, &["--practices"]);
    assert!(
        asked
            .stdout
            .contains("practices: PRA-ART-MODELS, PRA-SHARED"),
        "premise: the card has practices: {}",
        asked.stdout
    );
    let (before, document, raw) = card(&scene, &id, &[]);
    assert_eq!(before.stderr, "", "{}", before.show());
    assert_card_keys(&raw);
    assert_eq!(document["practices"], Value::Null);
    assert_eq!(document["cited"], Value::Null);
    assert!(
        !before
            .stdout
            .lines()
            .any(|line| line.starts_with("practices:") || line.starts_with("cited:")),
        "{}",
        before.stdout
    );
    let files = md_files(&scene.root);
    assert!(files.len() >= 10, "{files:?}");
    for file in &files {
        fs::set_permissions(scene.path(file), fs::Permissions::from_mode(0o000))
            .expect("chmod 000");
    }
    fs::set_permissions(
        scene.path(practices_scene::PRACTICES_DIR),
        fs::Permissions::from_mode(0o000),
    )
    .expect("chmod 000");
    let plain = spec(&scene, &["card", "show", &id]);
    plain.code(0);
    assert_eq!(plain.stderr, "", "M: practices filled on plain show");
    assert_eq!(plain.stdout, before.stdout);
    let json = spec(&scene, &["--json", "card", "show", &id]);
    json.code(0);
    assert_eq!(json.stderr, "");
    assert_eq!(json.stdout, raw);
    fs::set_permissions(
        scene.path(practices_scene::PRACTICES_DIR),
        fs::Permissions::from_mode(0o755),
    )
    .expect("restore");
}

/// "The card": the scan covers the title, the goal, each criterion, each
/// item's served text and, with `--history`, each history note, in that
/// first-seen order; an ID cited twice is listed once; an unresolved one
/// is left out.
#[test]
fn cited_covers_the_cards_texts_in_first_seen_order() {
    let scene = Scene::new("dp9-reach");
    let id = scene.card(
        "Model the hull to PRA-ART-NORMALS",
        "art",
        &["SCN-ARRIVAL"],
        Some("Keep PRA-ART-LOD, PRA-NOPE-X and PRA-ART-NORMALS in mind."),
        &["Meets PRA-ART-REALISM."],
        None,
    );
    question(&scene, &id, "Does PRA-DEV-STATE bind the hull's script?");
    move_card(
        &scene,
        &id,
        "sent-back",
        Some("Back: the names break PRA-SHARED-NAMING."),
    );
    let (_, document, raw) = card(&scene, &id, &["--practices"]);
    assert_card_keys(&raw);
    assert_eq!(
        document["cited"],
        json!([
            cited(NORMALS, "Normal maps", ART),
            cited(LOD, LOD_TITLE, ART),
            cited(REALISM, "Realism", ART),
            cited(DEV_STATE, "No hidden global state", DEV),
        ]),
        "{raw}"
    );
    let (_, document, _) = card(&scene, &id, &["--practices", "--history"]);
    assert_eq!(
        document["cited"],
        json!([
            cited(NORMALS, "Normal maps", ART),
            cited(LOD, LOD_TITLE, ART),
            cited(REALISM, "Realism", ART),
            cited(DEV_STATE, "No hidden global state", DEV),
            cited(NAMING, "Asset names", SHARED),
        ]),
        "a history note is scanned with --history alone"
    );
}

/// "Rules and edge cases": an ID two documents hold names its first holder,
/// as `spec show` resolves a REF.
#[test]
fn a_cited_id_held_twice_names_its_first_holder() {
    let scene = Scene::new("dp9-twice");
    let copy = "docs/spec/practices/aa-copy.md";
    scene.put(
        copy,
        "---\nclass: canon\nowner: owner\nreviewed: 2026-11-20\n---\n\n# A copy\n\n\
         ## Copied detail {#PRA-ART-LOD}\n\nA second holder.\n",
    );
    let shown = spec(&scene, &["--json", "show", LOD]);
    let first = shown.json()["nodes"][0].clone();
    assert_eq!(first["path"], copy, "premise: show's first holder: {first}");
    let (_, document, _) = card(&scene, "C-0001", &["--practices"]);
    assert_eq!(
        document["cited"],
        json!([cited(LOD, "Copied detail", copy)]),
        "{document}"
    );
}

// ----------------------------------------------------------------- DP10

/// `spec bundle <refs> --budget N --discipline D… --json`'s document.
fn bundled(scene: &Scene, refs: &[&str], budget: &str, disciplines: &[&str]) -> Value {
    let mut args = vec!["bundle"];
    args.extend_from_slice(refs);
    args.extend_from_slice(&["--budget", budget]);
    for discipline in disciplines {
        args.extend(["--discipline", discipline]);
    }
    doc(scene, &args).0
}

/// The text up to and including the `context.bundle:` line, and what
/// follows it.
fn split_at_bundle(text: &str) -> (&str, &str) {
    let at = text
        .find("\ncontext.bundle: ")
        .unwrap_or_else(|| panic!("a context.bundle line: {text}"));
    let end = at + 1 + text[at + 1..].find('\n').expect("the line ends") + 1;
    (&text[..end], &text[end..])
}

/// DP10: `--context` (default budget, then 3000): `context.bundle` is
/// `spec bundle SCN-ARRIVAL --budget N --discipline art --json` as a value,
/// layer 10 filled; `practices` and `cited` as `--practices` gives them;
/// the two lines right before `context.parent:`. M: the bundle asked
/// without the discipline.
#[test]
fn dp10_the_context_bundles_the_cards_discipline() {
    let scene = Scene::new("dp10");
    for (flags, budget) in [
        (&["--context"][..], "10000"),
        (&["--context", "--budget", "3000"][..], "3000"),
    ] {
        let (text, document, raw) = card(&scene, "C-0001", flags);
        assert_card_keys(&raw);
        let want = bundled(&scene, &["SCN-ARRIVAL"], budget, &["art"]);
        assert_eq!(
            document["context"]["bundle"], want,
            "{flags:?}: M: the bundle asked without the discipline"
        );
        assert!(
            !want["layers"]["practices"]
                .as_array()
                .expect("practices")
                .is_empty(),
            "premise: layer 10 filled at {budget}"
        );
        assert_eq!(
            document["practices"],
            art_practices(),
            "--context implies it"
        );
        assert_eq!(document["cited"], json!([cited(LOD, LOD_TITLE, ART)]));
        let lines: Vec<&str> = text.stdout.lines().collect();
        let parent = lines
            .iter()
            .position(|line| line.starts_with("context.parent:"))
            .unwrap_or_else(|| panic!("{}", text.stdout));
        assert_eq!(
            lines[parent - 2..parent],
            [
                format!("practices: {ART_ID}, {SHARED_ID}"),
                format!("cited: {LOD} | {LOD_TITLE} | {ART}"),
            ],
            "{}",
            text.stdout
        );
    }
}

/// About `bytes` bytes of a criterion naming no ID.
fn criteria(n: usize, bytes: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let mut line = format!("Criterion {i}: the lamp stays lit");
            while line.len() < bytes {
                line.push_str(" and steady");
            }
            line.truncate(bytes);
            line
        })
        .collect()
}

/// DP10: the lines count in `U`. A `sound` card with 25 criteria of 1 000
/// bytes and a goal citing 64 rules of a practice of 128 rules of about
/// 300 characters each (`sound`): the text before the body (`U`, the
/// practices' and cited lines in it) plus the bundle's `chars` stays
/// within 40 000, and the bundle alone at the same budget prints more than
/// the room. M: the lines not counted (the bundle fills the 64 lines' room
/// too and the text passes 40 000).
#[test]
fn dp10_the_lines_count_in_the_room() {
    let scene = Scene::new("dp10-room");
    let rules: Vec<(String, String, String)> = (1..=128)
        .map(|n| {
            (
                format!("PRA-ROOM-{n:03}"),
                format!("Room rule {n:03}"),
                prose(&format!("room-{n:03}"), 300),
            )
        })
        .collect();
    scene.put(
        "docs/spec/practices/sound/room.md",
        &practice_text("PRA-ROOM", "Room rules", &["sound"], &rules),
    );
    scene.commit("The room's rules");
    let goal = rules[..64]
        .iter()
        .map(|(id, _, _)| id.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let goal = format!("Keep {goal}.");
    assert!(goal.len() <= 4096, "a goal holds {} bytes", goal.len());
    let all = criteria(25, 1000);
    let criteria: Vec<&str> = all.iter().map(String::as_str).collect();
    let id = scene.card(
        "Mix the room",
        "sound",
        &["SCN-STORM"],
        Some(&goal),
        &criteria,
        None,
    );
    let (text, document, _) = card(&scene, &id, &["--context"]);
    assert_eq!(
        document["cited"].as_array().map(Vec::len),
        Some(64),
        "{document}"
    );
    let bundle = &document["context"]["bundle"];
    assert!(bundle.is_object(), "a bundle: {document}");
    let (head, body) = split_at_bundle(&text.stdout);
    assert!(
        head.lines()
            .filter(|line| line.starts_with("cited: "))
            .count()
            == 64,
        "the cited lines stand before the body"
    );
    let room = head.chars().count();
    let chars = usize::try_from(bundle["chars"].as_u64().expect("chars")).expect("usize");
    assert_eq!(chars, body.chars().count(), "chars is the body's");
    assert!(
        room + chars <= CEILING,
        "M: the lines not counted in U: U {room} + chars {chars} > {CEILING}"
    );
    assert!(text.stdout.chars().count() <= CEILING);
    let budget = bundle["budget"].as_u64().expect("a budget").to_string();
    let alone = bundled(&scene, &["SCN-STORM"], &budget, &["sound"]);
    let alone_chars = usize::try_from(alone["chars"].as_u64().expect("chars")).expect("usize");
    assert!(
        alone_chars > CEILING - room,
        "premise: the bundle alone prints more than the room: {alone_chars} vs {}",
        CEILING - room
    );
}

// ----------------------------------------------------------------- DP11

/// DP11: 40 practices of `art` (art.md, shared.md and 38 more): 32 listed
/// by path (art.md, p01 … p31), one note naming 8 more from p32.md; a goal
/// citing 80 distinct resolvable IDs: the first 64, in written order. M:
/// no `PRACTICES_MAX`.
#[test]
fn dp11_the_cards_caps() {
    let scene = Scene::new("dp11-card");
    for n in 1..=38 {
        scene.put(
            &format!("docs/spec/practices/art/p{n:02}.md"),
            &practice_text(
                &format!("PRA-P{n:02}"),
                &format!("Art practice {n:02}"),
                &["art"],
                &[(
                    format!("PRA-P{n:02}-RULE"),
                    format!("Rule of {n:02}"),
                    "Keep it.\n".to_owned(),
                )],
            ),
        );
    }
    let many: Vec<(String, String, String)> = (1..=80)
        .map(|n| {
            (
                format!("PRA-MANY-{n:02}"),
                format!("Many {n:02}"),
                "One of many.\n".to_owned(),
            )
        })
        .collect();
    scene.put(
        "docs/spec/many.md",
        &practice_text("PRA-MANY", "Many rules", &["production"], &many),
    );
    scene.commit("Forty practices of art and eighty rules");
    let goal = many
        .iter()
        .map(|(id, _, _)| id.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let id = scene.card("Cite them all", "art", &[], Some(&goal), &[], None);
    let (text, document, raw) = card(&scene, &id, &["--practices"]);
    assert_card_keys(&raw);
    let listed: Vec<&str> = document["practices"]
        .as_array()
        .expect("practices")
        .iter()
        .map(|entry| entry["path"].as_str().expect("a path"))
        .collect();
    let mut want = vec![ART.to_owned()];
    want.extend((1..=31).map(|n| format!("docs/spec/practices/art/p{n:02}.md")));
    assert_eq!(listed, want, "M: no PRACTICES_MAX");
    let note = "practices: 8 more practice documents of art, from docs/spec/practices/art/p32.md";
    assert!(
        document["notes"]
            .as_array()
            .expect("notes")
            .contains(&json!(note)),
        "{raw}"
    );
    assert!(text.stderr.contains(note), "{}", text.show());
    let names: Vec<&str> = document["cited"]
        .as_array()
        .expect("cited")
        .iter()
        .map(|entry| entry["name"].as_str().expect("a name"))
        .collect();
    let want: Vec<&str> = many[..64].iter().map(|(id, _, _)| id.as_str()).collect();
    assert_eq!(names, want, "the first 64 in first-seen order");
}

// ----------------------------------------------------------------- DP12

/// `task show <id> --json`: the document and its raw line.
fn package(scene: &Scene, id: &str) -> (Value, String) {
    doc(scene, &["task", "show", id])
}

/// DP12: T-0001's package (28 keys, 29 with comments', 30 with
/// references'), practices by path once each, the
/// bundle's disciplines and hash, the brief's section before Bundle);
/// T-0002's `[]` and `[]`; `schema_version` 1. M: a task no card lists
/// given every practice.
#[test]
fn dp12_the_task_package_carries_its_cards_practices() {
    let scene = Scene::new("dp12");
    let (document, raw) = package(&scene, "T-0001");
    assert_eq!(task_common::keys(&raw), PACKAGE_KEYS, "{raw}");
    assert_eq!(PACKAGE_KEYS[29], "notes");
    assert_eq!(
        PACKAGE_KEYS
            .iter()
            .position(|key| *key == "practices")
            .map(|at| PACKAGE_KEYS[at - 1]),
        Some("bundle")
    );
    let ordered = Ordered::of(&raw);
    if let Ordered::Array(items) = ordered.get("practices") {
        for item in items {
            assert_eq!(item.keys(), PRACTICE_KEYS, "{raw}");
        }
    }
    assert_eq!(document["schema_version"], json!(1));
    assert_eq!(
        document["practices"],
        json!([
            practice(ART_ID, ART_TITLE, ART, &["art"], 3),
            practice(DEV_ID, DEV_TITLE, DEV, &["development"], 2),
            practice(SHARED_ID, SHARED_TITLE, SHARED, &["art", "development"], 1),
        ]),
        "{raw}"
    );
    let reference = &document["bundle"];
    assert_eq!(reference["disciplines"], json!(["art", "development"]));
    assert_eq!(reference["node_ids"], json!(["SCN-ARRIVAL"]));
    let budget = reference["budget"].as_u64().expect("a budget").to_string();
    assert_eq!(budget, "10000");
    let both = bundled(&scene, &["SCN-ARRIVAL"], &budget, &["art", "development"]);
    assert_eq!(reference["bundle_hash"], both["bundle_hash"], "{raw}");
    let alone = bundled(&scene, &["SCN-ARRIVAL"], &budget, &[]);
    assert_ne!(
        reference["bundle_hash"], alone["bundle_hash"],
        "premise: the disciplines move the hash"
    );

    let brief = spec(&scene, &["task", "show", "T-0001"]);
    brief.code(0);
    let section = format!(
        "\nPractices: 3\n  {ART_ID} | {ART_TITLE} | rules 3\n  {DEV_ID} | {DEV_TITLE} | rules 2\n  \
         {SHARED_ID} | {SHARED_TITLE} | rules 1\n"
    );
    let at = brief
        .stdout
        .find(&section)
        .unwrap_or_else(|| panic!("the Practices section: {}", brief.stdout));
    let bundle_at = brief.stdout.find("\nBundle:").expect("a Bundle section");
    assert!(at < bundle_at, "before Bundle: {}", brief.stdout);
    if let Some(bindings) = brief.stdout.find("\nBindings:") {
        assert!(at < bindings, "before Bindings: {}", brief.stdout);
    }
    if let Some(runs) = brief.stdout.find("\nRuns:") {
        assert!(runs < at, "after Runs: {}", brief.stdout);
    }

    let (lone, raw) = package(&scene, "T-0002");
    assert_eq!(task_common::keys(&raw), PACKAGE_KEYS);
    assert_eq!(
        lone["practices"],
        json!([]),
        "M: a task no card lists given every practice"
    );
    assert_eq!(lone["bundle"]["disciplines"], json!([]));
    let plain = bundled(&scene, &["SCN-STORM"], "10000", &[]);
    assert_eq!(lone["bundle"]["bundle_hash"], plain["bundle_hash"]);
    assert_eq!(lone["schema_version"], json!(1));
}

/// DP12, "The cards' lookup": a cancelled card that lists the task still
/// gives its discipline.
#[test]
fn a_cancelled_card_still_gives_its_discipline() {
    let scene = Scene::new("dp12-cancelled");
    let card = scene.card("Hum the storm", "sound", &["SCN-STORM"], None, &[], None);
    scene.link(&card, &["T-0002"]);
    let sound: Vec<(String, String, String)> = vec![(
        "PRA-SOUND-LOUD".to_owned(),
        "Loudness".to_owned(),
        "Mix to the agreed loudness.\n".to_owned(),
    )];
    scene.put(
        "docs/spec/practices/sound/rules.md",
        &practice_text("PRA-SOUND", "Sound rules", &["sound"], &sound),
    );
    scene.commit("A sound practice");
    let cancel = spec(
        &scene,
        &["card", "cancel", &card, "--reason", "The storm is silent."],
    );
    cancel.code(0);
    let (document, _) = package(&scene, "T-0002");
    assert_eq!(document["bundle"]["disciplines"], json!(["sound"]));
    assert_eq!(
        document["practices"],
        json!([practice(
            "PRA-SOUND",
            "Sound rules",
            "docs/spec/practices/sound/rules.md",
            &["sound"],
            1
        )])
    );
}

// ----------------------------------------------------------------- DP13

/// DP13: `review PR-0001 --json`'s last key `cited` holds
/// `PRA-ART-NORMALS` alone. M: unresolved `PRA-NOPE-X` served.
#[test]
fn dp13_a_proposals_rationale_is_cited() {
    let scene = Scene::new("dp13");
    let id = scene.propose_cited();
    let (document, raw) = doc(&scene, &["review", &id]);
    let keys = task_common::keys(&raw);
    assert_eq!(keys.last().map(String::as_str), Some("cited"), "{raw}");
    assert_eq!(
        document["cited"],
        json!([cited(NORMALS, "Normal maps", ART)]),
        "M: unresolved served: {raw}"
    );
    let ordered = Ordered::of(&raw);
    if let Ordered::Array(items) = ordered.get("cited") {
        for item in items {
            assert_eq!(item.keys(), CITED_KEYS);
        }
    }
}

/// The card's lines of DP9, as the scene's constants give them (a premise
/// of every test here).
#[test]
fn the_scene_cites_as_written() {
    assert!(C1_GOAL.contains(LOD) && C1_GOAL.contains("PRA-NOPE-X"));
    assert!(!C1_TITLE.contains("PRA-") && !C1_CRITERION.contains("PRA-"));
}

/// The developer's iteration 2: a card's context and a task's package read
/// the practices in the bundle's own freshness step, so a walk warning
/// (here: a practices directory that cannot be listed) prints once, not
/// once per read. The card links no task (a linked task's state is read in
/// card-tasks' own step, which warns on its own, as before this slice).
/// M: a second freshness step (the warning twice).
#[test]
fn a_walk_warning_prints_once() {
    let scene = Scene::new("dp10-once");
    let card = scene.card(
        "Model the oars",
        "art",
        &["SCN-ARRIVAL"],
        Some(C1_GOAL),
        &[],
        None,
    );
    let locked = scene.path("docs/spec/practices/puppetry");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let warning = "cannot list `docs/spec/practices/puppetry`";
    for args in [
        &["card", "show", card.as_str(), "--context"][..],
        &["--json", "card", "show", card.as_str(), "--context"][..],
        &["card", "show", card.as_str(), "--practices"][..],
        &["task", "show", "T-0001"][..],
        &["--json", "task", "show", "T-0001"][..],
    ] {
        let run = spec(&scene, args);
        run.code(0);
        assert_eq!(
            run.stderr.matches(warning).count(),
            1,
            "{args:?}: the walk warning once: {}",
            run.show()
        );
    }
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("restore");
}

/// The JSON characters of one entry, as `serde_json` writes it (key order
/// changes no length).
fn json_chars(entry: &Value) -> usize {
    serde_json::to_string(entry).expect("JSON").chars().count()
}

/// A title of letters only that makes `entry(title)` exactly `chars` JSON
/// characters long.
fn sized_title(stem: &str, chars: usize, entry: impl Fn(&str) -> Value) -> String {
    let bare = json_chars(&entry(stem));
    assert!(bare < chars, "{stem}: {bare} >= {chars}");
    let title = format!("{stem}{}", "a".repeat(chars - bare));
    assert_eq!(json_chars(&entry(&title)), chars, "{title}");
    title
}

/// DP11, iteration 3's caps (`CITED_CHARS_MAX` 16 384, entries kept in
/// order while their JSON plus a comma fits): 32 citations of exactly 511
/// JSON characters fill the 16 384 to the character, so a 33rd is left out
/// with the note naming it; in a twin set whose first entry is one
/// character longer, the 32nd no longer fits. In a card and in a review.
/// M: the comma not counted, or `>=` for `>` (the boundary moves by one).
#[test]
fn cited_stops_at_16384_characters_to_the_character() {
    let scene = Scene::new("dp11-cited-chars");
    let path = "docs/spec/edge.md";
    let mut text =
        "---\nclass: canon\nowner: owner\nreviewed: 2026-11-20\n---\n\n# Edges\n\nSized \
                    rules.\n"
            .to_owned();
    let mut sets: Vec<Vec<String>> = Vec::new();
    for (set, first_extra) in [("X", 0usize), ("Y", 1usize)] {
        let mut ids = Vec::new();
        for n in 1..=33 {
            let id = format!("PRA-EDGE-{set}{n:02}");
            let size = if n == 1 { 511 + first_extra } else { 511 };
            let title = sized_title(
                &format!("Edge{set}{n:02}"),
                size,
                |title| json!({"written": id, "name": id, "title": title, "path": path}),
            );
            text.push_str(&format!("\n## {title} {{#{id}}}\n\nKept.\n"));
            ids.push(id);
        }
        sets.push(ids);
    }
    scene.put(path, &text);
    scene.commit("Sized rules");
    let (x, y) = (&sets[0], &sets[1]);
    assert_eq!(32 * (511 + 1), 16_384, "premise: 32 entries fill the cap");
    let card_x = scene.card("Cite X", "sound", &[], Some(&x.join(" ")), &[], None);
    let card_y = scene.card("Cite Y", "sound", &[], Some(&y[..32].join(" ")), &[], None);
    for (id, kept, first_left) in [(&card_x, 32, &x[32]), (&card_y, 31, &y[31])] {
        let (_, document, raw) = card(&scene, id, &["--practices"]);
        let names: Vec<&str> = document["cited"]
            .as_array()
            .expect("cited")
            .iter()
            .map(|entry| entry["name"].as_str().expect("a name"))
            .collect();
        assert_eq!(names.len(), kept, "{id}: {raw}");
        let note = format!("cited: 1 more cited IDs past 16384 characters, from {first_left}");
        assert!(
            document["notes"]
                .as_array()
                .expect("notes")
                .contains(&json!(note)),
            "{id}: {raw}"
        );
    }
    // The review's `cited`, the same rule.
    let id = scene.propose_edit(
        "SCN-ARRIVAL",
        "runs aground on the shingle",
        "runs hard aground on the shingle",
        &format!("Cites {}.", x.join(" ")),
    );
    let (review, raw) = doc(&scene, &["review", &id]);
    assert_eq!(review["cited"].as_array().map(Vec::len), Some(32), "{raw}");
    let note = format!(
        "cited: 1 more cited IDs past 16384 characters, from {}",
        x[32]
    );
    assert!(
        review["notes"]
            .as_array()
            .expect("notes")
            .contains(&json!(note)),
        "{raw}"
    );
}

/// DP11, iteration 3's caps (`PRACTICES_CHARS_MAX` 16 384): 16 practices of
/// `sound` whose entries are exactly 1 023 JSON characters fill the 16 384
/// to the character; a 17th is left out with the note naming its path; with
/// the first entry one character longer, the 16th is left out too. In a
/// card and in a task package.
#[test]
fn practices_stop_at_16384_characters_to_the_character() {
    for (label, first_extra, kept) in [("fits", 0usize, 16usize), ("over", 1usize, 15usize)] {
        let scene = Scene::new(&format!("dp11-practice-chars-{label}"));
        let mut paths = Vec::new();
        for n in 1..=17 {
            let id = format!("PRA-SIZED-{n:02}");
            let path = format!("docs/spec/practices/sound/sized-{n:02}.md");
            let size = if n == 1 { 1023 + first_extra } else { 1023 };
            let title = sized_title(
                &format!("Sized{n:02}"),
                size,
                |title| json!({"name": id, "title": title, "path": path, "disciplines": ["sound"], "rules": 1}),
            );
            scene.put(
                &path,
                &format!(
                    "---\nid: {id}\nclass: canon\nowner: owner\nreviewed: 2026-11-20\n\
                     disciplines: [sound]\n---\n\n# {title}\n\nOne rule.\n\n## Rule {{#{id}-R}}\n\nKept.\n"
                ),
            );
            paths.push(path);
        }
        scene.commit("Sized practices");
        assert_eq!(16 * (1023 + 1), 16_384, "premise");
        let (_, document, raw) = card(&scene, "C-0003", &["--practices"]);
        let listed: Vec<&str> = document["practices"]
            .as_array()
            .expect("practices")
            .iter()
            .map(|entry| entry["path"].as_str().expect("a path"))
            .collect();
        let want: Vec<&str> = paths[..kept].iter().map(String::as_str).collect();
        assert_eq!(listed, want, "{label}: {raw}");
        let note = format!(
            "practices: {} more practice documents of sound, from {}",
            17 - kept,
            paths[kept]
        );
        assert!(
            document["notes"]
                .as_array()
                .expect("notes")
                .contains(&json!(note)),
            "{label}: {raw}"
        );
        // The package of a task C-0003 lists.
        scene.link("C-0003", &["T-0002"]);
        let (package, raw) = package(&scene, "T-0002");
        assert_eq!(
            package["practices"].as_array().map(Vec::len),
            Some(kept),
            "{label}: {raw}"
        );
        assert!(
            package["notes"]
                .as_array()
                .expect("notes")
                .contains(&json!(note)),
            "{label}: {raw}"
        );
    }
}
