//! docs/features/proposal-apply.md AC-20, the scan half (the run half:
//! AC-01, AC-07, AC-11 loop over both fixtures in `proposal_create.rs` and
//! `proposal_apply.rs`): the new modules — core `patch`, `proposal`; store
//! `queue`, `update`, `worktree`; CLI `apply`, `inbox`, `preflight`,
//! `proposals`, `propose`, `review` — name no prefix or alias of either
//! fixture, no fixture slug, path or file, as core's `tests/check_genre.rs`
//! scans the check sources (string literals outside comments); and the
//! slice adds no dependency ("Roles": no manifest or lock change, no
//! `similar`, no `rusqlite_migration`). M: a hard-coded prefix; `similar`
//! added.
//!
//! docs/features/proposal-kinds.md AC-11, the scan half ("Rules",
//! "Genre"): its new sources, core and CLI `create`, are scanned as the
//! slice's and as docs/features/decision-apply.md's record sources (no
//! prefix, kind, `docs/` or `records/` literal), and no source of the
//! store, the CLI or the MCP server writes the kind `create` as a literal:
//! core's `CREATE_KIND` names it. M: a `"create"` or prefix literal
//! outside core.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;

use common::{FIXTURES, NON_RUST, fixture, read_text, repository_root};
use specengine_core::ProjectConfig;

/// The new sources of the slice, of docs/features/decision-apply.md
/// (core `record`, CLI `decide`), of docs/features/proposal-kinds.md
/// (core and CLI `create`) and of docs/features/task-package.md
/// (docs/canon/task-package.md "Genre": model, core and store tasks, CLI `task` and `package`, the MCP
/// task tools).
const NEW_SOURCES: [&str; 21] = [
    "crates/specengine-core/src/patch.rs",
    "crates/specengine-core/src/proposal.rs",
    "crates/specengine-core/src/record.rs",
    "crates/specengine-store/src/queue.rs",
    "crates/specengine-store/src/update.rs",
    "crates/specengine-store/src/worktree.rs",
    "crates/specengine-cli/src/apply.rs",
    "crates/specengine-cli/src/decide.rs",
    "crates/specengine-cli/src/inbox.rs",
    "crates/specengine-cli/src/preflight.rs",
    "crates/specengine-cli/src/proposals.rs",
    "crates/specengine-cli/src/propose.rs",
    "crates/specengine-cli/src/review.rs",
    "crates/specengine-core/src/create.rs",
    "crates/specengine-cli/src/create.rs",
    "crates/specengine-model/src/task.rs",
    "crates/specengine-core/src/task.rs",
    "crates/specengine-store/src/queue/tasks.rs",
    "crates/specengine-cli/src/task.rs",
    "crates/specengine-cli/src/package.rs",
    "crates/specengine-mcp/src/tasks.rs",
];

/// docs/features/proposal-kinds.md's new sources (AC-11).
const CREATE_SOURCES: [&str; 2] = [
    "crates/specengine-core/src/create.rs",
    "crates/specengine-cli/src/create.rs",
];

/// The crates whose sources take the kind names from core's constants.
const OUTSIDE_CORE: [&str; 3] = [
    "crates/specengine-store/src",
    "crates/specengine-cli/src",
    "crates/specengine-mcp/src",
];

/// docs/features/decision-apply.md's new sources (AC-03).
const RECORD_SOURCES: [&str; 2] = [
    "crates/specengine-core/src/record.rs",
    "crates/specengine-cli/src/decide.rs",
];

/// The document classes of the convention: a kind that is also a class
/// name is no project word (AC-03 "class names aside").
const CLASS_NAMES: [&str; 4] = ["canon", "decision", "spec", "generated"];

/// Paths, files and names of the fixtures and of this repository.
const PROJECT_NAMES: &[&str] = &[
    "fixtures",
    "spec-a",
    "spec-b",
    "docs/",
    "records/",
    "features/",
    "decisions",
    "CLAUDE.md",
    "README.md",
    "index.md",
    "stamina",
    "sprint",
    "glossary",
    "dry-run",
];

/// Every string literal of a Rust line outside comments (naive: the text
/// between pairs of `"`), as core's `check_genre.rs` takes them.
fn literals(line: &str) -> Vec<&str> {
    let code = line.trim_start();
    if code.starts_with("//") {
        return Vec::new();
    }
    line.split('"').skip(1).step_by(2).collect()
}

