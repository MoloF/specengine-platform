//! docs/features/specengine-guide.md (RM-0129) "Acceptance criteria" GU9
//! and GU10: the guide's pages are true, and this repository ships them
//! clean. The pages are read as `guide_source` reads them ("Data"); the
//! command list is the `spec` binary's own `--help` (beside this test
//! binary, refused when older than its sources or the pages); the tool list
//! is [`guide_source::KNOWN_TOOLS`] (MCP `mcp_guide.rs` holds it equal to
//! `tools/list`); the route table is every `.route("…")` literal of
//! `crates/specengine-http/src/app.rs`. The repository is only read.
//!
//! - GU9 `gu9_*`: each of the ten pages has the five `##` parts in order
//!   outside fenced code, its `### CLI`, `### MCP`, `### Screen` under `##
//!   How` in that order, at least one rule, every rule `GD-<TOPIC>-…`
//!   (Latin), a non-empty `summarises` whose every entry is a canon page of
//!   this repository holding the anchor, and at most 12 288 B; its title is
//!   "Data"'s. Every `spec <command>` a page names (a code span, a `$ spec`
//!   line of a `text` block, a `spec` line of a `sh` block) resolves in the
//!   CLI's command list, and every flag it writes is one of that command's;
//!   every backticked tool name is a tool; every `/api/…` route is in the
//!   daemon's route table (`:x` read as `{x}`). M (named): a page naming a
//!   removed command, run on a scratch page here (and a removed tool and
//!   route). Every `guide:<topic>#<ID>` cross-citation names a held rule
//!   (rule (7) makes it plain text, so `spec check` no longer judges it). M:
//!   `guide:records#GD-RECORDS-ONE-FIL` planted in a scratch page.
//! - GU10 `gu10_*` (at ship): `spec check` on this repository exits 0 with
//!   no `summary-stale`; `docs/index-guide.md` lists the ten pages; the root
//!   `docs/index.md` stays within its 10 240 B cap.

#[path = "../../specengine-cli/tests/guide_source/mod.rs"]
mod guide_source;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::SystemTime;

use guide_source::{
    GUIDE_ORDER, KNOWN_TOOLS, PAGE_CAP, PARTS, Page, TITLES, fenced_lines, repository_root,
    source_pages,
};
use serde_json::Value;

// ------------------------------------------------------------ the binary

fn newest_under(dir: &Path) -> Option<(PathBuf, SystemTime)> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let candidate = if entry.file_type().ok()?.is_dir() {
            newest_under(&path)
        } else {
            entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .map(|at| (path, at))
        };
        if let Some((path, at)) = candidate
            && newest.as_ref().is_none_or(|(_, best)| at > *best)
        {
            newest = Some((path, at));
        }
    }
    newest
}

/// The `spec` binary beside this test binary (its profile's directory),
/// refused when missing or older than a source of the crates it is built
/// from or a guide page (it embeds them).
fn spec_bin() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary");
    let path = exe
        .parent()
        .and_then(Path::parent)
        .expect("the profile directory")
        .join("spec");
    let build = "cargo build -p specengine-cli --bins (`cargo nextest run --workspace` builds it)";
    let built = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|_| panic!("{}: build it first: {build}", path.display()));
    let root = repository_root();
    let mut sources: Vec<PathBuf> = [
        "specengine-cli",
        "specengine-core",
        "specengine-model",
        "specengine-store",
        "specengine-code",
    ]
    .iter()
    .map(|krate| root.join("crates").join(krate).join("src"))
    .collect();
    sources.push(root.join("docs").join("guide"));
    for dir in sources {
        if let Some((newest, at)) = newest_under(&dir)
            && at > built
        {
            panic!(
                "{} is older than {}: rebuild it ({build})",
                path.display(),
                newest.display()
            );
        }
    }
    path
}

