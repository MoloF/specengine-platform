//! docs/features/plugin-skills.md over the committed `plugin/` tree: the
//! manifests, the closed file set, the skill files' front-matter and
//! budgets, and the version pin. Every check is a function over the tree
//! read into memory (paths relative to `plugin/`, `/`-joined, with their
//! bytes; `.DS_Store` ignored throughout); the committed tree gives no
//! problem, and each named mutation, applied in the test to an in-memory
//! or a scratch copy, gives the problem it names. "The README" is the root
//! `README.md` "Claude Code plugin" (its Files, A skill and Version
//! bullets).
//!
//! - AC-01: `marketplace.json` — `name` `specengine`, `owner.name` set,
//!   keys `name`, `owner`, `plugins` (and `metadata` holding only a
//!   `description` equal to `plugin.json`'s: the accepted deviation), one
//!   entry (`name`, `source`, `description`; no `version`) named as
//!   `plugin.json`, `source` `./specengine` holding
//!   `.claude-plugin/plugin.json`. M: `"./specengine-x"`; entry
//!   `spec-engine`.
//! - AC-02: `plugin.json` — `name` `specengine`, `version` `N.N.N`, keys
//!   within the README's. M: a `hooks` key.
//! - AC-03: exactly the README's six files (scratch copy, the real walk).
//!   M: `specengine/hooks/hooks.json`; `specengine/agents/x.md`;
//!   `README.md`.
//! - AC-04: `.mcp.json` is the README's value, `"type": "stdio"` the one
//!   admitted addition. M: `env.HOME`; `args` `["--root", "."]`; `command`
//!   `cargo`.
//! - AC-05: per skill exactly `name` and `description`, `name` = its
//!   directory, grammar, at most 64, not reserved; `description` a plain
//!   scalar of 1 to 1 024 characters; descriptions at most 1 200 B
//!   together, each body at most 4 096 B. M: `name` other than the
//!   directory; a 1 025-character description; `description: Use when:
//!   …`; a 4 097 B body; a skill `round`.
//! - AC-10: the BLAKE3 (`common/blake3.rs`) of `path \0 length \0 bytes`
//!   per file under `plugin/specengine/` in byte order of its relative
//!   path, and `version`, are the last of [`PINS`]; versions strictly
//!   increasing. M: a `SKILL.md` edited, `PINS` unchanged; an entry
//!   repeating the version.
//!
//! The repository is only read.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use common::blake3::blake3_hex;
use common::{Scratch, copy_dir, repository_root, write};
use serde_json::{Value, json};

/// `(version, hash)` of every released content of `plugin/specengine/`,
/// append-only, versions strictly increasing; the last is the current one
/// (the root `README.md` "Claude Code plugin", its Version bullet).
const PINS: &[(&str, &str)] = &[
    (
        "0.1.0",
        "1a9152610662a986b1c54b5369ce49e6584bcafff9350954462452953a355e32",
    ),
    (
        "0.1.1",
        "5346a2efc2402eda7d8ff6e757221f754224c9df2384d11ee65bc2d80aac34b8",
    ),
];

/// The README's closed file set (its Files bullet), relative to `plugin/`.
const FILES: [&str; 6] = [
    ".claude-plugin/marketplace.json",
    "specengine/.claude-plugin/plugin.json",
    "specengine/.mcp.json",
    "specengine/skills/ask-owner/SKILL.md",
    "specengine/skills/propose-spec-change/SKILL.md",
    "specengine/skills/read-spec/SKILL.md",
];

const MARKETPLACE: &str = ".claude-plugin/marketplace.json";
const PLUGIN_JSON: &str = "specengine/.claude-plugin/plugin.json";
const MCP_JSON: &str = "specengine/.mcp.json";

/// The keys `plugin.json` may hold (the README's Files bullet).
const PLUGIN_KEYS: [&str; 8] = [
    "name",
    "version",
    "description",
    "author",
    "license",
    "homepage",
    "repository",
    "keywords",
];

/// Skill names the task prompts own (07 §1.4).
const RESERVED: [&str; 4] = ["analyze", "implement", "prepare-task", "round"];