/// The offending literals of `text` (named `source`).
fn offenders(source: &str, text: &str, prefixes: &BTreeSet<String>, names: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        for literal in literals(line) {
            let names_prefix = prefixes.iter().any(|prefix| {
                literal == prefix
                    || literal.starts_with(&format!("{prefix}-"))
                    || literal.contains(&format!(" {prefix}-"))
                    || literal.contains(&format!("`{prefix}-"))
            });
            let names_project = names.iter().any(|name| literal.contains(name));
            if names_prefix || names_project {
                found.push(format!("{source}:{}: {literal:?}", number + 1));
            }
        }
    }
    found
}

fn fixture_prefixes() -> BTreeSet<String> {
    let mut prefixes = BTreeSet::new();
    for (name, _) in FIXTURES {
        let config = ProjectConfig::from_toml(&read_text(&fixture(name), "specengine.toml"))
            .expect("the fixture's config");
        for spec in config.scheme.prefixes() {
            prefixes.insert(spec.prefix.clone());
            prefixes.extend(spec.aliases_from.iter().cloned());
        }
    }
    prefixes
}

#[test]
fn ac20_the_new_modules_name_no_fixture_prefix_slug_or_path() {
    let prefixes = fixture_prefixes();
    assert!(
        prefixes.contains("RULE") && prefixes.contains("CMD") && prefixes.contains("QST"),
        "{prefixes:?}"
    );
    let mut names: Vec<&str> = PROJECT_NAMES.to_vec();
    names.extend(FIXTURES.iter().map(|(_, slug)| *slug));
    let mut found = Vec::new();
    for source in NEW_SOURCES {
        let text = fs::read_to_string(repository_root().join(source))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        found.extend(offenders(source, &text, &prefixes, &names));
    }
    assert!(
        found.is_empty(),
        "fixture names in the new modules:\n{}",
        found.join("\n")
    );
}

/// The `[ids]` prefixes, aliases and kinds of the fixtures (spec-a, spec-b,
/// the non-Rust spec-c) and of this repository's own `specengine.toml`, the
/// kinds that are class names
/// aside.
fn record_words() -> (BTreeSet<String>, BTreeSet<String>) {
    let mut prefixes = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let mut configs: Vec<String> = FIXTURES
        .iter()
        .map(|(name, _)| read_text(&fixture(name), "specengine.toml"))
        .collect();
    configs.push(read_text(&fixture(NON_RUST.0), "specengine.toml"));
    configs.push(read_text(&repository_root(), "specengine.toml"));
    for text in configs {
        let config = ProjectConfig::from_toml(&text).expect("a config");
        for spec in config.scheme.prefixes() {
            prefixes.insert(spec.prefix.clone());
            prefixes.extend(spec.aliases_from.iter().cloned());
            if !CLASS_NAMES.contains(&spec.kind.as_str()) {
                kinds.insert(spec.kind.clone());
            }
        }
    }
    (prefixes, kinds)
}

/// The literals of `text` (named `source`) that name a prefix (as the
/// scan above), equal a kind, or hold `Cost`, `docs/` or `records/`.
fn record_offenders(
    source: &str,
    text: &str,
    prefixes: &BTreeSet<String>,
    kinds: &BTreeSet<String>,
) -> Vec<String> {
    let mut found = offenders(source, text, prefixes, &["Cost", "docs/", "records/"]);
    for (number, line) in text.lines().enumerate() {
        for literal in literals(line) {
            if kinds.contains(literal) {
                found.push(format!("{source}:{}: {literal:?} (a kind)", number + 1));
            }
        }
    }
    found
}