/// `spec args` in `cwd` with a scratch `HOME`: (exit, stdout, stderr).
fn run_spec(cwd: &Path, home: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(spec_bin())
        .env_clear()
        .env("HOME", home)
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("spawn spec");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-guide-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(&path).expect("a directory");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ------------------------------------------------------------ the CLI's list

/// What `spec <path> --help` lists: its subcommands and its flags.
#[derive(Debug, Clone, Default)]
struct Help {
    commands: BTreeSet<String>,
    flags: BTreeSet<String>,
}

/// The names of a clap help's `Commands:` section and every `--flag` of
/// its `Options:` section.
fn parse_help(text: &str) -> Help {
    let mut help = Help::default();
    let mut section = "";
    for line in text.lines() {
        if !line.starts_with(' ') && line.ends_with(':') {
            section = line;
            continue;
        }
        if !line.starts_with(' ') {
            section = "";
            continue;
        }
        if section == "Commands:" {
            if let Some(name) = line.split_whitespace().next()
                && line.starts_with("  ")
                && !line.starts_with("          ")
            {
                help.commands.insert(name.to_owned());
            }
        } else if section.ends_with("Options:") {
            for word in line.split([' ', ',', '=', '[', '<']) {
                if let Some(flag) = word.strip_prefix("--")
                    && !flag.is_empty()
                    && flag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                {
                    help.flags.insert(format!("--{flag}"));
                }
            }
        }
    }
    help.commands.remove("help");
    help
}

/// The CLI's help by command path, asked once each.
struct Cli {
    home: PathBuf,
    cwd: PathBuf,
    seen: Mutex<BTreeMap<Vec<String>, Help>>,
    _scratch: Scratch,
}

impl Cli {
    fn new() -> Self {
        let scratch = Scratch::new("cli-help");
        let home = scratch.dir("home");
        let cwd = scratch.dir("cwd");
        Self {
            home,
            cwd,
            seen: Mutex::new(BTreeMap::new()),
            _scratch: scratch,
        }
    }

    /// `spec <path…> --help`'s lists; `None` when the path is no command.
    fn help(&self, path: &[String]) -> Option<Help> {
        if let Some(found) = self.seen.lock().expect("help").get(path) {
            return Some(found.clone());
        }
        let mut args: Vec<&str> = path.iter().map(String::as_str).collect();
        args.push("--help");
        let (code, stdout, _) = run_spec(&self.cwd, &self.home, &args);
        if code != 0 {
            return None;
        }
        let help = parse_help(&stdout);
        self.seen
            .lock()
            .expect("help")
            .insert(path.to_vec(), help.clone());
        Some(help)
    }
}

/// Why `command` (the words after `spec`) is untrue, if it is: an unknown
/// command or subcommand, or a flag the resolved command does not take.
fn command_problem(cli: &Cli, command: &str) -> Option<String> {
    let mut words = Vec::new();
    for word in command.split_whitespace() {
        if [">", "|", ";", "&&", "2>&1", "#"].contains(&word) {
            break;
        }
        words.push(word);
    }
    let mut path: Vec<String> = Vec::new();
    let mut help = cli.help(&path).expect("`spec --help` answers");
    let mut resolving = true;
    let mut flags = Vec::new();
    for word in words {
        if word.starts_with("--") {
            flags.push(word.split('=').next().unwrap_or(word).to_owned());
            continue;
        }
        if word.starts_with('-') || !resolving {
            continue;
        }
        if help.commands.is_empty() {
            resolving = false;
            continue;
        }
        if !help.commands.contains(word) {
            return Some(format!(
                "`spec {}` has no command `{word}` (it lists {:?})",
                path.join(" "),
                help.commands
            ));
        }
        path.push(word.to_owned());
        help = cli
            .help(&path)
            .unwrap_or_else(|| panic!("`spec {} --help` fails", path.join(" ")));
    }
    if path.is_empty() {
        return None;
    }
    for flag in flags {
        if !["--help", "--version"].contains(&flag.as_str()) && !help.flags.contains(&flag) {
            return Some(format!("`spec {}` takes no `{flag}`", path.join(" ")));
        }
    }
    None
}

// ------------------------------------------------------------ a page's names

/// The code spans of `text`'s lines outside fenced code, each with its
/// line.
fn code_spans(page: &Page) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (line, raw, inside) in fenced_lines(&page.source) {
        if inside {
            continue;
        }
        let mut rest = raw;
        while let Some(open) = rest.find('`') {
            let ticks = rest[open..].chars().take_while(|c| *c == '`').count();
            let fence = "`".repeat(ticks);
            let after = &rest[open + ticks..];
            let Some(close) = after.find(&fence) else {
                break;
            };
            out.push((line, after[..close].trim().to_owned()));
            rest = &after[close + ticks..];
        }
    }
    out
}

