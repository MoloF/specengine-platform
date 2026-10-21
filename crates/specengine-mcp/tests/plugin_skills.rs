//! docs/features/plugin-skills.md over the committed skill files and the
//! default build of `specengine-mcp`, spawned as `mcp_intake.rs` spawns it
//! (a cleared environment, a scratch `HOME`). Every check is a function of
//! the skills (directory name → text) and, where it names the tools, of
//! the live `tools/list`; the committed skills give no problem, and each
//! named mutation, applied to an in-memory copy, gives the problem it
//! names.
//!
//! - AC-06: every backticked `^[a-z][a-z0-9]*(_[a-z0-9]+)+$` in a skill is
//!   a tool or a property name anywhere in an input or output schema; all
//!   fifteen tools (eight, docs/features/task-package.md's five,
//!   docs/features/kind-decision.md's `report_decision` and
//!   docs/features/drift-queue.md's `report_drift`) backticked in
//!   some skill; no `mcp__`. M: `get_bundle`;
//!   `node_id`; `get_proposal` gone everywhere;
//!   `mcp__plugin_specengine_specengine__search`.
//! - AC-07: each `json` block is an object; as `tools/call` arguments of
//!   the tool named on the line above it (a temp git repository of
//!   `fixtures/spec-a`, a scratch `HOME`) no text starts with rmcp's
//!   `failed to deserialize parameters`; the `ask_question` example in
//!   `ask-owner` and the `propose_change` one in `propose-spec-change`
//!   exist. M: an extra key; `"severity": "urgent"` (a `report_discrepancy`
//!   example); the `ask_question` example deleted.
//! - AC-08: under `plugin/` no P2-3 word (`STACK_WORDS`), `mechanic`,
//!   `edge-case` (`has_word`); in skills, outside `json` blocks, no ASCII
//!   digit but a line-start `N. ` and no backticked enum value of an input
//!   schema. M: `.mcp.json` `command` `${HOME}/.cargo/bin/specengine-mcp`;
//!   `spec-writer`; "react"; "at most 16 IDs"; `` `normal` ``.
//! - AC-11: each skill backticks the names AC-11 lists for it and holds
//!   the precedence sentence. M: `distinct_from` out of `ask-owner`; the
//!   sentence dropped from one.
//! - docs/features/plugin-gate.md AC-15: five skills (`analyze`,
//!   `implement` join the three) under the same rules; `implement`
//!   backticks `claim_task`, `report_run`, `complete_task` and the table's
//!   arguments, `analyze` `submit_plan` and its arguments. M: `claim_task`
//!   dropped; `submit_plan` dropped.
//! - docs/features/plugin-roles.md R-6, T-3, T-4: six skills (`feature`
//!   joins the five) under the same rules, `feature` backticking
//!   `get_task`, `submit_plan`, `claim_task`, `report_run`,
//!   `complete_task`, `ask_question`, `report_discrepancy`, `task_id`;
//!   sixteen files under `plugin/` (the five agents too) in the word scan.
//!   M: `claim_task` dropped from `feature`; "at most 3 rounds" in it;
//!   `cargo` in the implementer. The agents' own rules:
//!   `plugin_roles.rs`.
//! - docs/features/code-tools.md RD-9 ("Plugin", 0.9.0): the eighteen
//!   tools (`find_symbols`, `refs`, `unmapped` join) backticked across the
//!   skills; `read-spec` backticks the three after the bundle and before
//!   its files step; the descriptions unchanged (`read-spec`'s byte for
//!   byte, the five model-invoked ones 1 170 bytes together, of 1 200).
//!   M: a code tool unnamed; the three before the bundle; a description
//!   edited or over 1 200 bytes.
//! - docs/features/bevy-roles-reads.md "Plugin" (P1): 0.10.0, `find_symbols`'
//!   `roles` (a key a tool returns: MINOR); no skill text changes.
//! - docs/features/roadmap-surfaces.md RD-10 ("Plugin", 0.11.0): the
//!   nineteenth tool, `get_roadmap`, backticked in `analyze` with the
//!   package's `roadmap`, and only there of the skills; the plan named
//!   advisory, its items changed only through the owner. M: `get_roadmap`
//!   unnamed.
//!
//! Compiles to nothing with `--features probes` (the ACs name the default
//! build). The repository is only read.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use common::read::{
    BAD_ARGUMENTS, CODE_TOOLS, Era, STACK_WORDS, Session, TOOLS, content_text, has_word,
};
use common::*;
use git::Sandbox;
use serde_json::Value;

/// The precedence sentence every skill holds verbatim (AC-11; the root
/// `README.md` "Claude Code plugin", its Content bullet).
const PRECEDENCE: &str = "The project's CLAUDE.md and its roles take precedence over this skill.";

/// AC-11's list: what each skill backticks.
const NAMES: [(&str, &[&str]); 6] = [
    // docs/features/code-tools.md "Plugin": the three code tools join.
    (
        "read-spec",
        &[
            "get_context_bundle",
            "get_node",
            "search",
            "get_tree",
            "find_symbols",
            "refs",
            "unmapped",
        ],
    ),
    (
        "ask-owner",
        &[
            "ask_question",
            "report_discrepancy",
            "get_proposal",
            "distinct_from",
            "working_answer",
            // docs/canon/plugin.md "Skills" (decision-apply).
            "record_id",
            // docs/features/drift-queue.md "report_drift and texts".
            "report_drift",
        ],
    ),
    ("propose-spec-change", &["propose_change", "span_hash"]),
    // docs/canon/plugin.md "Skills"; AC-15 of docs/features/plugin-gate.md.
    (
        "analyze",
        &[
            "get_task",
            "get_context_bundle",
            "ask_question",
            "report_discrepancy",
            "task_id",
            "working_answer",
            "submit_plan",
            "plan_md",
            "criteria",
            "affected_nodes",
            // docs/features/roadmap-surfaces.md RD-10 ("Plugin", 0.11.0).
            "get_roadmap",
            "roadmap",
        ],
    ),
    (
        "implement",
        &[
            "get_task",
            "claim_task",
            "role",
            "worktree",
            "task_id",
            "report_run",
            "outcome",
            "summary",
            "changed_files",
            "complete_task",
            // docs/features/check-binding.md "Plugin" (0.12.0).
            "check_binding",
        ],
    ),
    // docs/features/plugin-roles.md R-6.
    (
        "feature",
        &[
            "get_task",
            "submit_plan",
            "claim_task",
            "report_run",
            "complete_task",
            "ask_question",
            "report_discrepancy",
            "task_id",
        ],
    ),
];

/// The words no file under `plugin/` holds besides P2-3's (AC-08; the root
/// `README.md` "Claude Code plugin", its Content bullet).
const NEVER_WORDS: [&str; 2] = ["mechanic", "edge-case"];

/// The skills by directory name, their text.
type Skills = BTreeMap<String, String>;

fn plugin_dir() -> PathBuf {
    repository_root().join("plugin")
}

/// The committed skills under `plugin/specengine/skills/`.
fn committed() -> Skills {
    let dir = plugin_dir().join("specengine").join("skills");
    let mut skills = Skills::new();
    for entry in fs::read_dir(&dir).expect("the skills directory") {
        let entry = entry.expect("an entry");
        if !entry.file_type().expect("a file type").is_dir() {
            continue;
        }
        let name = entry.file_name().to_str().expect("UTF-8").to_owned();
        let text = fs::read_to_string(entry.path().join("SKILL.md"))
            .unwrap_or_else(|e| panic!("{name}/SKILL.md: {e}"));
        skills.insert(name, text);
    }
    assert_eq!(
        skills.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "analyze",
            "ask-owner",
            "feature",
            "implement",
            "propose-spec-change",
            "read-spec"
        ]
    );
    skills
}

