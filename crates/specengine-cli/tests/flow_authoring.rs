//! docs/features/flow-authoring.md (RM-0068), the CLI's half, over real files
//! and the real index: the skill's example, the gap rows' lines and the five
//! messages.
//!
//! - AC-01: `plugin/specengine/skills/write-flow/SKILL.md` itself, read on
//!   every run and never copied, so the example cannot rot: its `toml` fence
//!   goes after a real `spec init` in an empty git repository and is
//!   committed, each `json` block is proposed through the library and
//!   approved with a scripted consent (one `approve` each, and the owner's
//!   walk `approve_open`), then `spec flow` reads ten rows `measured` 0 and
//!   `spec check` gives 0 errors, 0 warnings. "Data"'s table is read back
//!   from `spec flow --json`, not from the block's YAML, so a block may spell
//!   its links as it likes. M: `repeat` dropped, in memory here, on the file
//!   by hand.
//! - AC-02: "Data"'s table against `fixtures/flow-gaps` (a public library's
//!   lending) and `fixtures/flow-hire` (a hired bicycle), ADR-0008: each row
//!   above 0 its line in text and JSON, a row at 0 `null`, every
//!   `not_configured` cause its prefix, `cannot` as it was. The surfaces
//!   beside it: `flow_read.rs`, core `flow_read.rs`, `mcp_flow.rs`, http
//!   `flow.rs`, `trace_views.rs`. `FlowGaps` drawing a stubbed row is the
//!   UI's: `ui/` is out of this file's reach.
//! - AC-03: the five messages as `spec check`, `spec tree` and `spec flow`
//!   give them, each still the code and severity it was (nothing newly
//!   refuses, ADR-0012), each followed once to see that its advice works. M:
//!   a literal list instead of `LINK_TYPES`.
//! - "Rules": a `specengine.toml` left untracked gets no check; a key of the
//!   project's own in `when` still draws `unknown-key`.
//! - The owner's rulings on the spec: `status` adds its list on class spec
//!   alone, `scope` and `reviewed` on any class; the `[ids]` line fires only
//!   on a part before the first `-` of `[A-Z][A-Z0-9]*` that is neither a
//!   prefix nor an alias.
//!
//! `spec init` and AC-04: `init.rs`. Skills, budgets and versions, AC-05:
//! `specengine-mcp/tests/plugin_skills.rs`, `plugin_files.rs`. The blocks as
//! real `propose_change` calls: `specengine-mcp/tests/mcp_flow_authoring.rs`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use common::git::Sandbox;
use common::graph::spec30;
use common::proposal::{LATER, NOW};
use common::{Run, Scratch, fixture, read_text, repository_root, spec_with, write};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveOpenRequest, ApproveRequest, CreateRequest, Env, Exit, GitEnv, Globals, OpenWalk,
    ProposedText, RoundTerminal, approve, approve_open, propose_create,
};
use specengine_model::LINK_TYPES;

/// The skill, root-relative: the file a model loads, never a copy of it.
const SKILL: &str = "plugin/specengine/skills/write-flow/SKILL.md";

/// A block's keys: "Data"'s cut left no `base`, and a `dry_run` would store
/// nothing.
const BLOCK_KEYS: [&str; 5] = ["author_role", "kind", "rationale", "target", "text"];

/// "Data": the eight files of the example.
const FILES: [&str; 8] = [
    "docs/spec/done.md",
    "docs/spec/join.md",
    "docs/spec/left.md",
    "docs/spec/lost.md",
    "docs/spec/open.md",
    "docs/spec/right.md",
    "docs/spec/root.md",
    "docs/spec/rule.md",
];

/// The ten rows, in the spec's table order.
const CODES: [&str; 10] = [
    "no-start",
    "two-starts",
    "unreachable",
    "no-way-to-end",
    "trap-cycle",
    "unjoined-parallel",
    "cycle-no-repeat",
    "definition-unused",
    "definition-once",
    "supply-unfed",
];

/// "Data", `cause`: a row above 0, verbatim.
const FOUND: [(&str, &str); 10] = [
    (
        "no-start",
        "no step here is a start: match the flow's first step in [[flow.start]], or, reading a \
         part, add a follows link into it from outside",
    ),
    (
        "two-starts",
        "more than one start: narrow [[flow.start]] to one",
    ),
    (
        "unreachable",
        "no path from a start reaches it: add a follows link on it, naming a step before it",
    ),
    (
        "no-way-to-end",
        "no path from it reaches an end: have a later step follow it, or match it in [[flow.end]]",
    ),
    (
        "trap-cycle",
        "a loop with no way out: have a step outside it follow a member, or match a member in \
         [[flow.end]]",
    ),
    (
        "unjoined-parallel",
        "a parallel fork with one branch, or branches that never meet: add a branch, or a later \
         step following them all",
    ),
    (
        "cycle-no-repeat",
        "a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}",
    ),
    (
        "definition-unused",
        "no document uses it: add a uses link to it, or drop it from [[flow.definition]]",
    ),
    (
        "definition-once",
        "only one document uses it: add a uses link from a second document, or fold it into \
         that document",
    ),
    (
        "supply-unfed",
        "these only supply each other: add a supplies link into them from a node outside",
    ),
];

/// "Data", `cause`: a row with no rule, by the table it needs, verbatim.
const NO_RULE: [(&str, &str); 3] = [
    (
        "start",
        "no [[flow.start]] rule: add one naming the first step by paths",
    ),
    (
        "end",
        "no [[flow.end]] rule: add one naming the last step by paths",
    ),
    (
        "definition",
        "no [[flow.definition]] rule: add one naming what steps share, by paths or kinds",
    ),
];

/// flow-read's `Needs`: the rows a project with no `[flow]` cannot measure.
const NEEDING_A_TABLE: [(&str, &str); 7] = [
    ("no-start", "start"),
    ("two-starts", "start"),
    ("unreachable", "start"),
    ("no-way-to-end", "end"),
    ("trap-cycle", "end"),
    ("definition-unused", "definition"),
    ("definition-once", "definition"),
];

/// `FlowGap`'s `NoStart` cause, which this slice keeps.
const NOTHING_TO_WALK: &str = "no start and no entering edge to walk from";

/// One `(from, to, type, kind, when)` as a document reads it.
type EdgeKey = (String, String, String, Option<String>, Option<String>);

