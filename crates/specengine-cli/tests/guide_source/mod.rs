//! docs/features/specengine-guide.md (RM-0129) "Data": the guide as its
//! source holds it, `docs/guide/<topic>.md`, read the way the spec says the
//! build embeds it and `spec guide` answers. Shared through `#[path]` by the
//! CLI's `guide*.rs`, MCP `mcp_guide.rs`, http `guide.rs` and eval
//! `guide_pages.rs`, so every crate compares a run with one reading of the
//! spec, never with another run of the code under test.
//!
//! - **A page**: front-matter between two `---` lines; `text` every byte
//!   after the closing `---` line ("the page without its front-matter");
//!   its title the first `# ` heading; its summary the first sentence of the
//!   paragraph after the title (up to the first `.` followed by whitespace
//!   or the paragraph's end); `summarises` and `reviewed` as written.
//! - **A rule**: a heading ending `{#GD-…}` outside fenced code; its title
//!   the heading's text before ` {#`; its `line` the page file's 1-based
//!   line (front-matter counted); its section the heading's line through the
//!   line before the next heading of its level or above, nested sections
//!   included.
//! - **The answers**: the index's text lines `<topic> | <title> | <summary>
//!   | rules <n>` and its JSON `{topics: [{topic, title, summary, rules}]}`;
//!   a page's JSON `{topic, title, summary, summarises, reviewed, rules,
//!   text}`; a rule's `{topic, rule: {id, title, line}, text}`; the exit-1
//!   lines of "The embedded guide (CLI)".
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// "Data", `GUIDE_ORDER`: the ten topics, in order.
pub const GUIDE_ORDER: [&str; 10] = [
    "start",
    "records",
    "features",
    "proposals",
    "cards",
    "practices",
    "auto",
    "markers",
    "hub",
    "documents",
];

/// "Data"'s table: each topic's title, in [`GUIDE_ORDER`].
pub const TITLES: [&str; 10] = [
    "Adopting SpecEngine",
    "Records, the spec tree and canon",
    "Features, criteria and decisions",
    "Proposals and the one writing door",
    "Cards, departments and flows",
    "Department practices",
    "The `[auto]` mode",
    "Code markers and the lock",
    "The hub",
    "The documents library",
];

/// "A page": the five `##` parts, required, in this order.
pub const PARTS: [&str; 5] = [
    "What it solves",
    "When to use it",
    "How",
    "Where it lives",
    "What not to do",
];

/// "Rules for a page": the Tier 2 cap, in bytes.
pub const PAGE_CAP: usize = 12_288;

/// `OUTPUT_CAP_CHARS`, which never binds on the guide.
pub const OUTPUT_CAP_CHARS: usize = 40_000;

/// The stdio server's tools once `get_guide` joins them (the MCP tests'
/// `TOOLS`, thirty-three, "all thirty-three named"), sorted. The eval crate
/// reads no MCP crate: MCP `mcp_guide.rs` holds this list equal to the
/// server's `tools/list`.
pub const KNOWN_TOOLS: [&str; 33] = [
    "ask_question",
    "check_binding",
    "claim_task",
    "complete_task",
    "create_card",
    "edit_card",
    "find_symbols",
    "get_card",
    "get_collisions",
    "get_context_bundle",
    "get_flow",
    "get_guide",
    "get_impact",
    "get_node",
    "get_proposal",
    "get_roadmap",
    "get_task",
    "get_trace",
    "get_tree",
    "list_cards",
    "mark_pairs",
    "move_card",
    "propose_change",
    "refs",
    "report_decision",
    "report_discrepancy",
    "report_drift",
    "report_run",
    "review_proposal",
    "search",
    "split_card",
    "submit_plan",
    "unmapped",
];

/// The repository root, from the including crate's manifest directory.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// `docs/guide/` of this repository.
pub fn source_dir() -> PathBuf {
    repository_root().join("docs").join("guide")
}

/// One rule of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    pub title: String,
    /// The page file's 1-based line of the rule's heading.
    pub line: usize,
    /// The rule section's bytes.
    pub text: String,
}

/// One heading outside fenced code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// The page file's 1-based line.
    pub line: usize,
    pub level: usize,
    /// The heading's text after its `#`s, as written (an attribute kept).
    pub text: String,
}