/// `skills` with the one occurrence of `from` in `skill` replaced by `to`.
fn edited(skills: &Skills, skill: &str, from: &str, to: &str) -> Skills {
    let mut skills = skills.clone();
    let text = &skills[skill];
    assert_eq!(text.matches(from).count(), 1, "{skill}: {from:?} once");
    let text = text.replacen(from, to, 1);
    skills.insert(skill.to_owned(), text);
    skills
}

/// One fenced `json` block: the line right above its fence, its body, the
/// 1-based line of the fence.
struct Block {
    above: String,
    body: String,
    line: usize,
}

/// The lines outside the `json` blocks (1-based number, text), and the
/// blocks.
type Split<'t> = (Vec<(usize, &'t str)>, Vec<Block>);

/// A skill split at its `json` fences: the other lines (1-based number,
/// text) and the blocks. An unclosed fence is a problem.
fn split(text: &str) -> Result<Split<'_>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut outside = Vec::new();
    let mut blocks = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        if lines[at] == "```json" {
            let close = lines[at + 1..]
                .iter()
                .position(|line| *line == "```")
                .ok_or(format!("line {}: an unclosed json fence", at + 1))?;
            blocks.push(Block {
                above: if at == 0 {
                    String::new()
                } else {
                    lines[at - 1].to_owned()
                },
                body: lines[at + 1..at + 1 + close].join("\n"),
                line: at + 1,
            });
            at += close + 2;
        } else {
            outside.push((at + 1, lines[at]));
            at += 1;
        }
    }
    Ok((outside, blocks))
}

/// The backticked spans of `line` (single backticks, non-empty).
fn spans(line: &str) -> Vec<&str> {
    let parts: Vec<&str> = line.split('`').collect();
    parts
        .iter()
        .enumerate()
        .filter(|(index, part)| index % 2 == 1 && index + 1 < parts.len() && !part.is_empty())
        .map(|(_, part)| *part)
        .collect()
}

/// Every backticked span outside the `json` blocks of `text`.
fn backticked(text: &str) -> Vec<String> {
    let (outside, _) = split(text).unwrap_or_default();
    outside
        .iter()
        .flat_map(|(_, line)| spans(line))
        .map(str::to_owned)
        .collect()
}

/// `^[a-z][a-z0-9]*(_[a-z0-9]+)+$`.
fn is_snake(token: &str) -> bool {
    let mut parts = token.split('_');
    let first = parts.next().unwrap_or_default();
    let rest: Vec<&str> = parts.collect();
    first.starts_with(|c: char| c.is_ascii_lowercase())
        && first
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && !rest.is_empty()
        && rest.iter().all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// What the default build's `tools/list` names.
struct Listed {
    tools: BTreeSet<String>,
    /// Every key of a `properties` object anywhere in an input or output
    /// schema.
    properties: BTreeSet<String>,
    /// Every string of an `enum` anywhere in an input schema.
    input_enums: BTreeSet<String>,
}

fn collect(schema: &Value, properties: &mut BTreeSet<String>, enums: &mut BTreeSet<String>) {
    match schema {
        Value::Object(object) => {
            for (key, value) in object {
                if key == "properties"
                    && let Some(names) = value.as_object()
                {
                    properties.extend(names.keys().cloned());
                }
                if key == "enum"
                    && let Some(values) = value.as_array()
                {
                    enums.extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
                }
                collect(value, properties, enums);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect(value, properties, enums);
            }
        }
        _ => {}
    }
}

/// The default build's `tools/list`, from a fresh server.
fn listed() -> Listed {
    let mut session = Session::open(Era::Legacy, &[], None, Home::Fresh);
    let list = session.tools();
    session.finish();
    let mut listed = Listed {
        tools: BTreeSet::new(),
        properties: BTreeSet::new(),
        input_enums: BTreeSet::new(),
    };
    for tool in list["tools"].as_array().expect("tools") {
        listed
            .tools
            .insert(tool["name"].as_str().expect("a name").to_owned());
        collect(
            &tool["inputSchema"],
            &mut listed.properties,
            &mut listed.input_enums,
        );
        let mut output_enums = BTreeSet::new();
        collect(
            &tool["outputSchema"],
            &mut listed.properties,
            &mut output_enums,
        );
    }
    assert_eq!(
        listed.tools.iter().map(String::as_str).collect::<Vec<_>>(),
        TOOLS,
        "the default build's twenty tools"
    );
    assert!(
        listed.properties.contains("node_ids"),
        "properties collected"
    );
    assert!(listed.input_enums.contains("high"), "input enums collected");
    listed
}

fn assert_names(problems: &[String], needle: &str, mutation: &str) {
    assert!(
        problems.iter().any(|problem| problem.contains(needle)),
        "{mutation}: want a problem naming {needle:?}, got {problems:#?}"
    );
}

// ------------------------------------------------------------------ AC-06

fn ac06_problems(skills: &Skills, listed: &Listed) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for (skill, text) in skills {
        if text.contains("mcp__") {
            problems.push(format!("{skill}: holds `mcp__`"));
        }
        for token in backticked(text) {
            if is_snake(&token)
                && !listed.tools.contains(&token)
                && !listed.properties.contains(&token)
            {
                problems.push(format!(
                    "{skill}: `{token}` is no tool and no schema property"
                ));
            }
            seen.insert(token);
        }
    }
    for tool in &listed.tools {
        if !seen.contains(tool) {
            problems.push(format!("skills: tool `{tool}` backticked nowhere"));
        }
    }
    problems
}

#[test]
fn ac06_skills_name_only_live_tools_and_properties() {
    let skills = committed();
    let listed = listed();
    assert_eq!(ac06_problems(&skills, &listed), Vec::<String>::new());

    // M: `get_bundle`.
    let bundle = edited(&skills, "read-spec", "`get_context_bundle`", "`get_bundle`");
    assert_names(
        &ac06_problems(&bundle, &listed),
        "`get_bundle` is no tool",
        "get_bundle",
    );
    // M: `node_id`.
    let node_id = edited(
        &skills,
        "ask-owner",
        "about in `node_ids`",
        "about in `node_id`",
    );
    assert_names(
        &ac06_problems(&node_id, &listed),
        "`node_id` is no tool",
        "node_id",
    );
    // M: `get_proposal` gone everywhere.
    let mut gone = skills.clone();
    for text in gone.values_mut() {
        *text = text.replace("`get_proposal`", "the status tool");
    }
    assert_ne!(gone, skills);
    assert_names(
        &ac06_problems(&gone, &listed),
        "tool `get_proposal` backticked nowhere",
        "get_proposal gone",
    );
    // M: the prefixed name.
    let prefixed = edited(
        &skills,
        "read-spec",
        "a term or a behaviour without a known ID: `search`",
        "a term or a behaviour without a known ID: `mcp__plugin_specengine_specengine__search`",
    );
    assert_names(
        &ac06_problems(&prefixed, &listed),
        "read-spec: holds `mcp__`",
        "mcp__ prefix",
    );
    // A non-snake span is not checked; a snake one in a json block neither.
    assert!(!is_snake("mcp__plugin_specengine_specengine__search"));
    assert!(!is_snake("search") && !is_snake("_x") && !is_snake("x_") && !is_snake("A_b"));
    assert!(is_snake("get_context_bundle") && is_snake("x2_y3"));
}

// ------------------------------------------------------------------ AC-07