/// docs/features/decision-apply.md AC-03, the scan half (the run half:
/// `decision_apply.rs`): the record's sources hold no literal naming an
/// `[ids]` prefix of spec-a, spec-b or this repository, none equal to a
/// kind of theirs but a class name, none holding `Cost`, `docs/` or
/// `records/`: a record's prefix, directory and headings come only from
/// the project's table and template. M: `"DEC"`.
#[test]
fn ac03_the_record_sources_name_no_prefix_kind_heading_or_directory() {
    let (prefixes, kinds) = record_words();
    assert!(
        prefixes.contains("DEC") && prefixes.contains("ADR") && prefixes.contains("QST"),
        "{prefixes:?}"
    );
    assert!(
        kinds.contains("question") && kinds.contains("rule") && !kinds.contains("decision"),
        "{kinds:?}"
    );
    let mut found = Vec::new();
    for source in RECORD_SOURCES {
        let text = fs::read_to_string(repository_root().join(source))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(text.len() > 1000, "{source} is read whole");
        found.extend(record_offenders(source, &text, &prefixes, &kinds));
    }
    assert!(
        found.is_empty(),
        "project words in the record sources:\n{}",
        found.join("\n")
    );
    // The scan sees what it must refuse.
    let sample = "let p = \"DEC\";\nlet k = \"question\";\nlet h = \"## Cost\";\n\
                  let d = format!(\"docs/records/{id}.md\");\nlet c = \"decision\";\n";
    let seen = record_offenders("sample.rs", sample, &prefixes, &kinds);
    let lines: Vec<&str> = seen
        .iter()
        .map(|found| found.split(':').nth(1).expect("a line"))
        .collect();
    assert_eq!(lines, ["1", "3", "4", "2"], "{seen:?}");
}

/// docs/features/proposal-kinds.md AC-11, the scan half: core and CLI
/// `create` hold no literal naming an `[ids]` prefix of spec-a, spec-b or
/// this repository, none equal to a kind of theirs, none holding `docs/`
/// or `records/` (the proposer names the path and the IDs, ADR-0008); no
/// `.rs` file of the store, the CLI or the MCP server holds the literal
/// `"create"` (core `CREATE_KIND` is its only spelling). M: a `"create"`
/// or prefix literal outside core.
#[test]
fn proposal_kinds_ac11_the_create_sources_name_no_prefix_kind_or_directory() {
    let (prefixes, kinds) = record_words();
    assert!(
        prefixes.contains("R") && prefixes.contains("REQ") && prefixes.contains("QST"),
        "{prefixes:?}"
    );
    let mut found = Vec::new();
    for source in CREATE_SOURCES {
        let text = fs::read_to_string(repository_root().join(source))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(text.len() > 1000, "{source} is read whole");
        found.extend(record_offenders(source, &text, &prefixes, &kinds));
    }
    assert!(
        found.is_empty(),
        "project words in the create sources:\n{}",
        found.join("\n")
    );
    let kind = specengine_core::intake::CREATE_KIND;
    assert_eq!(kind, "create");
    let mut literal_kinds = Vec::new();
    let mut scanned = 0;
    for dir in OUTSIDE_CORE {
        for (source, text) in rust_sources(&repository_root().join(dir)) {
            scanned += 1;
            for (number, line) in text.lines().enumerate() {
                if literals(line).contains(&kind) {
                    literal_kinds.push(format!("{source}:{}", number + 1));
                }
            }
        }
    }
    assert!(scanned >= 30, "{scanned} sources scanned");
    assert!(
        literal_kinds.is_empty(),
        "the kind `{kind}` written as a literal outside core:\n{}",
        literal_kinds.join("\n")
    );
    // The scan sees what it must refuse.
    let sample = "let k = \"create\";\n// \"create\" in a comment\n";
    assert_eq!(
        literals(sample.lines().next().unwrap()),
        ["create"],
        "the literal is seen"
    );
    assert!(literals(sample.lines().nth(1).unwrap()).is_empty());
}

/// Every `.rs` file under `dir`, recursively, by path: `(path, text)`.
fn rust_sources(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in fs::read_dir(&next).unwrap_or_else(|error| panic!("{next:?}: {error}")) {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text =
                    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
                out.push((path.display().to_string(), text));
            }
        }
    }
    out.sort();
    out
}

/// The scan sees what it must refuse.
#[test]
fn the_scan_sees_a_hard_coded_prefix() {
    let prefixes = fixture_prefixes();
    let names: Vec<&str> = PROJECT_NAMES.to_vec();
    let sample = "let a = \"RULE-STAM-REGEN\";\n// \"CMD-SYNC\" in a comment\nlet b = format!(\"see `QST-031`\");\nlet c = \"ok PR-0001\";\nlet d = \"fixtures/spec-a\";\n";
    let found = offenders("sample.rs", sample, &prefixes, &names);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].starts_with("sample.rs:1:"), "{found:?}");
    assert!(found[1].starts_with("sample.rs:3:"), "{found:?}");
    assert!(found[2].starts_with("sample.rs:5:"), "{found:?}");
}