/// A tree read from disk: regular files with their bytes, and every other
/// non-directory entry (a symlink, a socket) by path.
#[derive(Clone, Debug, Default)]
struct Tree {
    files: BTreeMap<String, Vec<u8>>,
    other: BTreeSet<String>,
}

/// Everything under `dir`, dot-named directories and files included,
/// `.DS_Store` ignored; symlinks never followed.
fn read_tree(dir: &Path) -> Tree {
    fn walk(root: &Path, dir: &Path, tree: &mut Tree) {
        for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let entry = entry.expect("a directory entry");
            let name = entry.file_name().to_str().expect("a UTF-8 name").to_owned();
            let kind = entry.file_type().expect("a file type");
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("under the root")
                .to_str()
                .expect("a UTF-8 path")
                .replace('\\', "/");
            if kind.is_dir() {
                walk(root, &entry.path(), tree);
            } else if name == ".DS_Store" {
                continue;
            } else if kind.is_file() {
                let bytes = fs::read(entry.path()).expect("a readable file");
                tree.files.insert(relative, bytes);
            } else {
                tree.other.insert(relative);
            }
        }
    }
    let mut tree = Tree::default();
    walk(dir, dir, &mut tree);
    tree
}

/// The committed `plugin/` of this repository.
fn committed() -> Tree {
    let dir = repository_root().join("plugin");
    assert!(dir.is_dir(), "plugin/ exists");
    read_tree(&dir)
}

/// `tree` with the one occurrence of `from` in `path` replaced by `to`.
fn edited(tree: &Tree, path: &str, from: &str, to: &str) -> Tree {
    let mut tree = tree.clone();
    let text = String::from_utf8(tree.files[path].clone()).expect("UTF-8");
    assert_eq!(text.matches(from).count(), 1, "{path}: {from:?} once");
    tree.files
        .insert(path.to_owned(), text.replacen(from, to, 1).into_bytes());
    tree
}

/// `tree` with `path` holding `value` as pretty JSON.
fn with_json(tree: &Tree, path: &str, value: &Value) -> Tree {
    let mut tree = tree.clone();
    let text = serde_json::to_string_pretty(value).expect("JSON") + "\n";
    tree.files.insert(path.to_owned(), text.into_bytes());
    tree
}

/// The parsed JSON at `path`, or a problem.
fn json_at(tree: &Tree, path: &str) -> Result<Value, String> {
    let bytes = tree
        .files
        .get(path)
        .ok_or_else(|| format!("plugin/{path}: missing"))?;
    serde_json::from_slice(bytes).map_err(|e| format!("plugin/{path}: not JSON: {e}"))
}

fn keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// Whether `problems` holds one containing `needle`.
fn names(problems: &[String], needle: &str) -> bool {
    problems.iter().any(|problem| problem.contains(needle))
}

fn assert_names(problems: &[String], needle: &str, mutation: &str) {
    assert!(
        names(problems, needle),
        "{mutation}: want a problem naming {needle:?}, got {problems:#?}"
    );
}

// ------------------------------------------------------------------ AC-01