/// A temp git repository of `fixtures/spec-a` (one commit) and a scratch
/// data `HOME`.
struct Repo {
    _scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy("spec-a", "repo");
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the fixture");
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            root,
            home,
        }
    }
}

/// The tool a block's line above names: its one backticked tool.
fn named_tool(block: &Block) -> Option<&str> {
    let named: Vec<&str> = spans(&block.above)
        .into_iter()
        .filter(|span| TOOLS.contains(span))
        .collect();
    match named[..] {
        [tool] => Some(tool),
        _ => None,
    }
}

/// AC-07 through `session`; returns the problems and the `(skill, tool)`
/// of every block that was called.
fn ac07_problems(skills: &Skills, session: &mut Session) -> (Vec<String>, Vec<(String, String)>) {
    let mut problems = Vec::new();
    let mut called = Vec::new();
    for (skill, text) in skills {
        let blocks = match split(text) {
            Ok((_, blocks)) => blocks,
            Err(problem) => {
                problems.push(format!("{skill}: {problem}"));
                continue;
            }
        };
        for block in blocks {
            let at = format!("{skill}:{}", block.line);
            let arguments: Value = match serde_json::from_str(&block.body) {
                Ok(value) => value,
                Err(e) => {
                    problems.push(format!("{at}: the json block does not parse: {e}"));
                    continue;
                }
            };
            if !arguments.is_object() {
                problems.push(format!("{at}: the json block is not an object"));
                continue;
            }
            let Some(tool) = named_tool(&block) else {
                problems.push(format!(
                    "{at}: the line above names no one tool: {:?}",
                    block.above
                ));
                continue;
            };
            let reply = session.call(tool, arguments);
            if let Some(error) = reply.get("error") {
                problems.push(format!("{at}: `{tool}` is a JSON-RPC error: {error}"));
                continue;
            }
            let text = content_text(result(&reply));
            if text.starts_with(BAD_ARGUMENTS.trim_end()) {
                problems.push(format!("{at}: `{tool}` refuses the arguments: {text}"));
            }
            called.push((skill.clone(), tool.to_owned()));
        }
    }
    for (skill, tool) in [
        ("ask-owner", "ask_question"),
        ("propose-spec-change", "propose_change"),
    ] {
        if !called
            .iter()
            .any(|(s, t)| s.as_str() == skill && t.as_str() == tool)
        {
            problems.push(format!("{skill}: no `{tool}` example"));
        }
    }
    (problems, called)
}

#[test]
fn ac07_the_examples_are_arguments_the_server_takes() {
    let skills = committed();
    let repo = Repo::new("plugin-examples");
    let mut session = Session::open(Era::Legacy, &[], Some(&repo.root), Home::At(&repo.home));
    let (problems, called) = ac07_problems(&skills, &mut session);
    assert_eq!(problems, Vec::<String>::new());
    assert_eq!(
        called,
        [
            ("ask-owner".to_owned(), "ask_question".to_owned()),
            (
                "propose-spec-change".to_owned(),
                "propose_change".to_owned()
            ),
        ]
    );

    // M: an extra key.
    let extra = edited(
        &skills,
        "ask-owner",
        "  \"node_ids\": [\"<ID>\"],\n",
        "  \"node_ids\": [\"<ID>\"],\n  \"priority\": \"<a word>\",\n",
    );
    let (problems, _) = ac07_problems(&extra, &mut session);
    assert_names(
        &problems,
        "`ask_question` refuses the arguments",
        "an extra key",
    );

    // M: `"severity": "urgent"` in a `report_discrepancy` example (the same
    // example with `high` is taken).
    let discrepancy = |severity: &str| {
        format!(
            "Arguments of `report_discrepancy`:\n```json\n{{\n  \"node_ids\": [\"<ID>\"],\n  \
             \"summary\": \"<what disagrees>\",\n  \"gap_type\": \"contradicts\",\n  \
             \"severity\": \"{severity}\",\n  \"evidence\": [{{\"file\": \"<path>\", \
             \"observed\": \"<what the code does>\", \"documented\": \"<what the spec says>\"}}],\n  \
             \"options\": [{{\"label\": \"<a fix>\", \"effect\": \"<its effect>\", \"price\": \"<its price>\"}}],\n  \
             \"recommendation\": 0,\n  \"working_answer\": \"<the answer you work with>\",\n  \
             \"author_role\": \"<your-role>\"\n}}\n```\n"
        )
    };
    let with_example = |severity: &str| {
        let mut skills = skills.clone();
        let text = skills.get_mut("ask-owner").unwrap();
        text.push('\n');
        text.push_str(&discrepancy(severity));
        skills
    };
    let (problems, called) = ac07_problems(&with_example("high"), &mut session);
    assert_eq!(problems, Vec::<String>::new(), "severity high");
    assert!(called.contains(&("ask-owner".to_owned(), "report_discrepancy".to_owned())));
    let (problems, _) = ac07_problems(&with_example("urgent"), &mut session);
    assert_names(
        &problems,
        "`report_discrepancy` refuses the arguments",
        "severity urgent",
    );

    // M: the `ask_question` example deleted.
    let text = &skills["ask-owner"];
    let fence = text.find("```json").expect("the example");
    let mut deleted = skills.clone();
    deleted.insert("ask-owner".to_owned(), text[..fence].to_owned());
    let (problems, _) = ac07_problems(&deleted, &mut session);
    assert_names(&problems, "ask-owner: no `ask_question` example", "deleted");

    // Not an object; not JSON; no tool named above.
    for (from, to, needle) in [
        ("```json\n{", "```json\n[{", "does not parse"),
        (
            "Arguments of `ask_question`:",
            "Arguments:",
            "names no one tool",
        ),
    ] {
        let mutated = edited(&skills, "ask-owner", from, to);
        let (problems, _) = ac07_problems(&mutated, &mut session);
        assert_names(&problems, needle, to);
    }
    let text = &skills["ask-owner"];
    let fence = text.find("```json").expect("the example");
    let mut array = skills.clone();
    array.insert(
        "ask-owner".to_owned(),
        format!("{}```json\n[\"<ID>\"]\n```\n", &text[..fence]),
    );
    let (problems, _) = ac07_problems(&array, &mut session);
    assert_names(&problems, "is not an object", "an array");
    session.finish();
}

// ------------------------------------------------------------------ AC-08

/// Every file under `plugin/`, `.DS_Store` aside: relative path, text.
fn plugin_files() -> BTreeMap<String, String> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(dir).expect("a directory") {
            let entry = entry.expect("an entry");
            let kind = entry.file_type().expect("a file type");
            if kind.is_dir() {
                walk(root, &entry.path(), out);
            } else if kind.is_file() && entry.file_name() != ".DS_Store" {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .expect("UTF-8")
                    .to_owned();
                let text = fs::read_to_string(entry.path()).expect("a text file");
                out.insert(relative, text);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&plugin_dir(), &plugin_dir(), &mut out);
    assert_eq!(out.len(), 16, "{:?}", out.keys());
    for agent in [
        "analyst",
        "implementer",
        "reviewer",
        "spec-author",
        "tester",
    ] {
        let path = format!("specengine/agents/{agent}.md");
        assert!(out.contains_key(&path), "{path} scanned");
    }
    out
}

/// The words over every file under `plugin/`.
fn word_problems(files: &BTreeMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    for (path, text) in files {
        for word in STACK_WORDS.iter().chain(&NEVER_WORDS) {
            if has_word(text, word) {
                problems.push(format!("plugin/{path}: the word {word:?}"));
            }
        }
    }
    problems
}

