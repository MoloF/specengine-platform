//! AC-06 of docs/features/spec-cli-switch.md: `check::worst_w` on a crafted
//! corpus whose expected W is computed here from the file sizes alone (the
//! spec's "Worst W": every canon `tier: 0` summed, the largest canon `tier:
//! 1`, the `[paths] index` file, the three largest of the other files that
//! are neither Tier 3 nor `class: generated`, failed front-matter
//! included). The corpus holds a canon tier 0; two canon tier 1 (the
//! smaller one larger than the third pooled file, so the reading "every
//! canon tier 0/1 file stays out of the pool" is pinned: documentation-system
//! §3 pools Tier 2 documents only); an index larger than the third live
//! Tier 2; a shipped spec and a superseded decision larger than any live
//! file; a generated non-index file; the largest live file with failed
//! front-matter; a fourth live file below the three. Each named mutation
//! (Tier 3 counted, tier 1 summed, the index pooled, k = 2, generated
//! counted, the failed file dropped) moves W off the expected value.
//!
//! Also: the paths are no project's (the index is `catalog/list.md`: which
//! file is the index comes from `[paths]` alone); the order of the files
//! does not matter; tier 0 files are summed; an index configured but not
//! walked, or not configured, adds 0; the report carries W on its summary
//! and in `counts.worst_w_bytes`, and `cannot-check` reports 0 (AC-07).

mod common;

use common::check::Config;
use specengine_core::check::{self, CheckFile, CheckInput, Verdict};

const TOML: &str = "\
[paths]
index = \"catalog/list.md\"

[ids]
R = { kind = \"requirement\", width = 2 }
";

/// A file of exactly `size` bytes: `front` between `---` fences (none when
/// empty), a heading, then `x` padding.
fn sized(front: &str, size: usize) -> String {
    let mut text = if front.is_empty() {
        "# Body\n".to_owned()
    } else {
        format!("---\n{front}\n---\n\n# Body\n")
    };
    assert!(text.len() + 1 < size, "{front}: {} bytes", text.len());
    text.push_str(&"x".repeat(size - text.len() - 1));
    text.push('\n');
    assert_eq!(text.len(), size);
    text
}

const TIER0: usize = 1_000;
const TIER1_LARGE: usize = 3_400;
const TIER1_SMALL: usize = 3_300;
const INDEX: usize = 5_000;
const FAILED: usize = 6_000;
const LIVE: [usize; 4] = [4_000, 3_100, 2_500, 1_500];
const SHIPPED: usize = 9_000;
const SUPERSEDED: usize = 8_000;
const GENERATED: usize = 7_000;

// The orders the criterion names, checked when compiling.
const _: () = {
    assert!(
        INDEX > LIVE[1],
        "the index is larger than the third live Tier 2"
    );
    assert!(
        SHIPPED > FAILED && SUPERSEDED > FAILED,
        "Tier 3 beyond any live file"
    );
    assert!(
        GENERATED > FAILED,
        "the generated file beyond any live file"
    );
    assert!(
        TIER1_SMALL > LIVE[1],
        "the smaller Tier 1 would enter a pool of three"
    );
    assert!(TIER1_LARGE > TIER1_SMALL, "the larger Tier 1 counts");
    assert!(FAILED > LIVE[0], "the failed file is the largest live file");
};

/// The crafted corpus, `(path, text)`.
fn corpus() -> Vec<(String, String)> {
    let canon = |tier: u8| {
        format!("class: canon\ntier: {tier}\nscope: [x]\nowner: o\nreviewed: 2026-09-29")
    };
    vec![
        ("guide.md".to_owned(), sized(&canon(0), TIER0)),
        ("area-a/README.md".to_owned(), sized(&canon(1), TIER1_LARGE)),
        ("area-b/README.md".to_owned(), sized(&canon(1), TIER1_SMALL)),
        (
            "catalog/list.md".to_owned(),
            sized("class: generated\ngenerator: make list\nsource: all", INDEX),
        ),
        // Would be Tier 3 if read: the YAML fails (`: ` in a plain scalar).
        (
            "notes/broken.md".to_owned(),
            sized("class: spec\nstatus: shipped\ntitle: a: b", FAILED),
        ),
        ("area-a/rules.md".to_owned(), sized(&canon(2), LIVE[0])),
        (
            "work/draft.md".to_owned(),
            sized("class: spec\nstatus: draft\nscope: [x]", LIVE[1]),
        ),
        (
            "records/R-01.md".to_owned(),
            sized("class: decision\nid: R-01\nstatus: accepted", LIVE[2]),
        ),
        (
            "work/doing.md".to_owned(),
            sized("class: spec\nstatus: in-progress\nscope: [x]", LIVE[3]),
        ),
        (
            "work/done.md".to_owned(),
            sized("class: spec\nstatus: shipped\nshipped: 2026-09-01", SHIPPED),
        ),
        (
            "records/R-02.md".to_owned(),
            sized(
                "class: decision\nid: R-02\nstatus: superseded-by R-01",
                SUPERSEDED,
            ),
        ),
        (
            "gen/out.md".to_owned(),
            sized(
                "class: generated\ngenerator: make out\nsource: all",
                GENERATED,
            ),
        ),
    ]
}

fn input_of(config: &Config, files: &[(String, String)]) -> CheckInput {
    let pairs: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    config.input(&pairs)
}

/// W of the corpus, from the sizes: Tier 0 + the largest Tier 1 + the
/// index + the three largest of failed and live.
const EXPECTED: u64 = (TIER0 + TIER1_LARGE + INDEX + FAILED + LIVE[0] + LIVE[1]) as u64;

#[test]
fn w_of_the_crafted_corpus_is_the_one_computed_from_its_sizes() {
    let config = Config::from_toml(TOML);
    let files = corpus();
    let mut input = input_of(&config, &files);

    // The corpus is what it claims: the broken file failed, the Tier 3
    // files are Tier 3, nothing else is.
    let broken = input
        .files
        .iter()
        .find(|file| file.path == "notes/broken.md")
        .unwrap();
    let parsed = broken.parsed.as_ref().unwrap();
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.code.as_str().starts_with("frontmatter-")),
        "{:?}",
        parsed.diagnostics
    );
    let tier3: Vec<&str> = input
        .files
        .iter()
        .filter(|file| file.parsed.as_ref().is_some_and(check::is_tier3_file))
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(tier3, ["work/done.md", "records/R-02.md"]);
    assert_eq!(check::worst_w(&input, &config.paths), EXPECTED);
    // Each named mutation lands elsewhere (these are the values it would give).
    let tier3_counted = (TIER0 + TIER1_LARGE + INDEX + SHIPPED + SUPERSEDED + FAILED) as u64;
    let tier1_summed = EXPECTED + TIER1_SMALL as u64;
    let index_pooled = (TIER0 + TIER1_LARGE + FAILED + INDEX + LIVE[0]) as u64;
    let k2 = EXPECTED - LIVE[1] as u64;
    let generated_counted = (TIER0 + TIER1_LARGE + INDEX + GENERATED + FAILED + LIVE[0]) as u64;
    let failed_dropped = (TIER0 + TIER1_LARGE + INDEX + LIVE[0] + LIVE[1] + LIVE[2]) as u64;
    let tier1_pooled = (TIER0 + TIER1_LARGE + INDEX + FAILED + LIVE[0] + TIER1_SMALL) as u64;
    for (mutation, value) in [
        ("Tier 3 counted", tier3_counted),
        ("tier 1 summed", tier1_summed),
        ("the index pooled", index_pooled),
        ("k = 2", k2),
        ("generated counted", generated_counted),
        ("the failed file dropped", failed_dropped),
        ("the smaller tier 1 pooled", tier1_pooled),
    ] {
        assert_ne!(value, EXPECTED, "{mutation} would not be seen");
    }

    // Independent of the order of the files.
    input.files.reverse();
    assert_eq!(check::worst_w(&input, &config.paths), EXPECTED);

    // The report: the summary's W and `counts.worst_w_bytes`.
    let report = config.run(&input);
    assert_eq!(report.counts.worst_w_bytes, EXPECTED);
    let summary = report.lines(false);
    let last = summary.last().unwrap();
    assert!(
        last.contains(&format!(", 0 stale, worst W {EXPECTED} B \u{2014} ")),
        "{last}"
    );
    let json: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(json["counts"]["worst_w_bytes"], EXPECTED);
}

