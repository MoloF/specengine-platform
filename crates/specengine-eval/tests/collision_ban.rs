//! docs/features/collision-pass.md (RM-0060) "Acceptance criteria", CP7:
//! the ledger is read by nothing but the pass (D5). Every `.rs` file under
//! `crates/*/src` is scanned with its comments blanked (string literals
//! kept: a table named in SQL counts): `collision_marks`, `CollisionMark`
//! (so `CollisionMarks` and `NewCollisionMark` too) and `mark_collisions`
//! appear only in the store's `queue/collisions.rs`, `queue.rs` and
//! `lib.rs`, the CLI's `collisions.rs`, `lib.rs` and `main.rs`, and MCP's
//! `collisions.rs`, `mirror.rs` and `server.rs`. No approve, gate, card,
//! stage, round, trace, health or backup module names them. M:
//! `collision_marks()` read in CLI `decide.rs`.
//!
//! The scan's own self-test: a token in a comment is no hit, in code or in a
//! string it is.

use std::fs;
use std::path::{Path, PathBuf};

/// The ledger's names, CP7 of
/// `docs/features/collision-pass.md` "Acceptance criteria".
const TOKENS: [&str; 3] = ["collision_marks", "CollisionMark", "mark_collisions"];

/// The files that may name them, relative to the repository root.
const ALLOWED: [&str; 9] = [
    "crates/specengine-store/src/queue/collisions.rs",
    "crates/specengine-store/src/queue.rs",
    "crates/specengine-store/src/lib.rs",
    "crates/specengine-cli/src/collisions.rs",
    "crates/specengine-cli/src/lib.rs",
    "crates/specengine-cli/src/main.rs",
    "crates/specengine-mcp/src/collisions.rs",
    "crates/specengine-mcp/src/mirror.rs",
    "crates/specengine-mcp/src/server.rs",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the repository root")
}

/// Every `.rs` file under `dir`, recursively, sorted.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.expect("an entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// `source` with every line and (nested) block comment blanked to spaces,
/// line ends kept; string, raw string, byte string and char literals kept
/// as written (a lifetime is no char literal).
fn without_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out: Vec<char> = chars.clone();
    let blank = |out: &mut Vec<char>, from: usize, to: usize| {
        for slot in out.iter_mut().take(to).skip(from) {
            if *slot != '\n' {
                *slot = ' ';
            }
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            let start = i;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            blank(&mut out, start, i);
        } else if c == '/' && next == Some('*') {
            let start = i;
            let mut depth = 0usize;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            blank(&mut out, start, i);
        } else if c == 'r' && (next == Some('"') || next == Some('#')) && raw_start(&chars, i) {
            // A raw string: r"…", r#"…"#, br#"…"#.
            let mut j = i + 1;
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) != Some(&'"') {
                i += 1;
                continue;
            }
            j += 1;
            loop {
                if j >= chars.len() {
                    break;
                }
                if chars[j] == '"' && (0..hashes).all(|k| chars.get(j + 1 + k) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                j += 1;
            }
            i = j;
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
        } else if c == '\'' {
            // A char literal `'x'`, `'\n'`, `'\u{..}'`; else a lifetime.
            if next == Some('\\') {
                let mut j = i + 2;
                while j < chars.len() && chars[j] != '\'' {
                    j += 1;
                }
                i = j + 1;
            } else if chars.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out.into_iter().collect()
}

/// Whether the `r` at `at` opens a raw string: no identifier character
/// before it, or a `b` with none before that.
fn raw_start(chars: &[char], at: usize) -> bool {
    let word = |index: Option<usize>| {
        index
            .and_then(|index| chars.get(index))
            .is_some_and(|c| c.is_alphanumeric() || *c == '_')
    };
    match at.checked_sub(1) {
        None => true,
        Some(before) if chars[before] == 'b' => !word(before.checked_sub(1)),
        Some(before) => !word(Some(before)),
    }
}

/// `(file, line, token)` of every token in `code`.
fn hits(file: &str, code: &str) -> Vec<(String, usize, &'static str)> {
    let mut found = Vec::new();
    for (number, line) in code.lines().enumerate() {
        for token in TOKENS {
            if line.contains(token) {
                found.push((file.to_owned(), number + 1, token));
            }
        }
    }
    found
}

/// CP7: every hit is in an allowed file; the store's `queue/collisions.rs`
/// and the CLI's `collisions.rs` hold some (the scan sees the pass).
#[test]
fn cp7_only_the_pass_names_the_ledger() {
    let root = repository_root();
    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("crates")).expect("crates") {
        let src = entry.expect("a crate").path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    assert!(files.len() > 100, "the walk found {} files", files.len());
    let mut outside = Vec::new();
    let mut inside = Vec::new();
    for path in &files {
        let relative = path
            .strip_prefix(&root)
            .expect("under the root")
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(path).expect("UTF-8 source");
        let found = hits(&relative, &without_comments(&source));
        if ALLOWED.contains(&relative.as_str()) {
            inside.extend(found);
        } else {
            outside.extend(found);
        }
    }
    assert_eq!(
        outside,
        Vec::<(String, usize, &str)>::new(),
        "the ledger named outside the pass"
    );
    for file in [
        "crates/specengine-store/src/queue/collisions.rs",
        "crates/specengine-cli/src/collisions.rs",
    ] {
        assert!(
            inside.iter().any(|(at, _, _)| at == file),
            "{file} names the ledger: {inside:?}"
        );
    }
}

/// The scan itself: comments blanked, code and strings kept.
#[test]
fn cp7_the_scan_blanks_comments_only() {
    let source = "// collision_marks in a comment\n/* CollisionMark /* nested */ \
                  mark_collisions */\nlet a = 'x'; let b: &'static str = \"collision_marks\";\n\
                  let c = r#\"mark_collisions\"#; queue.mark_collisions();\n";
    let found = hits("x.rs", &without_comments(source));
    assert_eq!(
        found,
        [
            ("x.rs".to_owned(), 3, "collision_marks"),
            ("x.rs".to_owned(), 4, "mark_collisions"),
        ]
    );
    let code = without_comments(source);
    assert!(code.contains("queue.mark_collisions()"));
    assert_eq!(
        code.lines().count(),
        source.lines().count(),
        "line ends kept"
    );
}