fn ac01_problems(tree: &Tree) -> Vec<String> {
    let mut problems = Vec::new();
    let market = match json_at(tree, MARKETPLACE) {
        Ok(value) => value,
        Err(problem) => return vec![problem],
    };
    let plugin = json_at(tree, PLUGIN_JSON).unwrap_or(Value::Null);
    let at = format!("plugin/{MARKETPLACE}");
    if !market.is_object() {
        return vec![format!("{at}: not an object")];
    }
    let top = keys(&market);
    for key in ["name", "owner", "plugins"] {
        if !top.contains(key) {
            problems.push(format!("{at}: no key `{key}`"));
        }
    }
    for key in &top {
        if !["name", "owner", "plugins", "metadata"].contains(key) {
            problems.push(format!("{at}: key `{key}` outside Data's"));
        }
    }
    if market["name"] != json!("specengine") {
        problems.push(format!("{at}: name {} is not `specengine`", market["name"]));
    }
    if !market["owner"]["name"]
        .as_str()
        .is_some_and(|name| !name.trim().is_empty())
    {
        problems.push(format!("{at}: owner.name not set"));
    }
    if let Some(metadata) = market.get("metadata") {
        if keys(metadata) != BTreeSet::from(["description"]) {
            problems.push(format!("{at}: metadata holds more than `description`"));
        } else if metadata["description"] != plugin["description"] {
            problems.push(format!("{at}: metadata.description is not plugin.json's"));
        }
    }
    let Some(entries) = market["plugins"].as_array() else {
        problems.push(format!("{at}: plugins is not an array"));
        return problems;
    };
    if entries.len() != 1 {
        problems.push(format!("{at}: {} plugin entries, not one", entries.len()));
    }
    for entry in entries {
        for key in keys(entry) {
            if !["name", "source", "description"].contains(&key) {
                problems.push(format!("{at}: entry key `{key}` outside Data's"));
            }
        }
        if entry["name"] != plugin["name"] || !entry["name"].is_string() {
            problems.push(format!(
                "{at}: entry name {} is not plugin.json's {}",
                entry["name"], plugin["name"]
            ));
        }
        if let Some(description) = entry.get("description")
            && *description != plugin["description"]
        {
            problems.push(format!("{at}: entry description is not plugin.json's"));
        }
        let source = entry["source"].as_str().unwrap_or_default();
        if source != "./specengine" {
            problems.push(format!(
                "{at}: entry source {source:?} is not \"./specengine\""
            ));
        }
        let held = source
            .strip_prefix("./")
            .map(|dir| format!("{dir}/.claude-plugin/plugin.json"));
        if !held.is_some_and(|path| tree.files.contains_key(&path)) {
            problems.push(format!(
                "{at}: entry source {source:?} holds no .claude-plugin/plugin.json"
            ));
        }
    }
    problems
}

#[test]
fn ac01_the_marketplace_lists_the_plugin_by_its_name_and_directory() {
    let tree = committed();
    assert_eq!(ac01_problems(&tree), Vec::<String>::new());

    // M: the source moved.
    let moved = edited(&tree, MARKETPLACE, "\"./specengine\"", "\"./specengine-x\"");
    let problems = ac01_problems(&moved);
    assert_names(&problems, "is not \"./specengine\"", "./specengine-x");
    assert_names(
        &problems,
        "holds no .claude-plugin/plugin.json",
        "./specengine-x",
    );

    // M: the entry renamed.
    let market = String::from_utf8(tree.files[MARKETPLACE].clone()).unwrap();
    let renamed = market.replacen(
        "\"name\": \"specengine\",\n      \"source\"",
        "\"name\": \"spec-engine\",\n      \"source\"",
        1,
    );
    assert_ne!(renamed, market, "the entry's name line");
    let mut spec_engine = tree.clone();
    spec_engine
        .files
        .insert(MARKETPLACE.to_owned(), renamed.into_bytes());
    assert_names(
        &ac01_problems(&spec_engine),
        "entry name \"spec-engine\"",
        "entry spec-engine",
    );

    // The admitted `metadata`: only `description`, equal to plugin.json's.
    let parsed = json_at(&tree, MARKETPLACE).unwrap();
    let mut without = parsed.clone();
    without.as_object_mut().unwrap().remove("metadata");
    assert_eq!(
        ac01_problems(&with_json(&tree, MARKETPLACE, &without)),
        Vec::<String>::new(),
        "no metadata"
    );
    let mut extra = parsed.clone();
    extra["metadata"]["version"] = json!("0.1.0");
    assert_names(
        &ac01_problems(&with_json(&tree, MARKETPLACE, &extra)),
        "metadata holds more",
        "metadata.version",
    );
    let mut other = parsed.clone();
    other["metadata"]["description"] = json!("Another description.");
    assert_names(
        &ac01_problems(&with_json(&tree, MARKETPLACE, &other)),
        "metadata.description is not plugin.json's",
        "metadata.description",
    );
    let mut versioned = parsed;
    versioned["plugins"][0]["version"] = json!("0.1.0");
    assert_names(
        &ac01_problems(&with_json(&tree, MARKETPLACE, &versioned)),
        "entry key `version`",
        "an entry version",
    );
}