#[test]
fn tier_0_files_are_summed_and_a_missing_index_adds_nothing() {
    let config = Config::from_toml(TOML);
    let mut files = corpus();
    files.push((
        "guide-2.md".to_owned(),
        sized(
            "class: canon\ntier: 0\nscope: [x]\nowner: o\nreviewed: 2026-09-29",
            700,
        ),
    ));
    let input = input_of(&config, &files);
    assert_eq!(check::worst_w(&input, &config.paths), EXPECTED + 700);

    // The index configured but not walked: 0 for the index, nothing else moves.
    let without: Vec<(String, String)> = corpus()
        .into_iter()
        .filter(|(path, _)| path != "catalog/list.md")
        .collect();
    let input = input_of(&config, &without);
    assert_eq!(
        check::worst_w(&input, &config.paths),
        EXPECTED - INDEX as u64
    );

    // No `[paths] index`: the file is one more `class: generated`, left out.
    let unindexed = Config::from_toml("[ids]\nR = { kind = \"requirement\", width = 2 }\n");
    let input = input_of(&unindexed, &corpus());
    assert_eq!(
        check::worst_w(&input, &unindexed.paths),
        EXPECTED - INDEX as u64
    );

    // The same file named `docs/index.md` is not the index here: it is a
    // generated file like any other (no path is known to the core).
    let renamed: Vec<(String, String)> = corpus()
        .into_iter()
        .map(|(path, text)| {
            if path == "catalog/list.md" {
                ("docs/index.md".to_owned(), text)
            } else {
                (path, text)
            }
        })
        .collect();
    let input = input_of(&config, &renamed);
    assert_eq!(
        check::worst_w(&input, &config.paths),
        EXPECTED - INDEX as u64
    );
}

#[test]
fn cannot_check_reports_w_as_zero() {
    let config = Config::from_toml(TOML);
    let mut input = input_of(&config, &corpus());
    assert_eq!(check::worst_w(&input, &config.paths), EXPECTED);
    input.files.push(CheckFile {
        path: "work/unread.md".to_owned(),
        size: 0,
        parsed: None,
        read_error: Some("Permission denied (os error 13)".to_owned()),
        bytes: Vec::new(),
    });
    let report = config.run(&input);
    assert_eq!(
        report.verdict,
        Verdict::CannotCheck,
        "{:?}",
        report.cannot_check
    );
    assert_eq!(report.counts.worst_w_bytes, 0);
    let summary = report.lines(false);
    assert!(
        summary
            .last()
            .unwrap()
            .ends_with(", worst W 0 B \u{2014} cannot-check"),
        "{summary:#?}"
    );
}
