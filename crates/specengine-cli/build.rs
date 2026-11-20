//! The build script of `specengine-cli` (task spec `specengine-guide`, "The
//! embedded guide (CLI)"; ADR-0085): std only, it reads
//! `docs/guide/*.md` and writes one file, `$OUT_DIR/guide.rs`, the guide
//! [`GUIDE_ORDER`] lists; it runs no process.
//!
//! Each page: its front-matter (`reviewed:`, `summarises:`), its title (the
//! first level-1 heading outside a fenced block), its summary (the first
//! sentence of the paragraph after that heading), its rules (every heading
//! outside a fenced block whose `{#…}` attribute is a `GD-` ID, its section
//! up to the next heading of its level or above) and its text (the bytes
//! after the front-matter's closing line). A title and a summary are as
//! written, Markdown kept (a rule's title is its heading's text before the
//! `{#…}` block). A sentence ends at the first `.` followed by whitespace or
//! the paragraph's end, so `spec.lock` never ends one.
//!
//! A build error, naming the file (and its line, where there is one): no
//! `docs/guide/` directory; a page that is not UTF-8, has no front-matter,
//! no `reviewed:`, a `summarises:` that is no list of strings, no title or
//! no paragraph after it; a topic absent from [`GUIDE_ORDER`], or an
//! ordered topic with no page; a rule ID the guide holds twice.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// The topics, in the order every reader lists them; a page's topic is its
/// file stem. A new page joins this order in the same change.
const GUIDE_ORDER: [&str; 10] = [
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

/// The guide's directory, from the package's.
const GUIDE_DIR: &str = "../../docs/guide";

/// The directory as an error names it, from the repository root.
const SHOWN_DIR: &str = "docs/guide";

/// The ID prefix of a guide rule.
const RULE_PREFIX: &str = "GD-";

fn main() {
    println!("cargo::rerun-if-changed={GUIDE_DIR}");
    // Cargo sets both for every build script; unset, the paths are the
    // package directory's own (a build script runs there).
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default();
    let out = std::env::var_os("OUT_DIR").unwrap_or_default();
    let dir = Path::new(&manifest).join(GUIDE_DIR);
    let source = match guide(&dir) {
        Ok(pages) => render(&pages),
        Err(message) => fail(&message),
    };
    let target = Path::new(&out).join("guide.rs");
    if let Err(error) = std::fs::write(&target, source) {
        fail(&format!("cannot write {}: {error}", target.display()));
    }
}

/// Stops the build with `message`.
fn fail(message: &str) -> ! {
    println!("cargo::error={message}");
    std::process::exit(1);
}

/// One rule as read.
struct Rule {
    id: String,
    title: String,
    line: usize,
    /// Its section in the page's text, byte offsets.
    start: usize,
    end: usize,
}

/// One page as read.
struct Page {
    topic: &'static str,
    title: String,
    summary: String,
    summarises: Vec<String>,
    reviewed: String,
    rules: Vec<Rule>,
    text: String,
}

/// Every page of `dir`, in [`GUIDE_ORDER`].
fn guide(dir: &Path) -> Result<Vec<Page>, String> {
    let entries = std::fs::read_dir(dir).map_err(|error| {
        format!(
            "{SHOWN_DIR}: the guide's directory cannot be read ({error}); every build embeds it"
        )
    })?;
    let mut stems = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("{SHOWN_DIR}: cannot be listed ({error})"))?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            return Err(format!(
                "{SHOWN_DIR}/{}: a page's name is not UTF-8",
                path.display()
            ));
        };
        stems.push(stem.to_owned());
    }
    stems.sort();
    for stem in &stems {
        if !GUIDE_ORDER.contains(&stem.as_str()) {
            return Err(format!(
                "{SHOWN_DIR}/{stem}.md: topic `{stem}` is not in GUIDE_ORDER \
                 (crates/specengine-cli/build.rs); a new page joins the order in the same change"
            ));
        }
    }
    let mut pages = Vec::with_capacity(GUIDE_ORDER.len());
    let mut held: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for topic in GUIDE_ORDER {
        let shown = format!("{SHOWN_DIR}/{topic}.md");
        if !stems.iter().any(|stem| stem == topic) {
            return Err(format!(
                "{shown}: missing; GUIDE_ORDER (crates/specengine-cli/build.rs) names topic `{topic}`"
            ));
        }
        let path = dir.join(format!("{topic}.md"));
        println!("cargo::rerun-if-changed={GUIDE_DIR}/{topic}.md");
        let bytes =
            std::fs::read(&path).map_err(|error| format!("{shown}: cannot be read ({error})"))?;
        let text = String::from_utf8(bytes).map_err(|error| {
            format!(
                "{shown}: not UTF-8 (byte {})",
                error.utf8_error().valid_up_to()
            )
        })?;
        let page = page(topic, &shown, &text)?;
        for rule in &page.rules {
            if let Some((file, line)) = held.get(&rule.id) {
                return Err(format!(
                    "{shown}:{}: rule `{}` repeats {file}:{line}; a rule ID is defined once in the guide",
                    rule.line, rule.id
                ));
            }
            held.insert(rule.id.clone(), (shown.clone(), rule.line));
        }
        pages.push(page);
    }
    Ok(pages)
}