/// One `(from, to, type, kind, when)` of the example, as `spec flow` reads it.
type EdgeRow = (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
);

/// "Data"'s table, read in the flow's direction: `follows` stands on the
/// later step, `uses` and `supplies` on the one naming.
const EDGES: [EdgeRow; 10] = [
    (
        "NODE-join",
        "NODE-done",
        "follows",
        Some("alternative"),
        Some("won"),
    ),
    ("NODE-join", "NODE-done", "supplies", None, None),
    (
        "NODE-join",
        "NODE-lost",
        "follows",
        Some("alternative"),
        Some("lost"),
    ),
    ("NODE-left", "NODE-join", "follows", None, None),
    ("NODE-left", "NODE-rule", "uses", None, None),
    (
        "NODE-lost",
        "NODE-join",
        "follows",
        Some("repeat"),
        Some("retried"),
    ),
    ("NODE-open", "NODE-left", "follows", Some("parallel"), None),
    ("NODE-open", "NODE-right", "follows", Some("parallel"), None),
    ("NODE-right", "NODE-join", "follows", None, None),
    ("NODE-right", "NODE-rule", "uses", None, None),
];

/// "Data"'s table: `(id, parent, stage)` of each file.
const NODES: [(&str, Option<&str>, Option<&str>); 8] = [
    ("NODE-done", Some("NODE-root"), Some("b")),
    ("NODE-join", Some("NODE-root"), Some("b")),
    ("NODE-left", Some("NODE-root"), Some("a")),
    ("NODE-lost", Some("NODE-root"), Some("b")),
    ("NODE-open", Some("NODE-root"), Some("a")),
    ("NODE-right", Some("NODE-root"), Some("a")),
    ("NODE-root", None, None),
    ("NODE-rule", Some("NODE-root"), None),
];

// --------------------------------------------------------- the skill's text

/// One `json` block, read as `propose_change` would take it.
#[derive(Clone, Debug)]
struct Block {
    target: String,
    text: String,
    rationale: String,
    author_role: String,
}

/// The skill's example.
#[derive(Clone, Debug)]
struct Example {
    /// The body of the one `toml` fence.
    config: String,
    /// The `json` blocks, in the file's order.
    blocks: Vec<Block>,
}

