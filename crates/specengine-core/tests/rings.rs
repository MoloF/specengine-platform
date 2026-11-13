//! docs/features/scope-rings.md (RM-0113) "Acceptance criteria", the core
//! half:
//!
//! - SR2: `ring_members` keeps each ring's nodes in the order given, less
//!   those an inner (earlier) ring holds and its own repeats; a node passing
//!   two rings' filters sits in the inner one. M: a node in two rings.
//! - SR4: `ring_note` and `unresolved_root`'s notes.
//! - SR5: core `map_toml.rs` and `rings.rs`, and CLI `rings.rs`, hold no
//!   kind and no state as a literal: the kinds every repository
//!   `specengine.toml` and preset declares, and the trace's five rollup
//!   words, read when the test runs. A planted literal in a probe source is
//!   red.
//!
//! The sources are only read.

mod common;

use std::collections::BTreeSet;

use common::repository_root;
use specengine_core::rings::{ring_members, ring_note, unresolved_root};
use specengine_core::trace::Rollup;

/// SR2: inner rings first, each node once, the order kept.
#[test]
fn sr2_a_node_sits_in_the_first_ring_listing_it() {
    let listed = vec![
        vec!["MEC-SPRINT", "MEC-STAMINA"],
        vec!["RULE-CORE-LOOP", "MEC-STAMINA", "RULE-STAM-REGEN"],
        vec![
            "DOM-GAME",
            "RULE-CORE-LOOP",
            "DOM-MOVEMENT",
            "MEC-SPRINT",
            "EDGE-STAM-ZERO",
        ],
        vec![],
        vec!["EDGE-STAM-ZERO", "A-101", "A-101"],
    ];
    assert_eq!(
        ring_members(listed),
        vec![
            vec!["MEC-SPRINT", "MEC-STAMINA"],
            vec!["RULE-CORE-LOOP", "RULE-STAM-REGEN"],
            vec!["DOM-GAME", "DOM-MOVEMENT", "EDGE-STAM-ZERO"],
            vec![],
            vec!["A-101"],
        ]
    );
    // The widest ring first takes all; the rest are empty.
    let whole = vec!["DOM-GAME", "MEC-SPRINT", "MEC-STAMINA", "RULE-CORE-LOOP"];
    assert_eq!(
        ring_members(vec![
            whole.clone(),
            vec!["MEC-SPRINT"],
            vec!["RULE-CORE-LOOP"]
        ]),
        vec![whole, vec![], vec![]]
    );
    assert_eq!(ring_members::<&str>(Vec::new()), Vec::<Vec<&str>>::new());
}

/// SR4: the note of a ring's root, the resolver's reason under its ring;
/// and of a root located at no node.
#[test]
fn sr4_the_unresolved_roots_note() {
    assert_eq!(
        ring_note(2, "Rules", "`DOM-NONE` resolves to no ID and no alias"),
        "rings: ring 2 `Rules`: `DOM-NONE` resolves to no ID and no alias"
    );
    assert_eq!(
        unresolved_root(1, "Movement", "DOM-NONE"),
        "rings: ring 1 `Movement`: `DOM-NONE` resolves to no node"
    );
    assert_eq!(
        unresolved_root(12, "All", "docs/spec/none.md"),
        "rings: ring 12 `All`: `docs/spec/none.md` resolves to no node"
    );
}

/// The five rollup words, from the trace.
fn states() -> BTreeSet<&'static str> {
    [
        Rollup::NoCard,
        Rollup::Departed,
        Rollup::Suspect,
        Rollup::InWork,
        Rollup::Confirmed,
    ]
    .into_iter()
    .map(Rollup::as_str)
    .collect()
}

