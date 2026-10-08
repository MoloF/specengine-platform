//! docs/features/plugin-roles.md R-3, R-6 ("Agent front-matter": "parse
//! strictly"): the front-matter of each committed role
//! `plugin/specengine/agents/*.md` and of the owner-invoked skill
//! `plugin/specengine/skills/feature/SKILL.md` is YAML, and a YAML parser
//! reads from it exactly its `key: value` lines, in order, each a string
//! but `disable-model-invocation`'s `true`. Claude Code loads a block it
//! cannot parse with every field dropped (D4), so a broken
//! `disallowedTools` leaves the role unrestricted: here that is a parse
//! error. Two parsers read each block: `serde-saphyr` (this crate's own
//! YAML dependency; duplicate keys an error, strict booleans) and the
//! spec parser `specengine_core::parse` (no `frontmatter-*` diagnostic).
//! The line grammar, the values and the budgets: `specengine-mcp`'s
//! `plugin_roles.rs`.
//!
//! M: `disallowedTools: [Write, Edit`; a description with `: ` inside
//! (`description: Use when: …`); a duplicate key; an unclosed quote; a
//! ` #` comment (YAML drops it, the line keeps it); `yes` for `true`; a
//! continuation line. The repository is only read.

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::Value;
use serde_saphyr::options::DuplicateKeyPolicy;
use specengine_model::IdScheme;

/// The files whose front-matter Claude Code reads, relative to the
/// repository.
const FILES: [&str; 6] = [
    "plugin/specengine/agents/analyst.md",
    "plugin/specengine/agents/implementer.md",
    "plugin/specengine/agents/reviewer.md",
    "plugin/specengine/agents/spec-author.md",
    "plugin/specengine/agents/tester.md",
    "plugin/specengine/skills/feature/SKILL.md",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the repository root")
}

fn committed() -> Vec<(&'static str, String)> {
    let root = repository_root();
    let agents: Vec<String> = fs::read_dir(root.join("plugin/specengine/agents"))
        .expect("the agents directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name != ".DS_Store")
        .collect();
    assert_eq!(agents.len(), 5, "{agents:?}");
    FILES
        .iter()
        .map(|path| {
            let text =
                fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            (*path, text)
        })
        .collect()
}

/// The mapping's entries in document order.
struct Entries(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Entries {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Ordered;
        impl<'de> Visitor<'de> for Ordered {
            type Value = Entries;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a mapping")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Entries, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry::<String, Value>()? {
                    entries.push(entry);
                }
                Ok(Entries(entries))
            }
        }
        deserializer.deserialize_map(Ordered)
    }
}

/// The block between the opening `---` line and the next `---` line.
fn block(text: &str) -> Result<&str, String> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or("no front-matter opening `---` line")?;
    let (block, _) = rest.split_once("\n---\n").ok_or("no closing `---` line")?;
    Ok(block)
}

/// The block's mapping as `serde-saphyr` reads it.
fn yaml(block: &str) -> Result<Vec<(String, Value)>, String> {
    let options = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::Error,
        strict_booleans: true,
    };
    serde_saphyr::from_str_with_options::<Entries>(block, options)
        .map(|entries| entries.0)
        .map_err(|error| error.to_string())
}

/// What the lines say: `key: value` each, a string but
/// `disable-model-invocation`'s boolean.
fn lines(block: &str) -> Vec<(String, Value)> {
    block
        .split('\n')
        .map(|line| {
            let (key, value) = line.split_once(": ").unwrap_or((line, ""));
            let value = if key == "disable-model-invocation" {
                Value::Bool(value == "true")
            } else {
                Value::String(value.to_owned())
            };
            (key.to_owned(), value)
        })
        .collect()
}

fn problems(path: &str, text: &str) -> Vec<String> {
    let block = match block(text) {
        Ok(block) => block,
        Err(problem) => return vec![format!("{path}: {problem}")],
    };
    let mut problems = Vec::new();
    match yaml(block) {
        Ok(entries) => {
            let said = lines(block);
            if entries != said {
                problems.push(format!(
                    "{path}: YAML reads {entries:?}, the lines say {said:?}"
                ));
            }
        }
        Err(error) => problems.push(format!("{path}: not YAML: {error}")),
    }
    let parsed = specengine_core::parse(path, text.as_bytes(), &IdScheme::default());
    for diagnostic in &parsed.diagnostics {
        let code = diagnostic.code.as_str();
        if code.starts_with("frontmatter-") {
            problems.push(format!(
                "{path}:{}: the spec parser: {code}",
                diagnostic.line
            ));
        }
    }
    problems
}