/// Digits and backticked enum values outside the `json` blocks of skills.
fn skill_text_problems(skills: &Skills, listed: &Listed) -> Vec<String> {
    let mut problems = Vec::new();
    for (skill, text) in skills {
        let Ok((outside, _)) = split(text) else {
            problems.push(format!("{skill}: an unclosed json fence"));
            continue;
        };
        for (number, line) in outside {
            let digits = line.trim_start_matches(|c: char| c.is_ascii_digit());
            let rest = if digits.len() < line.len() {
                digits.strip_prefix(". ").unwrap_or(line)
            } else {
                line
            };
            if rest.bytes().any(|b| b.is_ascii_digit()) {
                problems.push(format!("{skill}:{number}: a digit: {line:?}"));
            }
            for span in spans(line) {
                if listed.input_enums.contains(span) {
                    problems.push(format!("{skill}:{number}: the enum value `{span}`"));
                }
            }
        }
    }
    problems
}

#[test]
fn ac08_no_stack_word_digit_or_enum_value() {
    let files = plugin_files();
    let skills = committed();
    let listed = listed();
    assert_eq!(word_problems(&files), Vec::<String>::new());
    assert_eq!(skill_text_problems(&skills, &listed), Vec::<String>::new());

    // M: `.mcp.json` running the binary from the cargo directory.
    let mut cargo = files.clone();
    let mcp = cargo.get_mut("specengine/.mcp.json").unwrap();
    assert_eq!(mcp.matches("\"specengine-mcp\"").count(), 1);
    *mcp = mcp.replace(
        "\"specengine-mcp\"",
        "\"${HOME}/.cargo/bin/specengine-mcp\"",
    );
    assert_names(
        &word_problems(&cargo),
        "plugin/specengine/.mcp.json: the word \"cargo\"",
        "${HOME}/.cargo/bin",
    );

    // M: a role name, "react", a P2-3-free word list kept.
    let skill_file = "specengine/skills/ask-owner/SKILL.md";
    for (insert, word) in [
        ("as the project names it, say spec-writer", "spec-writer"),
        ("as the project names it; react to the reply", "react"),
        ("as the project names it; mind the mechanic", "mechanic"),
        ("as the project names it; an edge-case too", "edge-case"),
    ] {
        let mut mutated = files.clone();
        let text = mutated.get_mut(skill_file).unwrap();
        assert_eq!(text.matches("as the project names it.").count(), 1);
        *text = text.replacen("as the project names it.", &format!("{insert}."), 1);
        assert_names(
            &word_problems(&mutated),
            &format!("plugin/{skill_file}: the word {word:?}"),
            word,
        );
    }
    // Whole words only: "reaction", "nested", "cargoes" pass.
    let mut words = files.clone();
    words
        .get_mut(skill_file)
        .unwrap()
        .push_str("\nA reaction, nested lists, cargoes.\n");
    assert_eq!(word_problems(&words), Vec::<String>::new());

    // M: "at most 16 IDs".
    let digits = edited(
        &skills,
        "read-spec",
        "Call `get_context_bundle` with the IDs",
        "Call `get_context_bundle` with at most 16 IDs",
    );
    assert_names(
        &skill_text_problems(&digits, &listed),
        "read-spec:",
        "at most 16 IDs",
    );
    assert_names(&skill_text_problems(&digits, &listed), "a digit", "16");
    // A line-start `N. ` passes; a digit after it does not.
    let listed_step = edited(
        &skills,
        "read-spec",
        "3. **Files last.**",
        "4. **Files last.**",
    );
    assert_eq!(
        skill_text_problems(&listed_step, &listed),
        Vec::<String>::new()
    );
    let after_step = edited(
        &skills,
        "read-spec",
        "3. **Files last.**",
        "3. **Files last, step 3.**",
    );
    assert_names(
        &skill_text_problems(&after_step, &listed),
        "a digit",
        "step 3",
    );
    let indented = edited(
        &skills,
        "read-spec",
        "3. **Files last.**",
        " 3. **Files last.**",
    );
    assert_names(
        &skill_text_problems(&indented, &listed),
        "a digit",
        "indented",
    );
    // Digits inside a json block pass (`"recommendation": 0` would).
    let in_block = edited(
        &skills,
        "ask-owner",
        "  \"node_ids\": [\"<ID>\"],\n",
        "  \"node_ids\": [\"<ID>\", \"<ID2>\"],\n",
    );
    assert_eq!(
        skill_text_problems(&in_block, &listed),
        Vec::<String>::new()
    );

    // plugin-roles T-4, M: "at most 3 rounds" in `feature` (the bound in
    // words).
    let mut rounds = skills.clone();
    rounds
        .get_mut("feature")
        .unwrap()
        .push_str("\nAt most 3 rounds.\n");
    assert_names(
        &skill_text_problems(&rounds, &listed),
        "feature:",
        "at most 3 rounds",
    );
    assert_names(
        &skill_text_problems(&rounds, &listed),
        "a digit",
        "3 rounds",
    );
    // plugin-roles T-4, M: `cargo` in the implementer (the word scan covers
    // the agents).
    let mut agent = files.clone();
    agent
        .get_mut("specengine/agents/implementer.md")
        .unwrap()
        .push_str("\nBuild with cargo.\n");
    assert_names(
        &word_problems(&agent),
        "plugin/specengine/agents/implementer.md: the word \"cargo\"",
        "cargo in the implementer",
    );

    // M: a backticked enum value.
    let normal = edited(
        &skills,
        "ask-owner",
        "its `severity`,",
        "its `severity` (say `normal`),",
    );
    assert_names(
        &skill_text_problems(&normal, &listed),
        "the enum value `normal`",
        "`normal`",
    );
}

// ------------------------------------------------------------------ AC-11

fn ac11_problems(skills: &Skills) -> Vec<String> {
    let mut problems = Vec::new();
    for (skill, names) in NAMES {
        let Some(text) = skills.get(skill) else {
            problems.push(format!("{skill}: missing"));
            continue;
        };
        if !text.contains(PRECEDENCE) {
            problems.push(format!("{skill}: no precedence sentence"));
        }
        let spans = backticked(text);
        for name in names {
            if !spans.iter().any(|span| span == name) {
                problems.push(format!("{skill}: `{name}` not backticked"));
            }
        }
    }
    problems
}