/// A string key of a block.
fn string_key(
    object: &serde_json::Map<String, Value>,
    key: &str,
    at: &str,
) -> Result<String, String> {
    object[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{at}: `{key}` is no string"))
}

/// The block at `line` (of its fence, 1-based), under the line `above`.
fn read_block(line: usize, above: &str, body: &str) -> Result<Block, String> {
    let at = format!("{SKILL}:{line}");
    if !above.contains("`propose_change`") {
        return Err(format!(
            "{at}: the line above the block names no `propose_change`: {above:?}"
        ));
    }
    let value: Value =
        serde_json::from_str(body).map_err(|error| format!("{at}: no JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("{at}: not an object"))?;
    let keys: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if keys != BLOCK_KEYS {
        return Err(format!("{at}: the keys {keys:?}, not {BLOCK_KEYS:?}"));
    }
    if object["kind"] != "create" {
        return Err(format!("{at}: `kind` {}, not create", object["kind"]));
    }
    Ok(Block {
        target: string_key(object, "target", &at)?,
        text: string_key(object, "text", &at)?,
        rationale: string_key(object, "rationale", &at)?,
        author_role: string_key(object, "author_role", &at)?,
    })
}

/// The example of `skill`, or why it cannot be taken from the file: one
/// `toml` fence, the rest `json` fences each under a line naming the tool.
fn read_example(skill: &str) -> Result<Example, String> {
    let lines: Vec<&str> = skill.lines().collect();
    let mut configs = Vec::new();
    let mut blocks = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let fence = lines[at];
        if !fence.starts_with("```") {
            at += 1;
            continue;
        }
        let close = lines[at + 1..]
            .iter()
            .position(|line| *line == "```")
            .ok_or_else(|| format!("{SKILL}:{}: an unclosed fence", at + 1))?;
        let body = lines[at + 1..at + 1 + close].join("\n");
        match fence {
            "```toml" => configs.push(body),
            "```json" => {
                let above = if at == 0 { "" } else { lines[at - 1] };
                blocks.push(read_block(at + 1, above, &body)?);
            }
            other => {
                return Err(format!("{SKILL}:{}: a fence {other:?}", at + 1));
            }
        }
        at += close + 2;
    }
    match <[String; 1]>::try_from(configs) {
        Ok([config]) => Ok(Example { config, blocks }),
        Err(configs) => Err(format!("{} toml fences, not one", configs.len())),
    }
}

/// The skill as it stands in the repository.
fn skill_text() -> String {
    read_text(&repository_root(), SKILL)
}

/// [`read_example`] of the file, which must read.
fn the_example() -> Example {
    read_example(&skill_text()).unwrap_or_else(|problem| panic!("{problem}"))
}

/// `example` with the one occurrence of `from` among its texts replaced.
fn with_text(example: &Example, from: &str, to: &str) -> Example {
    let hits = example
        .blocks
        .iter()
        .filter(|block| block.text.contains(from))
        .count();
    assert_eq!(hits, 1, "{from:?} in one block");
    let mut example = example.clone();
    for block in &mut example.blocks {
        block.text = block.text.replacen(from, to, 1);
    }
    example
}

// ------------------------------------------------------------ the project

/// An empty git repository of `spec init`, a data `HOME`; git and the binary
/// run in the sandbox of `common/git.rs`.
struct Project {
    _scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
}

impl Project {
    /// `spec init` in a new repository with no commit, `config` after what it
    /// wrote, nothing committed.
    fn written(label: &str, config: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.dir("project");
        git.init(&root);
        let home = scratch.home("h");
        let project = Self {
            _scratch: scratch,
            git,
            root,
            home,
        };
        project.spec(&["init"]).code(0);
        let scaffold = read_text(&project.root, "specengine.toml");
        write(
            &project.root,
            "specengine.toml",
            format!("{scaffold}\n{config}\n"),
        );
        project
    }

    /// [`Project::written`], and the owner's commit of the config.
    fn configured(label: &str, config: &str) -> Self {
        let project = Self::written(label, config);
        project.git.add_all(&project.root);
        project.git.commit(&project.root, "the config");
        project
    }

    /// `spec args` at the root.
    fn spec(&self, args: &[&str]) -> Run {
        let vars: Vec<(OsString, OsString)> = self.git.vars();
        let mut env: Vec<(&str, &OsStr)> = vars
            .iter()
            .map(|(name, value)| (name.to_str().expect("a UTF-8 name"), value.as_os_str()))
            .collect();
        env.push(("HOME", self.home.as_os_str()));
        spec_with(&self.root, args, &env)
    }

    /// `spec args --json`, its document.
    fn json(&self, args: &[&str]) -> (Run, Value) {
        let mut all = args.to_vec();
        all.push("--json");
        let run = self.spec(&all);
        let document = run.json();
        (run, document)
    }

    fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    fn git_env(&self) -> GitEnv {
        GitEnv::new(&self.root, self.git.vars())
    }

    /// The block, stored through the library: its proposal's ID.
    fn propose(&self, block: &Block) -> String {
        let outcome = propose_create(
            &self.env(),
            &Globals::default(),
            &CreateRequest {
                target: block.target.clone(),
                base: None,
                text: ProposedText::Given(block.text.clone().into_bytes()),
                rationale: block.rationale.clone(),
                author_role: Some(block.author_role.clone()),
                author_model: None,
                run: None,
                login: None,
                now: NOW.to_owned(),
                git: self.git_env(),
                distinct_from: Vec::new(),
                keywords: Vec::new(),
            },
        )
        .unwrap_or_else(|error| panic!("propose {}: {error}", block.target));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "propose {}: {outcome:?}",
            block.target
        );
        outcome
            .document
            .id
            .clone()
            .unwrap_or_else(|| panic!("{}: stored under no ID", block.target))
    }

    /// `id` approved by the scripted owner, asked once: its commit.
    fn approve(&self, id: &str) -> String {
        let mut asked = 0;
        let mut consent = |_: &str| {
            asked += 1;
            true
        };
        let outcome = approve(
            &self.env(),
            &Globals::default(),
            &ApproveRequest {
                id: id.to_owned(),
                note: None,
                now: LATER.to_owned(),
                git: self.git_env(),
            },
            &mut consent,
        )
        .unwrap_or_else(|error| panic!("approve {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
        assert_eq!(asked, 1, "approve {id}: one question");
        outcome
            .document
            .applied_commit
            .clone()
            .unwrap_or_else(|| panic!("{id}: no commit"))
    }

    /// `spec approve --open` through the library, the owner pressing `y`.
    fn walk(&self) -> (OpenWalk, Vec<String>) {
        let mut owner = Owner::default();
        let walk = approve_open(
            &self.env(),
            &Globals::default(),
            &ApproveOpenRequest {
                now: LATER.to_owned(),
                git: self.git_env(),
            },
            &mut owner,
        )
        .unwrap_or_else(|error| panic!("approve --open: {error}"));
        (walk, owner.asked)
    }

    fn git_text(&self, args: &[&str]) -> String {
        self.git.git_text(&self.root, args)
    }
}

/// The owner's terminal, every key `y`.
#[derive(Default)]
struct Owner {
    asked: Vec<String>,
}

impl RoundTerminal for Owner {
    fn report(&mut self, _text: &str) {}

    fn ask(&mut self, question: &str) -> Option<bool> {
        self.asked.push(question.to_owned());
        Some(true)
    }

    fn tell(&mut self, _line: &str) {}
}

/// `example`'s config committed, each block proposed then approved in the
/// file's order, the commits checked.
fn each_approved(label: &str, example: &Example) -> Project {
    let project = Project::configured(label, &example.config);
    let ids: Vec<String> = example
        .blocks
        .iter()
        .map(|block| project.propose(block))
        .collect();
    for (id, block) in ids.iter().zip(&example.blocks) {
        let commit = project.approve(id);
        assert_eq!(
            project
                .git_text(&["show", "--format=", "--name-only", &commit])
                .trim(),
            block.target,
            "{id}: its commit holds its file and no other"
        );
    }
    project
}

// --------------------------------------------------------- reading the rows

/// `(from, to, type, kind, when)` of a document's edges.
fn edge_rows(document: &Value) -> BTreeSet<EdgeKey> {
    let text = |value: &Value| value.as_str().map(str::to_owned);
    document["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .map(|edge| {
            assert_eq!(edge["count"], json!(1), "{edge}");
            (
                text(&edge["from"]).expect("from"),
                text(&edge["to"]).expect("to"),
                text(&edge["type"]).expect("a type"),
                text(&edge["kind"]),
                text(&edge["when"]),
            )
        })
        .collect()
}

/// The `gap ...` lines of a text.
fn gap_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.starts_with("gap "))
        .collect()
}

