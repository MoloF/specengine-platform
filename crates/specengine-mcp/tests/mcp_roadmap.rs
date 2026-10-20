//! docs/features/roadmap-surfaces.md (Phase 6, slice 2b) RD-8, the MCP
//! half: `get_roadmap(release?, pool?)` = `spec roadmap [--release R |
//! --pool]`, one CLI library call (`View::Capped`), over scratch git
//! repositories of `fixtures/roadmap-a` (a harbor game's plan) and
//! `fixtures/roadmap-b` (a lending library's desk service: another
//! vocabulary, ADR-0008), `fixtures/spec-a` (no `[roadmap]`) and a
//! generated 600-item plan past the cut.
//!
//! - Listed as a read: the reads' annotations, the `_meta` size key, an
//!   object output schema with no `$ref`, a flat input of `release`
//!   (string or null) and `pool` (boolean or null), neither required; its
//!   description within 2 048 ASCII bytes, the determinism sentence, its
//!   CLI twin, the cut the CLI's, items changed only through
//!   `propose_change` and the owner (ADR-0035), no `[roadmap]` vocabulary
//!   (this repository's or a fixture's).
//! - M9, `get_roadmap_equals_the_cli`: in both eras every request answers
//!   as the CLI: `content` = `spec roadmap ... 2>&1`, `structuredContent` =
//!   `spec roadmap ... --json`, conforming to the output schema; exit 0
//!   (every group, a release, an all-done release, the pool, `null`s),
//!   exit 1 (a release not listed: an error result with the document),
//!   exit 2 (`release` with `pool: true`; no `[roadmap]`): an error result
//!   with the line, no document; on both fixtures. The whole-plan answer
//!   is also the fixture's hand-made oracle (`expected.txt` after the
//!   note lines, `expected.json`); with this repository's ready task on
//!   RM-0011 its line carries the task; past the cut, the tail and
//!   `truncated` (never the browser's uncut view).
//! - `get_task` carries the package's `roadmap` (the output schema's,
//!   strictly) and `next: true` follows the plan.
//!
//! AC-18 (`ac18_reads_add_no_table`, M12) is `mcp_index.rs`'.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::{Path, PathBuf};

use common::read::{
    ERAS, Era, STACK_WORDS, Session, TOOLS, checked_call, cli_env, content_text,
    flat_input_problems, has_word, output_schemas,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    Exit, GitEnv, Globals, OUTPUT_CAP_CHARS, TaskDecisionRequest, TaskNewRequest, task_approve,
    task_new,
};

const CLOCK: &str = "2026-10-07T09:00:00Z";

/// The sentence every description holds.
const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";

/// `[roadmap]` words of this repository's config and of both fixtures'
/// (prefixes, kinds, releases, priorities, units, directories): a tool
/// text names none of them (ADR-0008). Common English words a config may
/// also use (`must`, `should`, `could`, `now`, `next`, `later`, `plan`)
/// are left out: the description may say them as words.
const VOCABULARY: [&str; 17] = [
    "RM",
    "RM-0001",
    "mvp",
    "v1",
    "v2",
    "days",
    "docs/roadmap",
    "alpha",
    "beta",
    "launch",
    "plan-item",
    "harbor-zone",
    "WORK",
    "work-package",
    "p0",
    "p1",
    "p2",
];

// ------------------------------------------------------------------ setup

/// A scratch git repository of one fixture (or a generated plan) and a
/// `HOME` of its own.
struct Plan {
    _scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
}

impl Plan {
    /// A copy of `fixture`, committed once.
    fn of(label: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.copy(fixture, "plan");
        Self::committed(scratch, root)
    }

    /// The files `fill` writes into an empty directory, committed once.
    fn generated(label: &str, fill: impl FnOnce(&Path)) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.dir("plan");
        fill(&root);
        Self::committed(scratch, root)
    }

    fn committed(scratch: Scratch, root: PathBuf) -> Self {
        let git = Sandbox::new(scratch.path());
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the plan");
        let root = fs::canonicalize(&root).expect("the root");
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            git,
            root,
            home,
        }
    }

    fn git_env(&self) -> GitEnv {
        GitEnv::new(&self.root, self.git.vars())
    }

    /// A task on `nodes`, made and approved by the owner (consent yes)
    /// through the CLI library: its ID.
    fn ready(&self, nodes: &[&str]) -> String {
        let env = cli_env(&self.root, Some(&self.home));
        let made = task_new(
            &env,
            &Globals::default(),
            &TaskNewRequest {
                nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
                title: Some("Tune the regeneration".to_owned()),
                goal: Some("Make it so.".to_owned()),
                author_role: None,
                author_model: None,
                run: None,
                now: CLOCK.to_owned(),
                git: self.git_env(),
            },
        )
        .expect("task new");
        let id = made.id.clone().expect("an ID");
        let mut yes = |_: &str| true;
        let approved = task_approve(
            &env,
            &Globals::default(),
            &TaskDecisionRequest {
                id: id.clone(),
                note: None,
                now: CLOCK.to_owned(),
                git: self.git_env(),
            },
            &mut yes,
        )
        .expect("task approve");
        assert_eq!(approved.exit(), Exit::Answered, "{approved:?}");
        id
    }

    fn session(&self, era: Era) -> Session {
        Session::open(era, &[], Some(&self.root), Home::At(&self.home))
    }
}

