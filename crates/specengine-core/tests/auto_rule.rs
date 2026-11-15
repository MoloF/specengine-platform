//! docs/features/auto-rule.md, the core half of RL5, RL7 and RL10: the
//! `[auto]` table of `fixtures/auto-mini` read by `AutoRule::from_toml`
//! (docs/canon/auto-mode.md "Rule", "Rule hash"), pure, never failing.
//!
//! - RL5: each `rules/bad-*.toml` gives no rule and exactly one note,
//!   `specengine.toml@HEAD:<line>: <problem>; auto is off`, at the line
//!   `expected.json` pins (the appended table's line, counted in the whole
//!   file); `ProjectConfig` still reads the file.
//! - RL7: `rules/on.toml` and `rules/on-reordered.toml` give the same
//!   canonical bytes, equal to the string pinned here and in
//!   `expected.json`, and the same hash, `b3:` and the BLAKE3 of those
//!   bytes; a `[zones] docs` glob or a new `[ids]` prefix of kind
//!   `scenario` changes it.
//! - RL10: `ProjectConfig::from_toml` takes the file with the table.

mod auto_common;

use auto_common::{
    appended_line, bad_rules, base_config, config_with, edit, expected, rule, with_table,
};
use specengine_core::ProjectConfig;
use specengine_core::auto_toml::{AutoMode, AutoRule, branch_problem, rule_hash};

/// The path the notes name (the reading root's config).
const PATH: &str = "specengine.toml";

/// RL7: `rules/on.toml`'s canonical form, byte for byte (keys in the
/// canon's order, every named map and list in byte order).
const ON_CANONICAL: &str = "{\"format\":1,\"mode\":\"both\",\"zones\":{\"code\":[\"src/**\"],\"docs\":[\"docs/**\"],\"tests\":[\"tests/**\"]},\"exclude_zones\":{\"narrative\":[\"docs/narrative/**\"]},\"exclude_kinds\":{\"scenario\":[\"SC\"]},\"branch\":\"auto\",\"max_tasks\":10,\"max_applies\":20}";

fn read(config: &str) -> AutoRule {
    let read = AutoRule::from_toml(config.as_bytes(), PATH);
    assert!(read.notes.is_empty(), "{:?}", read.notes);
    assert!(!read.absent);
    read.rule.expect("the rule reads")
}

