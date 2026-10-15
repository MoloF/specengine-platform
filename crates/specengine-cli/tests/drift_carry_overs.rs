//! docs/features/drift-follow-up.md (4c-2), "Implementation": 4c-1's
//! carry-overs a test can pin (RD-4, Q6), as source scans; nothing is run
//! or written.
//!
//! - Every git command the CLI prints with a path after `--` names it as a
//!   quoted literal pathspec (`pathspec_word`, `':(literal)<path>'`): each
//!   `-- {name}` placeholder of a string literal (its continuation lines
//!   joined, as the compiler reads it) is bound to `pathspec_word(…)` in
//!   its file, or, positional (`-- {}`), has `pathspec_word(` among the
//!   arguments that follow. 4c-1 printed six raw (`create.rs` `left_behind`
//!   and step 9's `Made::Nothing` warning, `decide.rs` the same two).
//! - `round.rs` cites headings that exist: each `canon `<name>`,
//!   "<heading>"` and `task spec `<name>`, "<heading>"` of its comments
//!   names a heading of that document; the drift item's round is cited as
//!   canon `proposal-drift`, "Review, stage, round".

use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// Every `.rs` file under `dir`, sorted.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

// ------------------------------------------------------------ pathspecs

/// `text` with every string continuation joined as the compiler reads the
/// literal: an odd run of `\` ending a line drops its last `\`, the line
/// end and the next line's leading whitespace.
fn joined(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut run = 1;
        while chars.peek() == Some(&'\\') {
            chars.next();
            run += 1;
        }
        if run % 2 == 1 && chars.peek() == Some(&'\n') {
            out.push_str(&"\\".repeat(run - 1));
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
        } else {
            out.push_str(&"\\".repeat(run));
        }
    }
    out
}

/// Whether `joined` binds `name` with `pathspec_word(…)`: a `let` whose
/// pattern names it (`let name =`, `let (a, name) =`) and whose statement
/// (up to its `;`) calls `pathspec_word(`.
fn bound_to_pathspec(joined: &str, name: &str) -> bool {
    joined.match_indices("let ").any(|(at, _)| {
        let statement = &joined[at..];
        let statement = &statement[..statement.find(';').unwrap_or(statement.len())];
        let Some(pattern) = statement["let ".len()..].split(" =").next() else {
            return false;
        };
        let names: Vec<&str> = pattern
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')')
            .split(',')
            .map(|part| {
                let part = part.trim();
                let part = part.strip_prefix("mut ").unwrap_or(part);
                part.split(':').next().unwrap_or(part).trim()
            })
            .collect();
        names.contains(&name) && statement.contains("pathspec_word(")
    })
}

/// One `-- {…}` placeholder a printed git command passes as its path.
#[derive(Debug)]
struct Site {
    line: usize,
    /// The placeholder's name; empty for a positional `{}`.
    name: String,
    quoted: bool,
}

/// Every `-- {…}` placeholder of `text` (a Rust source) outside line
/// comments, whether it is quoted by `pathspec_word`.
fn pathspec_sites(text: &str) -> Vec<Site> {
    let joined = joined(text);
    let mut sites = Vec::new();
    for (at, _) in joined.match_indices("-- {") {
        let line_start = joined[..at].rfind('\n').map_or(0, |end| end + 1);
        if joined[line_start..].trim_start().starts_with("//") {
            continue;
        }
        let line = joined[..at].matches('\n').count() + 1;
        let mut rest = &joined[at + "-- ".len()..];
        while let Some(inner) = rest.strip_prefix('{') {
            if inner.starts_with('{') {
                break;
            }
            let Some(close) = inner.find('}') else {
                break;
            };
            let name = inner[..close].split(':').next().unwrap_or("").trim();
            let quoted = if name.is_empty() || name.bytes().all(|b| b.is_ascii_digit()) {
                let window: String = joined[at..].chars().take(400).collect();
                window.contains("pathspec_word(")
            } else {
                bound_to_pathspec(&joined, name)
            };
            sites.push(Site {
                line,
                name: name.to_owned(),
                quoted,
            });
            rest = &inner[close + 1..];
            match rest.strip_prefix(' ') {
                Some(next) if next.starts_with('{') => rest = next,
                _ => break,
            }
        }
    }
    sites
}