/// The direct dependency names of `manifest`'s `[dependencies]` and
/// `[dev-dependencies]` tables.
fn dependencies(manifest: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[dependencies]" || line == "[dev-dependencies]";
            continue;
        }
        if inside
            && !line.starts_with('#')
            && let Some((name, _)) = line.split_once('=')
        {
            let name = name.trim();
            out.insert(name.strip_suffix(".workspace").unwrap_or(name).to_owned());
        }
    }
    out
}

/// "Roles": the slice changes no manifest: the four crates keep their
/// direct dependencies, and neither `similar` nor `rusqlite_migration`
/// enters a manifest or the lock.
#[test]
fn ac20_no_dependency_is_added() {
    let root = repository_root();
    let set = |names: &[&str]| {
        names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>()
    };
    for (manifest, want) in [
        (
            "crates/specengine-cli/Cargo.toml",
            set(&[
                "specengine-model",
                "specengine-core",
                "specengine-store",
                "clap",
                "serde",
                "serde_json",
            ]),
        ),
        (
            "crates/specengine-store/Cargo.toml",
            set(&[
                "specengine-model",
                "specengine-core",
                "rusqlite",
                "blake3",
                "serde",
                "serde_json",
            ]),
        ),
        (
            "crates/specengine-core/Cargo.toml",
            set(&[
                "specengine-model",
                "pulldown-cmark",
                "serde-saphyr",
                "serde",
                "toml",
                "serde_json",
                "petgraph",
            ]),
        ),
        (
            "crates/specengine-mcp/Cargo.toml",
            set(&[
                "specengine-cli",
                "rmcp",
                "tokio",
                "getrandom",
                "serde",
                "serde_json",
                "clap",
            ]),
        ),
    ] {
        let text = read_text(&root, manifest);
        assert_eq!(dependencies(&text), want, "{manifest}");
    }
    let mut manifests = vec!["Cargo.toml".to_owned(), "Cargo.lock".to_owned()];
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        let path = entry.unwrap().path().join("Cargo.toml");
        if path.exists() {
            manifests.push(path.strip_prefix(&root).unwrap().display().to_string());
        }
    }
    for manifest in &manifests {
        let text = read_text(&root, manifest);
        for crate_name in ["similar", "rusqlite_migration", "rusqlite-migration"] {
            assert!(
                !text.lines().any(|line| {
                    let line = line.trim();
                    line == format!("name = \"{crate_name}\"")
                        || line.starts_with(&format!("{crate_name} ="))
                        || line.starts_with(&format!("{crate_name}."))
                }),
                "{manifest} names `{crate_name}`"
            );
        }
    }
}

/// docs/canon/task-package.md "Genre" (ADR-0027): the task
/// sources hold no literal naming an `[ids]` prefix of spec-a, spec-b or
/// this repository, none equal to a kind of theirs but a class name, none
/// holding `Cost`, `docs/` or `records/`: a task carries the project's
/// words as data only. M: a kind literal (`"mechanic"`) in the package.
#[test]
fn task_package_the_task_sources_name_no_prefix_kind_or_directory() {
    let (prefixes, kinds) = record_words();
    // 07 s1.2 P2-5: the non-Rust spec-c's words are scanned too.
    assert!(
        prefixes.contains("FEE") && kinds.contains("tariff") && kinds.contains("rider-need"),
        "{prefixes:?} {kinds:?}"
    );
    let sources = [
        "crates/specengine-model/src/task.rs",
        "crates/specengine-core/src/task.rs",
        "crates/specengine-store/src/queue/tasks.rs",
        "crates/specengine-cli/src/task.rs",
        "crates/specengine-cli/src/package.rs",
        "crates/specengine-mcp/src/tasks.rs",
    ];
    let mut found = Vec::new();
    for source in sources {
        let text = fs::read_to_string(repository_root().join(source))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(text.len() > 1000, "{source} is read whole");
        found.extend(record_offenders(source, &text, &prefixes, &kinds));
    }
    assert!(
        found.is_empty(),
        "project words in the task sources:\n{}",
        found.join("\n")
    );
    let sample = "let k = \"mechanic\";\n";
    assert_eq!(
        record_offenders("sample.rs", sample, &prefixes, &kinds).len(),
        1
    );
}

/// docs/features/round.md's sources (AC-04): core's pure half and the
/// CLI's commands.
const ROUND_SOURCES: [&str; 2] = [
    "crates/specengine-core/src/round.rs",
    "crates/specengine-cli/src/round.rs",
];