/// One page as its source holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub topic: String,
    pub path: PathBuf,
    pub source: String,
    /// The front-matter's lines between the two fences.
    pub front: Vec<String>,
    /// The lines the front-matter takes, both fences counted.
    pub front_lines: usize,
    /// Every byte after the closing fence's line.
    pub text: String,
    pub title: String,
    pub summary: String,
    pub summarises: Vec<String>,
    /// The page file's 1-based line of the `summarises:` key, 0 when none.
    pub summarises_line: usize,
    pub reviewed: String,
    pub headings: Vec<Heading>,
    pub rules: Vec<Rule>,
}

/// Each line of `text` with its line end, whether it lies inside fenced
/// code (a fence line itself counts as inside), from 1.
pub fn fenced_lines(text: &str) -> Vec<(usize, &str, bool)> {
    let mut inside = false;
    let mut out = Vec::new();
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let trimmed = line.trim_start();
        let fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if fence {
            inside = !inside;
            out.push((index + 1, line, true));
        } else {
            out.push((index + 1, line, inside));
        }
    }
    out
}

/// The ATX headings of `text` outside fenced code, `offset` added to each
/// line number.
pub fn headings_of(text: &str, offset: usize) -> Vec<Heading> {
    let mut out = Vec::new();
    for (line, raw, inside) in fenced_lines(text) {
        if inside {
            continue;
        }
        let body = raw.trim_end_matches(['\n', '\r']);
        let level = body.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&level) && body[level..].starts_with(' ') {
            out.push(Heading {
                line: line + offset,
                level,
                text: body[level..].trim().to_owned(),
            });
        }
    }
    out
}

/// A heading's `{#ID …}` attribute's ID, and its text before the attribute.
pub fn heading_id(text: &str) -> Option<(String, String)> {
    let body = text.strip_suffix('}')?;
    let at = body.rfind("{#")?;
    let id = body[at + 2..].split_whitespace().next()?.to_owned();
    Some((id, body[..at].trim().to_owned()))
}

/// The value of `key:` in `front`, as written, and its 1-based line within
/// the front-matter.
fn front_value<'a>(front: &'a [String], key: &str) -> Option<(usize, &'a str)> {
    front.iter().enumerate().find_map(|(index, line)| {
        line.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix(':'))
            .map(|value| (index + 1, value.trim()))
    })
}

fn unquoted(text: &str) -> String {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            text.strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(text)
        .to_owned()
}

/// `summarises` as written: a flow list `[a, b]` or a block list of `- a`
/// lines.
fn list_value(front: &[String], key: &str) -> (usize, Vec<String>) {
    let Some((line, value)) = front_value(front, key) else {
        return (0, Vec::new());
    };
    if let Some(inner) = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        let items = inner
            .split(',')
            .map(unquoted)
            .filter(|item| !item.is_empty())
            .collect();
        return (line, items);
    }
    if value.is_empty() {
        let items = front[line..]
            .iter()
            .map_while(|next| next.trim_start().strip_prefix("- ").map(unquoted))
            .collect();
        return (line, items);
    }
    (line, vec![unquoted(value)])
}

/// The first sentence of `paragraph`: up to the first `.` followed by
/// whitespace or the paragraph's end; the whole paragraph without one.
pub fn first_sentence(paragraph: &str) -> String {
    let chars: Vec<(usize, char)> = paragraph.char_indices().collect();
    for (n, (at, c)) in chars.iter().enumerate() {
        if *c == '.'
            && chars
                .get(n + 1)
                .is_none_or(|(_, next)| next.is_whitespace())
        {
            return paragraph[..=*at].to_owned();
        }
    }
    paragraph.to_owned()
}

/// `topic`'s page in `dir`, read.
pub fn page(dir: &Path, topic: &str) -> Page {
    let path = dir.join(format!("{topic}.md"));
    let source =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    page_of(topic, &path, &source)
}