#[test]
fn ac11_each_skill_names_its_names_and_yields_to_the_project() {
    let skills = committed();
    assert_eq!(ac11_problems(&skills), Vec::<String>::new());

    // M: `distinct_from` out of `ask-owner`.
    let out = edited(
        &skills,
        "ask-owner",
        "with every hit in `distinct_from`",
        "with every hit listed as distinct",
    );
    assert_names(
        &ac11_problems(&out),
        "ask-owner: `distinct_from` not backticked",
        "distinct_from",
    );
    // Unbackticked is not enough.
    let bare = edited(
        &skills,
        "ask-owner",
        "with every hit in `distinct_from`",
        "with every hit in distinct_from",
    );
    assert_names(
        &ac11_problems(&bare),
        "`distinct_from` not backticked",
        "bare",
    );

    // M: the sentence dropped from one (each in turn).
    for skill in skills.keys() {
        let dropped = edited(&skills, skill, &format!("{PRECEDENCE}\n"), "");
        assert_names(
            &ac11_problems(&dropped),
            &format!("{skill}: no precedence sentence"),
            skill,
        );
    }
    // Reworded is not verbatim.
    let reworded = edited(
        &skills,
        "propose-spec-change",
        PRECEDENCE,
        "The project's CLAUDE.md and roles take precedence over this skill.",
    );
    assert_names(
        &ac11_problems(&reworded),
        "propose-spec-change: no precedence sentence",
        "reworded",
    );

    // plugin-gate AC-15, M: `claim_task` dropped from `implement` (the
    // tool stays backticked in `analyze`, so only this check sees it).
    let unclaimed = edited(&skills, "implement", "`claim_task`", "the claim tool");
    assert_names(
        &ac11_problems(&unclaimed),
        "implement: `claim_task` not backticked",
        "claim_task dropped",
    );
    let listed = listed();
    assert_eq!(ac06_problems(&unclaimed, &listed), Vec::<String>::new());
    // M: `submit_plan` dropped from `analyze`.
    let unplanned = edited(&skills, "analyze", "`submit_plan`", "the plan tool");
    assert_names(
        &ac11_problems(&unplanned),
        "analyze: `submit_plan` not backticked",
        "submit_plan dropped",
    );
    // plugin-roles T-3, M: `claim_task` dropped from `feature` (still in
    // `implement` and `analyze`, so only this check sees it); each of
    // `feature`'s names in turn.
    for name in NAMES
        .iter()
        .find(|(skill, _)| *skill == "feature")
        .expect("feature's names")
        .1
    {
        let mut out = skills.clone();
        let text = out.get_mut("feature").unwrap();
        *text = text.replace(&format!("`{name}`"), "the tool");
        assert_names(
            &ac11_problems(&out),
            &format!("feature: `{name}` not backticked"),
            name,
        );
    }
    // `report_run` and `complete_task` out of `implement`.
    for name in ["report_run", "complete_task"] {
        let mut out = skills.clone();
        let text = out.get_mut("implement").unwrap();
        *text = text.replace(&format!("`{name}`"), "the tool");
        assert_names(
            &ac11_problems(&out),
            &format!("implement: `{name}` not backticked"),
            name,
        );
    }
}

// ------------------------------------------- decision-apply AC-16 (skill)

/// The lines of `text` (outside `json` blocks) that are bullets: `- …`.
fn bullets(text: &str) -> Vec<String> {
    let (outside, _) = split(text).unwrap_or_default();
    outside
        .iter()
        .map(|(_, line)| line.trim_start())
        .filter(|line| line.starts_with("- "))
        .map(str::to_owned)
        .collect()
}

/// docs/canon/plugin.md "Skills": `ask-owner` teaches
/// a queue hit with a `record` (the owner's decision: `get_node` on it, off
/// this branch `get_proposal` and its `choice`), one with only an
/// `answer` (rejected, the owner's answer), and an applied item's
/// `get_proposal` (`record_id`, `record_title`, `choice`,
/// `decision_note`); `choice` counts as the owner's answer.
fn ac16_problems(skills: &Skills) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(text) = skills.get("ask-owner") else {
        return vec!["ask-owner: missing".to_owned()];
    };
    let has =
        |line: &str, names: &[&str]| names.iter().all(|name| line.contains(&format!("`{name}`")));
    let items = bullets(text);
    if !items.iter().any(|line| {
        line.contains("with a `record`")
            && has(line, &["get_node", "record", "get_proposal", "choice"])
    }) {
        problems.push(
            "ask-owner: no hit with a `record` read by `get_node`, else `get_proposal`'s `choice`"
                .to_owned(),
        );
    }
    if !items
        .iter()
        .any(|line| line.contains("with only an `answer`") && line.contains("rejected"))
    {
        problems.push("ask-owner: no hit with only an `answer` (rejected)".to_owned());
    }
    if !items.iter().any(|line| {
        has(
            line,
            &[
                "get_proposal",
                "record_id",
                "record_title",
                "choice",
                "decision_note",
            ],
        )
    }) {
        problems.push(
            "ask-owner: no applied item's `get_proposal` with `record_id`, `record_title`, \
             `choice`"
                .to_owned(),
        );
    }
    let (outside, _) = split(text).unwrap_or_default();
    if !outside.iter().any(|(_, line)| {
        line.contains("Only the owner's") && has(line, &["decision_note", "choice"])
    }) {
        problems.push("ask-owner: `choice` is not among the owner's answers".to_owned());
    }
    problems
}

/// AC-16 of docs/features/decision-apply.md, the skill half (the pin
/// half: `plugin_files.rs` AC-10 at `0.1.3`): the committed `ask-owner`
/// teaches a decided item. M: the skill unedited (its 0.1.2 bullets, in
/// memory).
#[test]
fn decision_apply_ac16_ask_owner_teaches_a_decided_item() {
    let skills = committed();
    assert_eq!(ac16_problems(&skills), Vec::<String>::new());
    assert_eq!(ac11_problems(&skills), Vec::<String>::new());

    // M: the skill unedited: the 0.1.2 wording of each changed line.
    let mut unedited = edited(
        &skills,
        "ask-owner",
        "- A queue record with a `record`: the owner decided it, and that record is the owner's \
         decision. Read it with `get_node` on its `record`; off this branch it is not there yet: \
         `get_proposal` with the hit's `id` gives the owner's `choice`. Follow it where it settles \
         your question.\n",
        "",
    );
    for (from, to) in [
        (
            "- A queue record with only an `answer`: the owner rejected that record",
            "- A queue record with an `answer`: the owner rejected that record",
        ),
        (
            "- A queue record with neither: it is asked already.",
            "- A queue record with no `answer`: it is asked already.",
        ),
        (
            // "on a rejected record" since docs/features/kind-decision.md
            // (a decision is a third deciding kind).
            "- Later, `get_proposal` with the item's `id` as `proposal_id` gives its `status` and \
             the owner's `decision_note`; on a rejected record the note is the owner's answer; on \
             an applied one, `record_id` and `record_title` name the owner's decision record, and \
             its `choice` answers too.",
            "- Later, `get_proposal` with the record's `id` as `proposal_id` gives its `status` \
             and the owner's `decision_note`; on a rejected question or discrepancy the note is \
             the owner's answer.",
        ),
        (
            "Only the owner's `decision_note`, `choice` and an accepted decision are answers;",
            "Only the owner's `decision_note` and an accepted decision are answers;",
        ),
    ] {
        unedited = edited(&unedited, "ask-owner", from, to);
    }
    let problems = ac16_problems(&unedited);
    for needle in [
        "with a `record`",
        "with only an `answer`",
        "`record_id`",
        "`choice` is not among",
    ] {
        assert_names(&problems, needle, "the skill unedited");
    }
    assert_names(
        &ac11_problems(&unedited),
        "ask-owner: `record_id` not backticked",
        "the skill unedited",
    );
}