/// The `spec` commands a page names: code spans that open with `spec `,
/// `$ spec` lines of a `text` (or untagged) block and `spec` lines of a
/// `sh` block; each with its line.
fn commands(page: &Page) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = code_spans(page)
        .into_iter()
        .filter_map(|(line, span)| {
            span.strip_prefix("spec ")
                .map(|rest| (line, rest.to_owned()))
        })
        .collect();
    let mut language: Option<String> = None;
    for (line, raw, inside) in fenced_lines(&page.source) {
        let text = raw.trim_end_matches(['\n', '\r']);
        let trimmed = text.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            language = match language {
                Some(_) => None,
                None => Some(trimmed.trim_start_matches(['`', '~']).trim().to_owned()),
            };
            continue;
        }
        let Some(tag) = language.as_deref() else {
            continue;
        };
        if !inside {
            continue;
        }
        let command = match tag {
            "sh" | "bash" | "shell" => trimmed.strip_prefix("spec "),
            "text" | "" | "console" => trimmed.strip_prefix("$ spec "),
            _ => None,
        };
        if let Some(command) = command {
            out.push((line, command.to_owned()));
        }
    }
    out
}

/// A backticked name in the shape of a tool: one of the tools' verbs, `_`,
/// then more.
fn tool_shaped(name: &str) -> bool {
    const VERBS: [&str; 16] = [
        "get", "list", "create", "split", "edit", "move", "propose", "ask", "report", "review",
        "mark", "claim", "submit", "complete", "find", "check",
    ];
    name.split_once('_').is_some_and(|(verb, rest)| {
        VERBS.contains(&verb)
            && !rest.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
    })
}

/// The tool names a page backticks: a code span's first word in a tool's
/// shape (`get_card {card_id}` names `get_card`).
fn tools(page: &Page) -> Vec<(usize, String)> {
    code_spans(page)
        .into_iter()
        .filter_map(|(line, span)| {
            let name: String = span
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            tool_shaped(&name).then_some((line, name))
        })
        .collect()
}

/// The `/api/…` routes a page backticks, `:x` read as `{x}`, a query cut.
fn routes(page: &Page) -> Vec<(usize, String)> {
    code_spans(page)
        .into_iter()
        .filter_map(|(line, span)| {
            let at = span.find("/api/")?;
            let path: String = span[at..]
                .split_whitespace()
                .next()?
                .split('?')
                .next()?
                .to_owned();
            Some((line, normalised_route(&path)))
        })
        .collect()
}