// ------------------------------------------------------------------ AC-02

fn is_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn ac02_problems(tree: &Tree) -> Vec<String> {
    let plugin = match json_at(tree, PLUGIN_JSON) {
        Ok(value) => value,
        Err(problem) => return vec![problem],
    };
    let at = format!("plugin/{PLUGIN_JSON}");
    let mut problems = Vec::new();
    if !plugin.is_object() {
        return vec![format!("{at}: not an object")];
    }
    if plugin["name"] != json!("specengine") {
        problems.push(format!("{at}: name {} is not `specengine`", plugin["name"]));
    }
    if !plugin["version"].as_str().is_some_and(is_semver) {
        problems.push(format!("{at}: version {} is not N.N.N", plugin["version"]));
    }
    for key in keys(&plugin) {
        if !PLUGIN_KEYS.contains(&key) {
            problems.push(format!("{at}: key `{key}` outside Data's"));
        }
    }
    problems
}

#[test]
fn ac02_the_plugin_manifest_holds_data_s_keys() {
    let tree = committed();
    assert_eq!(ac02_problems(&tree), Vec::<String>::new());
    let parsed = json_at(&tree, PLUGIN_JSON).unwrap();

    let mut hooks = parsed.clone();
    hooks["hooks"] = json!("./hooks/hooks.json");
    assert_names(
        &ac02_problems(&with_json(&tree, PLUGIN_JSON, &hooks)),
        "key `hooks`",
        "a hooks key",
    );
    for version in ["0.1", "v0.1.0", "0.1.0-rc1", "0..1"] {
        let mut bad = parsed.clone();
        bad["version"] = json!(version);
        assert_names(
            &ac02_problems(&with_json(&tree, PLUGIN_JSON, &bad)),
            "is not N.N.N",
            version,
        );
    }
    let mut renamed = parsed;
    renamed["name"] = json!("spec-engine");
    assert_names(
        &ac02_problems(&with_json(&tree, PLUGIN_JSON, &renamed)),
        "is not `specengine`",
        "spec-engine",
    );
}

// ------------------------------------------------------------------ AC-03

fn ac03_problems(tree: &Tree) -> Vec<String> {
    let found: BTreeSet<&str> = tree.files.keys().map(String::as_str).collect();
    let wanted = BTreeSet::from(FILES);
    let mut problems: Vec<String> = found
        .difference(&wanted)
        .map(|path| format!("plugin/{path}: outside Data's six files"))
        .collect();
    problems.extend(
        wanted
            .difference(&found)
            .map(|path| format!("plugin/{path}: missing")),
    );
    problems.extend(
        tree.other
            .iter()
            .map(|path| format!("plugin/{path}: not a regular file")),
    );
    problems
}