/// The ten rows, each `measured` 0, in text and JSON.
fn assert_ten_rows_at_zero(project: &Project, context: &str) {
    let run = project.spec(&["flow"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{context}: {}", run.show());
    assert_eq!(
        gap_lines(&run.stdout),
        CODES
            .iter()
            .map(|code| format!("gap {code} | measured | 0"))
            .collect::<Vec<String>>(),
        "{context}: {}",
        run.show()
    );
    let (_, document) = project.json(&["flow"]);
    let gaps = document["gaps"].as_array().expect("gaps");
    assert_eq!(gaps.len(), 10, "{context}: {document}");
    // docs/features/doc-modal.md DM-3: `titles`, parallel to `items`.
    for (gap, code) in gaps.iter().zip(CODES) {
        assert_eq!(
            *gap,
            json!({"code": code, "state": "measured", "count": 0, "items": [], "titles": [],
                "cause": null}),
            "{context}"
        );
    }
}

/// `spec check`: exit 0, 0 errors, 0 warnings, in text and JSON.
fn assert_check_is_clean(project: &Project, context: &str) {
    let run = project.spec(&["check"]);
    run.code(0);
    assert!(
        run.stdout.contains(", 0 errors, 0 warnings, "),
        "{context}: {}",
        run.show()
    );
    let (run, check) = project.json(&["check"]);
    run.code(0);
    assert_eq!(check["counts"]["errors"], json!(0), "{context}: {check}");
    assert_eq!(check["counts"]["warnings"], json!(0), "{context}: {check}");
    assert_eq!(check["findings"], json!([]), "{context}: {check}");
    assert_eq!(check["verdict"], "clean", "{context}: {check}");
}

// ------------------------------------------------------------------- AC-01

/// The reader takes the committed file as the spec describes it: one `toml`
/// fence and the eight `json` blocks of "Data", one file each. When this
/// goes red the block is not machine-readable as the spec says, and the
/// panic names the line.
#[test]
fn ac01_the_skill_holds_a_config_and_eight_create_blocks() {
    let example = the_example();
    let targets: BTreeSet<&str> = example
        .blocks
        .iter()
        .map(|block| block.target.as_str())
        .collect();
    assert_eq!(example.blocks.len(), 8, "{:?}", example.blocks);
    assert_eq!(targets, BTreeSet::from(FILES), "one file each");
    assert_eq!(
        example.config,
        "[ids]\n\
         NODE = { kind = \"node\", shape = \"name\" }\n\
         [[flow.start]]\n\
         paths = [\"docs/spec/open.md\"]\n\
         [[flow.end]]\n\
         paths = [\"docs/spec/done.md\"]\n\
         [[flow.definition]]\n\
         paths = [\"docs/spec/rule.md\"]",
        "\"Data\": the config block"
    );
    for block in &example.blocks {
        let stem = block
            .target
            .strip_prefix("docs/spec/")
            .and_then(|name| name.strip_suffix(".md"))
            .expect("a file under the spec directory");
        assert!(
            block.text.starts_with(&format!(
                "---\nid: NODE-{stem}\nclass: canon\nowner: owner\nreviewed: 2026-01-01\n"
            )),
            "{}: \"Data\"'s four keys first: {:?}",
            block.target,
            block.text
        );
        assert_eq!(
            block.text.matches("\n# ").count(),
            1,
            "{}: one H1: {:?}",
            block.target,
            block.text
        );
    }
}

/// The reader names what it cannot take, so a rotten file reads as a problem
/// and not as a green run over nothing.
#[test]
fn ac01_the_reader_names_what_it_cannot_take() {
    let block = "`propose_change`:\n```json\n{\"kind\":\"create\",\"target\":\"docs/spec/a.md\",\
                 \"text\":\"---\\n---\\n\",\"rationale\":\"r\",\"author_role\":\"w\"}\n```\n";
    let config = "```toml\n[ids]\n```\n";
    let good = read_example(&format!("{config}{block}")).expect("a small example");
    assert_eq!(good.config, "[ids]");
    assert_eq!(good.blocks.len(), 1);
    assert_eq!(good.blocks[0].target, "docs/spec/a.md");
    assert_eq!(good.blocks[0].author_role, "w");

    for (label, text, needle) in [
        ("no config", block.to_owned(), "0 toml fences"),
        (
            "two configs",
            format!("{config}{config}{block}"),
            "2 toml fences",
        ),
        (
            "an unclosed fence",
            format!("{config}```json\n{{}}\n"),
            "an unclosed fence",
        ),
        (
            "no tool above",
            format!("{config}\n```json\n{{}}\n```\n"),
            "names no `propose_change`",
        ),
        (
            "no JSON",
            format!("{config}`propose_change`:\n```json\n{{\n```\n"),
            "no JSON",
        ),
        (
            "an array",
            format!("{config}`propose_change`:\n```json\n[]\n```\n"),
            "not an object",
        ),
        (
            "a base",
            format!("{config}{block}").replace("{\"kind\"", "{\"base\":null,\"kind\""),
            "the keys",
        ),
        (
            "a dry run",
            format!("{config}{block}").replace("{\"kind\"", "{\"dry_run\":true,\"kind\""),
            "the keys",
        ),
        (
            "an update",
            format!("{config}{block}").replace("\"create\"", "\"update\""),
            "not create",
        ),
        (
            "a target of no string",
            format!("{config}{block}").replace("\"docs/spec/a.md\"", "1"),
            "`target` is no string",
        ),
        (
            "another fence",
            format!("{config}```text\nx\n```\n"),
            "a fence",
        ),
    ] {
        let problem = read_example(&text).expect_err(label);
        assert!(problem.contains(needle), "{label}: {problem}");
    }
}

/// AC-01, the test that keeps the example alive: the skill's own blocks into
/// an empty git project, approved one by one, read ten rows at 0 and a clean
/// check. M: `repeat` dropped from the example (the file, by hand; in memory
/// in the next test).
#[test]
fn ac01_the_skills_example_reads_ten_rows_at_zero() {
    let example = the_example();
    let project = each_approved("fa-ac01", &example);

    assert_ten_rows_at_zero(&project, "the example");
    assert_check_is_clean(&project, "the example");

    // Every line has its place: the header, the rows, the nodes, the edges.
    let run = project.spec(&["flow"]);
    assert_eq!(
        run.stdout.lines().next(),
        Some("flow all | nodes 8, edges 10, depth -"),
        "{}",
        run.show()
    );

    // "Data"'s table, as the flow reads it, however a block spells its links.
    let (_, document) = project.json(&["flow"]);
    let want: BTreeSet<EdgeKey> = EDGES
        .iter()
        .map(|(from, to, link, kind, when)| {
            (
                (*from).to_owned(),
                (*to).to_owned(),
                (*link).to_owned(),
                kind.map(str::to_owned),
                when.map(str::to_owned),
            )
        })
        .collect();
    assert_eq!(edge_rows(&document), want, "{document}");
    let nodes = document["nodes"].as_array().expect("nodes");
    assert_eq!(nodes.len(), NODES.len(), "{document}");
    for (id, parent, stage) in NODES {
        let node = nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("no node {id}: {document}"));
        assert_eq!(node["group"], json!(parent), "{id}: {node}");
        assert_eq!(node["stage"], json!(stage), "{id}: {node}");
        assert_eq!(node["start"], json!(id == "NODE-open"), "{id}: {node}");
        assert_eq!(node["end"], json!(id == "NODE-done"), "{id}: {node}");
    }

    // The config and one commit per file; nothing left behind.
    assert_eq!(
        project.git_text(&["rev-list", "--count", "HEAD"]).trim(),
        "9"
    );
    assert_eq!(
        project.git_text(&["status", "--porcelain=v1", "--untracked-files=all"]),
        "",
        "every write is committed"
    );

    // One state, one result.
    for args in [
        &["flow"][..],
        &["check"],
        &["flow", "--json"],
        &["check", "--json"],
    ] {
        let (first, second) = (project.spec(args), project.spec(args));
        assert_eq!(
            (first.code, &first.stdout, &first.stderr),
            (second.code, &second.stdout, &second.stderr),
            "{args:?} repeated"
        );
    }
}

/// AC-01: the owner's own walk, the one the skill's fourth step names, takes
/// the eight in one run, one key each, and reads as the one-by-one path does.
#[test]
fn ac01_the_owners_walk_takes_the_example_whole() {
    let example = the_example();
    let project = Project::configured("fa-ac01-walk", &example.config);
    let ids: Vec<String> = example
        .blocks
        .iter()
        .map(|block| project.propose(block))
        .collect();

    let (walk, asked) = project.walk();
    assert_eq!(walk.exit(), Exit::Answered, "{walk:?}");
    assert_eq!(walk.left, Vec::<String>::new(), "{walk:?}");
    assert_eq!(
        walk.applied.iter().collect::<BTreeSet<_>>(),
        ids.iter().collect::<BTreeSet<_>>(),
        "{walk:?}"
    );
    assert_eq!(asked.len(), 8, "one key each: {asked:?}");

    assert_ten_rows_at_zero(&project, "the walk");
    assert_check_is_clean(&project, "the walk");
}

/// AC-01's named mutation, on every run and on the example as the skill
/// writes it: the loop's edge loses `repeat`, and the row the example exists
/// to keep at 0 fires, with its line. Nothing refuses it: the check stays
/// clean and every block is still applied. M: `repeat` dropped.
#[test]
fn ac01_repeat_dropped_turns_the_row_red() {
    let example = with_text(&the_example(), "kind: repeat, ", "");
    let project = each_approved("fa-ac01-m", &example);

    let run = project.spec(&["flow"]);
    run.code(0);
    let (_, line) = FOUND[6];
    let rows = gap_lines(&run.stdout);
    for (row, code) in rows.iter().zip(CODES) {
        if code == "cycle-no-repeat" {
            assert_eq!(
                *row,
                format!("gap {code} | measured | 1 | NODE-join + NODE-lost | {line}"),
                "{}",
                run.show()
            );
        } else {
            assert_eq!(*row, format!("gap {code} | measured | 0"), "{}", run.show());
        }
    }
    assert_eq!(rows.len(), 10, "{}", run.show());
    assert_check_is_clean(&project, "the mutation blocks nothing");
}

/// "Rules": a `specengine.toml` left untracked gets one scaffold comment and
/// one skill line, no check: `spec check` and `spec flow` answer as they do
/// over a committed one.
#[test]
fn an_untracked_config_gets_no_check() {
    let example = the_example();
    let untracked = Project::written("fa-untracked", &example.config);
    assert_eq!(
        untracked.git_text(&["status", "--porcelain=v1", "--untracked-files=all"]),
        "?? specengine.toml",
        "the config stands uncommitted"
    );
    assert_check_is_clean(&untracked, "an untracked config");
    untracked.spec(&["flow"]).code(0);

    let scaffold = read_text(&untracked.root, "specengine.toml");
    assert!(
        scaffold.contains("# Commit this file: a proposal needs a commit"),
        "the scaffold's one comment: {scaffold}"
    );
    assert!(
        skill_text()
            .lines()
            .any(|line| line.contains("commit") && line.contains("proposal needs a commit")),
        "the skill's one line"
    );
}

// ------------------------------------------------------------------- AC-02

/// A scratch `HOME` and a working directory outside every project.
struct Reader {
    scratch: Scratch,
    home: PathBuf,
    cwd: PathBuf,
}

impl Reader {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self { scratch, home, cwd }
    }

    /// `spec flow <args> --root <root>`, exit 0, nothing on stderr.
    fn text(&self, root: &Path, args: &[&str]) -> Run {
        let mut all = vec!["flow"];
        all.extend_from_slice(args);
        all.extend_from_slice(&["--root", root.to_str().expect("a UTF-8 root")]);
        let run = spec30(&self.home, &self.cwd, &all);
        run.code(0);
        assert_eq!(run.stderr, "", "{}", run.show());
        run
    }

    /// `spec flow <args> --json --root <root>`.
    fn json(&self, root: &Path, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        self.text(root, &all).json()
    }
}

