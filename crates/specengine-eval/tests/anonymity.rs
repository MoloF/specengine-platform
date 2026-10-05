//! AC-13 of docs/features/phase-0-spikes.md: `docs/`, `crates/` and
//! `fixtures/` carry no absolute paths, no pilot names and no non-English text.
//! AC-18 of docs/features/ui-shell.md: `ui/` is scanned the same way, except
//! `ui/dist` (build output) and every `node_modules` (installed packages);
//! `ui_tree_is_scanned_but_its_build_output_and_packages_are_not` pins that
//! on a scratch tree (mutations: `ui` unscanned, `dist` unskipped).
//! The ID prefixes of a runtime census config (08 §4.3;
//! `crates/specengine-import/README.md`) are not grepped here: such a
//! config may name single letters or prefixes this repository uses itself,
//! so a repository-wide grep cannot tell a leak from a coincidence.
//! `census_cli.rs` checks instead that the census stdout carries no corpus
//! string at all (fixture and `#[ignore]` pilot runs) and that the label
//! mapping lands only under `--out`.

use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// The scanned top-level areas of the repository.
const AREAS: [&str; 4] = ["docs", "crates", "fixtures", "ui"];

/// Directories under the scanned areas that hold generated or installed text
/// (repository-relative, matched by whole path components): the UI's build
/// output. `node_modules` is skipped wherever it is, see [`walk`].
const SKIPPED: [&str; 1] = ["ui/dist"];

/// Every UTF-8 text file under the [`AREAS`] of `root`, skipping build
/// directories, installed packages and version-control internals.
fn text_files_under(root: &Path) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    for area in AREAS {
        let dir = root.join(area);
        if dir.is_dir() {
            walk(root, &dir, &mut files);
        }
    }
    files
}

fn text_files() -> Vec<(PathBuf, String)> {
    let files = text_files_under(&repository_root());
    assert!(
        !files.is_empty(),
        "no text files found under the scanned areas"
    );
    files
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(dir).expect("readable directory") {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry.file_type().expect("file type");
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if name.starts_with('.') || name.starts_with("target") || name == "node_modules" {
                continue;
            }
            let relative = path.strip_prefix(root).unwrap_or(&path);
            if SKIPPED.iter().any(|skipped| relative == Path::new(skipped)) {
                continue;
            }
            walk(root, &path, out);
        } else if let Ok(text) = fs::read_to_string(&path) {
            out.push((path, text));
        }
    }
}

fn offending_lines(pattern: &str, needle: impl Fn(&str) -> bool) -> Vec<String> {
    offending_lines_outside(&[], pattern, needle)
}

/// Like [`offending_lines`], skipping files under the `exempt` directories
/// (repository-relative, matched by whole path components).
fn offending_lines_outside(
    exempt: &[&str],
    pattern: &str,
    needle: impl Fn(&str) -> bool,
) -> Vec<String> {
    let root = repository_root();
    lines_matching(&root, text_files(), exempt, pattern, needle)
}

/// `path:line: pattern` for every line of `files` (under `root`) that
/// `needle` accepts, files under `exempt` skipped.
fn lines_matching(
    root: &Path,
    files: Vec<(PathBuf, String)>,
    exempt: &[&str],
    pattern: &str,
    needle: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut hits = Vec::new();
    for (path, text) in files {
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if exempt.iter().any(|dir| relative.starts_with(dir)) {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if needle(line) {
                hits.push(format!("{}:{}: {pattern}", relative.display(), index + 1));
            }
        }
    }
    hits.sort();
    hits
}

#[test]
fn no_absolute_user_paths_in_docs_crates_or_fixtures() {
    // Assembled at run time so this file does not contain the patterns itself.
    for root in ["Users", "home", "Volumes", "private/tmp", "var/folders"] {
        let prefix = format!("/{root}/");
        let hits = offending_lines(&prefix, |line| line.contains(&prefix));
        assert!(
            hits.is_empty(),
            "absolute paths leaked:\n{}",
            hits.join("\n")
        );
    }
}

#[test]
fn no_pilot_names_in_docs_crates_or_fixtures() {
    let mut checked = 0;
    for variable in ["SPECENGINE_PILOT_A", "SPECENGINE_PILOT_B"] {
        let Some(value) = std::env::var_os(variable).filter(|v| !v.is_empty()) else {
            continue;
        };
        let path = fs::canonicalize(&value).unwrap_or_else(|_| PathBuf::from(&value));
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        checked += 1;
        let hits = offending_lines(&format!("<{variable} name>"), |line| line.contains(&name));
        assert!(
            hits.is_empty(),
            "the corpus name of {variable} leaked into the repository:\n{}",
            hits.join("\n")
        );
        let full = path.to_string_lossy().into_owned();
        let hits = offending_lines(&format!("<{variable} path>"), |line| line.contains(&full));
        assert!(
            hits.is_empty(),
            "the corpus path of {variable} leaked into the repository:\n{}",
            hits.join("\n")
        );
    }
    if checked == 0 {
        eprintln!("no SPECENGINE_PILOT_A/_B set: pilot-name check skipped, path check ran");
    }
}