/// One line of a file: its 1-based number, its start in the file, its
/// bytes without the line end.
struct Line<'a> {
    number: usize,
    start: usize,
    text: &'a str,
}

/// The lines of `text`, each without `\n` and a `\r` before it.
fn lines(text: &str) -> Vec<Line<'_>> {
    let mut found = Vec::new();
    let mut start = 0;
    for (index, raw) in text.split_inclusive('\n').enumerate() {
        let bare = raw.strip_suffix('\n').unwrap_or(raw);
        let bare = bare.strip_suffix('\r').unwrap_or(bare);
        found.push(Line {
            number: index + 1,
            start,
            text: bare,
        });
        start += raw.len();
    }
    found
}

/// Reads the page `topic` from `source`; `shown` names it in an error.
fn page(topic: &'static str, shown: &str, source: &str) -> Result<Page, String> {
    let all = lines(source);
    // An optional BOM, then front-matter iff the first line is `---`.
    let first = all
        .first()
        .map(|line| line.text.trim_start_matches('\u{feff}'));
    if first != Some("---") {
        return Err(format!(
            "{shown}:1: no front-matter; a guide page opens with `---`"
        ));
    }
    let Some(close) = all.iter().skip(1).position(|line| line.text == "---") else {
        return Err(format!(
            "{shown}:1: the front-matter is never closed by `---`"
        ));
    };
    let close = close + 1;
    let front = &all[1..close];
    let reviewed = reviewed(shown, front)?;
    let summarises = summarises(shown, front)?;
    // The text: the bytes after the closing line's line end.
    let offset = all.get(close + 1).map_or(source.len(), |line| line.start);
    let text = &source[offset..];
    let body: Vec<Line<'_>> = all[close + 1..]
        .iter()
        .map(|line| Line {
            number: line.number,
            start: line.start - offset,
            text: line.text,
        })
        .collect();
    let headings = headings(&body);
    let Some(title_at) = headings.iter().position(|heading| heading.level == 1) else {
        return Err(format!(
            "{shown}: no title; a guide page has a level-1 heading"
        ));
    };
    let title_heading = &headings[title_at];
    let title = written(&title_heading.content)
        .map_err(|problem| format!("{shown}:{}: the title {problem}", title_heading.line))?;
    let (summary_line, paragraph) = paragraph_after(&body, title_heading.line).ok_or_else(|| {
        format!(
            "{shown}:{}: no paragraph after the title; its first sentence is the page's summary",
            title_heading.line
        )
    })?;
    let summary = written(first_sentence(&paragraph))
        .map_err(|problem| format!("{shown}:{summary_line}: the summary {problem}"))?;
    let mut rules = Vec::new();
    for (index, heading) in headings.iter().enumerate() {
        let Some(id) = heading
            .id
            .as_deref()
            .filter(|id| id.starts_with(RULE_PREFIX))
        else {
            continue;
        };
        let end = headings[index + 1..]
            .iter()
            .find(|next| next.level <= heading.level)
            .map_or(text.len(), |next| next.start);
        let title = written(&heading.content).map_err(|problem| {
            format!("{shown}:{}: rule `{id}`'s title {problem}", heading.line)
        })?;
        rules.push(Rule {
            id: id.to_owned(),
            title,
            line: heading.line,
            start: heading.start,
            end,
        });
    }
    Ok(Page {
        topic,
        title,
        summary,
        summarises,
        reviewed,
        rules,
        text: text.to_owned(),
    })
}

/// The value of the top-level key `key` in the front-matter lines, its
/// line and the lines after it (a block list's items).
fn key_value<'a>(front: &'a [Line<'a>], key: &str) -> Option<(usize, &'a str, &'a [Line<'a>])> {
    front.iter().enumerate().find_map(|(index, line)| {
        let rest = line.text.strip_prefix(key)?.strip_prefix(':')?;
        Some((line.number, rest.trim(), &front[index + 1..]))
    })
}

