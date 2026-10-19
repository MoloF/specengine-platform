//! Comment pointers from code into the docs (docs/features/pointer-sweep.md
//! AC-04, enforcing AC-01, AC-02 and AC-03): every `.rs` file under
//! `crates/*/src` and `crates/*/tests` is scanned as text; the stale-pointer
//! check also reads every `.ts` and `.tsx` file under `ui/src`.
//!
//! - No pointer to a spec section that now only delegates (the stale
//!   patterns are assembled below so this file never matches itself).
//! - The 05 section on code layers is cited only for layers A–C, the Bevy
//!   detector and the schedule dump: exactly the "Stays" list of the spec.
//! - A citation of the compacted layer-A task spec names an AC, never a
//!   heading; the rewritten comments cite the canon instead.
//! - Comment citations (`docs/README.md` "Enforcement", Code pointers): in the
//!   comment text of the walk (C4: the Rust sources and tests, `ui/src`'s
//!   `.ts`, `.tsx` and `.css`, `ui/*.{ts,js}`, the `#` comments of the
//!   `Cargo.toml` files), never in code (C7), a documentation path,
//!   backticked or bare, then an optional `,`, one space and a quoted
//!   heading, further ones after `, "` or across a line ending in `,`; the
//!   first heading may open the next comment line when the path ends its
//!   line (C1). Every quoted heading exists verbatim in that file, on one
//!   line (C2); every documentation path names a file, but for fixture
//!   data and the named allowlist (C3). The UI's provisional types cite
//!   their sources this way (`ui/README.md` "Contract seam").
//! - The CLI's form of a canon citation (its sources may not hold `docs/`,
//!   CLI `genre.rs` AC-17): in a comment, `` canon `<name>`, "<h>"[, "<h>"] ``
//!   is read as `docs/canon/<name>.md` and its headings checked alike; a
//!   heading of this form may wrap onto the next comment line (joined by
//!   one space). docs/features/bevy-roles-reads.md review: the CLI's
//!   carry-over citations (canon `code-roles`, "Links") were unchecked.
//!
//! Named mutations: citing "RON bindings" in one row, or restoring one stale
//! pointer, turns this red; so do the grammar self-test's and the floors'
//! (docs/features/citation-sweep.md AC-01 to AC-09).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// `path` as a repository-relative path with `/`.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("under the repository")
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The text of the file at `path`, invalid UTF-8 replaced.
fn read_text(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Every file under `dir` with one of `extensions` (installed packages
/// skipped), as (repository-relative path with `/`, text), sorted by path.
fn walk(root: &Path, dir: &Path, extensions: &[&str], out: &mut Vec<(String, String)>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "node_modules") {
                walk(root, &path, extensions, out);
            }
        } else if path
            .extension()
            .is_some_and(|ext| extensions.iter().any(|wanted| ext == *wanted))
        {
            out.push((relative(root, &path), read_text(&path)));
        }
    }
}

/// The crate directories under `crates/`, sorted.
fn crate_dirs(root: &Path) -> Vec<PathBuf> {
    let mut crates: Vec<PathBuf> = fs::read_dir(root.join("crates"))
        .expect("crates/ readable")
        .map(|entry| entry.expect("crate entry").path())
        .filter(|path| path.is_dir())
        .collect();
    crates.sort();
    crates
}

