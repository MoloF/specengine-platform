//! AC-12 of docs/features/spec-check.md: budgets are whole-file UTF-8 bytes
//! (BOM and front-matter included) against the cap of the document's slot:
//! canon tier 0 → `tier0_bytes`, tier 1 → `tier1_bytes`, other canon →
//! `canon_bytes` (none when absent), decision → `decision_bytes`, the
//! `[paths] index` file → `index_bytes` whatever its class; others none.
//! Exactly the cap passes, cap + 1 is `budget`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use common::check::{Config, show, with_code};
use specengine_core::check::{CheckFile, CheckInput, Report};

const TOML: &str = "\
[paths]
tier0 = \"CLAUDE.md\"
index = \"docs/index.md\"

[ids]
ADR = { kind = \"decision\", width = 4 }

[budgets]
tier0_bytes    = 400
tier1_bytes    = 300
index_bytes    = 250
decision_bytes = 200
canon_bytes    = 350
";

const BOM: &[u8] = b"\xEF\xBB\xBF";
/// Cyrillic Ya: two UTF-8 bytes, one character.
const YA: char = '\u{044f}';

/// A file of exactly `size` bytes: optional BOM, `front` as front-matter, a
/// heading, then `YA` (two bytes each) and at most one `x`.
fn sized(front: &str, size: usize, bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if bom {
        bytes.extend_from_slice(BOM);
    }
    bytes.extend_from_slice(format!("---\n{front}---\n# T\n\n").as_bytes());
    assert!(bytes.len() + 1 < size, "{front}: {size} is too small");
    let rest = size - bytes.len() - 1;
    let mut body = String::new();
    for _ in 0..rest / 2 {
        body.push(YA);
    }
    if rest % 2 == 1 {
        body.push('x');
    }
    body.push('\n');
    bytes.extend_from_slice(body.as_bytes());
    assert_eq!(bytes.len(), size);
    bytes
}

fn run(config: &Config, path: &str, bytes: Vec<u8>) -> Report {
    let input = CheckInput {
        files: vec![CheckFile::parse(path, bytes, &config.scheme)],
        problems: Vec::new(),
    };
    config.run(&input)
}

/// `(slot, path, front-matter, cap)`.
const SLOTS: &[(&str, &str, &str, usize)] = &[
    (
        "tier0",
        "CLAUDE.md",
        "class: canon\ntier: 0\nowner: o\nreviewed: 2026-09-01\n",
        400,
    ),
    (
        "tier1",
        "docs/README.md",
        "class: canon\ntier: 1\nowner: o\nreviewed: 2026-09-01\n",
        300,
    ),
    (
        "canon",
        "docs/canon/a.md",
        "class: canon\ntier: 2\nowner: o\nreviewed: 2026-09-01\n",
        350,
    ),
    (
        "canon",
        "docs/canon/b.md",
        "class: canon\nowner: o\nreviewed: 2026-09-01\n",
        350,
    ),
    (
        "decision",
        "docs/decisions/ADR-0001.md",
        "id: ADR-0001\nclass: decision\nstatus: rejected\nscope: [x]\n",
        200,
    ),
    ("index", "docs/index.md", "class: generated\n", 250),
    (
        "index",
        "docs/index.md",
        "class: canon\ntier: 1\nowner: o\nreviewed: 2026-09-01\n",
        250,
    ),
];

#[test]
fn exactly_the_cap_passes_and_one_byte_more_is_budget() {
    let config = Config::from_toml(TOML);
    let mut failures = Vec::new();
    for &(slot, path, front, cap) in SLOTS {
        for bom in [false, true] {
            let at_cap = run(&config, path, sized(front, cap, bom));
            if !at_cap.findings.is_empty() {
                failures.push(format!(
                    "{slot} {path} bom={bom} at {cap}:\n{}",
                    show(&at_cap)
                ));
            }
            let over = run(&config, path, sized(front, cap + 1, bom));
            let budget = with_code(&over, "budget");
            let ok = over.findings.len() == 1
                && budget.len() == 1
                && budget[0].subject == slot
                && budget[0].line == 1
                && budget[0].message.contains(&(cap + 1).to_string())
                && budget[0].message.contains(&cap.to_string());
            if !ok {
                failures.push(format!(
                    "{slot} {path} bom={bom} at {}:\n{}",
                    cap + 1,
                    show(&over)
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn bytes_not_characters_and_not_the_body_alone() {
    let config = Config::from_toml(TOML);
    let front = "id: ADR-0001\nclass: decision\nstatus: rejected\nscope: [x]\n";
    let bytes = sized(front, 201, true);
    let text = std::str::from_utf8(&bytes[BOM.len()..]).unwrap();
    assert!(text.chars().count() < 200, "under the cap in characters");
    let body_start = text.find("# T").unwrap();
    assert!(text.len() - body_start < 200, "under the cap in body bytes");
    let report = run(&config, "docs/decisions/ADR-0001.md", bytes);
    assert_eq!(with_code(&report, "budget").len(), 1, "{}", show(&report));
}

#[test]
fn no_cap_for_specs_generated_and_class_less_documents() {
    let config = Config::from_toml(TOML);
    for (path, front) in [
        (
            "docs/features/f.md",
            "class: spec\nstatus: draft\nscope: [x]\n",
        ),
        ("docs/g.md", "class: generated\n"),
        ("docs/decisions/ADR-0002.md", "id: ADR-0002\n"),
    ] {
        let report = run(&config, path, sized(front, 5000, false));
        assert!(
            with_code(&report, "budget").is_empty(),
            "{path}:\n{}",
            show(&report)
        );
    }
    // Without `canon_bytes`, tier-2 canon has no cap.
    let open = Config::from_toml("[ids]\n");
    let report = run(
        &open,
        "docs/canon/a.md",
        sized(
            "class: canon\ntier: 2\nowner: o\nreviewed: 2026-09-01\n",
            50_000,
            false,
        ),
    );
    assert!(report.findings.is_empty(), "{}", show(&report));
}

#[test]
fn default_caps_apply_without_a_budgets_table() {
    let config = Config::from_toml("[ids]\nADR = { kind = \"decision\", width = 4 }\n");
    let front = "id: ADR-0001\nclass: decision\nstatus: rejected\nscope: [x]\n";
    let at = run(&config, "docs/ADR-0001.md", sized(front, 1536, false));
    assert!(at.findings.is_empty(), "{}", show(&at));
    let over = run(&config, "docs/ADR-0001.md", sized(front, 1537, false));
    assert_eq!(with_code(&over, "budget").len(), 1, "{}", show(&over));
}

// ------------------------------------------------- index shards (ADR-0030)
// docs/features/index-shards.md AC-07: `budget` (subject `index`, cap
// `index_bytes`) on `[paths] index` and on each live shard by path,
// whatever its class; the archive shard has no cap.

const SHARDED: &str = "\
[paths]
index = \"docs/index.md\"

[ids]
ADR = { kind = \"decision\", width = 4 }

[budgets]
index_bytes = 250
canon_bytes = 350

[[generators]]
command = \"make index\"
writes  = [\"docs/index.md\", \"docs/idx-arch.md\", \"docs/idx-live.md\", \"notes/idx-two.md\"]
index   = true
shards  = [
  { path = \"docs/idx-arch.md\", tier3 = true },
  { path = \"docs/idx-live.md\", claims = [\"docs/live/**\"] },
  { path = \"notes/idx-two.md\", claims = [\"notes/**\"] },
]
";

/// `(path, front-matter)` of the outputs; the second live shard is
/// `class: canon tier: 2` (its own cap 350 > 250): the slot is by path.
const OUTPUTS: &[(&str, &str)] = &[
    ("docs/index.md", "class: generated\n"),
    ("docs/idx-arch.md", "class: generated\n"),
    ("docs/idx-live.md", "class: generated\n"),
    (
        "notes/idx-two.md",
        "class: canon\ntier: 2\nowner: o\nreviewed: 2026-09-01\n",
    ),
];

fn budgets_of(config: &Config, size: usize) -> Vec<(String, String)> {
    let input = CheckInput {
        files: OUTPUTS
            .iter()
            .map(|(path, front)| CheckFile::parse(*path, sized(front, size, false), &config.scheme))
            .collect(),
        problems: Vec::new(),
    };
    let report = config.run(&input);
    let mut got: Vec<(String, String)> = with_code(&report, "budget")
        .iter()
        .map(|f| (f.path.clone(), f.subject.clone()))
        .collect();
    got.sort();
    got
}

#[test]
fn the_root_and_each_live_shard_are_capped_the_archive_shard_is_not() {
    let config = Config::from_toml(SHARDED);
    assert!(budgets_of(&config, 250).is_empty(), "at the cap");
    let pair = |path: &str| (path.to_owned(), "index".to_owned());
    assert_eq!(
        budgets_of(&config, 251),
        [
            pair("docs/idx-live.md"),
            pair("docs/index.md"),
            pair("notes/idx-two.md"),
        ],
        "cap + 1: the root and the live shards, not the archive"
    );
    // The archive shard far over the cap: still none.
    assert_eq!(budgets_of(&config, 20_000).len(), 3);
}