#[test]
fn ac03_the_plugin_tree_is_data_s_six_files() {
    assert_eq!(ac03_problems(&committed()), Vec::<String>::new());

    // On a scratch copy, through the walk: `.DS_Store` anywhere is ignored,
    // each extra file is named.
    let scratch = Scratch::new("plugin-files");
    let copy = scratch.join("plugin");
    copy_dir(&repository_root().join("plugin"), &copy, false);
    assert_eq!(ac03_problems(&read_tree(&copy)), Vec::<String>::new());
    for ds_store in [
        ".DS_Store",
        ".claude-plugin/.DS_Store",
        "specengine/.DS_Store",
        "specengine/skills/.DS_Store",
        "specengine/skills/read-spec/.DS_Store",
    ] {
        write(&copy, ds_store, b"\0\0\0\x01Bud1");
    }
    let ignored = read_tree(&copy);
    assert_eq!(ac03_problems(&ignored), Vec::<String>::new());
    assert_eq!(
        ignored.files.keys().map(String::as_str).collect::<Vec<_>>(),
        FILES
    );
    for extra in [
        "specengine/hooks/hooks.json",
        "specengine/agents/x.md",
        "README.md",
        "specengine/skills/read-spec/notes.md",
        ".claude-plugin/plugin.json",
    ] {
        write(&copy, extra, b"{}\n");
        assert_names(
            &ac03_problems(&read_tree(&copy)),
            &format!("plugin/{extra}: outside Data's six files"),
            extra,
        );
        fs::remove_file(copy.join(extra)).unwrap();
    }
    std::os::unix::fs::symlink("SKILL.md", copy.join("specengine/skills/read-spec/x.md"))
        .expect("a symlink");
    assert_names(
        &ac03_problems(&read_tree(&copy)),
        "plugin/specengine/skills/read-spec/x.md: not a regular file",
        "a symlink",
    );
    fs::remove_file(copy.join("specengine/skills/read-spec/x.md")).unwrap();
    fs::remove_file(copy.join("specengine/.mcp.json")).unwrap();
    assert_names(
        &ac03_problems(&read_tree(&copy)),
        "plugin/specengine/.mcp.json: missing",
        "no .mcp.json",
    );
}

// ------------------------------------------------------------------ AC-04

fn ac04_problems(tree: &Tree) -> Vec<String> {
    let mcp = match json_at(tree, MCP_JSON) {
        Ok(value) => value,
        Err(problem) => return vec![problem],
    };
    let data = json!({"specengine": {"command": "specengine-mcp"}});
    let typed = json!({"specengine": {"type": "stdio", "command": "specengine-mcp"}});
    if mcp == data || mcp == typed {
        Vec::new()
    } else {
        vec![format!("plugin/{MCP_JSON}: {mcp} is not Data's value")]
    }
}

#[test]
fn ac04_the_server_entry_is_data_s_value() {
    let tree = committed();
    assert_eq!(ac04_problems(&tree), Vec::<String>::new());
    let parsed = json_at(&tree, MCP_JSON).unwrap();

    let mut typed = parsed.clone();
    typed["specengine"]["type"] = json!("stdio");
    assert_eq!(
        ac04_problems(&with_json(&tree, MCP_JSON, &typed)),
        Vec::<String>::new(),
        "`\"type\": \"stdio\"` is admitted"
    );
    let mut mutations: Vec<(&str, Value)> = Vec::new();
    let mut env = parsed.clone();
    env["specengine"]["env"] = json!({"HOME": "${HOME}"});
    mutations.push(("env.HOME", env));
    let mut args = parsed.clone();
    args["specengine"]["args"] = json!(["--root", "."]);
    mutations.push(("args", args));
    let mut cargo = parsed.clone();
    cargo["specengine"]["command"] = json!("cargo");
    mutations.push(("command cargo", cargo));
    let mut sse = parsed.clone();
    sse["specengine"]["type"] = json!("sse");
    mutations.push(("type sse", sse));
    let renamed = json!({"spec": parsed["specengine"].clone()});
    mutations.push(("server spec", renamed));
    for (mutation, value) in mutations {
        assert_names(
            &ac04_problems(&with_json(&tree, MCP_JSON, &value)),
            "is not Data's value",
            mutation,
        );
    }
}

// ------------------------------------------------------------------ AC-05

/// A skill file split as the README's A skill bullet writes it: `name`,
/// `description`, the body after the closing `---` line.
struct Skill<'t> {
    name: &'t str,
    description: &'t str,
    body: &'t [u8],
}

fn split_skill<'t>(bytes: &'t [u8]) -> Result<Skill<'t>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("not UTF-8: {e}"))?;
    let rest = text
        .strip_prefix("---\n")
        .ok_or("no front-matter opening `---` line")?;
    let (front, _) = rest.split_once("\n---\n").ok_or("no closing `---` line")?;
    let body = &bytes[4 + front.len() + 5..];
    let lines: Vec<&str> = front.split('\n').collect();
    let keys: Vec<&str> = lines
        .iter()
        .map(|line| line.split_once(':').map_or(*line, |(key, _)| key))
        .collect();
    if keys != ["name", "description"] {
        return Err(format!(
            "front-matter keys {keys:?}, not [name, description]"
        ));
    }
    let value = |line: &'t str, key: &str| -> Result<&'t str, String> {
        line.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix(": "))
            .ok_or(format!("`{key}: ` with one space"))
    };
    Ok(Skill {
        name: value(lines[0], "name")?,
        description: value(lines[1], "description")?,
        body,
    })
}