/// `code`'s line in [`FOUND`].
fn found_line(code: &str) -> &'static str {
    FOUND
        .iter()
        .find(|(name, _)| *name == code)
        .unwrap_or_else(|| panic!("no line for {code}"))
        .1
}

/// `table`'s line in [`NO_RULE`].
fn no_rule_line(table: &str) -> &'static str {
    NO_RULE
        .iter()
        .find(|(name, _)| *name == table)
        .unwrap_or_else(|| panic!("no line for {table}"))
        .1
}

/// Each measured row of one reading against "Data": above 0 its line, in the
/// JSON and as the last field of its one text line, at 0 `null` and no field.
/// Returns the codes above 0.
fn assert_measured_rows(text: &str, document: &Value, context: &str) -> BTreeSet<String> {
    let lines = gap_lines(text);
    let rows = document["gaps"].as_array().expect("gaps");
    assert_eq!(lines.len(), rows.len(), "{context}");
    let mut fired = BTreeSet::new();
    for (line, gap) in lines.iter().zip(rows) {
        if gap["state"] != "measured" {
            continue;
        }
        let code = gap["code"].as_str().expect("a code");
        let count = gap["count"].as_u64().expect("a measured row's count");
        if count == 0 {
            assert_eq!(gap["cause"], Value::Null, "{context}: a row at 0: {gap}");
            assert_eq!(*line, format!("gap {code} | measured | 0"), "{context}");
            continue;
        }
        let want = found_line(code);
        assert_eq!(gap["cause"], want, "{context}: {gap}");
        assert!(
            line.starts_with(&format!("gap {code} | measured | {count} | "))
                && line.ends_with(&format!(" | {want}")),
            "{context}: {line}"
        );
        assert_eq!(
            line.matches(" | ").count(),
            4,
            "{context}: the code, state, count, items and cause: {line}"
        );
        fired.insert(code.to_owned());
    }
    fired
}

