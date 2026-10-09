//! docs/features/plugin-roles.md over the committed roles
//! `plugin/specengine/agents/*.md`, the owner-invoked skill
//! `plugin/specengine/skills/feature/SKILL.md`, the manifests' descriptions
//! and the default build of `specengine-mcp` (its live `tools/list`, from a
//! fresh server with a scratch `HOME`, as `plugin_skills.rs` reads it).
//! Every check is a function of the texts (and, where it names tools or
//! enum values, of the list); the committed tree gives no problem, and
//! each named mutation, applied to an in-memory copy, gives the problem it
//! names. The front-matter is read line by line under a grammar stricter
//! than YAML (one `key: value` line each, plain scalars only), so what
//! passes here is YAML whose fields are exactly these lines; the YAML
//! parse itself (Claude Code drops every field of a block it cannot
//! parse): `crates/specengine-core/tests/plugin_front_matter.rs`.
//!
//! - R-2: `plugin.json`, the marketplace's `metadata` and its entry each
//!   name the five roles and `/specengine:feature`. M: `tester` out of
//!   one; `/specengine:feature` out of one.
//! - R-3, T-2: "Agent front-matter" — `name`, `description`, `model`, and
//!   on analyst, spec-author, reviewer `disallowedTools`, in order, nothing
//!   else; `name` the stem, its grammar, no P2-3 role name; `description`
//!   a plain line of 1 to 1 024 characters naming SpecEngine, the five at
//!   most 1 000 B together; `model: inherit`; the body at most 4 096 B;
//!   `disallowedTools` exactly as "Roles", order free. M: `tools: Read,
//!   Grep, Glob` on analyst; `permissionMode: acceptEdits`; `effort:
//!   high`; `isolation: worktree`; `model: claude-opus-5-5`;
//!   `disallowedTools: [Write, Edit`; `Edit` out of reviewer's, `Write` out
//!   of spec-author's, `Bash` out of analyst's; `name` not the stem; `name:
//!   spec-writer`; descriptions over 1 000 B; a description breaking YAML
//!   (`: `); a body over 4 096 B.
//! - R-4, T-4: "Content" over the agents and the skill — no P2-3 word,
//!   `mechanic`, `edge-case`, `mcp__`; outside `json` blocks no digit but a
//!   line-start `N. `; no backticked input enum value; every backticked
//!   snake_case name a tool or a schema property; no backticked path; every
//!   backticked `specengine:<name>` a plugin skill or role; no letter
//!   outside ASCII; the precedence sentence of each. M: `cargo` in
//!   implementer; `mechanic` in spec-author; "at most 3 rounds" in the
//!   skill; `` `partial` `` in reviewer; `get_bundle` in tester;
//!   `mcp__plugin_specengine_specengine__get_task` in analyst; a precedence
//!   sentence dropped.
//! - R-5, T-2: each role's backticked names, analyst never `claim_task`,
//!   implementer and tester `CLAUDE.md`, every role "the project's
//!   language". M: `submit_plan` out of analyst; `claim_task` into
//!   analyst; `CLAUDE.md` out of implementer.
//! - R-6, T-3: the skill's four keys and their values, its names, `spec
//!   task new`, `spec task approve`, each role as `specengine:<name>`,
//!   `Stage | Role`, the leg-end sentence, the round bound in words; no
//!   word `poll`, no unnegated "wait". M: `disable-model-invocation`
//!   dropped; `argument-hint` first; a 4 097 B body; `specengine:tester`,
//!   `claim_task`, `spec task approve` or the leg-end sentence dropped;
//!   "poll `get_task` until it is ready" added; "wait until approved"
//!   added.
//!
//! Compiles to nothing with `--features probes` (the criteria name the
//! default build). The repository is only read.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use common::read::{Era, STACK_WORDS, Session, TOOLS, has_word};
use common::*;
use serde_json::Value;

/// "Roles": each role's file stem and its `disallowedTools` (none: no
/// key), in the table's order.
const ROLES: [(&str, &[&str]); 5] = [
    (
        "analyst",
        &["Write", "Edit", "MultiEdit", "NotebookEdit", "Bash"],
    ),
    (
        "spec-author",
        &["Write", "Edit", "MultiEdit", "NotebookEdit"],
    ),
    ("implementer", &[]),
    ("tester", &[]),
    ("reviewer", &["Write", "Edit", "MultiEdit", "NotebookEdit"]),
];

/// The plugin's skills (`specengine:<name>` names one of these or a role).
const SKILLS: [&str; 6] = [
    "analyze",
    "ask-owner",
    "feature",
    "implement",
    "propose-spec-change",
    "read-spec",
];

/// R-5: what each role backticks.
const ROLE_NAMES: [(&str, &[&str]); 5] = [
    (
        "analyst",
        &[
            "get_task",
            "get_context_bundle",
            "ask_question",
            "report_discrepancy",
            "task_id",
            "working_answer",
            "submit_plan",
        ],
    ),
    (
        "spec-author",
        &["get_node", "propose_change", "span_hash", "task_id"],
    ),
    (
        "implementer",
        &["get_task", "report_discrepancy", "task_id", "CLAUDE.md"],
    ),
    (
        "tester",
        &["get_task", "report_discrepancy", "task_id", "CLAUDE.md"],
    ),
    ("reviewer", &["get_task", "report_discrepancy", "task_id"]),
];

/// R-6: what the skill backticks.
const SKILL_NAMES: [&str; 8] = [
    "get_task",
    "submit_plan",
    "claim_task",
    "report_run",
    "complete_task",
    "ask_question",
    "report_discrepancy",
    "task_id",
];

/// "Content": the agents' precedence sentence, verbatim.
const ROLE_PRECEDENCE: &str =
    "The project's CLAUDE.md and its roles take precedence over this role.";