/// `file:line: word` for every word of `text` starting `block`, in any
/// case, comments included.
fn block_words(source: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        for word in line.split(|c: char| !c.is_alphanumeric()) {
            if word.to_ascii_lowercase().starts_with("block") {
                found.push(format!("{source}:{}: {word}", number + 1));
            }
        }
    }
    found
}

/// docs/features/round.md AC-04, the scan half (the run half:
/// `round.rs`): both round sources hold no literal naming an `[ids]`
/// prefix of spec-a, spec-b, the non-Rust spec-c or this repository, none
/// equal to a kind of theirs but a class name, none holding a fixture's
/// name, slug or path, `Cost`, `docs/` or `records/`; no word starting
/// `block` in any case, comments included (canon `round`, "Sheet"); and
/// the CLI's holds neither `ADR` nor `docs/` anywhere (as `genre.rs`
/// checks every CLI source). M: a kind or prefix literal; a `block` word.
#[test]
fn round_ac04_the_round_sources_name_no_prefix_kind_project_or_block() {
    let (prefixes, kinds) = record_words();
    assert!(
        prefixes.contains("FEE") && prefixes.contains("QST") && prefixes.contains("DEC"),
        "{prefixes:?}"
    );
    assert!(
        kinds.contains("question") && kinds.contains("mechanic"),
        "{kinds:?}"
    );
    let mut names: Vec<&str> = PROJECT_NAMES.to_vec();
    names.extend(FIXTURES.iter().map(|(_, slug)| *slug));
    names.push(NON_RUST.1);
    let mut found = Vec::new();
    for source in ROUND_SOURCES {
        let text = fs::read_to_string(repository_root().join(source))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(text.len() > 1000, "{source} is read whole");
        if source == "crates/specengine-cli/src/round.rs" {
            // docs/features/ui-round.md T-05: the daemon's staging call and
            // its body are in the scanned source.
            for name in [
                "pub fn round_stage_answers",
                "pub struct RoundAnswersBody",
                "pub fn round_limit",
                "fn answers_outcome_text",
            ] {
                assert!(text.contains(name), "{source} holds `{name}`");
            }
        }
        found.extend(record_offenders(source, &text, &prefixes, &kinds));
        found.extend(offenders(source, &text, &prefixes, &names));
        found.extend(block_words(source, &text));
        if source.starts_with("crates/specengine-cli/") {
            for (number, line) in text.lines().enumerate() {
                for word in ["ADR", "docs/"] {
                    if line.contains(word) {
                        found.push(format!("{source}:{}: {word}", number + 1));
                    }
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "project words in the round sources:\n{}",
        found.join("\n")
    );
    // The scan sees what it must refuse.
    let sample = "// Blocked until approved\nlet w = \"unblock\";\nlet k = \"BLOCKERS: \";\n";
    let seen = block_words("sample.rs", sample);
    assert_eq!(
        seen,
        ["sample.rs:1: Blocked", "sample.rs:3: BLOCKERS"],
        "{seen:?}"
    );
    let sample = "let k = \"question\";\nlet p = format!(\"QST-{n}\");\n";
    assert_eq!(
        record_offenders("sample.rs", sample, &prefixes, &kinds).len(),
        2
    );
}

/// docs/features/kind-interpretation.md AC-T2 ("Rules and edge cases",
/// Genre): no `.rs` file of the store, the CLI or the MCP server holds the
/// literal `"interpretation"`: core's `INTERPRETATION_KIND` is its only
/// spelling (the store's column, the MCP enum and the review's text take
/// the constant). The scan sees what it must refuse. M: the literal in
/// `apply.rs`.
#[test]
fn kind_interpretation_ac_t2_no_interpretation_literal_outside_core() {
    let kind = specengine_core::intake::INTERPRETATION_KIND;
    assert_eq!(kind, "interpretation");
    let mut found = Vec::new();
    let mut scanned = 0;
    for dir in OUTSIDE_CORE {
        for (source, text) in rust_sources(&repository_root().join(dir)) {
            scanned += 1;
            for (number, line) in text.lines().enumerate() {
                if literals(line).contains(&kind) {
                    found.push(format!("{source}:{}", number + 1));
                }
            }
        }
    }
    assert!(scanned >= 30, "{scanned} sources scanned");
    assert!(
        found.is_empty(),
        "the kind `{kind}` written as a literal outside core:\n{}",
        found.join("\n")
    );
    // Core spells it once, as its constant.
    let core = fs::read_to_string(repository_root().join("crates/specengine-core/src/intake.rs"))
        .expect("core intake.rs");
    let spelled: Vec<usize> = core
        .lines()
        .enumerate()
        .filter(|(_, line)| literals(line).contains(&kind))
        .map(|(number, _)| number + 1)
        .collect();
    assert_eq!(spelled.len(), 1, "core intake.rs lines {spelled:?}");
    let sample = "    if kind == \"interpretation\" {\n    // \"interpretation\"\n";
    assert_eq!(literals(sample.lines().next().unwrap()), [kind]);
    assert!(literals(sample.lines().nth(1).unwrap()).is_empty());
}

/// The kind comparisons docs/features/kind-decision.md AC-T2 refuses in
/// `line` (comments aside): `ProposalKind::Question` (or `Decision`), by
/// any path (`specengine_store::ProposalKind::Question`), with `==` or
/// `!=` on either side (a deciding kind is judged by `decides()`, the
/// row's shape or an exhaustive match).
fn kind_comparisons(line: &str) -> Vec<String> {
    let code = line.split("//").next().unwrap_or_default();
    let mut found = Vec::new();
    for kind in ["Question", "Decision"] {
        let name = format!("ProposalKind::{kind}");
        let mut from = 0;
        while let Some(at) = code[from..].find(&name) {
            let start = from + at;
            let end = start + name.len();
            from = end;
            // The next character ends the variant (not `QuestionX`).
            if code[end..]
                .chars()
                .next()
                .is_some_and(|next| next.is_alphanumeric() || next == '_')
            {
                continue;
            }
            // Before it: the path's `ident::` segments, then spaces.
            let mut before = code[..start]
                .trim_end_matches(|c: char| c.is_alphanumeric() || c == '_' || c == ':');
            before = before.trim_end();
            let after = code[end..].trim_start();
            if ["==", "!="]
                .iter()
                .any(|op| before.ends_with(op) || after.starts_with(op))
            {
                found.push(format!("{name}: {}", code.trim()));
            }
        }
    }
    found
}

/// docs/features/kind-decision.md AC-T2 ("Rules and edge cases", Kind
/// sites): no `.rs` file of the store, the CLI or the MCP server compares
/// a kind with `ProposalKind::Question` (or `Decision`) by `==` or `!=`;
/// the scan sees what it must refuse. M: `decide.rs:802` left (`let
/// question = proposal.kind == ProposalKind::Question;`).
#[test]
fn kind_decision_ac_t2_no_kind_compared_with_a_question() {
    let mut found = Vec::new();
    let mut scanned = 0;
    for dir in OUTSIDE_CORE {
        for (source, text) in rust_sources(&repository_root().join(dir)) {
            scanned += 1;
            for (number, line) in text.lines().enumerate() {
                for pattern in kind_comparisons(line) {
                    found.push(format!("{source}:{}: {pattern}", number + 1));
                }
            }
        }
    }
    assert!(scanned >= 30, "{scanned} sources scanned");
    assert!(
        found.is_empty(),
        "a kind compared by name, not by `decides()` or its shape:\n{}",
        found.join("\n")
    );
    // The scan sees each form, and not a comment, a match arm or another
    // kind.
    for line in [
        "    let question = proposal.kind == ProposalKind::Question;",
        "    if kind != ProposalKind::Question {",
        "    if ProposalKind::Decision==kind {",
        "        (ProposalKind::Question\n == kind)",
        "    let q = proposal.kind == specengine_store::ProposalKind::Question;",
        "    let q = proposal.kind==crate::ProposalKind::Decision;",
    ] {
        let joined = line.replace('\n', " ");
        assert!(!kind_comparisons(&joined).is_empty(), "{line}");
    }
    for line in [
        "    // kind == ProposalKind::Question",
        "        ProposalKind::Question | ProposalKind::Decision => {",
        "    let discrepancy = new.kind == ProposalKind::Discrepancy;",
        "            kind: ProposalKind::Question,",
        "    let x = kind == ProposalKind::QuestionLike;",
        "    let y = ProposalKind::Questions;",
    ] {
        assert!(kind_comparisons(line).is_empty(), "{line}");
    }
}