fn is_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

fn ac05_problems(tree: &Tree) -> Vec<String> {
    let mut problems = Vec::new();
    let mut descriptions = 0;
    let mut skills = 0;
    for (path, bytes) in &tree.files {
        let Some(dir) = path
            .strip_prefix("specengine/skills/")
            .and_then(|rest| rest.strip_suffix("/SKILL.md"))
        else {
            continue;
        };
        skills += 1;
        let at = format!("plugin/{path}");
        let skill = match split_skill(bytes) {
            Ok(skill) => skill,
            Err(problem) => {
                problems.push(format!("{at}: {problem}"));
                continue;
            }
        };
        if skill.name != dir {
            problems.push(format!("{at}: name {:?} is not its directory", skill.name));
        }
        if !is_skill_name(skill.name) || skill.name.len() > 64 {
            problems.push(format!("{at}: name {:?} breaks the grammar", skill.name));
        }
        if RESERVED.contains(&skill.name) {
            problems.push(format!("{at}: name {:?} is reserved", skill.name));
        }
        let description = skill.description;
        let characters = description.chars().count();
        if !(1..=1024).contains(&characters) {
            problems.push(format!(
                "{at}: description of {characters} characters, not 1 to 1 024"
            ));
        }
        if description
            .chars()
            .next()
            .is_some_and(|first| "-?:,[]{}#&*!|>'\"%@`".contains(first))
        {
            problems.push(format!("{at}: description starts with an indicator"));
        }
        if description.contains(": ") || description.contains(" #") || description.ends_with(':') {
            problems.push(format!("{at}: description is not a plain scalar"));
        }
        if description != description.trim() {
            problems.push(format!("{at}: description has outer whitespace"));
        }
        descriptions += description.len();
        if skill.body.len() > 4096 {
            problems.push(format!("{at}: body of {} B, over 4 096", skill.body.len()));
        }
    }
    if skills == 0 {
        problems.push("plugin/specengine/skills: no skill".to_owned());
    }
    if descriptions > 1200 {
        problems.push(format!(
            "skills: descriptions of {descriptions} B together, over 1 200"
        ));
    }
    problems
}

const READ_SPEC: &str = "specengine/skills/read-spec/SKILL.md";
const ASK_OWNER: &str = "specengine/skills/ask-owner/SKILL.md";

/// `tree` with the description of `path` replaced by `description`.
fn with_description(tree: &Tree, path: &str, description: &str) -> Tree {
    let text = String::from_utf8(tree.files[path].clone()).unwrap();
    let old = split_skill(text.as_bytes()).unwrap().description.to_owned();
    edited(
        tree,
        path,
        &format!("description: {old}\n"),
        &format!("description: {description}\n"),
    )
}

/// `tree` with the body of `path` padded or cut to `size` bytes.
fn with_body_size(tree: &Tree, path: &str, size: usize) -> Tree {
    let bytes = &tree.files[path];
    let body_len = split_skill(bytes).unwrap().body.len();
    let head = bytes[..bytes.len() - body_len].to_vec();
    let mut body = bytes[bytes.len() - body_len..].to_vec();
    body.resize(size, b'x');
    if size > 0 {
        body[size - 1] = b'\n';
    }
    let mut tree = tree.clone();
    tree.files.insert(path.to_owned(), [head, body].concat());
    tree
}