/// A scalar as written: quotes dropped.
fn unquoted(value: &str) -> &str {
    let value = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

/// `reviewed:`, required.
fn reviewed(shown: &str, front: &[Line<'_>]) -> Result<String, String> {
    match key_value(front, "reviewed") {
        Some((_, value, _)) if !unquoted(value).is_empty() => Ok(unquoted(value).to_owned()),
        Some((line, ..)) => Err(format!("{shown}:{line}: `reviewed:` is empty")),
        None => Err(format!("{shown}: the front-matter has no `reviewed:`")),
    }
}

/// `summarises:`: a flow list `[a, b]` on its line (or over several), or a
/// block list of `- a` items; absent, none.
fn summarises(shown: &str, front: &[Line<'_>]) -> Result<Vec<String>, String> {
    let Some((line, value, after)) = key_value(front, "summarises") else {
        return Ok(Vec::new());
    };
    let refused = || format!("{shown}:{line}: `summarises:` is no list of strings");
    if value.is_empty() {
        let mut entries = Vec::new();
        for item in after {
            let Some(entry) = item.text.trim_start().strip_prefix("- ") else {
                break;
            };
            entries.push(unquoted(entry).to_owned());
        }
        return if entries.is_empty() {
            Err(refused())
        } else {
            Ok(entries)
        };
    }
    let mut flow = value.to_owned();
    let mut rest = after.iter();
    while !flow.trim_end().ends_with(']') {
        let Some(next) = rest.next() else {
            return Err(refused());
        };
        flow.push(' ');
        flow.push_str(next.text.trim());
    }
    let inner = flow
        .trim()
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .ok_or_else(refused)?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|entry| {
            let entry = unquoted(entry);
            if entry.is_empty() || entry.contains(['[', ']', '{', '}']) {
                Err(refused())
            } else {
                Ok(entry.to_owned())
            }
        })
        .collect()
}

/// An ATX heading outside a fenced block.
struct Heading {
    level: usize,
    /// Its text without the `#`s, a closing sequence and the `{…}`
    /// attribute block.
    content: String,
    /// The attribute block's `#ID`, if any.
    id: Option<String>,
    line: usize,
    /// Its line's start in the text.
    start: usize,
}

/// A fenced block's opening line: its character and its length.
fn fence_of(line: &str) -> Option<(char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let mark = rest
        .chars()
        .next()
        .filter(|mark| matches!(mark, '`' | '~'))?;
    let length = rest.chars().take_while(|c| *c == mark).count();
    // A backtick fence's info string holds no backtick.
    (length >= 3 && !(mark == '`' && rest[length..].contains('`'))).then_some((mark, length))
}

/// `line` closes the block `open` opened.
fn closes(line: &str, open: (char, usize)) -> bool {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return false;
    }
    let rest = line[indent..].trim_end();
    rest.chars().all(|c| c == open.0) && rest.chars().count() >= open.1
}

/// Every ATX heading of `body` outside a fenced block.
fn headings(body: &[Line<'_>]) -> Vec<Heading> {
    let mut found = Vec::new();
    let mut open: Option<(char, usize)> = None;
    for line in body {
        if let Some(fence) = open {
            if closes(line.text, fence) {
                open = None;
            }
            continue;
        }
        if let Some(fence) = fence_of(line.text) {
            open = Some(fence);
            continue;
        }
        if let Some(heading) = heading(line) {
            found.push(heading);
        }
    }
    found
}

/// `line` as an ATX heading: at most three spaces, one to six `#`, then a
/// space or the line's end.
fn heading(line: &Line<'_>) -> Option<Heading> {
    let indent = line.text.len() - line.text.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line.text[indent..];
    let level = rest.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let after = &rest[level..];
    if !(after.is_empty() || after.starts_with([' ', '\t'])) {
        return None;
    }
    let mut content = after.trim();
    // A closing sequence: `#`s after a space, or the whole content.
    let without = content.trim_end_matches('#');
    if without.is_empty() || without.ends_with([' ', '\t']) {
        content = without.trim_end();
    }
    let (content, id) = attributes(content);
    Some(Heading {
        level,
        content: content.to_owned(),
        id,
        line: line.number,
        start: line.start,
    })
}

/// A heading's text split from its trailing `{…}` attribute block, and the
/// block's `#ID`.
fn attributes(content: &str) -> (&str, Option<String>) {
    let Some(inner_end) = content.strip_suffix('}') else {
        return (content, None);
    };
    let Some(open) = inner_end.rfind('{') else {
        return (content, None);
    };
    let block = &inner_end[open + 1..];
    let id = block
        .split_whitespace()
        .find_map(|part| part.strip_prefix('#'))
        .filter(|id| !id.is_empty())
        .map(str::to_owned);
    (content[..open].trim_end(), id)
}

/// The first paragraph after the heading on line `after`: its first line's
/// number and its lines joined by a space; `None` when the next block is
/// no paragraph.
fn paragraph_after(body: &[Line<'_>], after: usize) -> Option<(usize, String)> {
    let mut lines = body
        .iter()
        .skip_while(|line| line.number <= after)
        .skip_while(|line| line.text.trim().is_empty());
    let first = lines.next()?;
    let opening = first.text.trim_start();
    let indent = first.text.len() - opening.len();
    // A heading, a quote, a table, HTML, a fence, a list item or a break:
    // `**bold**` or `_word_` still opens a paragraph.
    let marker = |text: &str| text.is_empty() || text.starts_with([' ', '\t']);
    let block = indent > 3
        || opening.starts_with(['#', '>', '|', '<'])
        || opening.starts_with("```")
        || opening.starts_with("~~~")
        || ["-", "*", "+"]
            .iter()
            .any(|bullet| opening.strip_prefix(bullet).is_some_and(marker))
        || opening
            .split_once(['.', ')'])
            .is_some_and(|(number, rest)| {
                !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) && marker(rest)
            })
        || opening
            .trim_end()
            .chars()
            .all(|c| matches!(c, '-' | '*' | '_' | ' '));
    if block {
        return None;
    }
    let mut joined = opening.trim_end().to_owned();
    for line in lines {
        if line.text.trim().is_empty() {
            break;
        }
        joined.push(' ');
        joined.push_str(line.text.trim());
    }
    Some((first.number, joined))
}

/// `raw` trimmed, refused when empty.
fn written(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("is empty".to_owned());
    }
    Ok(trimmed.to_owned())
}

/// The first sentence of `paragraph`: up to its first `.` followed by
/// whitespace or the paragraph's end; the whole paragraph when there is
/// none.
fn first_sentence(paragraph: &str) -> &str {
    for (index, c) in paragraph.char_indices() {
        if c != '.' {
            continue;
        }
        let next = paragraph[index + 1..].chars().next();
        if next.is_none_or(char::is_whitespace) {
            return &paragraph[..=index];
        }
    }
    paragraph
}

/// The generated source: [`GUIDE_ORDER`] and `GUIDE`.
fn render(pages: &[Page]) -> String {
    let mut out = String::new();
    out.push_str(
        "// Generated by crates/specengine-cli/build.rs from docs/guide/*.md; never edited.\n\n",
    );
    let _ = writeln!(
        out,
        "/// The guide's topics, in the order every reader lists them: each a page,\n\
         /// `docs/guide/<topic>.md`.\n\
         pub const GUIDE_ORDER: [&str; {}] = {:?};\n",
        GUIDE_ORDER.len(),
        GUIDE_ORDER
    );
    let longest = pages.iter().map(|page| page.text.len()).max().unwrap_or(0);
    let _ = writeln!(
        out,
        "/// The longest page's text, in bytes: `spec guide`'s longest answer.\n\
         const GUIDE_LONGEST: usize = {longest};\n"
    );
    let _ = writeln!(
        out,
        "/// Every page of the guide, in [`GUIDE_ORDER`], as this build embedded it.\n\
         pub static GUIDE: [GuidePage; {}] = [",
        pages.len()
    );
    for page in pages {
        let _ = writeln!(out, "    GuidePage {{");
        let _ = writeln!(out, "        topic: {:?},", page.topic);
        let _ = writeln!(out, "        title: {:?},", page.title);
        let _ = writeln!(out, "        summary: {:?},", page.summary);
        let _ = writeln!(out, "        summarises: &{:?},", page.summarises);
        let _ = writeln!(out, "        reviewed: {:?},", page.reviewed);
        let _ = writeln!(out, "        rules: &[");
        for rule in &page.rules {
            let _ = writeln!(
                out,
                "            GuideRule {{ id: {:?}, title: {:?}, line: {}, start: {}, end: {} }},",
                rule.id, rule.title, rule.line, rule.start, rule.end
            );
        }
        let _ = writeln!(out, "        ],");
        let _ = writeln!(out, "        text: {:?},", page.text);
        let _ = writeln!(out, "    }},");
    }
    out.push_str("];\n");
    out
}