/// RD-4 / Q6: no git command the CLI prints names a raw path after `--`:
/// every such placeholder in `crates/specengine-cli/src` is a
/// `pathspec_word` (`':(literal)<path>'`). The scan sees the quoted ones
/// (`lock.rs`, `apply.rs`, `revisions.rs`) and refuses 4c-1's raw forms,
/// a continuation-split one included. M: one of the six printed paths raw
/// again.
#[test]
fn printed_git_paths_are_literal_pathspecs() {
    let src = repository_root().join("crates/specengine-cli/src");
    let mut seen = 0;
    let mut offenders = Vec::new();
    for path in rust_files(&src) {
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let file = path
            .strip_prefix(&src)
            .expect("under src")
            .display()
            .to_string();
        for site in pathspec_sites(&text) {
            seen += 1;
            if !site.quoted {
                offenders.push(format!(
                    "{file}:{} (joined): `-- {{{}}}`",
                    site.line, site.name
                ));
            }
        }
    }
    assert!(offenders.is_empty(), "raw paths printed: {offenders:#?}");
    assert!(seen >= 10, "only {seen} printed pathspecs seen");

    // The scan sees what it must refuse, and passes what it must not.
    let raw = "format!(\n    \"…(`git commit --only -m '{record_id}' -- \\\n     {top_path}`), \
               then …; or `git rm --cached -- \\\n     {top_path}`, delete the file\",\n)";
    let found: Vec<(String, bool)> = pathspec_sites(raw)
        .into_iter()
        .map(|site| (site.name, site.quoted))
        .collect();
    assert_eq!(
        found,
        [
            ("top_path".to_owned(), false),
            ("top_path".to_owned(), false)
        ]
    );
    let quoted = "let word = pathspec_word(&top_path);\nformat!(\"`git rm --cached -- {word}` \
                  drops it; `git reset -q -- \\\n {word}`\")";
    let sites = pathspec_sites(quoted);
    assert_eq!(sites.len(), 2, "{sites:?}");
    assert!(sites.iter().all(|site| site.quoted), "{sites:?}");
    let tuple = "let (text, locked) = (\n    pathspec_word(a),\n    pathspec_word(b),\n);\n\
                 format!(\"`git checkout -- {text} {locked}`\")";
    let sites = pathspec_sites(tuple);
    assert_eq!(sites.len(), 2);
    assert!(sites.iter().all(|site| site.quoted), "{sites:?}");
    let positional = "format!(\"`git checkout -- {}` restores it\",\n    pathspec_word(top_path))";
    let sites = pathspec_sites(positional);
    assert_eq!(sites.len(), 1, "{sites:?}");
    assert!(sites[0].quoted && sites[0].name.is_empty(), "{sites:?}");
    let unbound_positional = "format!(\"`git checkout -- {}` restores it\", top_path)";
    let sites = pathspec_sites(unbound_positional);
    assert_eq!(sites.len(), 1, "{sites:?}");
    assert!(!sites[0].quoted, "{sites:?}");
    let commented = "// `git rm --cached -- {top_path}` in a comment\nlet x = 1;";
    assert!(pathspec_sites(commented).is_empty());
    assert_eq!(
        joined("a \\\\\nb"),
        "a \\\\\nb",
        "an escaped backslash ends no line"
    );
}

// ------------------------------------------------------------- citations

/// A doc citation of a comment: `canon` or `task spec`, the document's
/// name, the heading and the comment's first line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Citation {
    class: &'static str,
    name: String,
    heading: String,
    line: usize,
}