/// docs/features/kind-interpretation.md AC-R12 ("Plugin"): the
/// propose-spec-change skill teaches the kind (`"kind": "interpretation"`,
/// a section "Carrying an accepted decision into a rule") and its three
/// fields `decision`, `how`, `why` by their names, a superseded decision
/// resent with its successor; the `spec-author` role names the kind; the
/// plugin is 0.4.0 or later (0.5.0 since docs/features/kind-decision.md;
/// `plugin_files.rs` pins its hash: a text changed with the version kept
/// turns that test red). The names are the server's:
/// each a property of `propose_change`. M: the field names dropped.
#[test]
fn kind_interpretation_ac_r12_the_plugin_texts_name_the_kind_and_its_fields() {
    let skills = committed();
    let skill = &skills["propose-spec-change"];
    assert!(
        skill.contains("\n## Carrying an accepted decision into a rule\n"),
        "{skill}"
    );
    assert!(skill.contains("`\"kind\": \"interpretation\"`"), "{skill}");
    for field in ["`decision`", "`how`", "`why`"] {
        assert!(skill.contains(field), "{field} in the skill");
    }
    assert!(
        skill.contains("superseded") && skill.contains("successor"),
        "the superseded decision's way out"
    );
    let role = fs::read_to_string(plugin_dir().join("specengine/agents/spec-author.md"))
        .expect("spec-author.md");
    assert!(
        role.contains("as an interpretation") && role.contains("`\"kind\": \"interpretation\"`"),
        "{role}"
    );
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(plugin_dir().join("specengine/.claude-plugin/plugin.json"))
            .expect("plugin.json"),
    )
    .expect("JSON");
    // 0.10.0 since docs/features/bevy-roles-reads.md; 0.11.0 since
    // docs/features/roadmap-surfaces.md; 0.12.0 since
    // docs/features/check-binding.md; 0.13.0 since
    // docs/features/show-bindings.md.
    assert_eq!(manifest["version"], "0.13.0");
    // The fields are `propose_change`'s properties on the wire.
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let properties = &tool(&list, "propose_change")["inputSchema"]["properties"];
    for field in ["decision", "how", "why"] {
        assert!(properties.get(field).is_some(), "{field}: {properties}");
    }
    drop(session.finish());
}

/// docs/features/kind-decision.md AC-R11, W24 ("Plugin"): `ask-owner`
/// teaches a third record, "A decision", by `report_decision`, naming its
/// fields (`text`, `working_answer`, `price_of_other`, `node_ids`,
/// `proposed_patch`), never a `decision` argument; its description names
/// decisions; `spec-author` sends a decision taken in the work through
/// `report_decision`, the rule's new text its `proposed_patch`; the plugin
/// and the marketplace are 0.5.0 (`plugin_files.rs` pins the 0.5.0 hash).
/// Each name is the server's: the tool and its input properties. M: the
/// text changed without the version (`plugin_files.rs`); the bullet
/// dropped.
#[test]
fn kind_decision_ac_r11_ask_owner_and_spec_author_send_a_decision() {
    let skills = committed();
    let skill = &skills["ask-owner"];
    let bullet = skill
        .lines()
        .find(|line| line.starts_with("- **A decision**, `report_decision`: "))
        .unwrap_or_else(|| panic!("no decision record in ask-owner:\n{skill}"));
    let fields = [
        "text",
        "working_answer",
        "price_of_other",
        "node_ids",
        "proposed_patch",
    ];
    for field in fields {
        assert!(bullet.contains(&format!("`{field}`")), "{field}: {bullet}");
    }
    assert!(
        !skill.contains("`decision`"),
        "no `decision` argument taught"
    );
    let (front, _) = skill
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .expect("front-matter");
    assert!(
        front.contains("a question, a discrepancy or a decision"),
        "{front}"
    );
    assert!(
        skill.contains("\nEach takes the nodes"),
        "three records, not two"
    );
    let role = fs::read_to_string(plugin_dir().join("specengine/agents/spec-author.md"))
        .expect("spec-author.md");
    let line = role
        .lines()
        .find(|line| line.contains("A decision taken in the work"))
        .expect("spec-author's decision line");
    assert!(
        line.contains("`report_decision`") && line.contains("`proposed_patch`"),
        "{line}"
    );
    for manifest in [
        "specengine/.claude-plugin/plugin.json",
        ".claude-plugin/marketplace.json",
    ] {
        let text = fs::read_to_string(plugin_dir().join(manifest)).expect("manifest");
        assert!(
            text.contains("raise questions, discrepancies and decisions"),
            "{manifest}"
        );
    }
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(plugin_dir().join("specengine/.claude-plugin/plugin.json"))
            .expect("plugin.json"),
    )
    .expect("JSON");
    // 0.10.0 since docs/features/bevy-roles-reads.md; 0.11.0 since
    // docs/features/roadmap-surfaces.md; 0.12.0 since
    // docs/features/check-binding.md; 0.13.0 since
    // docs/features/show-bindings.md.
    assert_eq!(manifest["version"], "0.13.0");
    // The names are the server's.
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let properties = &tool(&list, "report_decision")["inputSchema"]["properties"];
    for field in fields {
        assert!(properties.get(field).is_some(), "{field}: {properties}");
    }
    assert!(properties.get("decision").is_none(), "{properties}");
    drop(session.finish());
}

/// docs/features/kind-decision.md, iteration 2 (W24): `ask-owner` tells an
/// accepted decision hit ("- An accepted decision: its `answer` is only
/// the decision's title.", under "Reading the reply") from the record it
/// teaches ("- **A decision**, `report_decision`:"), one bullet each; a
/// rejected item is "a rejected record", never "a rejected question or
/// discrepancy". M: two "A decision" bullets.
#[test]
fn kind_decision_iteration2_ask_owner_tells_an_accepted_decision_from_a_decision_record() {
    let skills = committed();
    let skill = &skills["ask-owner"];
    let bullets: Vec<&str> = skill
        .lines()
        .filter(|line| line.starts_with("- A decision") || line.starts_with("- **A decision**"))
        .collect();
    assert_eq!(bullets.len(), 1, "{bullets:?}");
    assert!(
        bullets[0].starts_with("- **A decision**, `report_decision`: "),
        "{bullets:?}"
    );
    let reading = skill
        .split_once("\n## Reading the reply\n")
        .map(|(_, rest)| rest)
        .expect("a section \"Reading the reply\"");
    let reading = reading.split("\n## ").next().unwrap_or_default();
    let accepted: Vec<&str> = reading
        .lines()
        .filter(|line| line.starts_with("- An accepted decision: "))
        .collect();
    assert_eq!(
        accepted,
        [
            "- An accepted decision: its `answer` is only the decision's title. Read the decision \
          with `get_node` (its `id`, else its `path`) and follow it where it settles your \
          question; where it does not, yours is a different question (below)."
        ]
    );
    assert!(
        skill.contains("on a rejected record the note is the owner's answer"),
        "{skill}"
    );
    assert!(
        !skill.contains("rejected question or discrepancy"),
        "{skill}"
    );
}

/// docs/features/kind-amendment.md AC-R12, W36 ("Plugin"):
/// `propose-spec-change` teaches the kind in one bullet of "A new section
/// or file": `"kind": "amendment"`, `"base": null`, a new spec or decision
/// file whose front-matter declares `links: {amends: [ID]}`, approval
/// adding only that file (the node unwritten), applied once the document
/// is shipped, abandoned or superseded; its body within 4 096 bytes (the
/// descriptions' 1 200: `plugin_files.rs` AC-05); the plugin 0.6.0, a
/// MINOR step (`plugin_files.rs` pins the 0.6.0 hash: a text changed with
/// the version kept turns that test red). The names are the server's: the
/// kind one of `propose_change`'s `kind` enum, `base` its property. M: the
/// bullet dropped; the text changed, not the version.
#[test]
fn kind_amendment_ac_r12_propose_spec_change_teaches_the_amendment() {
    let skills = committed();
    let skill = &skills["propose-spec-change"];
    let section = skill
        .split("\n## A new section or file\n")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .expect("the section \"A new section or file\"");
    let bullet = bullets(section)
        .into_iter()
        .find(|bullet| bullet.contains("`\"kind\": \"amendment\"`"))
        .unwrap_or_else(|| panic!("no amendment bullet: {section}"));
    for phrase in [
        "a new spec or decision file",
        "`\"base\": null`",
        "`links: {amends: [ID]}`",
        "Approval adds only your file, the node unwritten",
        "shipped, abandoned or superseded",
    ] {
        assert!(bullet.contains(phrase), "{phrase:?} in {bullet:?}");
    }
    assert_eq!(skill.matches("\"kind\": \"amendment\"").count(), 1);
    let body = skill
        .splitn(3, "---\n")
        .nth(2)
        .expect("a front-matter and a body");
    assert!(body.len() <= 4096, "the body is {} bytes", body.len());
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(plugin_dir().join("specengine/.claude-plugin/plugin.json"))
            .expect("plugin.json"),
    )
    .expect("JSON");
    assert_eq!(
        manifest["version"], "0.13.0",
        "W36: 0.5.0 -> 0.6.0; norm-hash: 0.7.0; spec-lock: 0.7.1; rev-discipline: 0.7.2; \
         drift-queue: 0.8.0; code-tools: 0.9.0; bevy-roles-reads: 0.10.0; \
         roadmap-surfaces: 0.11.0; check-binding: 0.12.0; show-bindings: 0.13.0"
    );
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let schema = &tool(&list, "propose_change")["inputSchema"];
    assert!(
        schema["properties"]["kind"]["enum"]
            .as_array()
            .is_some_and(|kinds| kinds.contains(&Value::from("amendment"))),
        "{schema}"
    );
    assert!(schema["properties"].get("base").is_some(), "{schema}");
    drop(session.finish());
}