/// `:x` segments as `{x}`, `{*x}` as `{x}`.
fn normalised_route(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if let Some(name) = segment.strip_prefix(':') {
                format!("{{{name}}}")
            } else if let Some(name) = segment.strip_prefix("{*") {
                format!("{{{name}")
            } else {
                segment.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Every path of a `.route(` call in `crates/specengine-http/src/app.rs`,
/// comment lines left out, normalised.
fn route_table() -> BTreeSet<String> {
    let source = fs::read_to_string(
        repository_root()
            .join("crates")
            .join("specengine-http")
            .join("src")
            .join("app.rs"),
    )
    .expect("app.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(|line| format!("{line}\n"))
        .collect();
    let mut out = BTreeSet::new();
    for (at, _) in code.match_indices(".route(") {
        let rest = code[at + ".route(".len()..].trim_start();
        if let Some(path) = rest
            .strip_prefix('"')
            .and_then(|literal| literal.split_once('"'))
            .map(|(path, _)| path)
        {
            out.insert(normalised_route(path));
        }
    }
    assert!(
        out.contains("/api/projects/{p}/library"),
        "the table is read: {out:?}"
    );
    out
}

/// Every untrue name of `page`: commands, flags, tools, routes.
fn untrue(cli: &Cli, table: &BTreeSet<String>, page: &Page) -> Vec<String> {
    let at = |line: usize| format!("docs/guide/{}.md:{line}", page.topic);
    let mut out = Vec::new();
    for (line, command) in commands(page) {
        if let Some(problem) = command_problem(cli, &command) {
            out.push(format!("{}: {problem}", at(line)));
        }
    }
    for (line, name) in tools(page) {
        if !KNOWN_TOOLS.contains(&name.as_str()) {
            out.push(format!("{}: `{name}` is no tool", at(line)));
        }
    }
    for (line, route) in routes(page) {
        if !table.contains(&route) {
            out.push(format!("{}: `{route}` is no route of app.rs", at(line)));
        }
    }
    out
}

// ------------------------------------------------------------------ GU9

/// GU9: the ten pages, in `GUIDE_ORDER`, each titled as "Data"'s table,
/// with the five `##` parts in order outside fenced code and `### CLI`,
/// `### MCP`, `### Screen` under `## How` in that order; at most 12 288 B.
#[test]
fn gu9_each_page_has_its_parts() {
    let pages = source_pages();
    assert_eq!(pages.len(), GUIDE_ORDER.len());
    for (page, title) in pages.iter().zip(TITLES) {
        let topic = &page.topic;
        assert_eq!(page.title, title, "{topic}: \"Data\"'s title");
        let parts: Vec<&str> = page
            .headings
            .iter()
            .filter(|heading| heading.level == 2)
            .map(|heading| heading.text.as_str())
            .collect();
        assert_eq!(parts, PARTS, "{topic}: the five parts, in order");
        let how = page
            .headings
            .iter()
            .position(|heading| heading.level == 2 && heading.text == "How")
            .expect("How");
        let under_how: Vec<&str> = page.headings[how + 1..]
            .iter()
            .take_while(|heading| heading.level > 2)
            .filter(|heading| heading.level == 3)
            .map(|heading| heading.text.as_str())
            .collect();
        let order = ["CLI", "MCP", "Screen"];
        let positions: Vec<usize> = under_how
            .iter()
            .map(|name| {
                order
                    .iter()
                    .position(|known| known == name)
                    .unwrap_or_else(|| panic!("{topic}: `### {name}` under How"))
            })
            .collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{topic}: CLI, MCP, Screen in order: {under_how:?}"
        );
        assert!(
            page.source.len() <= PAGE_CAP,
            "{topic}: {} B over 12 288",
            page.source.len()
        );
        let levels: Vec<usize> = page
            .headings
            .iter()
            .filter(|heading| heading.level == 1)
            .map(|heading| heading.line)
            .collect();
        assert_eq!(levels.len(), 1, "{topic}: one title");
    }
}

/// GU9: at least one rule a page, every rule `GD-<TOPIC>-<NAME>` (ASCII
/// capitals, digits, `-`), each ID once in the guide.
#[test]
fn gu9_each_page_has_its_rules() {
    let pages = source_pages();
    let mut seen = BTreeSet::new();
    for page in &pages {
        let prefix = format!("GD-{}-", page.topic.to_ascii_uppercase());
        assert!(!page.rules.is_empty(), "{}: no rule", page.topic);
        for rule in &page.rules {
            assert!(
                rule.id.starts_with(&prefix) && rule.id.len() > prefix.len(),
                "{}: `{}` is not {prefix}…",
                page.topic,
                rule.id
            );
            assert!(
                rule.id
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-'),
                "{}: `{}` is not Latin capitals",
                page.topic,
                rule.id
            );
            assert!(seen.insert(rule.id.clone()), "`{}` twice", rule.id);
            assert!(!rule.title.is_empty(), "{}: a title", rule.id);
        }
    }
}

/// This repository's ID scheme.
fn scheme() -> specengine_model::IdScheme {
    let text = fs::read_to_string(repository_root().join("specengine.toml")).expect("the config");
    <specengine_model::IdScheme as specengine_core::IdSchemeToml>::from_toml(&text)
        .expect("this repository's scheme")
}

/// Why `entry` (`path[#anchor]`) names no canon page holding the anchor.
fn entry_problem(scheme: &specengine_model::IdScheme, entry: &str) -> Option<String> {
    let (path, anchor) = match entry.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (entry, None),
    };
    if !path.ends_with(".md") {
        return Some(format!("`{entry}` is no .md path"));
    }
    let full = repository_root().join(path);
    let Ok(bytes) = fs::read(&full) else {
        return Some(format!("`{entry}`: no such file"));
    };
    let text = String::from_utf8_lossy(&bytes);
    let front = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(front, _)| front)
        .unwrap_or_default();
    if !front.lines().any(|line| line.trim() == "class: canon") {
        return Some(format!("`{entry}` is no canon page"));
    }
    let anchor = anchor?;
    let parsed = specengine_core::parse(path, &bytes, scheme);
    let anchors: BTreeSet<&str> = parsed
        .anchors
        .iter()
        .map(|found| found.name.as_str())
        .collect();
    let ids: BTreeSet<&str> = parsed
        .nodes
        .iter()
        .filter_map(|node| node.id.as_deref())
        .collect();
    (!anchors.contains(anchor) && !ids.contains(anchor))
        .then(|| format!("`{entry}`: no anchor `{anchor}`"))
}