/// Every `.rs` file under `crates/*/src` and `crates/*/tests`, as
/// (repository-relative path with `/`, text), sorted by path.
fn rust_files() -> Vec<(String, String)> {
    let root = repository_root();
    let mut out = Vec::new();
    for krate in crate_dirs(&root) {
        for area in ["src", "tests"] {
            let dir = krate.join(area);
            if dir.is_dir() {
                walk(&root, &dir, &["rs"], &mut out);
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
    let root = repository_root();
    let mut out = Vec::new();
    walk(
        &root,
        &root.join("ui").join("src"),
        &["ts", "tsx"],
        &mut out,
    );
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

/// The citation walk (C4): [`source_files`], then `ui/src/**/*.css`,
/// `ui/*.{ts,js}`, the root `Cargo.toml` and `crates/*/Cargo.toml`.
fn cited_files() -> Vec<(String, String)> {
    let root = repository_root();
    let mut out = source_files();
    let mut styles = Vec::new();
    walk(&root, &root.join("ui").join("src"), &["css"], &mut styles);
    assert!(
        styles
            .iter()
            .any(|(path, _)| path == "ui/src/styles/app.css"),
        "the walk missed ui/src/styles/app.css ({} style files)",
        styles.len()
    );
    out.extend(styles);
    let mut configs: Vec<PathBuf> = fs::read_dir(root.join("ui"))
        .expect("ui/ readable")
        .map(|entry| entry.expect("ui entry").path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext == "ts" || ext == "js")
        })
        .collect();
    configs.sort();
    configs.push(root.join("Cargo.toml"));
    configs.extend(
        crate_dirs(&root)
            .into_iter()
            .map(|krate| krate.join("Cargo.toml"))
            .filter(|path| path.is_file()),
    );
    for path in configs {
        out.push((relative(&root, &path), read_text(&path)));
    }
    for wanted in ["ui/vite.config.ts", "ui/eslint.config.js", "Cargo.toml"] {
        assert!(
            out.iter().any(|(path, _)| path == wanted),
            "the walk missed {wanted}"
        );
    }
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

/// The documentation roots (C1): a path read in comment text is a citation
/// only when it starts at one of these; `*` stands for one path segment's
/// name, a trailing `/` for anything below. Fixture and sample layouts
/// (`docs/spec/…`, `docs/records/…`, `book/…`, this file's own `a/…`
/// samples) lie outside, and so does any other top-level `docs/` file: the
/// tests' temporary corpora name a dozen (`docs/a.md`, `docs/s.md`, …).
const DOC_ROOTS: [&str; 11] = [
    "CLAUDE.md",
    "README.md",
    "docs/README.md",
    "docs/index*.md",
    "docs/archive/",
    "docs/canon/",
    "docs/decisions/",
    "docs/features/",
    "docs/specs/",
    "crates/*/README.md",
    "ui/README.md",
];

/// Whether `name` matches the one-segment pattern `pattern` (at most one
/// `*`).
fn segment_matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((head, tail)) => {
            name.len() >= head.len() + tail.len() && name.starts_with(head) && name.ends_with(tail)
        }
    }
}

/// Whether `path` starts at one of `roots` (see [`DOC_ROOTS`]); a path with
/// an empty, `.` or `..` segment never does.
fn in_roots(path: &str, roots: &[&str]) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    if segments
        .iter()
        .any(|segment| segment.is_empty() || *segment == "." || *segment == "..")
    {
        return false;
    }
    roots.iter().any(|root| match root.strip_suffix('/') {
        Some(_) => path.starts_with(root),
        None => {
            let patterns: Vec<&str> = root.split('/').collect();
            patterns.len() == segments.len()
                && patterns
                    .iter()
                    .zip(&segments)
                    .all(|(pattern, name)| segment_matches(pattern, name))
        }
    })
}

/// Whether `relative` names a file under `base`, each component spelled
/// exactly as on disk (the owner's macOS folds case; the check must not).
fn names_a_file(base: &Path, relative: &str) -> bool {
    let mut path = base.to_path_buf();
    for part in relative.split('/') {
        let Ok(entries) = fs::read_dir(&path) else {
            return false;
        };
        if !entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_str() == Some(part))
        {
            return false;
        }
        path.push(part);
    }
    path.is_file()
}

/// How a walked file writes its comments (C4).
#[derive(Clone, Copy, PartialEq)]
enum Syntax {
    /// `.rs`, `.ts`, `.tsx`, `.js`: `//`, `///`, `//!`, `/*`, `/**`, and
    /// the lines inside a `/* … */` opened on a comment line (a leading `*`
    /// stripped there; outside one, a `*`-led line is code).
    Slashes,
    /// `.css`: `/* … */` only, its continuation lines with or without `*`.
    Css,
    /// `Cargo.toml`: a line-leading `#`.
    Hash,
}