// ------------------------------------------------- code-tools RD-9

/// `read-spec`'s description as drift-queue (0.8.0) left it:
/// docs/features/code-tools.md "Plugin" changes bodies only.
const READ_SPEC_DESCRIPTION: &str = "Read the project's spec through the SpecEngine tools before \
                                     reading or changing behaviour the spec describes, or when \
                                     asked what the spec says. A context bundle first (a search \
                                     first when no ID is known), then a node, its links or the \
                                     tree; spec files only for what the bundle lacks.";

/// The model-invoked skills, whose descriptions every session loads.
const MODEL_INVOKED: [&str; 5] = [
    "analyze",
    "ask-owner",
    "implement",
    "propose-spec-change",
    "read-spec",
];

/// The front-matter `description` of a skill's text.
fn description_of(text: &str) -> Option<&str> {
    text.strip_prefix("---\n")?
        .split("\n---\n")
        .next()?
        .lines()
        .find_map(|line| line.strip_prefix("description: "))
}

/// The byte offset of `read-spec`'s files step in `body`: the first
/// numbered step (`N. `) after `from` that speaks of files (reading L19 of
/// the phase A report: the step "Files last" keeps the word).
fn files_step(body: &str, from: usize) -> Option<usize> {
    let mut at = 0;
    for line in body.split_inclusive('\n') {
        let numbered = line.split_once(". ").is_some_and(|(number, _)| {
            !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
        });
        if at > from && numbered && line.to_ascii_lowercase().contains("files") {
            return Some(at);
        }
        at += line.len();
    }
    None
}

/// The code reads `read-spec` names (docs/features/code-tools.md RD-9):
/// its own list, not `CODE_TOOLS` (docs/features/check-binding.md Q-D:
/// `read-spec` unchanged; `check_binding` is `implement`'s).
const READ_SPEC_CODE_TOOLS: [&str; 3] = ["find_symbols", "refs", "unmapped"];

/// RD-9's problems: `read-spec` backticks each code read, its first
/// mention after the first `get_context_bundle` and before the files
/// step; its description is [`READ_SPEC_DESCRIPTION`]; the five
/// model-invoked descriptions are 1 170 bytes together, within 1 200.
fn rd9_problems(skills: &Skills) -> Vec<String> {
    let mut problems = Vec::new();
    let text = &skills["read-spec"];
    if description_of(text) != Some(READ_SPEC_DESCRIPTION) {
        problems.push(format!(
            "read-spec: the description changed: {:?}",
            description_of(text)
        ));
    }
    let total: usize = MODEL_INVOKED
        .iter()
        .map(|skill| description_of(&skills[*skill]).map_or(0, str::len))
        .sum();
    if total != 1_170 {
        problems.push(format!(
            "descriptions: {total} bytes together, not 1 170 (of 1 200)"
        ));
    }
    let body = text.splitn(3, "---\n").nth(2).unwrap_or_default();
    let Some(bundle) = body.find("`get_context_bundle`") else {
        problems.push("read-spec: no `get_context_bundle`".to_owned());
        return problems;
    };
    let Some(files) = files_step(body, bundle) else {
        problems.push("read-spec: no files step after the bundle".to_owned());
        return problems;
    };
    for tool in READ_SPEC_CODE_TOOLS {
        match body.find(&format!("`{tool}`")) {
            None => problems.push(format!("read-spec: `{tool}` not backticked")),
            Some(at) if at < bundle => {
                problems.push(format!("read-spec: `{tool}` before the bundle"));
            }
            Some(at) if at > files => {
                problems.push(format!("read-spec: `{tool}` after the files step"));
            }
            Some(_) => {}
        }
    }
    problems
}

/// docs/features/code-tools.md RD-9 ("Plugin"): `read-spec` names
/// `find_symbols`, `refs`, `unmapped` after the bundle and before files
/// (an item at a line, what binds a node, where code has no spec); the
/// descriptions are unchanged; the eighteen tools are backticked across
/// the skills (AC-06 with `TOOLS`). M: a code tool unnamed; the three
/// before the bundle; a description edited.
#[test]
fn code_tools_rd9_read_spec_names_the_code_tools_between_bundle_and_files() {
    let skills = committed();
    assert_eq!(rd9_problems(&skills), Vec::<String>::new());
    // Nineteen since docs/features/roadmap-surfaces.md (`get_roadmap`,
    // named in `analyze`: `rd10_analyze_names_get_roadmap`); twenty since
    // docs/features/check-binding.md (`check_binding`, named in
    // `implement`: `check_binding_implement_names_the_tool`).
    assert_eq!(TOOLS.len(), 20);
    assert!(CODE_TOOLS.iter().all(|tool| TOOLS.contains(tool)));
    assert!(
        READ_SPEC_CODE_TOOLS
            .iter()
            .all(|tool| CODE_TOOLS.contains(tool))
    );
    assert!(!READ_SPEC_CODE_TOOLS.contains(&"check_binding"), "Q-D");

    // M: a code tool unnamed (every backticked `refs` reworded).
    let mut unnamed = skills.clone();
    let text = unnamed["read-spec"].replace("`refs`", "the binding tool");
    unnamed.insert("read-spec".to_owned(), text);
    assert_ne!(unnamed, skills);
    assert_names(
        &rd9_problems(&unnamed),
        "read-spec: `refs` not backticked",
        "refs unnamed",
    );
    assert_names(
        &ac11_problems(&unnamed),
        "read-spec: `refs` not backticked",
        "refs unnamed (AC-11's list)",
    );
    // M: the three named before the bundle (right after the precedence
    // sentence, which every skill holds verbatim).
    let early = edited(
        &skills,
        "read-spec",
        PRECEDENCE,
        &format!("{PRECEDENCE} Start with `find_symbols`, `refs` or `unmapped`."),
    );
    for tool in READ_SPEC_CODE_TOOLS {
        assert_names(
            &rd9_problems(&early),
            &format!("read-spec: `{tool}` before the bundle"),
            "the code tools first",
        );
    }
    // M: the description edited (a word added: also over the 1 170).
    let longer = edited(
        &skills,
        "read-spec",
        READ_SPEC_DESCRIPTION,
        &format!("{READ_SPEC_DESCRIPTION} Code too."),
    );
    assert_names(
        &rd9_problems(&longer),
        "read-spec: the description changed",
        "description edited",
    );
    // " Code too." is 10 bytes.
    assert_names(
        &rd9_problems(&longer),
        "1180 bytes together",
        "description sum",
    );
}

