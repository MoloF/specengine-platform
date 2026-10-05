//! Comment pointers from code into the docs (docs/features/pointer-sweep.md
//! AC-04, enforcing AC-01, AC-02 and AC-03): every `.rs` file under
//! `crates/*/src` and `crates/*/tests` is scanned as text; the stale-pointer
//! and Data-form checks also read every `.ts` and `.tsx` file under `ui/src`
//! (docs/features/ui-shell.md "Roles", AC-06: each provisional type cites an
//! existing heading).
//!
//! - No pointer to a spec section that now only delegates (the stale
//!   patterns are assembled below so this file never matches itself).
//! - The 05 section on code layers is cited only for layers A–C, the Bevy
//!   detector and the schedule dump: exactly the "Stays" list of the spec.
//! - A citation of the compacted layer-A task spec names an AC, never a
//!   heading; the rewritten comments cite the canon instead.
//! - Every citation in the spec's Data form (a backticked `.md` path, one
//!   space, a double-quoted heading, further headings after `, `) names
//!   headings that exist verbatim in that file, each quoted heading on one
//!   line.
//!
//! Named mutations: citing "RON bindings" in one row, or restoring one stale
//! pointer, turns this red.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// Every `.rs` file under `crates/*/src` and `crates/*/tests`, as
/// (repository-relative path with `/`, text), sorted by path.
fn rust_files() -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
            .map(|entry| entry.expect("directory entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let bytes = fs::read(&path).expect("readable source file");
                let relative = path
                    .strip_prefix(root)
                    .expect("under the repository")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((relative, String::from_utf8_lossy(&bytes).into_owned()));
            }
        }
    }
    let root = repository_root();
    let mut crates: Vec<PathBuf> = fs::read_dir(root.join("crates"))
        .expect("crates/ readable")
        .map(|entry| entry.expect("crate entry").path())
        .filter(|path| path.is_dir())
        .collect();
    crates.sort();
    let mut out = Vec::new();
    for krate in crates {
        for area in ["src", "tests"] {
            let dir = krate.join(area);
            if dir.is_dir() {
                walk(&root, &dir, &mut out);
            }
        }
    }
    assert!(
        out.len() > 100,
        "the walk found only {} files: wrong root?",
        out.len()
    );
    out
}

/// Every `.ts` and `.tsx` file under `ui/src` (installed packages skipped),
/// as (repository-relative path with `/`, text), sorted by path.
fn ui_sources() -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
            .map(|entry| entry.expect("directory entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "node_modules") {
                    walk(root, &path, out);
                }
            } else if path
                .extension()
                .is_some_and(|ext| ext == "ts" || ext == "tsx")
            {
                let bytes = fs::read(&path).expect("readable source file");
                let relative = path
                    .strip_prefix(root)
                    .expect("under the repository")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((relative, String::from_utf8_lossy(&bytes).into_owned()));
            }
        }
    }
    let root = repository_root();
    let mut out = Vec::new();
    walk(&root, &root.join("ui").join("src"), &mut out);
    assert!(
        out.iter()
            .any(|(path, _)| path == "ui/src/api/provisional.ts"),
        "the ui walk missed ui/src/api/provisional.ts ({} files)",
        out.len()
    );
    out
}

/// [`rust_files`] and then [`ui_sources`].
fn source_files() -> Vec<(String, String)> {
    let mut out = rust_files();
    out.extend(ui_sources());
    out
}

/// Pointers to spec sections that now only delegate to the canon or a README.
const STALE: [&str; 3] = [
    concat!("05 §", "5.3"),
    concat!("04 §", "4"),
    concat!("04 §", "3–4"),
];

/// The 05 section on code layers.
const LAYERS: &str = concat!("05 §", "5.1");

/// The compacted layer-A task spec.
const LAYER_A_SPEC: &str = concat!("layer-a-identity", ".md");