/// A page from its `source`.
pub fn page_of(topic: &str, path: &Path, source: &str) -> Page {
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    assert_eq!(
        lines.first().map(|line| line.trim_end()),
        Some("---"),
        "{}: no front-matter",
        path.display()
    );
    let close = lines
        .iter()
        .skip(1)
        .position(|line| line.trim_end() == "---")
        .map(|at| at + 1)
        .unwrap_or_else(|| panic!("{}: an unclosed front-matter", path.display()));
    let front: Vec<String> = lines[1..close]
        .iter()
        .map(|line| line.trim_end_matches(['\n', '\r']).to_owned())
        .collect();
    let front_lines = close + 1;
    let text: String = lines[front_lines..].concat();
    let headings = headings_of(&text, front_lines);
    let title_heading = headings
        .iter()
        .find(|heading| heading.level == 1)
        .unwrap_or_else(|| panic!("{}: no `# ` title", path.display()));
    let title = title_heading.text.clone();
    let after_title: Vec<&str> = source
        .split_inclusive('\n')
        .skip(title_heading.line)
        .map(|line| line.trim_end_matches(['\n', '\r']))
        .skip_while(|line| line.trim().is_empty())
        .take_while(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect();
    let summary = first_sentence(&after_title.join(" "));
    let (key_line, summarises) = list_value(&front, "summarises");
    let summarises_line = if key_line == 0 { 0 } else { key_line + 1 };
    let reviewed = front_value(&front, "reviewed")
        .map(|(_, value)| unquoted(value))
        .unwrap_or_default();
    let file_lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut rules = Vec::new();
    for (n, heading) in headings.iter().enumerate() {
        let Some((id, rule_title)) = heading_id(&heading.text) else {
            continue;
        };
        if !id.starts_with("GD-") {
            continue;
        }
        let end = headings[n + 1..]
            .iter()
            .find(|next| next.level <= heading.level)
            .map_or(file_lines.len(), |next| next.line - 1);
        rules.push(Rule {
            id,
            title: rule_title,
            line: heading.line,
            text: file_lines[heading.line - 1..end].concat(),
        });
    }
    Page {
        topic: topic.to_owned(),
        path: path.to_path_buf(),
        source: source.to_owned(),
        front,
        front_lines,
        text,
        title,
        summary,
        summarises,
        summarises_line,
        reviewed,
        headings,
        rules,
    }
}

/// The ten pages of `dir`, in [`GUIDE_ORDER`].
pub fn pages(dir: &Path) -> Vec<Page> {
    GUIDE_ORDER.iter().map(|topic| page(dir, topic)).collect()
}

/// The ten pages of this repository.
pub fn source_pages() -> Vec<Page> {
    pages(&source_dir())
}

// ------------------------------------------------------------ the answers

/// `spec guide`'s text: one line a topic.
pub fn index_text(pages: &[Page]) -> String {
    pages
        .iter()
        .map(|page| {
            format!(
                "{} | {} | {} | rules {}\n",
                page.topic,
                page.title,
                page.summary,
                page.rules.len()
            )
        })
        .collect()
}

/// `spec guide --json`'s document.
pub fn index_json(pages: &[Page]) -> Value {
    json!({
        "topics": pages
            .iter()
            .map(|page| json!({
                "topic": page.topic,
                "title": page.title,
                "summary": page.summary,
                "rules": page.rules.len(),
            }))
            .collect::<Vec<Value>>()
    })
}

/// A rule's `{id, title, line}`.
pub fn rule_entry(rule: &Rule) -> Value {
    json!({"id": rule.id, "title": rule.title, "line": rule.line})
}

/// `spec guide <topic> --json`'s document.
pub fn page_json(page: &Page) -> Value {
    json!({
        "topic": page.topic,
        "title": page.title,
        "summary": page.summary,
        "summarises": page.summarises,
        "reviewed": page.reviewed,
        "rules": page.rules.iter().map(rule_entry).collect::<Vec<Value>>(),
        "text": page.text,
    })
}

/// `spec guide <topic>#<ID> --json`'s document.
pub fn rule_json(page: &Page, rule: &Rule) -> Value {
    json!({"topic": page.topic, "rule": rule_entry(rule), "text": rule.text})
}

/// A JSON string, as `serde_json` writes it.
fn quoted(text: &str) -> String {
    serde_json::to_string(text).expect("a string encodes")
}

/// A rule's `{"id","title","line"}`, compact, in "Data"'s key order.
fn rule_entry_text(rule: &Rule) -> String {
    format!(
        "{{\"id\":{},\"title\":{},\"line\":{}}}",
        quoted(&rule.id),
        quoted(&rule.title),
        rule.line
    )
}

/// `spec guide --json`'s stdout byte for byte: compact, the keys in
/// "Data"'s order (`topic`, `title`, `summary`, `rules`), one LF.
pub fn index_json_text(pages: &[Page]) -> String {
    let topics: Vec<String> = pages
        .iter()
        .map(|page| {
            format!(
                "{{\"topic\":{},\"title\":{},\"summary\":{},\"rules\":{}}}",
                quoted(&page.topic),
                quoted(&page.title),
                quoted(&page.summary),
                page.rules.len()
            )
        })
        .collect();
    format!("{{\"topics\":[{}]}}\n", topics.join(","))
}

/// `spec guide <topic> --json`'s stdout byte for byte: `topic`, `title`,
/// `summary`, `summarises`, `reviewed`, `rules`, `text`, one LF.
pub fn page_json_text(page: &Page) -> String {
    let summarises: Vec<String> = page.summarises.iter().map(|entry| quoted(entry)).collect();
    let rules: Vec<String> = page.rules.iter().map(rule_entry_text).collect();
    format!(
        "{{\"topic\":{},\"title\":{},\"summary\":{},\"summarises\":[{}],\"reviewed\":{},\
         \"rules\":[{}],\"text\":{}}}\n",
        quoted(&page.topic),
        quoted(&page.title),
        quoted(&page.summary),
        summarises.join(","),
        quoted(&page.reviewed),
        rules.join(","),
        quoted(&page.text)
    )
}

/// `spec guide <topic>#<ID> --json`'s stdout byte for byte: `topic`,
/// `rule` (`id`, `title`, `line`), `text`, one LF.
pub fn rule_json_text(page: &Page, rule: &Rule) -> String {
    format!(
        "{{\"topic\":{},\"rule\":{},\"text\":{}}}\n",
        quoted(&page.topic),
        rule_entry_text(rule),
        quoted(&rule.text)
    )
}

/// The exit-1 line of an unknown topic, no line end.
pub fn unknown_topic(topic: &str) -> String {
    format!(
        "spec: no guide topic `{topic}`; topics: {}",
        GUIDE_ORDER.join(", ")
    )
}

/// The exit-1 line of an unknown rule in `page`, no line end.
pub fn unknown_rule(page: &Page, id: &str) -> String {
    let ids: Vec<&str> = page.rules.iter().map(|rule| rule.id.as_str()).collect();
    format!(
        "spec: no rule `{id}` in guide topic `{}`; rules: {}",
        page.topic,
        ids.join(", ")
    )
}

/// One read of the guide: its REF (`None` for the index), the MCP
/// arguments, the text and the JSON the spec gives it.
#[derive(Debug, Clone)]
pub struct Form {
    pub reference: Option<String>,
    pub arguments: Value,
    pub text: String,
    pub json: Value,
    /// `--json`'s stdout byte for byte, the keys in "Data"'s order.
    pub json_text: String,
}

/// Every answering read: the index, each topic, each rule of each topic.
pub fn forms(pages: &[Page]) -> Vec<Form> {
    let mut out = vec![Form {
        reference: None,
        arguments: json!({}),
        text: index_text(pages),
        json: index_json(pages),
        json_text: index_json_text(pages),
    }];
    for page in pages {
        out.push(Form {
            reference: Some(page.topic.clone()),
            arguments: json!({"topic": page.topic}),
            text: page.text.clone(),
            json: page_json(page),
            json_text: page_json_text(page),
        });
        for rule in &page.rules {
            out.push(Form {
                reference: Some(format!("{}#{}", page.topic, rule.id)),
                arguments: json!({"topic": page.topic, "rule": rule.id}),
                text: rule.text.clone(),
                json: rule_json(page, rule),
                json_text: rule_json_text(page, rule),
            });
        }
    }
    out
}

/// Strings only the guide holds: every rule ID, every title and every
/// summary, for "contains no guide text".
pub fn guide_needles(pages: &[Page]) -> Vec<String> {
    let mut out = Vec::new();
    for page in pages {
        out.push(page.title.clone());
        out.push(page.summary.clone());
        out.extend(page.rules.iter().map(|rule| rule.id.clone()));
    }
    out
}

/// The needles of [`guide_needles`] found in `text`.
pub fn guide_text_in(text: &str, pages: &[Page]) -> Vec<String> {
    guide_needles(pages)
        .into_iter()
        .filter(|needle| text.contains(needle.as_str()))
        .collect()
}