fn assert_names(problems: &[String], needle: &str, mutation: &str) {
    assert!(
        problems.iter().any(|problem| problem.contains(needle)),
        "{mutation}: want a problem naming {needle:?}, got {problems:#?}"
    );
}

#[test]
fn each_role_and_the_skill_parse_as_their_lines() {
    let files = committed();
    for (path, text) in &files {
        assert_eq!(problems(path, text), Vec::<String>::new(), "{path}");
        let entries = yaml(block(text).unwrap()).unwrap();
        let keys: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys[..2], ["name", "description"], "{path}");
    }
    // The values Claude Code gets, read by the parser: the analyst's tools
    // one string, the skill's flag a boolean.
    let analyst = yaml(block(&files[0].1).unwrap()).unwrap();
    assert_eq!(
        analyst.last(),
        Some(&(
            "disallowedTools".to_owned(),
            Value::String("Write, Edit, MultiEdit, NotebookEdit, Bash".to_owned())
        ))
    );
    let feature = yaml(block(&files[5].1).unwrap()).unwrap();
    assert_eq!(
        feature.last(),
        Some(&("disable-model-invocation".to_owned(), Value::Bool(true)))
    );
}

#[test]
fn a_broken_block_is_named() {
    let files = committed();
    let (analyst, text) = (files[0].0, &files[0].1);
    let tools = "disallowedTools: Write, Edit, MultiEdit, NotebookEdit, Bash\n";
    let edited = |from: &str, to: &str| {
        assert_eq!(text.matches(from).count(), 1, "{from:?} once");
        text.replacen(from, to, 1)
    };

    // M: `disallowedTools: [Write, Edit` — unparseable, every field lost.
    let unclosed = edited(tools, "disallowedTools: [Write, Edit\n");
    let found = problems(analyst, &unclosed);
    assert_names(&found, "agents/analyst.md: not YAML", "[Write, Edit");
    assert_names(&found, "the spec parser: frontmatter-yaml", "[Write, Edit");

    // M: a description with `: ` inside.
    let description = text
        .lines()
        .find(|line| line.starts_with("description: "))
        .unwrap()
        .to_owned();
    let colon = edited(
        &format!("{description}\n"),
        "description: Use when: a SpecEngine task needs analysis.\n",
    );
    assert_names(
        &problems(analyst, &colon),
        "not YAML",
        "`: ` in the description",
    );

    // A duplicate key; an unclosed quote.
    for (from, to) in [
        ("model: inherit\n", "model: inherit\nmodel: inherit\n"),
        ("name: analyst\n", "name: 'analyst\n"),
    ] {
        assert_names(&problems(analyst, &edited(from, to)), "not YAML", to);
    }

    // Parseable, but not what the lines say: a ` #` comment, a
    // continuation line, a flow sequence.
    for (from, to) in [
        (
            tools,
            "disallowedTools: Write, Edit, MultiEdit, NotebookEdit #, Bash\n",
        ),
        ("model: inherit\n", "model: inherit\n  and more\n"),
        (
            tools,
            "disallowedTools: [Write, Edit, MultiEdit, NotebookEdit, Bash]\n",
        ),
    ] {
        assert_names(&problems(analyst, &edited(from, to)), "YAML reads", to);
    }

    // `yes` is no boolean under strict booleans: the flag would be a string.
    let (feature, skill) = (files[5].0, &files[5].1);
    let yes = skill.replacen(
        "disable-model-invocation: true\n",
        "disable-model-invocation: yes\n",
        1,
    );
    assert_ne!(&yes, skill);
    assert_names(&problems(feature, &yes), "YAML reads", "yes");
    let unopened = text.replacen("---\n", "", 1);
    assert_names(
        &problems(analyst, &unopened),
        "no front-matter opening",
        "unopened",
    );
}