/// Every `canon `<name>`, "<heading>"` and `task spec `<name>`,
/// "<heading>"` of `text`'s line comments, a comment's lines joined.
fn citations(text: &str) -> Vec<Citation> {
    let mut blocks: Vec<(usize, String)> = Vec::new();
    let mut open = false;
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let comment = ["//!", "///", "//"]
            .into_iter()
            .find_map(|marker| trimmed.strip_prefix(marker));
        match comment {
            Some(body) => {
                if open {
                    let block = &mut blocks.last_mut().expect("a block").1;
                    block.push(' ');
                    block.push_str(body.trim());
                } else {
                    blocks.push((number + 1, body.trim().to_owned()));
                    open = true;
                }
            }
            None => open = false,
        }
    }
    let mut found = Vec::new();
    for (line, block) in &blocks {
        for (marker, class) in [("canon `", "canon"), ("task spec `", "task spec")] {
            for (at, _) in block.match_indices(marker) {
                let start = at + marker.len();
                let Some(end) = block[start..].find('`') else {
                    continue;
                };
                let name = &block[start..start + end];
                let Some(after) = block[start + end + 1..].strip_prefix(", \"") else {
                    continue;
                };
                let Some(close) = after.find('"') else {
                    continue;
                };
                found.push(Citation {
                    class,
                    name: name.to_owned(),
                    heading: after[..close].to_owned(),
                    line: *line,
                });
            }
        }
    }
    found
}

/// The headings of the Markdown file `path` (`#` marks, spaces and a
/// trailing `{#…}` dropped); `None` when it does not exist.
fn headings(path: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .filter(|line| line.starts_with('#'))
            .map(|line| {
                let line = line.trim_start_matches('#').trim();
                match line.rfind(" {#") {
                    Some(at) if line.ends_with('}') => line[..at].trim().to_owned(),
                    _ => line.to_owned(),
                }
            })
            .collect(),
    )
}

/// RD-4 (`round.rs`'s pointer): every doc heading `round.rs` cites exists
/// (a canon under `docs/canon/`, a task spec under `docs/features/` or the
/// archive); the drift item's round is cited as canon `proposal-drift`,
/// "Review, stage, round", no longer task spec `drift-queue`, "Round" (a
/// heading it lacks). M: the old pointer back.
#[test]
fn round_cites_headings_that_exist() {
    let root = repository_root();
    let text =
        fs::read_to_string(root.join("crates/specengine-cli/src/round.rs")).expect("round.rs");
    let found = citations(&text);
    assert!(found.len() >= 5, "{found:#?}");
    let mut missing = Vec::new();
    for citation in &found {
        let candidates = match citation.class {
            "canon" => vec![
                root.join("docs/canon")
                    .join(format!("{}.md", citation.name)),
            ],
            _ => vec![
                root.join("docs/features")
                    .join(format!("{}.md", citation.name)),
                root.join("docs/archive")
                    .join(format!("{}.md", citation.name)),
            ],
        };
        let known = candidates
            .iter()
            .filter_map(|path| headings(path.as_path()))
            .any(|headings| headings.contains(&citation.heading));
        if !known {
            missing.push(format!(
                "round.rs:{}: {} `{}`, {:?}",
                citation.line, citation.class, citation.name, citation.heading
            ));
        }
    }
    assert!(missing.is_empty(), "cited headings missing: {missing:#?}");
    assert!(
        found.iter().any(|citation| citation.class == "canon"
            && citation.name == "proposal-drift"
            && citation.heading == "Review, stage, round"),
        "{found:#?}"
    );

    // The scan sees a stale pointer, a comment's lines joined.
    let stale = citations(
        "        // decided on a terminal (task spec\n        // `drift-queue`, \"Round\").\n",
    );
    assert_eq!(
        stale,
        [Citation {
            class: "task spec",
            name: "drift-queue".to_owned(),
            heading: "Round".to_owned(),
            line: 1,
        }]
    );
    assert!(
        !headings(&root.join("docs/features/drift-queue.md"))
            .expect("drift-queue.md")
            .contains(&"Round".to_owned())
    );
}