#[test]
fn ac05_each_skill_file_has_data_s_front_matter_and_budgets() {
    let tree = committed();
    assert_eq!(ac05_problems(&tree), Vec::<String>::new());
    let skills: Vec<&String> = tree
        .files
        .keys()
        .filter(|path| path.ends_with("/SKILL.md"))
        .collect();
    assert_eq!(skills.len(), 3, "{skills:?}");

    // M: `name` other than the directory.
    let renamed = edited(&tree, READ_SPEC, "name: read-spec\n", "name: read-specs\n");
    assert_names(&ac05_problems(&renamed), "is not its directory", "name");

    // M: a 1 025-character description (1 024 has no length problem).
    let long = with_description(&tree, READ_SPEC, &"a".repeat(1025));
    assert_names(
        &ac05_problems(&long),
        "description of 1025 characters",
        "1 025",
    );
    let longest = with_description(&tree, READ_SPEC, &"a".repeat(1024));
    assert!(!names(&ac05_problems(&longest), "description of"));
    assert!(names(&ac05_problems(&longest), "together, over 1 200"));
    let empty = with_description(&tree, READ_SPEC, "");
    assert_names(
        &ac05_problems(&empty),
        "description of 0 characters",
        "empty",
    );

    // M: `description: Use when: …`.
    let indicator = with_description(&tree, ASK_OWNER, "Use when: the spec is silent.");
    assert_names(
        &ac05_problems(&indicator),
        "is not a plain scalar",
        "Use when:",
    );
    for (description, needle) in [
        ("- a list item", "starts with an indicator"),
        ("\"quoted\"", "starts with an indicator"),
        ("`get_node` first", "starts with an indicator"),
        ("Read the spec #first", "is not a plain scalar"),
        ("Read the spec:", "is not a plain scalar"),
    ] {
        let mutated = with_description(&tree, ASK_OWNER, description);
        assert_names(&ac05_problems(&mutated), needle, description);
    }

    // M: a 4 097 B body (4 096 B passes).
    let over = with_body_size(&tree, ASK_OWNER, 4097);
    assert_names(&ac05_problems(&over), "body of 4097 B", "4 097 B");
    let at_cap = with_body_size(&tree, ASK_OWNER, 4096);
    assert_eq!(ac05_problems(&at_cap), Vec::<String>::new(), "4 096 B");

    // M: a skill `round` (its directory and name).
    let mut round = edited(&tree, READ_SPEC, "name: read-spec\n", "name: round\n");
    let bytes = round.files.remove(READ_SPEC).unwrap();
    round
        .files
        .insert("specengine/skills/round/SKILL.md".to_owned(), bytes);
    let problems = ac05_problems(&round);
    assert_names(&problems, "name \"round\" is reserved", "round");
    assert!(!names(&problems, "is not its directory"), "{problems:#?}");

    // Grammar and the two keys.
    for (from, to, needle) in [
        (
            "name: read-spec\n",
            "name: Read-Spec\n",
            "breaks the grammar",
        ),
        (
            "name: read-spec\n",
            "name: read--spec\n",
            "breaks the grammar",
        ),
        (
            "name: read-spec\n",
            "name: read-spec\nallowed-tools: x\n",
            "front-matter keys",
        ),
        (
            "name: read-spec\n",
            "name:  read-spec\n",
            "is not its directory",
        ),
        ("---\nname", "--- \nname", "no front-matter opening"),
    ] {
        let mutated = edited(&tree, READ_SPEC, from, to);
        assert_names(&ac05_problems(&mutated), needle, to);
    }
    let long_name = format!("name: {}\n", "a".repeat(65));
    let mutated = edited(&tree, READ_SPEC, "name: read-spec\n", &long_name);
    assert_names(
        &ac05_problems(&mutated),
        "breaks the grammar",
        "65 characters",
    );
}

// ------------------------------------------------------------------ AC-10

/// The BLAKE3 of `path \0 length \0 bytes` per file under
/// `plugin/specengine/`, paths relative to it, in byte order.
fn content_hash(tree: &Tree) -> String {
    let mut input = Vec::new();
    for (path, bytes) in &tree.files {
        if let Some(relative) = path.strip_prefix("specengine/") {
            input.extend_from_slice(relative.as_bytes());
            input.push(0);
            input.extend_from_slice(bytes.len().to_string().as_bytes());
            input.push(0);
            input.extend_from_slice(bytes);
        }
    }
    blake3_hex(&input)
}