/// AC-02 on `fixtures/flow-gaps`, every gap planted once: each row above 0
/// has its one line in text and JSON, a row at 0 has `null`, and between the
/// scopes every line of the table is read. M: a row left blank; a cause on a
/// row at 0.
#[test]
fn ac02_each_row_above_zero_has_its_line_and_a_row_at_zero_none() {
    let reader = Reader::new("fa-ac02");
    let root = fixture("flow-gaps");
    let mut fired = BTreeSet::new();
    for scope in [
        &[][..],
        &["WING-BROKEN"],
        &["WING-HEADLESS"],
        &["WING-LEND"],
        &["WING-LIBRARY"],
    ] {
        let run = reader.text(&root, scope);
        let document = reader.json(&root, scope);
        fired.extend(assert_measured_rows(
            &run.stdout,
            &document,
            &format!("flow-gaps {scope:?}"),
        ));
    }
    assert_eq!(
        fired.iter().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(CODES),
        "every line of the table read"
    );
}

/// ADR-0008: a bicycle hire reads the same words as a library, each row
/// above 0 and each `not_configured` row; none names a project.
#[test]
fn ac02_another_project_reads_the_same_lines() {
    let reader = Reader::new("fa-ac02-hire");
    let root = fixture("flow-hire");
    let run = reader.text(&root, &[]);
    let document = reader.json(&root, &[]);
    assert_eq!(
        assert_measured_rows(&run.stdout, &document, "flow-hire"),
        BTreeSet::from(["unjoined-parallel".to_owned()])
    );
    assert!(
        gap_lines(&run.stdout).contains(
            &format!(
                "gap unjoined-parallel | measured | 1 | JRN-UNLOCK | {}",
                found_line("unjoined-parallel")
            )
            .as_str()
        ),
        "{}",
        run.show()
    );
}

/// AC-02: every `not_configured` cause keeps its prefix and is "Data"'s line,
/// on two projects with no `[flow]`.
#[test]
fn ac02_every_not_configured_cause_keeps_its_prefix() {
    let reader = Reader::new("fa-ac02-rules");
    for name in ["flow-relay", "flow-hire"] {
        let root = fixture(name);
        let run = reader.text(&root, &[]);
        let document = reader.json(&root, &[]);
        let lines = gap_lines(&run.stdout);
        for (code, table) in NEEDING_A_TABLE {
            let at = CODES
                .iter()
                .position(|row| *row == code)
                .expect("a row of the ten");
            let gap = &document["gaps"][at];
            let want = no_rule_line(table);
            assert_eq!(gap["code"], code, "{name}: {gap}");
            assert_eq!(gap["state"], "not_configured", "{name}: {gap}");
            assert_eq!(gap["count"], Value::Null, "{name}: {gap}");
            assert_eq!(gap["cause"], want, "{name}: {gap}");
            assert!(
                want.starts_with(&format!("no [[flow.{table}]] rule: ")),
                "the table's prefix: {want}"
            );
            assert_eq!(
                lines[at],
                format!("gap {code} | not_configured | - | {want}"),
                "{name}"
            );
        }
    }
}

/// "Data": `NoStart` and `Config` stay as they were.
#[test]
fn ac02_a_row_that_cannot_keeps_its_cause() {
    let reader = Reader::new("fa-ac02-cannot");
    let document = reader.json(&fixture("flow-gaps"), &["WING-HEADLESS"]);
    assert_eq!(document["gaps"][2]["code"], "unreachable", "{document}");
    assert_eq!(document["gaps"][2]["state"], "cannot", "{document}");
    assert_eq!(document["gaps"][2]["cause"], NOTHING_TO_WALK, "{document}");

    let root = reader.scratch.copy("flow-gaps", "bad");
    let config = read_text(&root, "specengine.toml");
    write(
        &root,
        "specengine.toml",
        format!("{config}\n[[flow.definition]]\nkinds = [\"nothing\"]\n"),
    );
    let at = read_text(&root, "specengine.toml").lines().count();
    let document = reader.json(&root, &["WING-BROKEN"]);
    for row in [7, 8] {
        let gap = &document["gaps"][row];
        assert_eq!(gap["state"], "cannot", "{gap}");
        assert!(
            gap["cause"]
                .as_str()
                .expect("a cause")
                .starts_with(&format!("specengine.toml:{at}: ")),
            "{gap}"
        );
    }
}

/// One state, one result: the rows' words repeat byte for byte.
#[test]
fn ac02_the_lines_repeat_byte_for_byte() {
    let reader = Reader::new("fa-ac02-bytes");
    let root = fixture("flow-gaps");
    for args in [
        &["WING-BROKEN"][..],
        &["WING-BROKEN", "--json"],
        &["--depth", "1"],
    ] {
        assert_eq!(
            reader.text(&root, args).stdout,
            reader.text(&root, args).stdout,
            "{args:?}"
        );
    }
}

// ------------------------------------------------------------------- AC-03

/// A scratch project of one prefix `NODE`, `tail` closing its config: no git,
/// the check reads the files.
struct Teaching {
    _scratch: Scratch,
    home: PathBuf,
    root: PathBuf,
}

impl Teaching {
    fn new(label: &str, tail: &str, files: &[(&str, String)]) -> Self {
        let scratch = Scratch::new(label);
        let home = scratch.home("h");
        let root = scratch.dir("project");
        write(
            &root,
            "specengine.toml",
            format!(
                "[project]\nslug = \"teach\"\n\n[ids]\nNODE = {{ kind = \"node\", shape = \
                 \"name\" }}\n{tail}"
            ),
        );
        for (path, text) in files {
            write(&root, path, text);
        }
        Self {
            _scratch: scratch,
            home,
            root,
        }
    }

    fn spec(&self, args: &[&str]) -> Run {
        spec30(&self.home, &self.root, args)
    }

    /// `spec check --json`.
    fn check(&self) -> Value {
        self.spec(&["check", "--json"]).json()
    }
}

/// A canon document `id`, `extra` closing its front-matter.
fn canon(id: &str, extra: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nowner: owner\nreviewed: 2026-01-01\n{extra}---\n\n# A \
         document\n"
    )
}

/// The one finding of `code` at `path` naming `subject`.
fn finding<'a>(check: &'a Value, code: &str, path: &str, subject: &str) -> &'a Value {
    let found: Vec<&Value> = check["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|item| item["code"] == code && item["path"] == path && item["subject"] == subject)
        .collect();
    assert_eq!(found.len(), 1, "{code} {path} {subject}: {check}");
    found[0]
}

/// The findings of `code`.
fn findings<'a>(check: &'a Value, code: &str) -> Vec<&'a Value> {
    check["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|item| item["code"] == code)
        .collect()
}