/// `fixtures/<name>/expected.json`, `expected.txt`.
fn oracle(name: &str) -> (Value, String) {
    let json = fs::read_to_string(fixture(name).join("expected.json")).expect("expected.json");
    let text = fs::read_to_string(fixture(name).join("expected.txt")).expect("expected.txt");
    (serde_json::from_str(&json).expect("JSON"), text)
}

/// The listed tool `get_roadmap` of a fresh server in `era`.
fn listed(era: Era) -> Value {
    let mut session = Session::open(era, &[], None, Home::Fresh);
    let list = session.tools();
    let done = session.finish();
    assert!(done.status.success(), "{era:?}: {}", done.stderr);
    tool(&list, "get_roadmap").clone()
}

/// A generated plan of `count` pool items with long titles (a lighthouse
/// keeper's log: words of its own), past the cut.
fn bulk_plan(root: &Path, count: usize) {
    write(
        root,
        "specengine.toml",
        "[project]\nslug = \"beacon-log\"\n\n[paths]\nroots = [\"log\"]\n\n[ids]\nLAMP = { kind = \"lamp-entry\", width = 4 }\n\n[roadmap]\nprefix = \"LAMP\"\ndir = \"log\"\nreleases = [\"winter\"]\npriorities = [\"bright\", \"dim\"]\nunit = \"nights\"\n",
    );
    for n in 1..=count {
        write(
            root,
            &format!("log/LAMP-{n:04}.md"),
            format!(
                "---\nclass: spec\nid: LAMP-{n:04}\nstatus: draft\nscope: [beacon]\npriority: bright\nestimate: 1-2\n---\n\n# Lamp entry {n:04} with a long title that fills the line of the plan\n\nGoal.\n"
            ),
        );
    }
}

// ------------------------------------------------------------- listing