fn version_key(version: &str) -> Option<(u64, u64, u64)> {
    if !is_semver(version) {
        return None;
    }
    let mut parts = version.split('.').map(|part| part.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

fn ac10_problems(tree: &Tree, pins: &[(&str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(&(last_version, last_hash)) = pins.last() else {
        return vec!["PINS: empty".to_owned()];
    };
    let mut previous: Option<(u64, u64, u64)> = None;
    for (version, hash) in pins {
        let Some(key) = version_key(version) else {
            problems.push(format!("PINS: {version:?} is not N.N.N"));
            continue;
        };
        if previous.is_some_and(|earlier| key <= earlier) {
            problems.push(format!("PINS: {version} does not increase"));
        }
        previous = Some(key);
        if hash.len() != 64 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            problems.push(format!("PINS: {version} hash is not lowercase hex"));
        }
    }
    let plugin = json_at(tree, PLUGIN_JSON).unwrap_or(Value::Null);
    if plugin["version"] != json!(last_version) {
        problems.push(format!(
            "PINS: the last version {last_version} is not plugin.json's {}",
            plugin["version"]
        ));
    }
    let hash = content_hash(tree);
    if hash != last_hash {
        problems.push(format!(
            "PINS: plugin/specengine/ hashes {hash}, not the last pin's {last_hash}"
        ));
    }
    problems
}

#[test]
fn ac10_the_content_and_version_are_the_last_pin() {
    let tree = committed();
    assert_eq!(ac10_problems(&tree, PINS), Vec::<String>::new());
    // Deterministic: the same files, the same hash; read twice.
    assert_eq!(content_hash(&committed()), content_hash(&tree));
    // The hash covers `plugin/specengine/` only, `.DS_Store` aside.
    let mut outside = tree.clone();
    outside
        .files
        .insert("README.md".to_owned(), b"outside\n".to_vec());
    assert_eq!(content_hash(&outside), content_hash(&tree));

    // M: a `SKILL.md` edited, PINS unchanged.
    let edited_skill = edited(
        &tree,
        READ_SPEC,
        "# Reading the spec\n",
        "# Reading the spec.\n",
    );
    assert_names(
        &ac10_problems(&edited_skill, PINS),
        "not the last pin's",
        "a SKILL.md edited",
    );
    // A renamed file with the same bytes changes the hash too.
    let mut moved = tree.clone();
    let bytes = moved.files.remove(READ_SPEC).unwrap();
    moved
        .files
        .insert("specengine/skills/read-specs/SKILL.md".to_owned(), bytes);
    assert_ne!(content_hash(&moved), content_hash(&tree));

    // M: an entry repeating the version.
    let (version, hash) = *PINS.last().unwrap();
    let repeated: Vec<(&str, &str)> = PINS.iter().copied().chain([(version, hash)]).collect();
    assert_names(
        &ac10_problems(&tree, &repeated),
        "does not increase",
        "a repeated version",
    );
    // A bumped version without its pin (PATCH past the last pin, so the
    // case stays a mutation whatever the last pin is), and a pin older
    // than the last.
    let (major, minor, patch) = version_key(version).unwrap();
    let next = format!("{major}.{minor}.{}", patch + 1);
    let parsed = json_at(&tree, PLUGIN_JSON).unwrap();
    let mut bumped = parsed;
    bumped["version"] = json!(next);
    assert_names(
        &ac10_problems(&with_json(&tree, PLUGIN_JSON, &bumped), PINS),
        "is not plugin.json's",
        &format!("{next} unpinned"),
    );
    let older: Vec<(&str, &str)> = [(version, hash), ("0.0.9", hash)].to_vec();
    assert_names(
        &ac10_problems(&tree, &older),
        "does not increase",
        "0.0.9 last",
    );
    assert!(
        version_key("0.10.0") > version_key("0.9.9"),
        "numeric, not textual, order"
    );
}