/// AC-03: an unknown link type's message lists the 15 types, in the
/// constant's order; still a warning, the document still clean of errors.
#[test]
fn ac03_an_unknown_link_type_lists_the_types() {
    assert_eq!(LINK_TYPES.len(), 15, "the 15 types");
    let corpus = Teaching::new(
        "fa-ac03-link",
        "",
        &[(
            "docs/spec/a.md",
            canon("NODE-a", "links:\n  relates: [NODE-a]\n"),
        )],
    );
    let run = corpus.spec(&["check", "--json"]);
    run.code(0);
    let check = run.json();
    let item = finding(&check, "unknown-link-type", "docs/spec/a.md", "relates");
    assert_eq!(
        item["message"],
        format!(
            "link type `relates` is not one of the shared link types; kept; the types: {}",
            LINK_TYPES.join(", ")
        ),
        "{check}"
    );
    assert_eq!(item["severity"], "warning", "nothing newly refuses: {item}");
    assert_eq!(item["line"], json!(7), "{item}");
    assert_eq!(check["counts"]["errors"], json!(0), "{check}");
    assert_eq!(check["counts"]["warnings"], json!(1), "{check}");
}

/// AC-03's mutation: the list stands in one place, so a sixteenth type cannot
/// leave the message behind. A literal copy of the fifteen reads the same
/// today, so only the source tells it from the constant. M: a literal list
/// instead of `LINK_TYPES`.
#[test]
fn ac03_the_list_is_the_constant() {
    let source = read_text(
        &repository_root(),
        "crates/specengine-core/src/front_matter.rs",
    );
    assert!(
        source.contains("LINK_TYPES.join(\", \")"),
        "the message joins the constant"
    );
    for pair in LINK_TYPES.windows(2) {
        let (one, other) = (pair[0], pair[1]);
        assert!(
            !source.contains(&format!("{one}, {other}")),
            "front_matter.rs spells {one}, {other}"
        );
    }
}

/// AC-03 and "Messages", with the owner's ruling: `status` adds its list on
/// class spec alone, a decision's being another; `scope` and `reviewed` add
/// theirs on any class; any other key as today.
#[test]
fn ac03_a_missing_key_says_what_it_wants() {
    let corpus = Teaching::new(
        "fa-ac03-key",
        "",
        &[
            (
                "docs/features/change.md",
                "---\nclass: spec\n---\n\n# A change\n".to_owned(),
            ),
            (
                "docs/spec/bare.md",
                "---\nclass: canon\n---\n\n# Bare\n".to_owned(),
            ),
            (
                "docs/spec/ruling.md",
                "---\nid: NODE-ruling\nclass: decision\n---\n\n# A ruling\n".to_owned(),
            ),
        ],
    );
    let run = corpus.spec(&["check", "--json"]);
    run.code(1);
    let check = run.json();
    for (path, key, want) in [
        (
            "docs/features/change.md",
            "status",
            "key `status` is required for class spec: one of draft | in-progress | shipped | \
             abandoned",
        ),
        (
            "docs/features/change.md",
            "scope",
            "key `scope` is required for class spec: a list, as [root]",
        ),
        (
            "docs/spec/bare.md",
            "reviewed",
            "key `reviewed` is required for class canon: a date, YYYY-MM-DD",
        ),
        (
            "docs/spec/bare.md",
            "owner",
            "key `owner` is required for class canon",
        ),
        (
            "docs/spec/ruling.md",
            "status",
            "key `status` is required for class decision",
        ),
        (
            "docs/spec/ruling.md",
            "scope",
            "key `scope` is required for class decision: a list, as [root]",
        ),
    ] {
        let item = finding(&check, "key-missing", path, key);
        assert_eq!(item["message"], want, "{item}");
        assert_eq!(item["severity"], "error", "as it was: {item}");
    }
}

/// AC-03: `[ids] probe = 1` names no Rust type, on every read of the config;
/// still exit 2 at its line. An array reaches `RawPrefix` by position, so its
/// first item is what is refused: no table in the words, no Rust type either.
#[test]
fn ac03_a_prefix_that_is_no_table_names_no_rust_type() {
    for value in ["1", "\"x\"", "true", "[1]"] {
        let scratch = Scratch::new("fa-ac03-probe");
        let home = scratch.home("h");
        let root = scratch.dir("project");
        write(
            &root,
            "specengine.toml",
            format!("[project]\nslug = \"probe\"\n\n[ids]\nprobe = {value}\n"),
        );
        for args in [&["tree"][..], &["flow"], &["check"]] {
            let run = spec30(&home, &root, args);
            run.code(2);
            let said = format!("{}{}", run.stdout, run.stderr);
            assert!(
                said.contains("specengine.toml:5: ")
                    && (value == "[1]"
                        || said.contains("a prefix table: `kind`, then `shape` or `width`")),
                "{value} {args:?}: {}",
                run.show()
            );
            for word in ["RawPrefix", "struct", "Spanned"] {
                assert!(!said.contains(word), "{value} {args:?}: {}", run.show());
            }
        }
    }
}

/// AC-03: `no_selector` says what `when` holds, from `[flow]` (the health half
/// is `health_rows`, the one function). Followed, it reads.
#[test]
fn ac03_no_selector_says_what_when_holds() {
    let corpus = Teaching::new(
        "fa-ac03-selector",
        "\n[[flow.end]]\n",
        &[("docs/spec/a.md", canon("NODE-a", ""))],
    );
    let (run, document) = {
        let run = corpus.spec(&["flow", "--json"]);
        run.code(0);
        let document = run.json();
        (run, document)
    };
    let line = read_text(&corpus.root, "specengine.toml")
        .lines()
        .position(|line| line == "[[flow.end]]")
        .expect("the table")
        + 1;
    assert_eq!(
        document["gaps"][3]["code"],
        "no-way-to-end",
        "{}",
        run.show()
    );
    assert_eq!(document["gaps"][3]["state"], "cannot", "{}", run.show());
    assert_eq!(
        document["gaps"][3]["cause"],
        format!(
            "specengine.toml:{line}: `[[flow.end]]` without a selector: give `kinds`, \
             `classes`, `paths` or `when`; `when` is a table of front-matter key = \"value\""
        ),
        "{}",
        run.show()
    );

    let followed = Teaching::new(
        "fa-ac03-selector-followed",
        "\n[[flow.end]]\nwhen = { phase = \"last\" }\n",
        &[("docs/spec/a.md", canon("NODE-a", "phase: last\n"))],
    );
    let document = followed.spec(&["flow", "--json"]).json();
    assert_eq!(document["gaps"][3]["state"], "measured", "{document}");
}

