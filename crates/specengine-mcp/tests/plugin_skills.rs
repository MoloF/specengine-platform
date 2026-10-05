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
//!   eight tools backticked in some skill; no `mcp__`. M: `get_bundle`;
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

use common::read::{BAD_ARGUMENTS, Era, STACK_WORDS, Session, TOOLS, content_text, has_word};
use common::*;
use git::Sandbox;
use serde_json::Value;

/// The precedence sentence every skill holds verbatim (AC-11; the root
/// `README.md` "Claude Code plugin", its Content bullet).
const PRECEDENCE: &str = "The project's CLAUDE.md and its roles take precedence over this skill.";

/// AC-11's list: what each skill backticks.
const NAMES: [(&str, &[&str]); 3] = [
    (
        "read-spec",
        &["get_context_bundle", "get_node", "search", "get_tree"],
    ),
    (
        "ask-owner",
        &[
            "ask_question",
            "report_discrepancy",
            "get_proposal",
            "distinct_from",
            "working_answer",
        ],
    ),
    ("propose-spec-change", &["propose_change", "span_hash"]),
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
        ["ask-owner", "propose-spec-change", "read-spec"]
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
        "the default build's eight tools"
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
    assert_eq!(out.len(), 6, "{:?}", out.keys());
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
}