/// The comment text of each line of `text` (file `path`, C7): `Some` with
/// what follows the marker, up to `*/`, on a comment line; `None` on code.
fn comment_lines<'a>(path: &str, text: &'a str) -> Vec<Option<&'a str>> {
    let syntax = if path.ends_with(".css") {
        Syntax::Css
    } else if path.ends_with(".toml") {
        Syntax::Hash
    } else {
        Syntax::Slashes
    };
    let mut open = false;
    text.lines()
        .map(|line| {
            let trimmed = line.trim();
            if syntax == Syntax::Hash {
                return trimmed.strip_prefix('#').map(str::trim);
            }
            if open {
                let body = match trimmed.strip_prefix('*') {
                    Some(rest) if !rest.starts_with('/') => rest,
                    _ => trimmed,
                };
                return Some(block_text(body, &mut open));
            }
            if let Some(rest) = trimmed.strip_prefix("/*") {
                let rest = if rest.starts_with("*/") {
                    rest
                } else {
                    rest.strip_prefix(['*', '!']).unwrap_or(rest)
                };
                open = true;
                return Some(block_text(rest, &mut open));
            }
            match trimmed.strip_prefix("//") {
                Some(rest) if syntax == Syntax::Slashes => {
                    Some(rest.strip_prefix(['/', '!']).unwrap_or(rest).trim())
                }
                _ => None,
            }
        })
        .collect()
}

/// `body` up to its `*/` (which closes the block), trimmed.
fn block_text<'a>(body: &'a str, open: &mut bool) -> &'a str {
    match body.find("*/") {
        Some(end) => {
            *open = false;
            body[..end].trim()
        }
        None => body.trim(),
    }
}

/// Whether byte `b` may sit in a documentation path.
fn path_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'/' | b'-')
}

/// The `.md` paths in comment text `text`, as byte ranges: a run of path
/// bytes ending in `.md`, not continued by a name (`.mdx`, `.md.bak`,
/// `x.md/`) and not part of a pattern (a `*`, `{`, `}`, `<` or `>` next to
/// it).
fn md_paths(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let pattern = |b: Option<&u8>| matches!(b, Some(b'*' | b'{' | b'}' | b'<' | b'>'));
    let mut out = Vec::new();
    for (at, _) in text.match_indices(".md") {
        let end = at + ".md".len();
        let continued = match bytes.get(end) {
            Some(b'.') => bytes.get(end + 1).is_some_and(u8::is_ascii_alphanumeric),
            Some(&b) => path_byte(b),
            None => false,
        };
        let start = bytes[..at]
            .iter()
            .rposition(|&b| !path_byte(b))
            .map_or(0, |before| before + 1);
        if continued
            || start == at
            || pattern(bytes.get(end))
            || (start > 0 && pattern(bytes.get(start - 1)))
        {
            continue;
        }
        out.push((start, end));
    }
    out
}

/// How a citation names its file.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Form {
    /// `` `<path>` ``.
    Backticked,
    /// `<path>`.
    Bare,
    /// `` canon `<name>` ``, read as `docs/canon/<name>.md`.
    Cli,
}

/// A citation in comment text: the cited file and its quoted headings
/// (none: path-only).
#[derive(Debug)]
struct Citation {
    /// `<source>:<line>`.
    at: String,
    file: String,
    headings: Vec<String>,
    /// The 1-based line of each heading.
    heading_lines: Vec<usize>,
    form: Form,
    /// The first heading opens the next comment line.
    next_line: bool,
}

impl Citation {
    /// The citing source file.
    fn source(&self) -> &str {
        self.at
            .rsplit_once(':')
            .map_or(self.at.as_str(), |(source, _)| source)
    }
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

/// Line `row + 1`'s comment text when it opens with a quote.
fn next_opening<'a>(comments: &[Option<&'a str>], row: usize) -> Option<(usize, &'a str)> {
    comments
        .get(row + 1)
        .copied()
        .flatten()
        .filter(|text| text.starts_with('"'))
        .map(|text| (row + 1, text))
}