/// AC-03: `id: TOPIC-one` under no such prefix shows a `TOPIC = ` line, the
/// prefix the part before the first `-`; an ID with no `-` keeps today's
/// text; still an error, as it was. `<kind>` stays as written: "Data" names
/// only `<PREFIX>` as filled in.
#[test]
fn ac03_an_id_under_no_prefix_shows_the_line_that_takes_it() {
    let today = |id: &str| {
        format!("`id: {id}` is not an ID of a prefix configured in `[ids]`; the document has no ID")
    };
    let files = [
        ("docs/spec/one.md", canon("TOPIC-one", "")),
        ("docs/spec/two.md", canon("TOPIC-two-three", "")),
        ("docs/spec/plain.md", canon("PLAIN", "")),
    ];
    let corpus = Teaching::new("fa-ac03-id", "", &files);
    let run = corpus.spec(&["check", "--json"]);
    run.code(1);
    let check = run.json();
    let line = "TOPIC = { kind = \"<kind>\", shape = \"name\" }";
    for (path, id) in [
        ("docs/spec/one.md", "TOPIC-one"),
        ("docs/spec/two.md", "TOPIC-two-three"),
    ] {
        let item = finding(&check, "id-not-in-scheme", path, id);
        assert_eq!(
            item["message"],
            format!("{}; add to [ids]: {line}", today(id)),
            "{item}"
        );
        assert_eq!(item["severity"], "error", "as it was: {item}");
        assert_eq!(item["line"], json!(2), "{item}");
    }
    assert_eq!(
        finding(&check, "id-not-in-scheme", "docs/spec/plain.md", "PLAIN")["message"],
        today("PLAIN"),
        "{check}"
    );

    // Followed, the line takes both IDs.
    let tail = format!("{}\n", line.replace("<kind>", "topic"));
    let followed = Teaching::new("fa-ac03-id-followed", &tail, &files);
    let check = followed.check();
    let left: Vec<&str> = findings(&check, "id-not-in-scheme")
        .iter()
        .map(|item| item["subject"].as_str().expect("a subject"))
        .collect();
    assert_eq!(left, ["PLAIN"], "{check}");
}

/// The owner's ruling on `id-not-in-scheme`: the line fires only when the part
/// before the first `-` is `[A-Z][A-Z0-9]*`, is neither a configured prefix
/// nor an alias, and the body reads under the `name` shape the line would
/// give. Every other shape keeps today's text: `topic-low` (`[ids]` would
/// refuse the prefix), a mixed-script prefix (ADR-0009), `NODE-Bad_Name!` (the
/// prefix is held), an alias's, and three bodies `name_body` refuses:
/// `TOPIC-a_b`, one with a Cyrillic letter and the empty `TOPIC-`, for which
/// the line would change nothing. A shape that passes still fires, digits and
/// all. M: the hint on every ID with a `-`.
#[test]
fn an_id_keeps_todays_text_unless_its_prefix_could_be_added() {
    let today = |id: &str| {
        format!("`id: {id}` is not an ID of a prefix configured in `[ids]`; the document has no ID")
    };
    let mixed = "T\u{041E}PIC-one";
    let cyrillic_body = "TOPIC-a\u{0431}";
    let corpus = Teaching::new(
        "fa-ac03-id-edges",
        "TASK = { kind = \"task\", width = 4, aliases_from = [\"JOB\"] }\n",
        &[
            ("docs/spec/low.md", canon("topic-low", "")),
            ("docs/spec/mixed.md", canon(mixed, "")),
            ("docs/spec/bad.md", canon("NODE-Bad_Name!", "")),
            ("docs/spec/alias.md", canon("JOB-x", "")),
            ("docs/spec/held.md", canon("TASK-12", "")),
            ("docs/spec/snake.md", canon("TOPIC-a_b", "")),
            ("docs/spec/script.md", canon(cyrillic_body, "")),
            ("docs/spec/empty.md", canon("TOPIC-", "")),
            ("docs/spec/fires.md", canon("R2D2-one", "")),
        ],
    );
    let run = corpus.spec(&["check", "--json"]);
    run.code(1);
    let check = run.json();
    for item in check["findings"].as_array().expect("findings") {
        let message = item["message"].as_str().expect("a message");
        if item["path"] == "docs/spec/fires.md" {
            assert_eq!(
                message,
                format!(
                    "{}; add to [ids]: R2D2 = {{ kind = \"<kind>\", shape = \"name\" }}",
                    today("R2D2-one")
                ),
                "{item}"
            );
        } else {
            assert!(!message.contains("add to [ids]"), "{item}");
        }
    }
    for (path, id) in [
        ("docs/spec/low.md", "topic-low"),
        ("docs/spec/bad.md", "NODE-Bad_Name!"),
        ("docs/spec/snake.md", "TOPIC-a_b"),
        ("docs/spec/script.md", cyrillic_body),
        ("docs/spec/empty.md", "TOPIC-"),
    ] {
        assert_eq!(
            finding(&check, "id-not-in-scheme", path, id)["message"],
            today(id),
            "{check}"
        );
    }
}

// ------------------------------------------------------------------- Rules

/// "Rules": a key of the project's own in `when` still draws `unknown-key`, by
/// design; the selector works all the same; `paths` needs none (AC-01 reads 0
/// warnings over the skill's own config).
#[test]
fn a_key_of_the_projects_own_in_when_still_draws_unknown_key() {
    let corpus = Teaching::new(
        "fa-when",
        "\n[[flow.start]]\nwhen = { phase = \"first\" }\n",
        &[("docs/spec/a.md", canon("NODE-a", "phase: first\n"))],
    );
    let check = corpus.check();
    assert_eq!(check["counts"]["errors"], json!(0), "{check}");
    assert_eq!(check["counts"]["warnings"], json!(1), "{check}");
    assert_eq!(
        findings(&check, "unknown-key").len(),
        1,
        "the project's own key: {check}"
    );
    let document = corpus.spec(&["flow", "--json"]).json();
    let node = document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["id"] == "NODE-a")
        .expect("NODE-a");
    assert_eq!(node["start"], json!(true), "{document}");
}