/// GU9: each page's `summarises` is non-empty, and every entry names a
/// canon page of this repository holding its anchor (core's anchors: heading
/// slugs, `{#…}`, `<a id>`; or a section ID).
#[test]
fn gu9_summarises_resolves() {
    let pages = source_pages();
    let scheme = scheme();
    let mut problems = Vec::new();
    for page in &pages {
        if page.summarises.is_empty() {
            problems.push(format!("{}: no summarises", page.topic));
        }
        for entry in &page.summarises {
            if let Some(problem) = entry_problem(&scheme, entry) {
                problems.push(format!("{}: {problem}", page.topic));
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// GU9: every command, flag, tool and route the ten pages name exists.
#[test]
fn gu9_every_name_a_page_gives_exists() {
    let pages = source_pages();
    let cli = Cli::new();
    let table = route_table();
    let mut problems = Vec::new();
    let mut counted = (0, 0, 0);
    for page in &pages {
        counted.0 += commands(page).len();
        counted.1 += tools(page).len();
        counted.2 += routes(page).len();
        problems.extend(untrue(&cli, &table, page));
    }
    assert!(
        problems.is_empty(),
        "untrue names:\n{}",
        problems.join("\n")
    );
    assert!(
        counted.0 >= 10 && counted.1 >= 10 && counted.2 >= 1,
        "the pages name commands, tools and routes: {counted:?}"
    );
    let named: BTreeSet<String> = pages
        .iter()
        .flat_map(|page| routes(page).into_iter().map(|(_, route)| route))
        .collect();
    for route in ["/api/guide", "/api/guide/{topic}"] {
        assert!(
            named.contains(route),
            "the guide's routes are named: {named:?}"
        );
    }
}

/// GU9, M (named) "a page naming a removed command", on a scratch page:
/// `spec frobnicate`, `spec card frobnicate`, a removed flag, a tool and a
/// route that do not exist are each named; the real ones beside them are
/// not.
#[test]
fn gu9_a_removed_name_is_caught() {
    let cli = Cli::new();
    let table = route_table();
    let source = "---\nclass: canon\nreviewed: 2026-11-20\nsummarises: [docs/canon/spec-guide.md]\n---\n\n\
                  # Probe\n\nA probe page.\n\n## What it solves\n\nRun `spec frobnicate`, \
                  `spec card frobnicate C-0001`, `spec check --frobnicate`, \
                  `spec guide proposals` and `spec card show C-0001 --json`.\n\n\
                  ```text\n$ spec unfrobnicate\n```\n\n\
                  Call `get_frobnicated {topic}` or `get_guide {topic}`; read \
                  `GET /api/frobnicate/:topic` or `GET /api/guide/:topic`.\n";
    let page = guide_source::page_of("probe", Path::new("probe.md"), source);
    let mut problems = untrue(&cli, &table, &page);
    problems.sort();
    let joined = problems.join("\n");
    for needle in [
        "no command `frobnicate`",
        "has no command `frobnicate` (",
        "takes no `--frobnicate`",
        "no command `unfrobnicate`",
        "`get_frobnicated` is no tool",
        "`/api/frobnicate/{topic}` is no route",
    ] {
        assert!(joined.contains(needle), "{needle}: {joined}");
    }
    assert_eq!(problems.len(), 6, "{joined}");
    assert!(!joined.contains("get_guide"), "{joined}");
    assert!(!joined.contains("/api/guide/"), "{joined}");
}

// ------------------------------------------------------------ GU9 citations

/// One `guide:<topic>[#<ID>]` a page writes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Citation {
    line: usize,
    topic: String,
    rule: Option<String>,
}

/// Every guide citation of `page` outside fenced code: `guide:` standing
/// alone (the char before it no letter, digit, `_`, `-`, `.`, `/` nor `:`,
/// recognition rule (5)'s left boundary), a topic (`[a-z][a-z0-9-]*`), and
/// optionally `#` and a rule ID (`[A-Z0-9-]+`). The template `guide://…`
/// and a placeholder `guide:<topic>` are no citation.
fn citations(page: &Page) -> Vec<Citation> {
    let mut out = Vec::new();
    for (line, raw, inside) in fenced_lines(&page.source) {
        if inside {
            continue;
        }
        for (at, _) in raw.match_indices("guide:") {
            let alone = raw[..at]
                .chars()
                .next_back()
                .is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':')));
            if !alone {
                continue;
            }
            let rest = &raw[at + "guide:".len()..];
            if !rest.starts_with(|c: char| c.is_ascii_lowercase()) {
                continue;
            }
            let topic: String = rest
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                .collect();
            let after = &rest[topic.len()..];
            let rule = after.strip_prefix('#').map(|id| {
                id.chars()
                    .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '-')
                    .collect::<String>()
                    .trim_end_matches('-')
                    .to_owned()
            });
            out.push(Citation { line, topic, rule });
        }
    }
    out
}