/// Every `kind = "…"` of every `specengine.toml` under `root` (its own and
/// each under `fixtures/`) and of `scripts/presets/*.toml`, lowercased.
fn kinds(root: &std::path::Path) -> BTreeSet<String> {
    fn configs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !(name.starts_with('.') || name.starts_with("target") || name == "node_modules")
                {
                    configs(&path, out);
                }
            } else if name == "specengine.toml"
                || (path
                    .parent()
                    .is_some_and(|parent| parent.ends_with("scripts/presets"))
                    && name.ends_with(".toml"))
            {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    configs(&root.join("fixtures"), &mut files);
    configs(&root.join("scripts"), &mut files);
    files.push(root.join("specengine.toml"));
    let mut out = BTreeSet::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for line in text.lines() {
            let mut rest = line;
            while let Some(at) = rest.find("kind") {
                let after = rest[at + 4..].trim_start();
                rest = &rest[at + 4..];
                let Some(value) = after.strip_prefix('=') else {
                    continue;
                };
                let value = value.trim_start();
                if let Some(quoted) = value.strip_prefix('"')
                    && let Some(end) = quoted.find('"')
                {
                    out.insert(quoted[..end].to_lowercase());
                }
            }
        }
    }
    out
}

/// The string literals of the code lines of `source` (comment lines and
/// `//` tails left out), each with its line.
fn literals(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        let mut quoted = false;
        let mut escaped = false;
        let mut current = String::new();
        let mut previous = ' ';
        for letter in line.chars() {
            if quoted {
                match (escaped, letter) {
                    (true, _) => {
                        current.push(letter);
                        escaped = false;
                    }
                    (false, '\\') => escaped = true,
                    (false, '"') => {
                        out.push((number + 1, std::mem::take(&mut current)));
                        quoted = false;
                    }
                    _ => current.push(letter),
                }
            } else if letter == '"' && previous != '\'' {
                quoted = true;
            } else if letter == '/' && previous == '/' {
                break;
            }
            previous = letter;
        }
    }
    out
}

/// `path:line: literal` for each literal of `source` holding a state word
/// or a kind as a word (a run of letters, digits, `_` and `-`, compared
/// without case).
fn hits(path: &str, source: &str, kinds: &BTreeSet<String>) -> Vec<String> {
    let states = states();
    let mut out = Vec::new();
    for (line, literal) in literals(source) {
        let lower = literal.to_lowercase();
        let words: BTreeSet<&str> = lower
            .split(|letter: char| !(letter.is_alphanumeric() || letter == '_' || letter == '-'))
            .filter(|word| !word.is_empty())
            .collect();
        let state = states.iter().any(|state| {
            lower == *state
                || lower.starts_with(&format!("{state} "))
                || lower.ends_with(&format!(" {state}"))
                || lower.contains(&format!(" {state} "))
        });
        let kind = words.iter().any(|word| kinds.contains(*word));
        if state || kind {
            out.push(format!("{path}:{line}: {literal:?}"));
        }
    }
    out
}

/// The sources SR5 names.
const SOURCES: [&str; 3] = [
    "crates/specengine-core/src/map_toml.rs",
    "crates/specengine-core/src/rings.rs",
    "crates/specengine-cli/src/rings.rs",
];

/// SR5: no kind and no state literal in the rings' sources.
#[test]
fn sr5_the_rings_sources_hold_no_kind_or_state_literal() {
    let root = repository_root();
    let kinds = kinds(&root);
    for kind in ["mechanic", "rule", "decision", "scene"] {
        assert!(kinds.contains(kind), "`{kind}` is read: {kinds:?}");
    }
    let mut found = Vec::new();
    let mut read = 0;
    for path in SOURCES {
        let source = std::fs::read_to_string(root.join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        read += literals(&source).len();
        found.extend(hits(path, &source, &kinds));
    }
    assert!(read > 10, "the literals are read: {read}");
    assert!(
        found.is_empty(),
        "a kind or a state is a literal of the rings' code (SR5, ADR-0008):\n{}",
        found.join("\n")
    );
}

/// SR5's positive control: a planted kind and state are caught; a
/// comment, a word holding a kind and a backticked key are not.
#[test]
fn sr5_the_literal_scan_catches_a_kind_and_a_state() {
    let kinds: BTreeSet<String> = ["mechanic", "rule"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let source = "\
// \"mechanic\" in a comment
const WORDS: &str = \"rules and rulers\";
let state = \"in work\";
let pick = kind == \"mechanic\"; // \"rule\"
let key = \"`kinds`: a kind holds a control character\";
let fill = format!(\"{} confirmed\", name);
";
    assert_eq!(
        hits("probe.rs", source, &kinds),
        [
            "probe.rs:3: \"in work\"",
            "probe.rs:4: \"mechanic\"",
            "probe.rs:6: \"{} confirmed\"",
        ]
    );
}