/// "Content": the skills' precedence sentence, verbatim.
const SKILL_PRECEDENCE: &str =
    "The project's CLAUDE.md and its roles take precedence over this skill.";

/// "Content": the skill's leg-end sentence, verbatim.
const LEG_END: &str = "This leg ends here: never wait for the approval.";

/// R-5: every role writes in this.
const LANGUAGE: &str = "the project's language";

/// The words no plugin file holds besides P2-3's ("Content").
const NEVER_WORDS: [&str; 2] = ["mechanic", "edge-case"];

/// "Skill front-matter": `feature`'s keys and the values "Data" gives the
/// two after `description`.
const SKILL_KEYS: [&str; 4] = [
    "name",
    "description",
    "argument-hint",
    "disable-model-invocation",
];
const ARGUMENT_HINT: &str = "<requirement in words> or <task ID>";

const PLUGIN_JSON: &str = "specengine/.claude-plugin/plugin.json";
const MARKETPLACE: &str = ".claude-plugin/marketplace.json";
const FEATURE: &str = "skills/feature/SKILL.md";

/// The texts under `plugin/specengine/`: `agents/<stem>.md` and
/// [`FEATURE`], by path relative to it.
type Texts = BTreeMap<String, String>;

fn plugin_dir() -> PathBuf {
    repository_root().join("plugin")
}

/// The committed agents and the skill `feature`.
fn committed() -> Texts {
    let dir = plugin_dir().join("specengine");
    let mut texts = Texts::new();
    for entry in fs::read_dir(dir.join("agents")).expect("the agents directory") {
        let entry = entry.expect("an entry");
        let name = entry.file_name().to_str().expect("UTF-8").to_owned();
        if name == ".DS_Store" {
            continue;
        }
        let text = fs::read_to_string(entry.path()).unwrap_or_else(|e| panic!("{name}: {e}"));
        texts.insert(format!("agents/{name}"), text);
    }
    let feature = fs::read_to_string(dir.join(FEATURE)).expect("the feature skill");
    texts.insert(FEATURE.to_owned(), feature);
    assert_eq!(
        texts.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "agents/analyst.md",
            "agents/implementer.md",
            "agents/reviewer.md",
            "agents/spec-author.md",
            "agents/tester.md",
            FEATURE,
        ]
    );
    texts
}

/// `agents/<stem>.md` of a role.
fn agent(stem: &str) -> String {
    format!("agents/{stem}.md")
}

/// `texts` with the one occurrence of `from` in `path` replaced by `to`.
fn edited(texts: &Texts, path: &str, from: &str, to: &str) -> Texts {
    let mut texts = texts.clone();
    let text = &texts[path];
    assert_eq!(text.matches(from).count(), 1, "{path}: {from:?} once");
    let text = text.replacen(from, to, 1);
    texts.insert(path.to_owned(), text);
    texts
}

/// `texts` with every occurrence (at least one) of `from` in `path`
/// replaced by `to`.
fn replaced(texts: &Texts, path: &str, from: &str, to: &str) -> Texts {
    let mut texts = texts.clone();
    let text = &texts[path];
    assert!(text.contains(from), "{path}: {from:?} present");
    let text = text.replace(from, to);
    texts.insert(path.to_owned(), text);
    texts
}

/// `texts` with `line` appended to the body of `path`.
fn appended(texts: &Texts, path: &str, line: &str) -> Texts {
    let mut texts = texts.clone();
    texts
        .get_mut(path)
        .unwrap()
        .push_str(&format!("\n{line}\n"));
    texts
}

fn assert_names(problems: &[String], needle: &str, mutation: &str) {
    assert!(
        problems.iter().any(|problem| problem.contains(needle)),
        "{mutation}: want a problem naming {needle:?}, got {problems:#?}"
    );
}

// ------------------------------------------------------- the front-matter

/// A front-matter block read under the strict grammar: its `(key, value)`
/// lines in order and the body after the closing `---` line.
struct Front<'t> {
    fields: Vec<(&'t str, &'t str)>,
    body: &'t str,
}

/// The block of `text` under the strict grammar: opened by the first line
/// `---`, closed by the next; each line between `key: value`, `key` of
/// ASCII letters and `-`, one space, a non-empty value.
fn front(text: &str) -> Result<Front<'_>, String> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or("no front-matter opening `---` line")?;
    let (block, body) = rest.split_once("\n---\n").ok_or("no closing `---` line")?;
    let mut fields = Vec::new();
    for (index, line) in block.split('\n').enumerate() {
        let parsed = line.split_once(": ").filter(|(key, value)| {
            !key.is_empty()
                && key.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-')
                && !value.is_empty()
        });
        let Some((key, value)) = parsed else {
            return Err(format!(
                "front-matter line {} is no `key: value`: {line:?}",
                index + 2
            ));
        };
        fields.push((key, value));
    }
    Ok(Front { fields, body })
}

/// A plain one-line YAML scalar: no indicator first, no `: `, ` #` or
/// trailing `:`, no outer whitespace.
fn plain_problems(at: &str, key: &str, value: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if value
        .chars()
        .next()
        .is_some_and(|first| "-?:,[]{}#&*!|>'\"%@`".contains(first))
    {
        problems.push(format!("{at}: {key} starts with an indicator"));
    }
    if value.contains(": ") || value.contains(" #") || value.ends_with(':') {
        problems.push(format!("{at}: {key} is not a plain scalar"));
    }
    if value != value.trim() {
        problems.push(format!("{at}: {key} has outer whitespace"));
    }
    problems
}

/// `^[a-z0-9]+(-[a-z0-9]+)*$`.
fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// The description's own checks: 1 to 1 024 characters, a plain scalar.
fn description_problems(at: &str, description: &str) -> Vec<String> {
    let mut problems = plain_problems(at, "description", description);
    let characters = description.chars().count();
    if !(1..=1024).contains(&characters) {
        problems.push(format!(
            "{at}: description of {characters} characters, not 1 to 1 024"
        ));
    }
    problems
}