/// Why each citation of `page` names nothing in `pages`: a topic out of
/// `GUIDE_ORDER`, or a rule its topic's page does not hold.
fn citation_problems(pages: &[Page], page: &Page) -> Vec<String> {
    let mut out = Vec::new();
    for citation in citations(page) {
        let at = format!("docs/guide/{}.md:{}", page.topic, citation.line);
        let Some(cited) = pages.iter().find(|known| known.topic == citation.topic) else {
            out.push(format!("{at}: `guide:{}` names no topic", citation.topic));
            continue;
        };
        if let Some(rule) = &citation.rule
            && !cited.rules.iter().any(|held| &held.id == rule)
        {
            out.push(format!(
                "{at}: `guide:{}#{rule}` names no rule of {}",
                citation.topic, citation.topic
            ));
        }
    }
    out
}

/// GU9: rule (7) makes a guide citation plain text, so `spec check` no
/// longer judges the pages' cross-citations: every `guide:<topic>#<ID>`
/// they write (outside fenced code) names a topic of `GUIDE_ORDER` and a
/// rule that topic's page holds. The pages hold such citations.
#[test]
fn gu9_every_guide_citation_names_a_held_rule() {
    let pages = source_pages();
    let mut problems = Vec::new();
    let mut counted = 0;
    for page in &pages {
        let found = citations(page);
        counted += found
            .iter()
            .filter(|citation| citation.rule.is_some())
            .count();
        problems.extend(citation_problems(&pages, page));
    }
    assert!(problems.is_empty(), "{problems:#?}");
    assert!(
        counted >= 10,
        "the pages cite one another: {counted} citations"
    );
    assert!(
        pages
            .iter()
            .all(|page| GUIDE_ORDER.contains(&page.topic.as_str())),
        "the topics are GUIDE_ORDER's"
    );
}