/// docs/features/roadmap-surfaces.md RD-10 ("Plugin", 0.11.0): `analyze`
/// backticks `get_roadmap` and the package's `roadmap` (AC-11's list), the
/// one skill naming the tool (AC-06: every tool somewhere); the step says
/// the plan is advisory and its items change only through a proposal the
/// owner decides (ADR-0035). M: `get_roadmap` unnamed.
#[test]
fn rd10_analyze_names_get_roadmap() {
    let skills = committed();
    assert_eq!(ac11_problems(&skills), Vec::<String>::new());
    let naming: Vec<&String> = skills
        .iter()
        .filter(|(_, text)| backticked(text).iter().any(|span| span == "get_roadmap"))
        .map(|(skill, _)| skill)
        .collect();
    assert_eq!(naming, ["analyze"], "the skills naming `get_roadmap`");
    let analyze = &skills["analyze"];
    let step = analyze
        .lines()
        .find(|line| line.contains("`get_roadmap`"))
        .expect("a line naming get_roadmap");
    for words in ["advisory", "owner"] {
        assert!(step.contains(words), "{words:?} in {step:?}");
    }

    // M: `get_roadmap` unnamed.
    let unnamed = edited(&skills, "analyze", "`get_roadmap`", "the plan tool");
    assert_names(
        &ac11_problems(&unnamed),
        "analyze: `get_roadmap` not backticked",
        "get_roadmap unnamed",
    );
    assert_names(
        &ac06_problems(&unnamed, &listed()),
        "tool `get_roadmap` backticked nowhere",
        "get_roadmap unnamed (AC-06)",
    );
}

/// docs/features/check-binding.md "Plugin": `implement`'s step 5, exactly
/// (696 bytes with its line feed).
const CHECK_STEP: &str = "5. **Check the bindings.** Before the report, call `check_binding` once per \
target, its `id` (else its `path`) as `node`; it judges this session's \
worktree. Mend what your change broke: a binding that no longer reaches its \
code, spec and code that both changed, a marker naming a revision its node \
has not reached, or one it cannot verify; else raise it with \
`report_discrepancy` and keep working on your `working_answer`. The owner \
settles code that moved ahead of its spec, a spec ahead of its code and a \
binding not yet locked: name them in the report, never hide them. An error \
result with a report is a finding, not a failed call; where the project \
binds no code, the report says so: go on.";

/// The row states of `spec verify` (docs/canon/spec-verify.md "States")
/// a skill never names: the skill says what they mean, in words.
const STATE_WORDS: [&str; 7] = [
    "cannot_verify",
    "predated",
    "bad_marker",
    "broken_binding",
    "spec_ahead",
    "code_ahead",
    "cannot-verify",
];

/// docs/features/check-binding.md "Plugin" (0.12.0): `implement`
/// backticks `check_binding` (AC-11's list), the one skill naming it
/// (AC-06: every tool somewhere; `read-spec` unchanged, Q-D); its step
/// comes after the claim's and before the report's, says the tool judges this session's
/// worktree and parts what the agent mends from what the owner settles,
/// with no state word in the skill; the body stays within 4 096 bytes.
/// M: `check_binding` unnamed; a state word in the step.
#[test]
fn check_binding_implement_names_the_tool() {
    let skills = committed();
    assert_eq!(ac11_problems(&skills), Vec::<String>::new());
    let naming: Vec<&String> = skills
        .iter()
        .filter(|(_, text)| backticked(text).iter().any(|span| span == "check_binding"))
        .map(|(skill, _)| skill)
        .collect();
    assert_eq!(naming, ["implement"], "the skills naming `check_binding`");
    let implement = &skills["implement"];
    let lines: Vec<&str> = implement.lines().collect();
    let step = lines
        .iter()
        .position(|line| line.contains("`check_binding`"))
        .expect("a line naming check_binding");
    let report = lines
        .iter()
        .position(|line| line.contains("`report_run`"))
        .expect("a line naming report_run");
    let claim = lines
        .iter()
        .position(|line| line.contains("`claim_task`"))
        .expect("a line naming claim_task");
    assert!(
        claim < step && step < report,
        "the check comes after the claim and before the report"
    );
    assert_eq!(CHECK_STEP.len() + 1, 696, "the spec's figure");
    assert_eq!(lines[step], CHECK_STEP, "the spec's step 5");
    for words in ["this session's worktree", "owner", "report_discrepancy"] {
        assert!(
            lines[step].contains(words),
            "{words:?} in {:?}",
            lines[step]
        );
    }
    for word in STATE_WORDS {
        assert!(!has_word(implement, word), "{word:?} in implement");
    }
    let body = implement
        .splitn(3, "---\n")
        .nth(2)
        .expect("a front-matter and a body");
    assert!(body.len() <= 4096, "the body is {} bytes", body.len());

    // M: `check_binding` unnamed.
    let unnamed = edited(&skills, "implement", "`check_binding`", "the binding check");
    assert_names(
        &ac11_problems(&unnamed),
        "implement: `check_binding` not backticked",
        "check_binding unnamed",
    );
    assert_names(
        &ac06_problems(&unnamed, &listed()),
        "tool `check_binding` backticked nowhere",
        "check_binding unnamed (AC-06)",
    );
}

// ------------------------------------------------- show-bindings (7a-1)

/// docs/features/show-bindings.md "Plugin 0.13.0": `read-spec`'s step 2,
/// its first bullet, word for word.
const READ_SPEC_NODE_BULLET: &str = "   - one node's current text: `get_node` with its `id`; add `with: [\"links\"]` for its links when you need what it points at and what points at it, `with: [\"bindings\"]` for the code bound to it and whether that code still matches the spec (each binding's `sync`);";

/// docs/features/show-bindings.md "Plugin 0.13.0": `read-spec` names the
/// bindings read in its step 2's first bullet (the spec's text, once, the
/// first bullet after "Then narrow", before the code reads'); its
/// description and the code reads (RD-9) unchanged; no state word; the
/// body within 4 096 bytes. M: the bullet missing or reworded.
#[test]
fn read_spec_names_the_bindings_read() {
    let skills = committed();
    assert_eq!(rd9_problems(&skills), Vec::<String>::new());
    let text = &skills["read-spec"];
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| *line == READ_SPEC_NODE_BULLET)
        .expect("the spec's bullet, word for word");
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("with: [\"bindings\"]"))
            .count(),
        1,
        "the bindings read named once"
    );
    assert!(
        lines[at - 1].contains("Then narrow"),
        "the first bullet of step 2: after {:?}",
        lines[at - 1]
    );
    let symbols = lines
        .iter()
        .position(|line| line.contains("`find_symbols`"))
        .expect("find_symbols' bullet");
    assert!(at < symbols, "before the code reads");
    for word in STATE_WORDS {
        assert!(!has_word(text, word), "{word:?} in read-spec");
    }
    let body = text.splitn(3, "---\n").nth(2).expect("a body");
    assert!(body.len() <= 4096, "the body is {} bytes", body.len());
    // M: the bullet reworded.
    let reworded = edited(
        &skills,
        "read-spec",
        "`with: [\"bindings\"]` for the code bound to it",
        "`with` for its code",
    );
    assert!(
        !reworded["read-spec"]
            .lines()
            .any(|line| line == READ_SPEC_NODE_BULLET),
        "the mutation is seen"
    );
}