#[test]
fn no_stale_section_pointer_is_left() {
    let mut hits = Vec::new();
    for (path, text) in source_files() {
        for (index, line) in text.lines().enumerate() {
            for pattern in STALE {
                if line.contains(pattern) {
                    hits.push(format!(
                        "{path}:{}: {pattern:?}: {}",
                        index + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "stale pointers (cite the canon or the README heading instead):\n{}",
        hits.join("\n")
    );
}

/// The "Stays" list of the spec: files that keep the layers section, with
/// the number of hits each.
const STAYS: [(&str, usize); 12] = [
    ("crates/specengine-code/src/bevy/mod.rs", 1),
    ("crates/specengine-code/src/items.rs", 1),
    ("crates/specengine-code/src/lib.rs", 1),
    ("crates/specengine-code/src/qpath.rs", 1),
    ("crates/specengine-code/tests/bevy_detector.rs", 1),
    ("crates/specengine-eval/src/bevy/mod.rs", 2),
    ("crates/specengine-eval/src/main.rs", 2),
    ("crates/specengine-eval/src/ra/mod.rs", 1),
    ("crates/specengine-eval/tests/bevy_cli.rs", 1),
    ("crates/specengine-eval/tests/build_graph.rs", 1),
    ("crates/specengine-eval/tests/ra_cli.rs", 1),
    ("crates/specengine-ra/src/lib.rs", 1),
];

#[test]
fn the_layers_section_is_cited_only_for_layers_the_detector_and_the_dump() {
    let mut found: BTreeMap<String, usize> = BTreeMap::new();
    let mut off_topic = Vec::new();
    for (path, text) in rust_files() {
        for (index, line) in text.lines().enumerate() {
            if line.contains(LAYERS) {
                *found.entry(path.clone()).or_default() += 1;
                let lower = line.to_lowercase();
                if !["layer", "detector", "dump"]
                    .iter()
                    .any(|topic| lower.contains(topic))
                {
                    off_topic.push(format!("{path}:{}: {}", index + 1, line.trim()));
                }
            }
        }
    }
    let expected: BTreeMap<String, usize> = STAYS
        .iter()
        .map(|(path, count)| ((*path).to_owned(), *count))
        .collect();
    assert_eq!(found, expected, "hits of {LAYERS:?} per file");
    assert_eq!(found.values().sum::<usize>(), 14);
    assert!(
        off_topic.is_empty(),
        "{LAYERS:?} cited for something other than a layer, the detector or the dump:\n{}",
        off_topic.join("\n")
    );
}

/// `rest` starts with `,? AC-` and two digits.
fn names_an_ac(rest: &str) -> bool {
    let rest = rest.strip_prefix(',').unwrap_or(rest);
    rest.strip_prefix(" AC-").is_some_and(|digits| {
        digits.len() >= 2 && digits.as_bytes()[..2].iter().all(u8::is_ascii_digit)
    })
}

#[test]
fn the_layer_a_task_spec_is_cited_only_by_ac() {
    let mut bad = Vec::new();
    let mut count = 0;
    for (path, text) in rust_files() {
        for (index, line) in text.lines().enumerate() {
            for (at, _) in line.match_indices(LAYER_A_SPEC) {
                count += 1;
                if !names_an_ac(&line[at + LAYER_A_SPEC.len()..]) {
                    bad.push(format!("{path}:{}: {}", index + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{LAYER_A_SPEC} cited without an AC (its headings are gone; cite the canon):\n{}",
        bad.join("\n")
    );
    // At least the four rewritten module comments that keep their ACs.
    assert!(count >= 4, "only {count} citations of {LAYER_A_SPEC}");
}

/// The canon as the Data form writes it.
const CANON: &str = "`docs/canon/code-identity.md`";

/// The module comment: the leading `//!` lines (after any `#![…]`).
fn module_comment(text: &str) -> String {
    text.lines()
        .skip_while(|line| line.starts_with("#!["))
        .take_while(|line| line.starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_rewritten_comments_cite_the_canon() {
    let files: BTreeMap<String, String> = rust_files().into_iter().collect();
    let text = |path: &str| {
        files
            .get(path)
            .unwrap_or_else(|| panic!("{path} is gone"))
            .clone()
    };
    for (path, heading) in [
        (
            "crates/specengine-code/tests/qpath_units.rs",
            Some("Units and `qpath`"),
        ),
        (
            "crates/specengine-code/tests/marker_levels.rs",
            Some("Marker grammar"),
        ),
        (
            "crates/specengine-code/tests/ron_ambiguity.rs",
            Some("RON ambiguity"),
        ),
        // Without a heading the citation means the opening paragraph.
        ("crates/specengine-eval/tests/identity_literals.rs", None),
    ] {
        let comment = module_comment(&text(path));
        let needle = match heading {
            Some(heading) => format!("{CANON} \"{heading}\""),
            None => CANON.to_owned(),
        };
        assert!(
            comment.contains(&needle),
            "{path}: the module comment does not cite {needle}:\n{comment}"
        );
    }
    let needle = format!("{CANON} \"Eval outputs\"");
    assert!(
        text("crates/specengine-eval/tests/ast_hash_cli.rs").contains(&needle),
        "ast_hash_cli.rs does not cite {needle}"
    );
}

/// A citation in the Data form: the cited file and its quoted headings.
struct Citation {
    at: String,
    file: String,
    headings: Vec<String>,
}

/// The comment text of `line`: after `//!`, `///`, `//` or a block
/// comment's leading `*` when the line is a comment, else the whole line.
fn comment_text(line: &str) -> &str {
    let trimmed = line.trim_start();
    for prefix in ["//!", "///", "//", "*"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.trim_start();
        }
    }
    trimmed
}

/// `"Heading"` at the start of `text`: the heading and the rest of the line;
/// `Err` when the quote never closes on this line.
fn quoted(text: &str) -> Option<Result<(&str, &str), ()>> {
    let body = text.strip_prefix('"')?;
    Some(match body.find('"') {
        Some(end) => Ok((&body[..end], &body[end + 1..])),
        None => Err(()),
    })
}

/// Every Data-form citation in `text`; a quoted heading that does not close
/// on its own line goes to `broken`.
fn citations(path: &str, text: &str, broken: &mut Vec<String>) -> Vec<Citation> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let mut search = 0;
        while let Some(found) = line[search..].find(".md` \"") {
            let end = search + found + ".md".len();
            search = end;
            let Some(open) = line[..end].rfind('`') else {
                continue;
            };
            let file = &line[open + 1..end];
            if file.is_empty() || file.contains(char::is_whitespace) {
                continue;
            }
            let at = format!("{path}:{}", index + 1);
            let mut headings = Vec::new();
            let mut rest = &line[end + 2..];
            let mut row = index;
            loop {
                match quoted(rest) {
                    Some(Ok((heading, after))) => {
                        headings.push(heading.to_owned());
                        rest = after;
                    }
                    Some(Err(())) => {
                        broken.push(format!(
                            "{path}:{}: the quoted heading wraps: {}",
                            row + 1,
                            lines[row].trim()
                        ));
                        break;
                    }
                    None => break,
                }
                if rest.starts_with(", \"") {
                    rest = &rest[", ".len()..];
                } else if rest.trim_end() == "," && row + 1 < lines.len() {
                    // A heading list may wrap between headings, never inside one.
                    let next = comment_text(lines[row + 1]);
                    if !next.starts_with('"') {
                        break;
                    }
                    row += 1;
                    rest = next;
                } else {
                    break;
                }
            }
            out.push(Citation {
                at,
                file: file.to_owned(),
                headings,
            });
        }
    }
    out
}

/// The `#` headings of a Markdown file, fenced code skipped.
fn headings_of(file: &Path) -> Vec<String> {
    let text =
        fs::read_to_string(file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    let mut fenced = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let level = line.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&level) && line[level..].starts_with(' ') {
            out.push(line[level..].trim().to_owned());
        }
    }
    out
}

#[test]
fn every_quoted_heading_of_a_data_form_citation_exists_in_its_file() {
    let root = repository_root();
    let mut broken = Vec::new();
    let mut all = Vec::new();
    for (path, text) in source_files() {
        all.extend(citations(&path, &text, &mut broken));
    }
    // The UI's provisional types cite their sources (ui-shell AC-06).
    let ui_citations = all
        .iter()
        .filter(|citation| citation.at.starts_with("ui/src/api/provisional.ts:"))
        .count();
    assert!(
        ui_citations >= 20,
        "only {ui_citations} citations read in ui/src/api/provisional.ts"
    );
    let mut headings_by_file: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut missing = Vec::new();
    let mut quoted_headings = 0;
    for citation in &all {
        let file = root.join(&citation.file);
        if !file.is_file() {
            broken.push(format!("{}: no file {}", citation.at, citation.file));
            continue;
        }
        let known = headings_by_file
            .entry(citation.file.clone())
            .or_insert_with(|| headings_of(&file));
        for heading in &citation.headings {
            quoted_headings += 1;
            if !known.contains(heading) {
                missing.push(format!(
                    "{}: {:?} is not a heading of {}",
                    citation.at, heading, citation.file
                ));
            }
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
    assert!(
        missing.is_empty(),
        "{}\nheadings there: {headings_by_file:#?}",
        missing.join("\n")
    );
    eprintln!(
        "{} Data-form citations, {quoted_headings} quoted headings, files {:?}",
        all.len(),
        headings_by_file.keys().collect::<Vec<_>>()
    );
    assert!(
        quoted_headings >= 23,
        "only {quoted_headings} quoted headings in {} citations",
        all.len()
    );
}

#[test]
fn the_citation_scanner_reads_lists_and_rejects_a_wrapped_heading() {
    let mut broken = Vec::new();
    let text = concat!(
        "//! (`a/b.md` \"One\", \"Two `x`\"; `c.md`, \"Not data form\")\n",
        "/// `d.md` \"Three\",\n",
        "/// \"Four\" and more\n",
        "// `e.md` \"Wrapped\n",
        "// heading\"\n",
        " * Sources: `f.md` \"Five\";\n",
        " * `g.md` \"Six\",\n",
        " * \"Seven\".\n",
    );
    let found = citations("t.rs", text, &mut broken);
    let shape: Vec<(&str, &str, Vec<&str>)> = found
        .iter()
        .map(|c| {
            (
                c.at.as_str(),
                c.file.as_str(),
                c.headings.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            ("t.rs:1", "a/b.md", vec!["One", "Two `x`"]),
            ("t.rs:2", "d.md", vec!["Three", "Four"]),
            ("t.rs:4", "e.md", vec![]),
            ("t.rs:6", "f.md", vec!["Five"]),
            ("t.rs:7", "g.md", vec!["Six", "Seven"]),
        ]
    );
    assert_eq!(broken.len(), 1, "{broken:?}");
    assert!(broken[0].starts_with("t.rs:4: the quoted heading wraps"));
}