/// GU9, M: a typo planted in a scratch page, `guide:records#GD-RECORDS-ONE-FIL`,
/// is named, and so is an unknown topic; a held rule, a topic alone, the
/// placeholder `guide:<topic>#<ID>`, the template `guide://{topic}`, a
/// non-standing `myguide:` and a citation inside fenced code are not.
#[test]
fn gu9_a_mistyped_guide_citation_is_caught() {
    let pages = source_pages();
    let source = "---\nclass: canon\nreviewed: 2026-11-20\nsummarises: [docs/canon/spec-guide.md]\n---\n\n\
                  # Probe\n\nA probe page.\n\n## What it solves\n\n\
                  See `guide:records#GD-RECORDS-ONE-FIL` and [[guide:recrods#GD-RECORDS-ONE-FILE]].\n\
                  Held: guide:records#GD-RECORDS-ONE-FILE, guide:hub, `guide:<topic>#<ID>`, \
                  `guide://{topic}`, myguide:nope#GD-X.\n\n\
                  ```text\nguide:nope#GD-NOPE\n```\n";
    let probe = guide_source::page_of("probe", Path::new("probe.md"), source);
    let problems = citation_problems(&pages, &probe);
    assert_eq!(
        problems,
        [
            "docs/guide/probe.md:13: `guide:records#GD-RECORDS-ONE-FIL` names no rule of records",
            "docs/guide/probe.md:13: `guide:recrods` names no topic",
        ],
        "{problems:#?}"
    );
}

// ------------------------------------------------------------------ GU10

/// GU10 (at ship): `spec check` on this repository exits 0, its verdict
/// `clean`, with no `summary-stale`.
#[test]
fn gu10_this_repository_checks_clean_with_no_summary_stale() {
    let scratch = Scratch::new("gu10-check");
    let home = scratch.dir("home");
    let root = repository_root();
    let root_text = root.to_str().expect("a UTF-8 root");
    let (code, stdout, stderr) = run_spec(&root, &home, &["--root", root_text, "check", "--json"]);
    let document: Value = serde_json::from_str(stdout.trim_end())
        .unwrap_or_else(|error| panic!("spec check --json: {error}\n{stdout}\n{stderr}"));
    let stale: Vec<&Value> = document["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| finding["code"] == "summary-stale")
        .collect();
    assert!(stale.is_empty(), "summary-stale at ship: {stale:#?}");
    assert_eq!(code, 0, "{stdout}\n{stderr}");
    assert_eq!(document["verdict"], "clean", "{stdout}");
}

/// GU10 (at ship): `docs/index-guide.md` lists the ten pages; the root
/// index stays within `[budgets] index_bytes` (10 240).
#[test]
fn gu10_the_guide_shard_lists_the_pages_and_the_root_index_fits() {
    let root = repository_root();
    let shard =
        fs::read_to_string(root.join("docs").join("index-guide.md")).expect("docs/index-guide.md");
    for topic in GUIDE_ORDER {
        assert!(
            shard.contains(&format!("guide/{topic}.md")),
            "docs/index-guide.md lists {topic}.md"
        );
    }
    let index = fs::metadata(root.join("docs").join("index.md"))
        .expect("docs/index.md")
        .len();
    assert!(index <= 10_240, "docs/index.md is {index} B, over 10 240");
    let config = fs::read_to_string(root.join("specengine.toml")).expect("the config");
    assert!(
        config.contains("docs/index-guide.md"),
        "the generator writes the shard"
    );
}