// ------------------------------------------------------------------- R-3

fn r3_problems(texts: &Texts) -> Vec<String> {
    let mut problems = Vec::new();
    let mut descriptions = 0;
    let stems: BTreeSet<&str> = texts
        .keys()
        .filter_map(|path| path.strip_prefix("agents/")?.strip_suffix(".md"))
        .collect();
    let roles: BTreeSet<&str> = ROLES.iter().map(|(stem, _)| *stem).collect();
    for missing in roles.difference(&stems) {
        problems.push(format!("agents/{missing}.md: missing"));
    }
    for stem in &stems {
        let at = format!("agents/{stem}.md");
        let disallowed = ROLES
            .iter()
            .find(|(role, _)| role == stem)
            .map(|(_, tools)| *tools);
        if disallowed.is_none() {
            problems.push(format!("{at}: not one of the five roles"));
        }
        let parsed = match front(&texts[&at]) {
            Ok(parsed) => parsed,
            Err(problem) => {
                problems.push(format!("{at}: {problem}"));
                continue;
            }
        };
        let keys: Vec<&str> = parsed.fields.iter().map(|(key, _)| *key).collect();
        let mut wanted = vec!["name", "description", "model"];
        if disallowed.is_some_and(|tools| !tools.is_empty()) {
            wanted.push("disallowedTools");
        }
        if keys != wanted {
            problems.push(format!(
                "{at}: front-matter keys {keys:?}, not [{}]",
                wanted.join(", ")
            ));
        }
        let value = |key: &str| {
            parsed
                .fields
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, value)| *value)
        };
        for (key, value) in &parsed.fields {
            if *key != "description" {
                problems.extend(plain_problems(&at, key, value));
            }
        }
        let name = value("name").unwrap_or_default();
        if name != *stem {
            problems.push(format!("{at}: name {name:?} is not its file stem"));
        }
        if !is_name(name) {
            problems.push(format!("{at}: name {name:?} breaks the grammar"));
        }
        if STACK_WORDS.iter().any(|word| has_word(name, word)) {
            problems.push(format!("{at}: name {name:?} is a P2-3 role name"));
        }
        let description = value("description").unwrap_or_default();
        problems.extend(description_problems(&at, description));
        if !description.contains("SpecEngine") {
            problems.push(format!("{at}: description is not scoped to SpecEngine"));
        }
        descriptions += description.len();
        let model = value("model").unwrap_or_default();
        if model != "inherit" {
            problems.push(format!("{at}: model {model:?}, not inherit"));
        }
        if let (Some(wanted), Some(listed)) = (disallowed, value("disallowedTools")) {
            let tools: Vec<&str> = listed.split(", ").collect();
            let set: BTreeSet<&str> = tools.iter().copied().collect();
            if set.len() != tools.len() {
                problems.push(format!("{at}: disallowedTools repeats a tool"));
            }
            let wanted: BTreeSet<&str> = wanted.iter().copied().collect();
            for tool in wanted.difference(&set) {
                problems.push(format!("{at}: disallowedTools lacks {tool}"));
            }
            for tool in set.difference(&wanted) {
                problems.push(format!("{at}: disallowedTools holds {tool}"));
            }
        }
        let body = parsed.body.len();
        if body > 4096 {
            problems.push(format!("{at}: body of {body} B, over 4 096"));
        }
    }
    if descriptions > 1000 {
        problems.push(format!(
            "agents: descriptions of {descriptions} B together, over 1 000"
        ));
    }
    problems
}

/// `texts` with the description of `path` replaced by `description`.
fn with_description(texts: &Texts, path: &str, description: &str) -> Texts {
    let old = front(&texts[path])
        .unwrap()
        .fields
        .iter()
        .find(|(key, _)| *key == "description")
        .unwrap()
        .1
        .to_owned();
    edited(
        texts,
        path,
        &format!("description: {old}\n"),
        &format!("description: {description}\n"),
    )
}

/// `texts` with the body of `path` padded or cut to `size` bytes.
fn with_body_size(texts: &Texts, path: &str, size: usize) -> Texts {
    let text = &texts[path];
    let body = front(text).unwrap().body.len();
    let head = text[..text.len() - body].to_owned();
    let mut padded = text[text.len() - body..].to_owned();
    while padded.len() < size {
        padded.push('x');
    }
    padded.truncate(size);
    padded.pop();
    padded.push('\n');
    let mut texts = texts.clone();
    texts.insert(path.to_owned(), head + &padded);
    texts
}

