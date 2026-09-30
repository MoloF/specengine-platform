//! Iteration 2 of docs/features/spec-cli-check.md: core's `walk_gap`, the
//! one predicate by which the index comparison (§11.5) and
//! `spec export index` stop on an incomplete walk, over inputs built in
//! memory. A gap is a listed file that could not be read, a directory that
//! could not be listed (`""`: the root), a missing root when `[paths]
//! roots` was written; a missing default root and a skipped non-UTF-8 name
//! are none. The first gap is the byte-smallest path; on the same path an
//! unreadable file comes before a missing root, before an unlisted
//! directory; the input's order does not matter.

use specengine_core::check::{CheckFile, CheckInput, Problem, ProblemKind, WalkGap, walk_gap};
use specengine_core::{IdSchemeToml as _, Paths};
use specengine_model::IdScheme;

fn written() -> Paths {
    Paths::from_toml("[paths]\nroots = [\"docs\", \"notes.md\"]\n").expect("written roots")
}

fn read_file(path: &str) -> CheckFile {
    let scheme = IdScheme::from_toml("").expect("an empty scheme");
    CheckFile::parse(path, b"# A\n".to_vec(), &scheme)
}

fn problem(kind: ProblemKind, path: &str) -> Problem {
    Problem {
        kind,
        path: path.to_owned(),
    }
}

fn input(files: Vec<CheckFile>, problems: Vec<Problem>) -> CheckInput {
    CheckInput { files, problems }
}

/// `walk_gap` of `input` and of `input` reversed, which must agree.
fn gap_both_ways(input: &CheckInput, paths: &Paths) -> Option<(String, &'static str)> {
    let describe = |gap: Option<WalkGap<'_>>| {
        gap.map(|gap| {
            let kind = match gap {
                WalkGap::Unreadable { .. } => "unreadable",
                WalkGap::MissingRoot { .. } => "missing-root",
                WalkGap::UnlistedDir { .. } => "unlisted-dir",
            };
            (gap.path().to_owned(), kind)
        })
    };
    let forward = describe(walk_gap(input, paths));
    let mut reversed = input.clone();
    reversed.files.reverse();
    reversed.problems.reverse();
    let backward = describe(walk_gap(&reversed, paths));
    assert_eq!(forward, backward, "the input's order changed the gap");
    forward
}

#[test]
fn a_complete_walk_has_no_gap() {
    let complete = input(
        vec![read_file("docs/a.md"), read_file("docs/b.md")],
        vec![
            problem(ProblemKind::SkippedName, "docs"),
            problem(ProblemKind::MissingRoot, "docs/archive"),
        ],
    );
    // A missing default root is no gap; a missing written root is.
    assert_eq!(gap_both_ways(&complete, &Paths::default()), None);
    assert_eq!(
        gap_both_ways(&complete, &written()),
        Some(("docs/archive".to_owned(), "missing-root"))
    );
    assert_eq!(gap_both_ways(&CheckInput::default(), &written()), None);
}

#[test]
fn each_kind_is_a_gap_with_its_path_and_error() {
    let unreadable = input(
        vec![
            read_file("docs/a.md"),
            CheckFile::unreadable("docs/b.md", "Permission denied (os error 13)"),
        ],
        Vec::new(),
    );
    assert_eq!(
        walk_gap(&unreadable, &Paths::default()),
        Some(WalkGap::Unreadable {
            path: "docs/b.md",
            error: "Permission denied (os error 13)",
        })
    );
    let root = input(Vec::new(), vec![problem(ProblemKind::UnreadableDir, "")]);
    let gap = walk_gap(&root, &Paths::default()).expect("the root");
    assert_eq!(gap, WalkGap::UnlistedDir { path: "" });
    assert_eq!(gap.path(), "");
}

#[test]
fn the_first_gap_is_the_byte_smallest_path() {
    // Byte order, not case-folded or locale order: `Z` (0x5A) < `a`
    // (0x61), ` ` (0x20) < `/` (0x2F), and a multi-byte `\u{e9}` (0xC3 …)
    // after every ASCII letter.
    let gaps = input(
        vec![
            CheckFile::unreadable("docs/a/b.md", "e1"),
            CheckFile::unreadable("docs/\u{e9}.md", "e2"),
            read_file("docs/A.md"),
        ],
        vec![
            problem(ProblemKind::UnreadableDir, "docs/a b"),
            problem(ProblemKind::MissingRoot, "notes.md"),
            problem(ProblemKind::UnreadableDir, "docs/Zeta"),
            problem(ProblemKind::SkippedName, "docs/0"),
        ],
    );
    assert_eq!(
        gap_both_ways(&gaps, &written()),
        Some(("docs/Zeta".to_owned(), "unlisted-dir"))
    );
    let mut without = gaps.clone();
    without.problems.retain(|p| p.path != "docs/Zeta");
    assert_eq!(
        gap_both_ways(&without, &written()),
        Some(("docs/a b".to_owned(), "unlisted-dir"))
    );
    without.problems.retain(|p| p.path != "docs/a b");
    assert_eq!(
        gap_both_ways(&without, &written()),
        Some(("docs/a/b.md".to_owned(), "unreadable"))
    );
    without.files.retain(|f| f.path != "docs/a/b.md");
    assert_eq!(
        gap_both_ways(&without, &written()),
        Some(("docs/\u{e9}.md".to_owned(), "unreadable"))
    );
    without.files.retain(|f| f.path != "docs/\u{e9}.md");
    assert_eq!(
        gap_both_ways(&without, &written()),
        Some(("notes.md".to_owned(), "missing-root"))
    );
    // The root itself comes first of all.
    let mut with_root = gaps;
    with_root
        .problems
        .push(problem(ProblemKind::UnreadableDir, ""));
    assert_eq!(
        gap_both_ways(&with_root, &written()),
        Some((String::new(), "unlisted-dir"))
    );
}

#[test]
fn on_one_path_a_file_then_a_missing_root_then_a_directory() {
    let path = "docs/x.md";
    let mut tied = input(
        vec![CheckFile::unreadable(path, "e")],
        vec![
            problem(ProblemKind::UnreadableDir, path),
            problem(ProblemKind::MissingRoot, path),
        ],
    );
    assert_eq!(
        gap_both_ways(&tied, &written()),
        Some((path.to_owned(), "unreadable"))
    );
    tied.files.clear();
    assert_eq!(
        gap_both_ways(&tied, &written()),
        Some((path.to_owned(), "missing-root"))
    );
    // A missing default root is no gap: the directory is.
    assert_eq!(
        gap_both_ways(&tied, &Paths::default()),
        Some((path.to_owned(), "unlisted-dir"))
    );
}