/// The quoted headings from `rest` (on comment line `row`, opening with
/// `"`), each with its 1-based line: further ones after `, "`, or on the
/// next comment line after a line-ending `,`. A heading that does not close
/// on its line goes to `broken` (C2).
fn heading_list<'a>(
    path: &str,
    lines: &[&str],
    comments: &[Option<&'a str>],
    mut row: usize,
    mut rest: &'a str,
    broken: &mut Vec<String>,
) -> Vec<(usize, String)> {
    let mut headings = Vec::new();
    loop {
        match quoted(rest) {
            Some(Ok((heading, after))) => {
                headings.push((row + 1, heading.to_owned()));
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
        if let Some(next) = rest.strip_prefix(", ").filter(|next| next.starts_with('"')) {
            rest = next;
        } else if rest.trim_end() == "," {
            match next_opening(comments, row) {
                Some((next_row, next)) => {
                    row = next_row;
                    rest = next;
                }
                None => break,
            }
        } else {
            break;
        }
    }
    headings
}

/// Every citation in the comment text of `text` (file `path`): each
/// documentation path under `roots` with its headings (C1, C2, C7), then
/// the CLI form. A quoted heading that does not close goes to `broken`.
fn citations(path: &str, text: &str, roots: &[&str], broken: &mut Vec<String>) -> Vec<Citation> {
    let lines: Vec<&str> = text.lines().collect();
    let comments = comment_lines(path, text);
    let mut out = Vec::new();
    for (index, comment) in comments.iter().enumerate() {
        let Some(comment) = *comment else {
            continue;
        };
        for (start, end) in md_paths(comment) {
            let file = &comment[start..end];
            if !in_roots(file, roots) {
                continue;
            }
            let backticked = start > 0
                && comment.as_bytes()[start - 1] == b'`'
                && comment[end..].starts_with('`');
            let rest = &comment[end + usize::from(backticked)..];
            let rest = rest.strip_prefix(',').unwrap_or(rest);
            let opening = match rest.strip_prefix(' ') {
                Some(first) if first.starts_with('"') => Some((index, first)),
                _ if rest.trim().is_empty() => next_opening(&comments, index),
                _ => None,
            };
            let (heading_lines, headings) = opening
                .map_or_else(Vec::new, |(row, first)| {
                    heading_list(path, &lines, &comments, row, first, broken)
                })
                .into_iter()
                .unzip();
            out.push(Citation {
                at: format!("{path}:{}", index + 1),
                file: file.to_owned(),
                headings,
                heading_lines,
                form: if backticked {
                    Form::Backticked
                } else {
                    Form::Bare
                },
                next_line: opening.is_some_and(|(row, _)| row != index),
            });
        }
        out.extend(cli_citations(path, &lines, &comments, index, broken));
    }
    out
}

/// The CLI form's opening: `canon` and a backticked canon name.
const CLI_FORM: &str = "canon `";

/// Every CLI-form citation starting on comment line `index`:
/// `` canon `<name>`, "<h>"[, "<h>"] `` (the first heading on this line or,
/// after a trailing `,`, at the start of the next comment line), read as
/// `docs/canon/<name>.md`. A heading may wrap onto the next comment line
/// (joined by one space); one that never closes goes to `broken`.
fn cli_citations(
    path: &str,
    lines: &[&str],
    comments: &[Option<&str>],
    index: usize,
    broken: &mut Vec<String>,
) -> Vec<Citation> {
    let Some(line) = comments[index] else {
        return Vec::new();
    };
    let next_comment = |row: usize| comments.get(row + 1).copied().flatten();
    let mut out = Vec::new();
    let mut search = 0;
    while let Some(found) = line[search..].find(CLI_FORM) {
        let start = search + found + CLI_FORM.len();
        search = start;
        let Some(close) = line[start..].find('`') else {
            continue;
        };
        let name = &line[start..start + close];
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            continue;
        }
        let Some(after) = line[start + close + 1..].strip_prefix(',') else {
            continue;
        };
        let (mut row, mut rest) = match after.strip_prefix(' ') {
            Some(rest) if rest.starts_with('"') => (index, rest),
            _ if after.trim_end().is_empty() => match next_comment(index) {
                Some(next) if next.starts_with('"') => (index + 1, next),
                _ => continue,
            },
            _ => continue,
        };
        let next_line = row != index;
        let mut headings = Vec::new();
        let mut heading_lines = Vec::new();
        loop {
            match quoted(rest) {
                Some(Ok((heading, after))) => {
                    headings.push(heading.to_owned());
                    heading_lines.push(row + 1);
                    rest = after;
                }
                Some(Err(())) => {
                    let joined =
                        next_comment(row).and_then(|next| next.find('"').map(|end| (next, end)));
                    let Some((next, end)) = joined else {
                        broken.push(format!(
                            "{path}:{}: the quoted heading never closes: {}",
                            row + 1,
                            lines[row].trim()
                        ));
                        break;
                    };
                    headings.push(format!("{} {}", rest[1..].trim_end(), &next[..end]));
                    heading_lines.push(row + 1);
                    row += 1;
                    rest = &next[end + 1..];
                }
                None => break,
            }
            if rest.starts_with(", \"") {
                rest = &rest[", ".len()..];
            } else if rest.trim_end() == "," {
                match next_comment(row) {
                    Some(next) if next.starts_with('"') => {
                        row += 1;
                        rest = next;
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }
        out.push(Citation {
            at: format!("{path}:{}", index + 1),
            file: format!("docs/canon/{name}.md"),
            headings,
            heading_lines,
            form: Form::Cli,
            next_line,
        });
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

/// The citations of the whole walk and the wrapped headings found.
struct Scan {
    citations: Vec<Citation>,
    broken: Vec<String>,
}

fn scan_repository() -> Scan {
    let mut broken = Vec::new();
    let mut all = Vec::new();
    for (path, text) in cited_files() {
        all.extend(citations(&path, &text, &DOC_ROOTS, &mut broken));
    }
    Scan {
        citations: all,
        broken,
    }
}

#[test]
fn every_cited_heading_exists_verbatim_on_one_line() {
    let root = repository_root();
    let Scan { citations, broken } = scan_repository();
    let mut headings_by_file: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut missing = Vec::new();
    let mut missed_in = BTreeSet::new();
    for citation in citations
        .iter()
        .filter(|citation| !citation.headings.is_empty())
    {
        // A file that is not there is the path check's finding.
        if !names_a_file(&root, &citation.file) {
            continue;
        }
        let known = headings_by_file
            .entry(citation.file.clone())
            .or_insert_with(|| headings_of(&root.join(&citation.file)));
        for (heading, line) in citation.headings.iter().zip(&citation.heading_lines) {
            if !known.contains(heading) {
                missing.push(format!(
                    "{}:{line}: {heading:?} is not a heading of {}",
                    citation.source(),
                    citation.file
                ));
                missed_in.insert(citation.file.clone());
            }
        }
    }
    let there: BTreeMap<&String, &Vec<String>> = headings_by_file
        .iter()
        .filter(|(file, _)| missed_in.contains(*file))
        .collect();
    assert!(
        broken.is_empty() && missing.is_empty(),
        "{}\nheadings there: {there:#?}",
        broken
            .iter()
            .chain(&missing)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Documentation paths named on purpose though no such file exists (C3):
/// (source file, cited path, why). An entry matching no citation fails.
const DELIBERATE_MISSES: [(&str, &str, &str); 4] = [
    (
        "crates/specengine-core/tests/check_mentions.rs",
        "docs/features/feat.md",
        "the test's `feat/` mentions dangle because this file is absent",
    ),
    (
        "crates/specengine-core/tests/check_scopes.rs",
        "docs/features/c.md",
        "a feature document of the test's own in-memory corpus",
    ),
    (
        "crates/specengine-cli/tests/kind_amendment.rs",
        "docs/features/regen-delay.md",
        "the new spec the test's amendment creates in a temporary repository",
    ),
    (
        "crates/specengine-mcp/tests/mcp_door.rs",
        "docs/features/regen-delay.md",
        "the new spec the test's refused amendment must never create",
    ),
];

#[test]
fn every_documentation_path_names_a_file() {
    let root = repository_root();
    let fixtures = root.join("fixtures");
    let mut corpora: Vec<PathBuf> = fs::read_dir(&fixtures)
        .expect("fixtures/ readable")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.is_dir())
        .collect();
    corpora.sort();
    let Scan { citations, .. } = scan_repository();
    let mut exists: BTreeMap<String, bool> = BTreeMap::new();
    let mut in_fixtures: BTreeMap<String, bool> = BTreeMap::new();
    let mut missing = Vec::new();
    let mut fixture_data = Vec::new();
    let mut allowed = BTreeSet::new();
    for citation in &citations {
        let file = &citation.file;
        if *exists
            .entry(file.clone())
            .or_insert_with(|| names_a_file(&root, file))
        {
            continue;
        }
        if *in_fixtures
            .entry(file.clone())
            .or_insert_with(|| corpora.iter().any(|corpus| names_a_file(corpus, file)))
        {
            fixture_data.push(citation.at.clone());
            continue;
        }
        if let Some(entry) = DELIBERATE_MISSES
            .iter()
            .find(|(source, cited, _)| *source == citation.source() && *cited == file.as_str())
        {
            allowed.insert((entry.0, entry.1));
            continue;
        }
        missing.push(format!("{}: no file {file}", citation.at));
    }
    eprintln!(
        "{} citations; fixture data at {fixture_data:?}; allowlisted {allowed:?}",
        citations.len()
    );
    assert!(missing.is_empty(), "{}", missing.join("\n"));
    let dead: Vec<_> = DELIBERATE_MISSES
        .iter()
        .filter(|(source, cited, _)| !allowed.contains(&(*source, *cited)))
        .collect();
    assert!(
        dead.is_empty(),
        "allowlist entries matching no missing citation: {dead:?}"
    );
}

/// The citation counts against their floors (docs/features/citation-sweep.md
/// AC-08): a reader that stops reading a shape drops below one.
#[test]
fn the_citation_counts_hold_their_floors() {
    let Scan { citations, .. } = scan_repository();
    let headings = |keep: &dyn Fn(&Citation) -> bool| -> usize {
        citations
            .iter()
            .filter(|citation| keep(citation))
            .map(|citation| citation.headings.len())
            .sum()
    };
    let bare_ui = headings(&|c| c.form == Form::Bare && c.at.starts_with("ui/src/"));
    let bare_crates = headings(&|c| c.form == Form::Bare && c.at.starts_with("crates/"));
    let backticked = headings(&|c| c.form == Form::Backticked);
    let next_line = headings(&|c| c.next_line && c.form != Form::Cli);
    let css = headings(&|c| c.source().ends_with(".css"));
    let toml = headings(&|c| c.source().ends_with(".toml"));
    let all_headings = headings(&|_| true);
    let path_only: BTreeSet<&str> = citations
        .iter()
        .filter(|citation| citation.headings.is_empty() && citation.form != Form::Cli)
        .map(|citation| citation.file.as_str())
        .collect();
    // The UI's provisional types cite their sources (`ui/README.md`
    // "Contract seam").
    let provisional = citations
        .iter()
        .filter(|citation| citation.at.starts_with("ui/src/api/provisional.ts:"))
        .count();
    // The CLI's sources hold no `docs/` (AC-17): each canon citation read
    // there is in the CLI form.
    let cli_form = citations
        .iter()
        .filter(|citation| {
            citation.at.starts_with("crates/specengine-cli/src/")
                && citation.form == Form::Cli
                && !citation.headings.is_empty()
        })
        .count();
    let counts = [
        ("quoted headings", all_headings, FLOORS[0]),
        ("bare-form headings in ui/src", bare_ui, FLOORS[1]),
        ("bare-form headings in crates", bare_crates, FLOORS[2]),
        ("backticked headings", backticked, FLOORS[3]),
        ("first headings on the next line", next_line, FLOORS[4]),
        (".css headings", css, FLOORS[5]),
        ("Cargo.toml headings", toml, FLOORS[6]),
        ("distinct path-only files", path_only.len(), FLOORS[7]),
        (
            "ui/src/api/provisional.ts citations",
            provisional,
            FLOORS[8],
        ),
        (
            "CLI-form citations in the CLI's sources",
            cli_form,
            FLOORS[9],
        ),
    ];
    for (name, count, floor) in counts {
        eprintln!("{name}: measured {count}, floor {floor}");
    }
    let low: Vec<String> = counts
        .iter()
        .filter(|(_, count, floor)| count < floor)
        .map(|(name, count, floor)| format!("{name}: {count} < {floor}"))
        .collect();
    assert!(low.is_empty(), "below the floor:\n{}", low.join("\n"));
}

/// The floors of [`the_citation_counts_hold_their_floors`], in its order:
/// the first eight about 90 % of the count measured on the end state of
/// docs/features/citation-sweep.md, rounded down (1235, 172, 253, 699, 71,
/// 4, 9, 122; `.css` at least 3, AC-08), so a comment edit elsewhere does
/// not trip them; the last two the floors this test held before (20, 24).
/// The quoted headings' earlier floor, 23, lies below the first.
const FLOORS: [usize; 10] = [1111, 154, 227, 629, 63, 3, 8, 109, 20, 24];

/// `found` as (`<source>:<line>`, cited file, headings), for the self-tests.
fn shape(found: &[Citation]) -> Vec<(&str, &str, Vec<&str>)> {
    found
        .iter()
        .map(|c| {
            (
                c.at.as_str(),
                c.file.as_str(),
                c.headings.iter().map(String::as_str).collect(),
            )
        })
        .collect()
}

/// docs/features/citation-sweep.md AC-07: every read shape and every
/// negative case, in a `.rs`, a `.css` and a `Cargo.toml` text, under the
/// real roots plus the samples' `a/`. M: code lines read too; `/**` taken
/// as code.
#[test]
fn the_citation_grammar_reads_every_shape_and_nothing_else() {
    let roots: Vec<&str> = DOC_ROOTS.iter().copied().chain(["a/"]).collect();
    let rust = concat!(
        "//! (`a/b.md` \"One\", \"Two `x`\"; `a/c.md`, \"Three\")\n",
        "/// a/d.md \"Four\" and a/e.md, \"Five\"\n",
        "// `a/f.md` \"Six\",\n",
        "// \"Seven\" more\n",
        "/** a/g.md \"Eight\" */\n",
        "/* a/h.md, \"Nine\" */\n",
        "/**\n",
        " * Sources: a/i.md\n",
        " * \"Ten\", \"Eleven\"\n",
        " */\n",
        "*x = r#\"a/y.md \"Deref\"\"#;\n",
        "//! `a/j.md`,\n",
        "//! \"Twelve\"\n",
        "let s = r#\"a/k.md \"Code\"\"#;\n",
        "// docs/spec/a.md \"Fixture\"; `b/c.md` \"Outside\"\n",
        "// a/l.md AC-13 (\"Label\")\n",
        "// a/m.md, as \"Prose\" says\n",
        "// a/n.md \"A\" \"B\"; a/o.md \"C\" and \"D\"\n",
        "// a/p.md#anchor \"Anchor\"\n",
        "// \"Unbound\" (a/q.md)\n",
        "// `a/r.md` \"Wrapped\n",
        "// heading\"\n",
        "// a/s.md\n",
        "\"Not a heading\";\n",
        "// a/t.mdx \"X\", a/u.md.bak \"Y\", a/*.md \"Z\", <a/v.md> \"W\"\n",
        "/*\n",
        "   a/w.md \"Thirteen\"\n",
        "*/ let t = \"a/x.md \\\"Code\\\"\";\n",
    );
    let mut broken = Vec::new();
    let found = citations("t.rs", rust, &roots, &mut broken);
    assert_eq!(
        shape(&found),
        [
            ("t.rs:1", "a/b.md", vec!["One", "Two `x`"]),
            ("t.rs:1", "a/c.md", vec!["Three"]),
            ("t.rs:2", "a/d.md", vec!["Four"]),
            ("t.rs:2", "a/e.md", vec!["Five"]),
            ("t.rs:3", "a/f.md", vec!["Six", "Seven"]),
            ("t.rs:5", "a/g.md", vec!["Eight"]),
            ("t.rs:6", "a/h.md", vec!["Nine"]),
            ("t.rs:8", "a/i.md", vec!["Ten", "Eleven"]),
            ("t.rs:12", "a/j.md", vec!["Twelve"]),
            ("t.rs:16", "a/l.md", vec![]),
            ("t.rs:17", "a/m.md", vec![]),
            ("t.rs:18", "a/n.md", vec!["A"]),
            ("t.rs:18", "a/o.md", vec!["C"]),
            ("t.rs:19", "a/p.md", vec![]),
            ("t.rs:20", "a/q.md", vec![]),
            ("t.rs:21", "a/r.md", vec![]),
            ("t.rs:23", "a/s.md", vec![]),
            ("t.rs:27", "a/w.md", vec!["Thirteen"]),
        ]
    );
    // A miss is named at the heading's own line.
    let lines: Vec<&[usize]> = found.iter().map(|c| c.heading_lines.as_slice()).collect();
    assert_eq!(
        lines[..9],
        [
            &[1, 1][..],
            &[1],
            &[2],
            &[2],
            &[3, 4],
            &[5],
            &[6],
            &[9, 9],
            &[13]
        ]
    );
    let forms: Vec<(Form, bool)> = found.iter().map(|c| (c.form, c.next_line)).collect();
    assert_eq!(forms[0], (Form::Backticked, false));
    assert_eq!(forms[2], (Form::Bare, false));
    assert_eq!(forms[7], (Form::Bare, true));
    assert_eq!(forms[8], (Form::Backticked, true));
    assert_eq!(
        broken,
        ["t.rs:21: the quoted heading wraps: // `a/r.md` \"Wrapped"]
    );

    let css = concat!(
        "* { box-sizing: border-box; }\n",
        "/* Layout (a/b.md, \"One\"):\n",
        "   the grid, a/c.md \"Two\"\n",
        " * a/d.md \"Three\" */\n",
        ".a::after { content: 'a/e.md \"Code\"'; }\n",
        "// a/f.md \"Not CSS\"\n",
        "/* a/g.md \"Four\" */ .b { }\n",
    );
    let found = citations("t.css", css, &roots, &mut broken);
    assert_eq!(
        shape(&found),
        [
            ("t.css:2", "a/b.md", vec!["One"]),
            ("t.css:3", "a/c.md", vec!["Two"]),
            ("t.css:4", "a/d.md", vec!["Three"]),
            ("t.css:7", "a/g.md", vec!["Four"]),
        ]
    );

    let toml = concat!(
        "[package]\n",
        "# the build (a/b.md, \"One\").\n",
        "name = \"a/c.md \\\"Code\\\"\" # a/d.md \"Trailing\"\n",
        "// a/e.md \"Not TOML\"\n",
    );
    let found = citations("Cargo.toml", toml, &roots, &mut broken);
    assert_eq!(shape(&found), [("Cargo.toml:2", "a/b.md", vec!["One"])]);
    assert_eq!(broken.len(), 1, "{broken:?}");
}

/// The roots read this repository's documentation, never a fixture's or a
/// sample's layout.
#[test]
fn the_documentation_roots_are_this_repositorys() {
    for inside in [
        "CLAUDE.md",
        "README.md",
        "docs/README.md",
        "docs/index-canon.md",
        "docs/archive/initial-architecture-spec.md",
        "docs/canon/spec-check.md",
        "docs/decisions/ADR-0022-x.md",
        "docs/features/ui-graph.md",
        "docs/specs/specengine-platform/05-architecture.md",
        "crates/specengine-cli/README.md",
        "ui/README.md",
    ] {
        assert!(in_roots(inside, &DOC_ROOTS), "{inside} is outside");
    }
    for outside in [
        "docs/a.md",
        "docs/INDEX.md",
        "docs/spec/a.md",
        "docs/records/R-12.md",
        "book/glossary.md",
        "a/b.md",
        "b.md",
        "crates/specengine-cli/src/README.md",
        "crates/README.md",
        "ui/src/README.md",
        "fixtures/spec-a/docs/features/stamina-tuning.md",
        "plugin/specengine/skills/feature/SKILL.md",
        "docs/features/../x.md",
        "docs//x.md",
        "./README.md",
    ] {
        assert!(!in_roots(outside, &DOC_ROOTS), "{outside} is inside");
    }
}

/// The CLI form (`` canon `<name>`, "<h>" ``, docs/features/bevy-roles-reads.md
/// review). M: the form left unread; a wrapped heading not joined. One
/// heading, two citations on a line, a list, a list after a trailing `,`,
/// a heading wrapped onto the next comment line, one that never closes;
/// never in code, never a name of other characters, never without a
/// heading.
#[test]
fn the_citation_scanner_reads_the_cli_form() {
    let mut broken = Vec::new();
    let text = concat!(
        "//! (canon `code-roles`, \"Links\"; canon `spec-verify`, \"Rows\", \"States\")\n",
        "/// the queue (canon `tasks`,\n",
        "/// \"Backup\") and (canon `symbol-index`, \"Crates\n",
        "/// and seams\", \"Lookup\"): more\n",
        "// canon `x`, \"Never closes\n",
        "let s = \"canon `decision-record`, \";\n",
        "// canon `Not_A-name`, \"X\"; canon `round`: no heading\n",
    );
    let found = citations("t.rs", text, &DOC_ROOTS, &mut broken);
    assert_eq!(
        shape(&found),
        [
            ("t.rs:1", "docs/canon/code-roles.md", vec!["Links"]),
            (
                "t.rs:1",
                "docs/canon/spec-verify.md",
                vec!["Rows", "States"]
            ),
            ("t.rs:2", "docs/canon/tasks.md", vec!["Backup"]),
            (
                "t.rs:3",
                "docs/canon/symbol-index.md",
                vec!["Crates and seams", "Lookup"]
            ),
            ("t.rs:5", "docs/canon/x.md", vec![]),
        ]
    );
    assert_eq!(broken.len(), 1, "{broken:?}");
    assert!(
        broken[0].starts_with("t.rs:5: the quoted heading never closes"),
        "{broken:?}"
    );
}