#[test]
fn r3_each_agent_has_the_front_matter_of_its_role() {
    let texts = committed();
    assert_eq!(r3_problems(&texts), Vec::<String>::new());

    let analyst = agent("analyst");
    let analyst_tools = "disallowedTools: Write, Edit, MultiEdit, NotebookEdit, Bash\n";
    // M: `tools: Read, Grep, Glob` on analyst (added; in place of
    // `disallowedTools`).
    for (from, to) in [
        (
            "model: inherit\n",
            "model: inherit\ntools: Read, Grep, Glob\n",
        ),
        (analyst_tools, "tools: Read, Grep, Glob\n"),
    ] {
        let mutated = edited(&texts, &analyst, from, to);
        assert_names(
            &r3_problems(&mutated),
            "agents/analyst.md: front-matter keys",
            to,
        );
    }
    // M: the keys D4 ignores or that move the role, each added.
    for (stem, key) in [
        ("analyst", "permissionMode: acceptEdits"),
        ("implementer", "effort: high"),
        ("tester", "isolation: worktree"),
        ("reviewer", "skills: read-spec"),
        ("spec-author", "hooks: none"),
    ] {
        let mutated = edited(
            &texts,
            &agent(stem),
            "model: inherit\n",
            &format!("model: inherit\n{key}\n"),
        );
        assert_names(
            &r3_problems(&mutated),
            &format!("agents/{stem}.md: front-matter keys"),
            key,
        );
    }
    // M: `model: claude-opus-5-5`.
    let model = edited(
        &texts,
        &agent("implementer"),
        "model: inherit\n",
        "model: claude-opus-5-5\n",
    );
    assert_names(
        &r3_problems(&model),
        "agents/implementer.md: model \"claude-opus-5-5\", not inherit",
        "claude-opus-5-5",
    );
    // M: `disallowedTools: [Write, Edit` (broken YAML: every field lost).
    let broken = edited(
        &texts,
        &analyst,
        analyst_tools,
        "disallowedTools: [Write, Edit\n",
    );
    let problems = r3_problems(&broken);
    assert_names(
        &problems,
        "agents/analyst.md: disallowedTools starts with an indicator",
        "[Write, Edit",
    );
    assert_names(&problems, "disallowedTools lacks Bash", "[Write, Edit");
    // M: `Edit` out of reviewer's, `Write` out of spec-author's, `Bash` out
    // of analyst's.
    let four = "disallowedTools: Write, Edit, MultiEdit, NotebookEdit\n";
    for (stem, from, to, tool) in [
        (
            "reviewer",
            four,
            "disallowedTools: Write, MultiEdit, NotebookEdit\n",
            "Edit",
        ),
        (
            "spec-author",
            four,
            "disallowedTools: Edit, MultiEdit, NotebookEdit\n",
            "Write",
        ),
        ("analyst", analyst_tools, four, "Bash"),
    ] {
        let mutated = edited(&texts, &agent(stem), from, to);
        assert_eq!(
            r3_problems(&mutated),
            [format!("agents/{stem}.md: disallowedTools lacks {tool}")],
            "{tool} out of {stem}'s"
        );
    }
    // Order free; a repeat, an extra tool, the key on a role without it.
    let reordered = edited(
        &texts,
        &analyst,
        analyst_tools,
        "disallowedTools: Bash, NotebookEdit, MultiEdit, Edit, Write\n",
    );
    assert_eq!(r3_problems(&reordered), Vec::<String>::new(), "order free");
    for (to, needle) in [
        (
            "disallowedTools: Write, Edit, MultiEdit, NotebookEdit, Bash, Bash\n",
            "disallowedTools repeats a tool",
        ),
        (
            "disallowedTools: Write, Edit, MultiEdit, NotebookEdit, Bash, Agent\n",
            "disallowedTools holds Agent",
        ),
        (
            "disallowedTools: Write,Edit,MultiEdit,NotebookEdit,Bash\n",
            "disallowedTools holds Write,Edit",
        ),
    ] {
        let mutated = edited(&texts, &analyst, analyst_tools, to);
        assert_names(&r3_problems(&mutated), needle, to);
    }
    let restricted = edited(
        &texts,
        &agent("implementer"),
        "model: inherit\n",
        "model: inherit\ndisallowedTools: Bash\n",
    );
    assert_names(
        &r3_problems(&restricted),
        "agents/implementer.md: front-matter keys",
        "disallowedTools on implementer",
    );
    let unrestricted = edited(&texts, &agent("reviewer"), four, "");
    assert_names(
        &r3_problems(&unrestricted),
        "agents/reviewer.md: front-matter keys",
        "no disallowedTools on reviewer",
    );

    // M: `name` not the stem; `name: spec-writer` (here and with the file
    // renamed, so only the role-name rule sees it).
    let renamed = edited(&texts, &analyst, "name: analyst\n", "name: analyzer\n");
    assert_names(
        &r3_problems(&renamed),
        "agents/analyst.md: name \"analyzer\" is not its file stem",
        "name not the stem",
    );
    let writer = edited(
        &texts,
        &agent("spec-author"),
        "name: spec-author\n",
        "name: spec-writer\n",
    );
    let problems = r3_problems(&writer);
    assert_names(
        &problems,
        "name \"spec-writer\" is a P2-3 role name",
        "spec-writer",
    );
    assert_names(&problems, "is not its file stem", "spec-writer");
    let mut moved = writer.clone();
    let text = moved.remove(&agent("spec-author")).unwrap();
    moved.insert(agent("spec-writer"), text);
    let problems = r3_problems(&moved);
    assert_names(
        &problems,
        "agents/spec-writer.md: name \"spec-writer\" is a P2-3 role name",
        "spec-writer.md",
    );
    assert_names(
        &problems,
        "agents/spec-author.md: missing",
        "spec-writer.md",
    );
    assert!(
        !problems.iter().any(|p| p.contains("is not its file stem")),
        "{problems:#?}"
    );
    for (to, needle) in [
        ("name: Analyst\n", "breaks the grammar"),
        ("name: analyst-\n", "breaks the grammar"),
        ("name:  analyst\n", "has outer whitespace"),
        ("name:analyst\n", "is no `key: value`"),
    ] {
        let mutated = edited(&texts, &analyst, "name: analyst\n", to);
        assert_names(&r3_problems(&mutated), needle, to);
    }

    // M: descriptions over 1 000 B together (1 000 B passes).
    let total: usize = ROLES
        .iter()
        .map(|(stem, _)| {
            let text = &texts[&agent(stem)];
            front(text).unwrap().fields[1].1.len()
        })
        .sum();
    assert!(total <= 1000, "{total} B");
    let own = front(&texts[&analyst]).unwrap().fields[1].1.len();
    let pad = |size: usize| format!("SpecEngine {}", "a".repeat(size - "SpecEngine ".len()));
    let full = with_description(&texts, &analyst, &pad(own + 1000 - total));
    assert_eq!(r3_problems(&full), Vec::<String>::new(), "1 000 B together");
    let over = with_description(&texts, &analyst, &pad(own + 1001 - total));
    assert_eq!(
        r3_problems(&over),
        ["agents: descriptions of 1001 B together, over 1 000"],
        "1 001 B together"
    );
    // One description: 1 025 characters; not SpecEngine's; breaking YAML.
    let long = with_description(&texts, &analyst, &pad(1025));
    assert_names(
        &r3_problems(&long),
        "agents/analyst.md: description of 1025 characters",
        "1 025",
    );
    let unscoped = with_description(&texts, &agent("tester"), "Test the code and report.");
    assert_names(
        &r3_problems(&unscoped),
        "agents/tester.md: description is not scoped to SpecEngine",
        "unscoped",
    );
    for (description, needle) in [
        (
            "Review a SpecEngine task: return findings or accept.",
            "description is not a plain scalar",
        ),
        (
            "Review a SpecEngine task #now",
            "description is not a plain scalar",
        ),
        (
            "- Review a SpecEngine task",
            "description starts with an indicator",
        ),
        (
            "\"Review a SpecEngine task\"",
            "description starts with an indicator",
        ),
        (
            "`reviewer` for SpecEngine tasks",
            "description starts with an indicator",
        ),
    ] {
        let mutated = with_description(&texts, &agent("reviewer"), description);
        assert_names(&r3_problems(&mutated), needle, description);
    }

    // M: a body over 4 096 B (4 096 B passes).
    let at_cap = with_body_size(&texts, &agent("implementer"), 4096);
    assert_eq!(r3_problems(&at_cap), Vec::<String>::new(), "4 096 B");
    let over = with_body_size(&texts, &agent("implementer"), 4097);
    assert_eq!(
        r3_problems(&over),
        ["agents/implementer.md: body of 4097 B, over 4 096"],
        "4 097 B"
    );
    // The block's grammar: a blank or comment line, a missing close.
    for (from, to, needle) in [
        (
            "model: inherit\n",
            "model: inherit\n\n",
            "is no `key: value`",
        ),
        (
            "model: inherit\n",
            "model: inherit\n# a comment\n",
            "is no `key: value`",
        ),
        ("---\nname", "--- \nname", "no front-matter opening"),
    ] {
        let mutated = edited(&texts, &agent("tester"), from, to);
        assert_names(&r3_problems(&mutated), needle, to);
    }
    // An unlisted sixth role.
    let mut sixth = texts.clone();
    let text = texts[&agent("tester")].replacen("name: tester\n", "name: x\n", 1);
    sixth.insert(agent("x"), text);
    assert_names(
        &r3_problems(&sixth),
        "agents/x.md: not one of the five roles",
        "agents/x.md",
    );
}