/// RL7. M: hashing the table's text (the reordered file hashes apart);
/// leaving the globs out (the pinned bytes differ).
#[test]
fn rl7_the_hash_is_the_canonical_forms_and_ignores_order_and_comments() {
    assert_eq!(
        expected()["rule_hash_input"]["on.toml"].as_str(),
        Some(ON_CANONICAL),
        "expected.json pins the same bytes"
    );
    let on = read(&config_with("on.toml"));
    let reordered = read(&config_with("on-reordered.toml"));
    assert_eq!(on.canonical, ON_CANONICAL);
    assert_eq!(reordered.canonical, ON_CANONICAL);
    assert_eq!(on.canonical_form().to_text(), ON_CANONICAL);
    let want = format!("b3:{}", blake3::hash(ON_CANONICAL.as_bytes()).to_hex());
    assert_eq!(on.hash, want);
    assert_eq!(reordered.hash, want);
    assert_eq!(rule_hash(ON_CANONICAL), want);
    assert_eq!(want.len(), 3 + 64);
    assert!(
        want[3..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );

    // A zone's glob changes it.
    let globbed = edit(
        &base_config(),
        "docs      = [\"docs/**\"]",
        "docs      = [\"docs/**\", \"notes/**\"]",
    );
    let changed = read(&with_table(&globbed, &rule("on.toml")));
    assert_ne!(changed.hash, want, "a [zones] docs glob");
    assert!(
        changed
            .canonical
            .contains("\"docs\":[\"docs/**\",\"notes/**\"]"),
        "{}",
        changed.canonical
    );

    // A second prefix of an excluded kind changes it.
    let prefixed = edit(
        &base_config(),
        "SC  = { kind = \"scenario\",    width = 3 }\n",
        "SC  = { kind = \"scenario\",    width = 3 }\nSCN = { kind = \"scenario\", width = 2 }\n",
    );
    let changed = read(&with_table(&prefixed, &rule("on.toml")));
    assert_ne!(changed.hash, want, "an [ids] prefix of kind scenario");
    assert!(
        changed
            .canonical
            .contains("\"exclude_kinds\":{\"scenario\":[\"SC\",\"SCN\"]}"),
        "{}",
        changed.canonical
    );

    // A zone nobody lists changes nothing: only the named zones are hashed.
    let unlisted = edit(
        &base_config(),
        "narrative = [\"docs/narrative/**\"]\n",
        "narrative = [\"docs/narrative/**\"]\nart       = [\"art/**\"]\n",
    );
    assert_eq!(read(&with_table(&unlisted, &rule("on.toml"))).hash, want);
}

/// The rule's fields and defaults, `rules/on.toml`.
#[test]
fn the_on_rule_reads_with_its_defaults() {
    let on = read(&config_with("on.toml"));
    assert_eq!(on.mode, AutoMode::Both);
    assert_eq!(on.zone_names(), ["code", "docs", "tests"]);
    assert_eq!(on.excluded_zone_names(), ["narrative"]);
    assert_eq!(on.excluded_kind_names(), ["scenario"]);
    assert_eq!(on.exclude_kinds[0].prefixes, ["SC"]);
    assert_eq!(on.branch, "auto");
    assert_eq!((on.max_tasks, on.max_applies), (10, 20));
    let docs = &on.zones[1];
    assert!(docs.covers("docs/spec/core.md") && docs.covers("docs/narrative/intro.md"));
    assert!(!docs.covers("src/lib.rs"));
    for (name, mode) in [
        ("local.toml", AutoMode::Local),
        ("hub.toml", AutoMode::Hub),
        ("off.toml", AutoMode::Off),
    ] {
        assert_eq!(read(&config_with(name)).mode, mode, "{name}");
    }
}

/// No table: absent, no rule, no note.
#[test]
fn no_table_is_absent_with_no_note() {
    let read = AutoRule::from_toml(base_config().as_bytes(), PATH);
    assert!(read.absent);
    assert!(read.rule.is_none());
    assert!(read.notes.is_empty(), "{:?}", read.notes);
}

/// RL5 (core). Each `rules/bad-*.toml`: no rule, not absent, exactly one
/// note at the pinned line; the file still reads as a project config.
/// M: the reader's error propagated (a note missing, or two).
#[test]
fn rl5_each_bad_rule_is_off_with_one_note_at_its_line() {
    let pinned = expected();
    let pinned = pinned["bad_rules"].as_object().expect("bad_rules");
    let on_disk = bad_rules();
    assert_eq!(
        pinned.keys().cloned().collect::<Vec<_>>(),
        on_disk,
        "expected.json pins every rules/bad-*.toml"
    );
    assert!(on_disk.len() >= 15, "{on_disk:?}");
    let mut failures = Vec::new();
    for name in &on_disk {
        let want = &pinned[name];
        let line = appended_line(want["line"].as_u64().unwrap() as usize);
        let config = config_with(name);
        let read = AutoRule::from_toml(config.as_bytes(), PATH);
        if read.rule.is_some() || read.absent || read.notes.len() != 1 {
            failures.push(format!("{name}: {read:?}"));
            continue;
        }
        let text = read.notes[0].text();
        let head = format!("{PATH}@HEAD:{line}: ");
        let ok = match want["problem"].as_str() {
            // The TOML reader's own words.
            None => text.starts_with(&head) && text.ends_with("; auto is off"),
            // A prefix: the rest is another reader's message.
            Some(prefix) if prefix.ends_with(": ") => {
                text.starts_with(&format!("{head}{prefix}")) && text.ends_with("; auto is off")
            }
            Some(problem) => text == format!("{head}{problem}; auto is off"),
        };
        if !ok {
            failures.push(format!("{name}: {text:?}, want {head}{}", want["problem"]));
        }
        if let Err(error) = ProjectConfig::from_toml(&config) {
            failures.push(format!("{name}: ProjectConfig refuses it: {error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every key is judged whatever `mode` says: an `off` rule with an unknown
/// key is a note too.
#[test]
fn an_off_rule_is_judged_too() {
    let config = with_table(&base_config(), &format!("{}speed = 1\n", rule("off.toml")));
    let read = AutoRule::from_toml(config.as_bytes(), PATH);
    assert!(read.rule.is_none());
    assert_eq!(
        read.notes
            .iter()
            .map(specengine_core::auto_toml::AutoNote::text)
            .collect::<Vec<_>>(),
        [format!(
            "{PATH}@HEAD:{}: unknown key `speed` in `[auto]`; auto is off",
            appended_line(4)
        )]
    );
}

/// The branch name's rule (docs/canon/auto-mode.md "Rule", **Judging**).
#[test]
fn branch_names() {
    for good in ["auto", "auto/night-1", "a.b_c-d", "A1", &"x".repeat(64)] {
        assert_eq!(branch_problem(good), None, "{good}");
    }
    for bad in [
        "",
        "-auto",
        ".auto",
        "/auto",
        "a..b",
        "a//b",
        "auto/",
        "auto.",
        "auto.lock",
        "a b",
        "a~b",
        "a:b",
        "\u{e9}t\u{e9}",
        &"x".repeat(65),
    ] {
        assert_eq!(
            branch_problem(bad),
            Some(format!(
                "`branch` `{bad}` is no branch name (letters, digits, `.`, `_`, `-`, `/`; 1 to 64 bytes)"
            )),
            "{bad:?}"
        );
    }
}

/// RL10 (core): the project config takes `[auto]` (`RawFile` name-checks
/// it). M: `auto` left out of `RawFile` (an unknown key).
#[test]
fn rl10_the_project_config_accepts_the_table() {
    for name in [
        "on.toml",
        "on-reordered.toml",
        "local.toml",
        "hub.toml",
        "off.toml",
    ] {
        let config = config_with(name);
        ProjectConfig::from_toml(&config)
            .unwrap_or_else(|error| panic!("{name}: ProjectConfig refuses [auto]: {error}"));
    }
}