/// `get_roadmap` is listed as a read in both eras (see the module docs).
/// M: a writer's annotations; `pool` an integer; a required `release`; a
/// release name in the description.
#[test]
fn get_roadmap_is_listed_as_a_read() {
    assert!(TOOLS.contains(&"get_roadmap"));
    for era in ERAS {
        let tool = listed(era);
        assert_eq!(
            tool["annotations"],
            json!({"readOnlyHint": true, "destructiveHint": false, "openWorldHint": false}),
            "{era:?}: the reads' annotations"
        );
        assert_eq!(
            tool["_meta"],
            json!({"anthropic/maxResultSizeChars": specengine_mcp::MAX_RESULT_CHARS}),
            "{era:?}: _meta"
        );
        assert_eq!(tool["outputSchema"]["type"], json!("object"));
        assert!(!tool.to_string().contains("\"$ref\""), "{era:?}: a $ref");
        let input = &tool["inputSchema"];
        assert_eq!(
            flat_input_problems("get_roadmap", input),
            Vec::<String>::new()
        );
        let properties: Vec<&String> = input["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .collect();
        assert_eq!(properties, ["pool", "release"], "{era:?}: {input}");
        let types = |name: &str| {
            let mut names: Vec<String> = input["properties"][name]["type"]
                .as_array()
                .unwrap_or_else(|| panic!("{name}: a type list: {input}"))
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect();
            names.sort();
            names
        };
        assert_eq!(types("release"), ["null", "string"]);
        assert_eq!(types("pool"), ["boolean", "null"]);
        let required = input["required"].as_array().map_or(0, Vec::len);
        assert_eq!(required, 0, "{era:?}: nothing required: {input}");

        let description = tool["description"].as_str().expect("a description");
        assert!(description.len() <= 2048, "{} bytes", description.len());
        assert!(description.is_ascii());
        for part in [DETERMINISM, "`spec roadmap", "propose_change", "owner"] {
            assert!(
                description.contains(part),
                "{era:?}: no {part:?} in {description:?}"
            );
        }
        assert!(
            description.contains(&format!("{OUTPUT_CAP_CHARS} characters")),
            "{era:?}: the cut is the CLI's: {description:?}"
        );
        let mut texts = vec![description.to_owned()];
        for name in ["release", "pool"] {
            if let Some(doc) = input["properties"][name]["description"].as_str() {
                texts.push(doc.to_owned());
            }
        }
        for text in &texts {
            for word in VOCABULARY {
                assert!(
                    !has_word_cased(text, word),
                    "{era:?}: the vocabulary word {word:?} in {text:?}"
                );
            }
            for word in STACK_WORDS {
                assert!(!has_word(text, word), "{era:?}: {word:?} in {text:?}");
            }
        }
    }
}

/// `text` has `word` as a whole word, case-sensitive (a word character is
/// an ASCII letter, digit, `_` or `-`; `/` inside the word is literal).
fn has_word_cased(text: &str, word: &str) -> bool {
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

// ------------------------------------------------------------------ M9

/// Each fixture: its name, a release with open items, an all-done
/// release, a name its config does not list.
const FIXTURES: [(&str, &str, &str, &str); 2] = [
    ("roadmap-a", "beta", "alpha", "someday"),
    ("roadmap-b", "next", "now", "Next"),
];

/// M9: every request answers as the CLI (see the module docs), in both
/// eras, on both fixtures; the whole plan is also the hand-made oracle.
#[test]
fn get_roadmap_equals_the_cli() {
    for (name, release, done, unlisted) in FIXTURES {
        let plan = Plan::of(&format!("mr-cli-{name}"), name);
        let env = cli_env(&plan.root, Some(&plan.home));
        let globals = Globals::default();
        let calls = [
            (json!({}), false),
            (json!({"release": release}), false),
            (json!({"release": done}), false),
            (json!({"pool": true}), false),
            (json!({"release": null, "pool": null}), false),
            (json!({"pool": false}), false),
            (json!({"release": unlisted}), true),
            (json!({"release": release, "pool": true}), true),
            (json!({"release": "nope"}), true),
        ];
        let (expected_json, expected_txt) = oracle(name);
        let notes: Vec<String> = expected_json["notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|note| format!("note: {}\n", note.as_str().unwrap()))
            .collect();
        for era in ERAS {
            let mut session = plan.session(era);
            let schemas = output_schemas(&session.tools());
            let mut answers = Vec::new();
            for (args, error) in &calls {
                let result = checked_call(
                    &mut session,
                    &schemas,
                    "get_roadmap",
                    args.clone(),
                    &env,
                    &globals,
                );
                assert_eq!(
                    result["isError"],
                    json!(error),
                    "{name} {era:?} {args}: {result}"
                );
                answers.push(result);
            }
            // The whole plan: the oracle, its notes first.
            let whole = content_text(&answers[0]);
            assert!(
                whole.ends_with(&expected_txt),
                "{name} {era:?}: the text is expected.txt: {whole}"
            );
            for note in &notes {
                assert!(whole.contains(note.as_str()), "{name}: {note:?} in {whole}");
            }
            assert_eq!(
                answers[0]["structuredContent"], expected_json,
                "{name} {era:?}: the document is expected.json"
            );
            // `null`s and `pool: false` are the whole plan.
            for at in [4, 5] {
                assert_eq!(content_text(&answers[at]), whole, "{name} {era:?} #{at}");
            }
            // Exit 1: the document, its reason.
            let missing = &answers[6]["structuredContent"];
            assert!(
                missing["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains(&format!("\"{unlisted}\""))),
                "{name} {era:?}: {missing}"
            );
            assert_eq!(missing["groups"], json!([]), "{name}: {missing}");
            // Exit 2: the line, no document.
            assert!(answers[7].get("structuredContent").is_none());
            assert!(
                content_text(&answers[7]).starts_with("spec: "),
                "{name} {era:?}: {}",
                content_text(&answers[7])
            );
            let done = session.finish();
            assert!(done.status.success(), "{era:?}: {}", done.stderr);
        }
    }

    // With this repository's ready task on an item: its line carries it.
    for (name, release, item) in [
        ("roadmap-a", "beta", "RM-0011"),
        ("roadmap-b", "next", "WORK-012"),
    ] {
        task_cell_equals_the_cli(name, release, item);
    }

    // No `[roadmap]` (spec-a): the CLI's exit-2 line.
    let plan = Plan::of("mr-cli-none", "spec-a");
    let env = cli_env(&plan.root, Some(&plan.home));
    for era in ERAS {
        let mut session = plan.session(era);
        let schemas = output_schemas(&session.tools());
        let result = checked_call(
            &mut session,
            &schemas,
            "get_roadmap",
            json!({}),
            &env,
            &Globals::default(),
        );
        assert_eq!(result["isError"], json!(true));
        assert!(
            content_text(&result).contains("has no [roadmap]"),
            "{era:?}: {}",
            content_text(&result)
        );
        drop(session.finish());
    }

    // Past the cut: the capped view, its tail (never the browser's).
    let plan = Plan::generated("mr-cli-cut", |root| bulk_plan(root, 600));
    let env = cli_env(&plan.root, Some(&plan.home));
    for era in ERAS {
        let mut session = plan.session(era);
        let schemas = output_schemas(&session.tools());
        let result = checked_call(
            &mut session,
            &schemas,
            "get_roadmap",
            json!({}),
            &env,
            &Globals::default(),
        );
        let document = &result["structuredContent"];
        assert_eq!(document["truncated"], json!(true), "{era:?}");
        let omitted = document["omitted"].as_u64().expect("omitted");
        assert!(omitted > 0, "{era:?}: {omitted}");
        let text = content_text(&result);
        assert!(
            text.ends_with(&format!(
                "[truncated: {omitted} of 600 items not shown; add --release R or --pool]\n"
            )),
            "{era:?}: the tail: {}",
            &text[text.len().saturating_sub(200)..]
        );
        drop(session.finish());
    }
}

/// [`get_roadmap_equals_the_cli`]'s task cell: `fixture`'s copy with a
/// ready task on `item` (in `release`), `{}` and `{"release": release}`
/// as the CLI, both eras, the item's line and JSON carrying the task.
fn task_cell_equals_the_cli(name: &str, release: &str, item: &str) {
    let plan = Plan::of(&format!("mr-cli-task-{name}"), name);
    assert_eq!(plan.ready(&[item]), "T-0001");
    let env = cli_env(&plan.root, Some(&plan.home));
    for era in ERAS {
        let mut session = plan.session(era);
        let schemas = output_schemas(&session.tools());
        for args in [json!({}), json!({"release": release})] {
            let result = checked_call(
                &mut session,
                &schemas,
                "get_roadmap",
                args.clone(),
                &env,
                &Globals::default(),
            );
            let line = content_text(&result)
                .lines()
                .find(|line| line.contains(&format!(" {item} | ")))
                .unwrap_or_else(|| panic!("{name} {era:?} {args}: {item} listed"))
                .to_owned();
            assert!(line.ends_with(" | task T-0001 ready"), "{era:?}: {line}");
            let tasks: Vec<Value> = result["structuredContent"]["groups"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|group| group["items"].as_array().unwrap().clone())
                .filter(|listed| listed["id"] == json!(item))
                .map(|listed| listed["tasks"].clone())
                .collect();
            assert_eq!(tasks, [json!([{"id": "T-0001", "status": "ready"}])]);
        }
        drop(session.finish());
    }
}

// --------------------------------------------------------------- get_task

/// RD-7, RD-5 over MCP: `get_task` on roadmap-a answers as `spec task
/// show` with the package's `roadmap` (its output schema's, strictly),
/// and `next: true` takes the task of the item listed first, not the
/// lowest number.
#[test]
fn get_task_carries_the_roadmap() {
    let plan = Plan::of("mr-task", "roadmap-a");
    assert_eq!(plan.ready(&["RM-0050"]), "T-0001");
    assert_eq!(plan.ready(&["RM-0011"]), "T-0002");
    let env = cli_env(&plan.root, Some(&plan.home));
    let (expected_json, _) = oracle("roadmap-a");
    let beta = &expected_json["groups"][1];
    assert_eq!(beta["name"], json!("beta"));
    for era in ERAS {
        let mut session = plan.session(era);
        let schemas = output_schemas(&session.tools());
        for args in [json!({"task_id": "T-0002"}), json!({"next": true})] {
            let result = checked_call(
                &mut session,
                &schemas,
                "get_task",
                args.clone(),
                &env,
                &Globals::default(),
            );
            let package = &result["structuredContent"];
            assert_eq!(package["id"], json!("T-0002"), "{era:?} {args}");
            assert_eq!(
                package["roadmap"],
                json!({"item": "RM-0011", "group": "release", "release": "beta",
                       "position": 5, "of": beta["open"], "priority": "should",
                       "waits_on": ["RM-0010"]}),
                "{era:?} {args}"
            );
            assert!(
                content_text(&result).contains(
                    "\nRoadmap: RM-0011, release beta, position 5 of 8, waits on RM-0010\n"
                ),
                "{era:?} {args}: {}",
                content_text(&result)
            );
        }
        drop(session.finish());
    }
}