// ------------------------------------------------------------------- R-4

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
        collect(
            &tool["outputSchema"],
            &mut listed.properties,
            &mut BTreeSet::new(),
        );
    }
    assert_eq!(
        listed.tools.iter().map(String::as_str).collect::<Vec<_>>(),
        TOOLS
    );
    assert!(
        listed.input_enums.contains("partial"),
        "input enums collected"
    );
    listed
}

/// The lines of `text` outside its `json` blocks: 1-based number, text.
fn outside_json(text: &str) -> Vec<(usize, &str)> {
    let mut outside = Vec::new();
    let mut inside = false;
    for (index, line) in text.lines().enumerate() {
        if inside {
            inside = line != "```";
        } else if line == "```json" {
            inside = true;
        } else {
            outside.push((index + 1, line));
        }
    }
    outside
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
fn backticked(text: &str) -> Vec<&str> {
    outside_json(text)
        .into_iter()
        .flat_map(|(_, line)| spans(line))
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

/// A span holding a path: a `/` with a path character on both sides.
fn is_path(span: &str) -> bool {
    let path_char = |c: char| c.is_ascii_alphanumeric() || "._-".contains(c);
    let chars: Vec<char> = span.chars().collect();
    chars
        .windows(3)
        .any(|w| w[1] == '/' && path_char(w[0]) && path_char(w[2]))
}

fn r4_problems(texts: &Texts, listed: &Listed) -> Vec<String> {
    let mut problems = Vec::new();
    let known: BTreeSet<&str> = SKILLS
        .iter()
        .chain(ROLES.iter().map(|(stem, _)| stem))
        .copied()
        .collect();
    for (path, text) in texts {
        for word in STACK_WORDS.iter().chain(&NEVER_WORDS) {
            if has_word(text, word) {
                problems.push(format!("{path}: the word {word:?}"));
            }
        }
        if text.contains("mcp__") {
            problems.push(format!("{path}: holds `mcp__`"));
        }
        if let Some(letter) = text.chars().find(|c| c.is_alphabetic() && !c.is_ascii()) {
            problems.push(format!("{path}: the letter {letter:?} outside ASCII"));
        }
        let sentence = if path == FEATURE {
            SKILL_PRECEDENCE
        } else {
            ROLE_PRECEDENCE
        };
        if !text.contains(sentence) {
            problems.push(format!("{path}: no precedence sentence"));
        }
        for (number, line) in outside_json(text) {
            let digits = line.trim_start_matches(|c: char| c.is_ascii_digit());
            let rest = if digits.len() < line.len() {
                digits.strip_prefix(". ").unwrap_or(line)
            } else {
                line
            };
            if rest.bytes().any(|b| b.is_ascii_digit()) {
                problems.push(format!("{path}:{number}: a digit: {line:?}"));
            }
            for span in spans(line) {
                if listed.input_enums.contains(span) {
                    problems.push(format!("{path}:{number}: the enum value `{span}`"));
                }
                if is_snake(span)
                    && !listed.tools.contains(span)
                    && !listed.properties.contains(span)
                {
                    problems.push(format!(
                        "{path}:{number}: `{span}` is no tool and no schema property"
                    ));
                }
                if is_path(span) {
                    problems.push(format!("{path}:{number}: the path `{span}`"));
                }
                if let Some(name) = span.strip_prefix("specengine:")
                    && !known.contains(name)
                {
                    problems.push(format!(
                        "{path}:{number}: `{span}` names no plugin skill or role"
                    ));
                }
            }
        }
    }
    problems
}

#[test]
fn r4_the_agents_and_the_skill_keep_the_content_rules() {
    let texts = committed();
    let listed = listed();
    assert_eq!(r4_problems(&texts, &listed), Vec::<String>::new());

    // M: `cargo` in implementer; `mechanic` in spec-author; P2-3's role
    // names; whole words only.
    for (stem, line, word) in [
        ("implementer", "Build it with cargo first.", "cargo"),
        (
            "spec-author",
            "Keep each mechanic in its own section.",
            "mechanic",
        ),
        (
            "analyst",
            "Hand the plan to the spec-writer.",
            "spec-writer",
        ),
        ("reviewer", "Mind every edge-case.", "edge-case"),
        ("tester", "Run nextest.", "nextest"),
    ] {
        let mutated = appended(&texts, &agent(stem), line);
        assert_eq!(
            r4_problems(&mutated, &listed),
            [format!("agents/{stem}.md: the word {word:?}")],
            "{word} in {stem}"
        );
    }
    let words = appended(
        &texts,
        &agent("tester"),
        "A reaction, nested lists, cargoes.",
    );
    assert_eq!(r4_problems(&words, &listed), Vec::<String>::new());

    // M: "at most 3 rounds" in the skill (the bound in words); a line-start
    // `N. ` passes, an indented one does not.
    let rounds = appended(&texts, FEATURE, "Stop after at most 3 rounds.");
    assert_names(
        &r4_problems(&rounds, &listed),
        "skills/feature/SKILL.md:",
        "at most 3 rounds",
    );
    assert_names(&r4_problems(&rounds, &listed), "a digit", "3 rounds");
    let step = appended(&texts, &agent("tester"), "7. Hand over.");
    assert_eq!(r4_problems(&step, &listed), Vec::<String>::new(), "7. ");
    let indented = appended(&texts, &agent("tester"), " 7. Hand over.");
    assert_names(&r4_problems(&indented, &listed), "a digit", "indented");
    let block = appended(&texts, FEATURE, "```json\n{\"recommendation\": 0}\n```");
    assert_eq!(r4_problems(&block, &listed), Vec::<String>::new(), "json");

    // M: `` `partial` `` in reviewer (an input enum value).
    let partial = appended(&texts, &agent("reviewer"), "A run may end `partial`.");
    assert_names(
        &r4_problems(&partial, &listed),
        "agents/reviewer.md:",
        "`partial`",
    );
    assert_names(
        &r4_problems(&partial, &listed),
        "the enum value `partial`",
        "`partial`",
    );
    // M: `get_bundle` in tester.
    let bundle = replaced(
        &texts,
        &agent("tester"),
        "`get_context_bundle`",
        "`get_bundle`",
    );
    assert_names(
        &r4_problems(&bundle, &listed),
        "`get_bundle` is no tool and no schema property",
        "get_bundle",
    );
    // M: the prefixed tool name in analyst.
    let prefixed = replaced(
        &texts,
        &agent("analyst"),
        "`get_task`",
        "`mcp__plugin_specengine_specengine__get_task`",
    );
    assert_names(
        &r4_problems(&prefixed, &listed),
        "agents/analyst.md: holds `mcp__`",
        "mcp__",
    );
    // M: a precedence sentence dropped (each in turn); reworded; the skill
    // holding the roles' sentence.
    for path in texts.keys() {
        let sentence = if path == FEATURE {
            SKILL_PRECEDENCE
        } else {
            ROLE_PRECEDENCE
        };
        let dropped = replaced(&texts, path, sentence, "");
        assert_eq!(
            r4_problems(&dropped, &listed),
            [format!("{path}: no precedence sentence")],
            "{path}"
        );
    }
    let reworded = replaced(
        &texts,
        &agent("reviewer"),
        ROLE_PRECEDENCE,
        "The project's CLAUDE.md and roles take precedence over this role.",
    );
    assert_names(
        &r4_problems(&reworded, &listed),
        "agents/reviewer.md: no precedence sentence",
        "reworded",
    );
    let swapped = replaced(&texts, FEATURE, SKILL_PRECEDENCE, ROLE_PRECEDENCE);
    assert_names(
        &r4_problems(&swapped, &listed),
        "skills/feature/SKILL.md: no precedence sentence",
        "the roles' sentence in the skill",
    );
    // A path, a skill or role that does not exist, a letter outside ASCII.
    for (line, needle) in [
        ("Keep `docs/specs` current.", "the path `docs/specs`"),
        (
            "Load `specengine:review`.",
            "`specengine:review` names no plugin",
        ),
        (
            "Write it na\u{ef}vely.",
            "the letter '\u{ef}' outside ASCII",
        ),
    ] {
        let mutated = appended(&texts, &agent("spec-author"), line);
        assert_names(&r4_problems(&mutated, &listed), needle, line);
    }
    assert!(!is_path("/specengine:feature <task ID>") && !is_path("Stage | Role"));
}

// ------------------------------------------------------------------- R-5

fn r5_problems(texts: &Texts) -> Vec<String> {
    let mut problems = Vec::new();
    for (stem, names) in ROLE_NAMES {
        let path = agent(stem);
        let Some(text) = texts.get(&path) else {
            problems.push(format!("{path}: missing"));
            continue;
        };
        let spans = backticked(text);
        for name in names {
            if !spans.contains(name) {
                problems.push(format!("{path}: `{name}` not backticked"));
            }
        }
        if !text.contains(LANGUAGE) {
            problems.push(format!("{path}: no \"{LANGUAGE}\""));
        }
        if stem == "analyst" && text.contains("claim_task") {
            problems.push(format!("{path}: names `claim_task`"));
        }
    }
    problems
}

#[test]
fn r5_each_role_names_its_tools_and_its_project() {
    let texts = committed();
    assert_eq!(r5_problems(&texts), Vec::<String>::new());

    // M: `submit_plan` out of analyst (everywhere; unbackticked is not
    // enough).
    for to in ["the plan tool", "submit_plan"] {
        let out = replaced(&texts, &agent("analyst"), "`submit_plan`", to);
        assert_eq!(
            r5_problems(&out),
            ["agents/analyst.md: `submit_plan` not backticked"],
            "{to}"
        );
    }
    // M: `claim_task` into analyst.
    let claimed = appended(&texts, &agent("analyst"), "Then call `claim_task`.");
    assert_eq!(
        r5_problems(&claimed),
        ["agents/analyst.md: names `claim_task`"]
    );
    // M: `CLAUDE.md` out of implementer (backticked; the precedence
    // sentence keeps it bare).
    let bare = replaced(&texts, &agent("implementer"), "`CLAUDE.md`", "CLAUDE.md");
    assert_eq!(
        r5_problems(&bare),
        ["agents/implementer.md: `CLAUDE.md` not backticked"]
    );
    // Each role's names in turn, and its language.
    for (stem, names) in ROLE_NAMES {
        let path = agent(stem);
        for name in names {
            let out = replaced(&texts, &path, &format!("`{name}`"), "the name");
            assert_names(
                &r5_problems(&out),
                &format!("{path}: `{name}` not backticked"),
                name,
            );
        }
        let mute = replaced(&texts, &path, LANGUAGE, "the language");
        assert_eq!(
            r5_problems(&mute),
            [format!("{path}: no \"{LANGUAGE}\"")],
            "{stem}"
        );
    }
}

// ------------------------------------------------------------------- R-6

/// The clauses of `text` (split at `.`, `;`, `:`, `!`, `?` and line ends).
fn clauses(text: &str) -> Vec<&str> {
    text.split(['.', ';', ':', '!', '?', '\n'])
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .collect()
}

/// The lowercase words of `text` (ASCII letters and `'`).
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphabetic() && c != '\'')
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn r6_problems(texts: &Texts) -> Vec<String> {
    let at = FEATURE;
    let Some(text) = texts.get(FEATURE) else {
        return vec![format!("{at}: missing")];
    };
    let mut problems = Vec::new();
    match front(text) {
        Ok(parsed) => {
            let keys: Vec<&str> = parsed.fields.iter().map(|(key, _)| *key).collect();
            if keys != SKILL_KEYS {
                problems.push(format!(
                    "{at}: front-matter keys {keys:?}, not [{}]",
                    SKILL_KEYS.join(", ")
                ));
            } else {
                let values: Vec<&str> = parsed.fields.iter().map(|(_, value)| *value).collect();
                if values[0] != "feature" {
                    problems.push(format!("{at}: name {:?}, not feature", values[0]));
                }
                problems.extend(description_problems(at, values[1]));
                if values[2] != ARGUMENT_HINT {
                    problems.push(format!("{at}: argument-hint {:?}, not Data's", values[2]));
                }
                if values[3] != "true" {
                    problems.push(format!(
                        "{at}: disable-model-invocation {:?}, not true",
                        values[3]
                    ));
                }
            }
            if parsed.body.len() > 4096 {
                problems.push(format!("{at}: body of {} B, over 4 096", parsed.body.len()));
            }
        }
        Err(problem) => problems.push(format!("{at}: {problem}")),
    }
    let spans = backticked(text);
    for name in SKILL_NAMES {
        if !spans.contains(&name) {
            problems.push(format!("{at}: `{name}` not backticked"));
        }
    }
    for (stem, _) in ROLES {
        let role = format!("specengine:{stem}");
        if !spans.contains(&role.as_str()) {
            problems.push(format!("{at}: `{role}` not backticked"));
        }
    }
    for command in ["spec task new", "spec task approve"] {
        if !spans.iter().any(|span| span.starts_with(command)) {
            problems.push(format!("{at}: no `{command}`"));
        }
    }
    if !spans.contains(&"Stage | Role") {
        problems.push(format!("{at}: no `Stage | Role`"));
    }
    if !text.contains(LEG_END) {
        problems.push(format!("{at}: no leg-end sentence"));
    }
    if !words(text).iter().any(|word| word == "three") {
        problems.push(format!("{at}: no round bound in words"));
    }
    for clause in clauses(text) {
        let clause_words = words(clause);
        if clause_words.iter().any(|word| word.starts_with("poll")) {
            problems.push(format!("{at}: polls: {clause:?}"));
        }
        let waits = clause_words.iter().any(|word| word.starts_with("wait"));
        let negated = clause_words
            .iter()
            .any(|word| ["never", "not", "no", "nothing"].contains(&word.as_str()));
        if waits && !negated {
            problems.push(format!("{at}: waits: {clause:?}"));
        }
    }
    problems
}

#[test]
fn r6_the_skill_runs_a_task_and_never_waits() {
    let texts = committed();
    assert_eq!(r6_problems(&texts), Vec::<String>::new());

    // M: `disable-model-invocation` dropped; `argument-hint` first.
    let invocation = "disable-model-invocation: true\n";
    let hint = format!("argument-hint: {ARGUMENT_HINT}\n");
    let dropped = edited(&texts, FEATURE, invocation, "");
    assert_eq!(
        r6_problems(&dropped),
        [format!(
            "{FEATURE}: front-matter keys [\"name\", \"description\", \"argument-hint\"], not \
             [name, description, argument-hint, disable-model-invocation]"
        )]
    );
    let first = edited(&texts, FEATURE, &hint, "");
    let first = edited(
        &first,
        FEATURE,
        "---\nname: feature\n",
        &format!("---\n{hint}name: feature\n"),
    );
    assert_names(
        &r6_problems(&first),
        "skills/feature/SKILL.md: front-matter keys",
        "argument-hint first",
    );
    for (from, to, needle) in [
        (invocation, "disable-model-invocation: false\n", "not true"),
        (hint.as_str(), "argument-hint: <task ID>\n", "not Data's"),
        ("name: feature\n", "name: features\n", "not feature"),
    ] {
        let mutated = edited(&texts, FEATURE, from, to);
        assert_names(&r6_problems(&mutated), needle, to);
    }
    // M: a 4 097 B body (4 096 B passes).
    let at_cap = with_body_size(&texts, FEATURE, 4096);
    assert_eq!(r6_problems(&at_cap), Vec::<String>::new(), "4 096 B");
    let over = with_body_size(&texts, FEATURE, 4097);
    assert_names(&r6_problems(&over), "body of 4097 B, over 4 096", "4 097 B");

    // M: `specengine:tester`, `claim_task`, `spec task approve` or the
    // leg-end sentence dropped; each other name in turn.
    for (from, to, needle) in [
        (
            "`specengine:tester`",
            "the tester",
            "`specengine:tester` not backticked",
        ),
        (
            "`claim_task`",
            "the claim tool",
            "`claim_task` not backticked",
        ),
        (
            "spec task approve",
            "approve on a terminal",
            "no `spec task approve`",
        ),
        ("spec task new", "a new task", "no `spec task new`"),
        ("`Stage | Role`", "routing", "no `Stage | Role`"),
        (LEG_END, "", "no leg-end sentence"),
        ("three", "the default", "no round bound in words"),
    ] {
        let mutated = replaced(&texts, FEATURE, from, to);
        assert_names(&r6_problems(&mutated), needle, from);
    }
    for name in SKILL_NAMES {
        let out = replaced(&texts, FEATURE, &format!("`{name}`"), "the tool");
        assert_names(
            &r6_problems(&out),
            &format!("`{name}` not backticked"),
            name,
        );
    }
    for (stem, _) in ROLES {
        let role = format!("`specengine:{stem}`");
        let out = replaced(&texts, FEATURE, &role, "the role");
        assert_names(&r6_problems(&out), &format!("{role} not backticked"), stem);
    }

    // M: "poll `get_task` until it is ready" added; "wait until approved"
    // added (no negation in its clause). Negated waits pass.
    for (line, needle) in [
        ("Then poll `get_task` until it is ready.", "polls"),
        ("Polling the task is fine.", "polls"),
        ("Wait until the task is approved.", "waits"),
        ("Then wait until approved, and go on.", "waits"),
    ] {
        let mutated = appended(&texts, FEATURE, line);
        assert_names(&r6_problems(&mutated), needle, line);
    }
    for line in [
        "Never wait for the owner.",
        "The work does not wait for the owner.",
    ] {
        let mutated = appended(&texts, FEATURE, line);
        assert_eq!(r6_problems(&mutated), Vec::<String>::new(), "{line}");
    }
}

// ------------------------------------------------------------------- R-2

fn r2_problems(plugin: &str, marketplace: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let plugin: Value = serde_json::from_str(plugin).expect("plugin.json");
    let market: Value = serde_json::from_str(marketplace).expect("marketplace.json");
    // docs/features/kind-decision.md W24: 0.5.0.
    if plugin["version"] != "0.5.0" {
        problems.push(format!(
            "plugin.json: version {}, not 0.5.0",
            plugin["version"]
        ));
    }
    let descriptions = [
        ("plugin.json", &plugin["description"]),
        (
            "marketplace.json metadata",
            &market["metadata"]["description"],
        ),
        (
            "marketplace.json entry",
            &market["plugins"][0]["description"],
        ),
    ];
    for (at, description) in descriptions {
        let text = description.as_str().unwrap_or_default();
        if description != &plugin["description"] {
            problems.push(format!("{at}: description is not plugin.json's"));
        }
        for (stem, _) in ROLES {
            if !has_word(text, stem) {
                problems.push(format!("{at}: description does not name {stem}"));
            }
        }
        if !text.contains("/specengine:feature") {
            problems.push(format!(
                "{at}: description does not name /specengine:feature"
            ));
        }
    }
    problems
}

#[test]
fn r2_the_three_descriptions_name_the_roles_and_the_skill() {
    let plugin = fs::read_to_string(plugin_dir().join(PLUGIN_JSON)).expect("plugin.json");
    let market = fs::read_to_string(plugin_dir().join(MARKETPLACE)).expect("marketplace");
    assert_eq!(r2_problems(&plugin, &market), Vec::<String>::new());

    // M: `tester` out of one; `/specengine:feature` out of one.
    let untested = market.replacen(" tester and", " and", 1);
    assert_ne!(untested, market);
    let problems = r2_problems(&plugin, &untested);
    assert_names(
        &problems,
        "marketplace.json metadata: description does not name tester",
        "tester out of the metadata",
    );
    assert_names(&problems, "description is not plugin.json's", "unequal");
    let both = |from: &str, to: &str| (plugin.replace(from, to), market.replace(from, to));
    let (plugin_cut, market_cut) = both("/specengine:feature", "a skill");
    let problems = r2_problems(&plugin_cut, &market_cut);
    for at in [
        "plugin.json",
        "marketplace.json metadata",
        "marketplace.json entry",
    ] {
        assert_names(
            &problems,
            &format!("{at}: description does not name /specengine:feature"),
            at,
        );
    }
    let (plugin_cut, market_cut) = both("spec-author", "author");
    assert_names(
        &r2_problems(&plugin_cut, &market_cut),
        "plugin.json: description does not name spec-author",
        "spec-author",
    );
    let older = plugin.replacen("\"0.5.0\"", "\"0.2.0\"", 1);
    assert_names(&r2_problems(&older, &market), "not 0.5.0", "0.2.0");
}