/// The only places allowed to hold raw Cyrillic: self-written Russian test
/// text. Working answer to Q5 of `crates/specengine-core/README.md` ("Open
/// owner questions"): these two fixture directories are exempt from the
/// ADR-0024 check, everything else (including `docs/`) stays scanned.
/// If the owner answers "no", this list becomes empty and that text turns
/// into escapes generated at test time.
const RUSSIAN_TEST_TEXT: [&str; 2] = ["fixtures/spec-b", "fixtures/token-calibration"];

fn has_cyrillic(line: &str) -> bool {
    line.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

#[test]
fn all_text_is_english_no_cyrillic_letters() {
    // Mixed-script test data elsewhere is explicit escapes, never raw letters.
    let hits = offending_lines_outside(&RUSSIAN_TEST_TEXT, "cyrillic", has_cyrillic);
    assert!(
        hits.is_empty(),
        "non-English text (ADR-0024):\n{}",
        hits.join("\n")
    );
}

/// The exemption is exactly the two directories: a sibling whose name only
/// starts with an exempt one is still scanned, and so is everything else.
#[test]
fn cyrillic_exemption_is_exactly_the_two_russian_fixture_directories() {
    let root = repository_root();
    for dir in RUSSIAN_TEST_TEXT {
        assert!(root.join(dir).is_dir(), "{dir} exists");
    }
    let exempt = |relative: &str| {
        RUSSIAN_TEST_TEXT
            .iter()
            .any(|dir| Path::new(relative).starts_with(dir))
    };
    assert!(exempt("fixtures/spec-b/specengine.toml"));
    assert!(exempt("fixtures/token-calibration/samples/a.md"));
    for scanned in [
        "fixtures/spec-b2/x.md",
        "fixtures/spec-a/x.md",
        "fixtures/token-calibration-extra/x.md",
        "fixtures/x/fixtures/spec-b/x.md",
        "docs/features/spec-parser.md",
        "crates/specengine-core/tests/references.rs",
    ] {
        assert!(!exempt(scanned), "{scanned} is scanned");
    }
    // The exempt directories really are where the Russian text is: without
    // the exemption the check finds lines, and only in them.
    let unexempted = offending_lines("cyrillic", has_cyrillic);
    assert!(
        RUSSIAN_TEST_TEXT.iter().all(|dir| unexempted
            .iter()
            .any(|hit| hit.starts_with(&format!("{dir}/")))),
        "each exempt directory holds Russian text: {unexempted:?}"
    );
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-anonymity-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("parent directory");
        fs::write(&path, text).expect("scratch file written");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// AC-18 of docs/features/ui-shell.md on a scratch tree: a raw Cyrillic
/// letter in `ui/src` is found; the same letter in `ui/dist` or in a
/// `node_modules` is not; `dist` elsewhere (a crate, `ui/src/dist`) is still
/// scanned. The letter is assembled at run time.
#[test]
fn ui_tree_is_scanned_but_its_build_output_and_packages_are_not() {
    let letter = char::from_u32(0x0416).expect("a Cyrillic letter");
    let line = format!("const label = \"{letter}\";\n");
    let tree = Scratch::new("ui-tree");
    for relative in [
        "ui/src/mocks/harbor-sim/fixtures.ts",
        "ui/src/dist/x.ts",
        "ui/index.html",
        "ui/dist/x.js",
        "ui/dist/assets/index.js",
        "ui/node_modules/react/index.js",
        "ui/node_modules/.pnpm/react@19.3.0/node_modules/react/index.js",
        "ui/src/node_modules/x.js",
        "crates/x/dist/y.rs",
        "docs/x/dist/y.md",
    ] {
        tree.write(relative, &line);
    }
    tree.write("docs/english.md", "plain English\n");
    let hits = lines_matching(
        &tree.0,
        text_files_under(&tree.0),
        &RUSSIAN_TEST_TEXT,
        "cyrillic",
        has_cyrillic,
    );
    assert_eq!(
        hits,
        [
            "crates/x/dist/y.rs:1: cyrillic",
            "docs/x/dist/y.md:1: cyrillic",
            "ui/index.html:1: cyrillic",
            "ui/src/dist/x.ts:1: cyrillic",
            "ui/src/mocks/harbor-sim/fixtures.ts:1: cyrillic",
        ]
    );
}

/// The real walk reads the UI sources and never its build output or its
/// installed packages, whether or not they exist right now.
#[test]
fn the_repository_walk_reads_ui_sources_and_skips_dist_and_node_modules() {
    let root = repository_root();
    let relatives: Vec<PathBuf> = text_files()
        .into_iter()
        .map(|(path, _)| path.strip_prefix(&root).unwrap_or(&path).to_path_buf())
        .collect();
    for expected in [
        "ui/package.json",
        "ui/src/main.tsx",
        "ui/src/styles/tokens.css",
    ] {
        assert!(
            relatives.iter().any(|path| path == Path::new(expected)),
            "{expected} is not scanned"
        );
    }
    let leaked: Vec<&PathBuf> = relatives
        .iter()
        .filter(|path| {
            path.starts_with("ui/dist")
                || path
                    .components()
                    .any(|component| component.as_os_str() == "node_modules")
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "scanned build output or packages: {leaked:?}"
    );
}
