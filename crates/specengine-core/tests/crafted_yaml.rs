//! AC-07 of docs/features/spec-parser.md: crafted YAML cannot take the
//! process down. Nesting up to `MAX_DEPTH` (the root mapping counts as 1)
//! of flow and block mappings, sequences, their mixes and mappings used as
//! keys parses; one level more, an alias bomb (billion laughs) and 100 000
//! nested openers each end as exactly one `frontmatter-yaml` within 1 s.
//! Every parse runs on a worker thread with the default 2 MiB stack in the
//! test (debug) profile, so a stack too small for the cap, or a missing
//! budget, shows as a crashed process or a timeout (red), never as a hang
//! of the suite. No depth in this file is a literal: all derive from
//! `specengine_core::MAX_DEPTH`.

mod common;

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use specengine_core::MAX_DEPTH;
use specengine_model::{DiagnosticCode, FmValue, IdScheme, ParsedFile, PrefixSpec};

const LIMIT: Duration = Duration::from_secs(1);

fn x_scheme() -> IdScheme {
    IdScheme::new(vec![PrefixSpec::number("X", "x", 1)]).unwrap()
}

/// Parses on a worker thread; panics when the result is not back within 1 s.
fn parse_within_limit(name: &str, text: String) -> (ParsedFile, Duration) {
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    thread::Builder::new()
        .name(name.to_owned())
        .stack_size(2 << 20)
        .spawn(move || {
            let parsed = specengine_core::parse("crafted.md", text.as_bytes(), &x_scheme());
            let _ = sender.send(parsed);
        })
        .expect("worker thread");
    match receiver.recv_timeout(LIMIT) {
        Ok(parsed) => (parsed, started.elapsed()),
        Err(error) => panic!("{name}: no result within {LIMIT:?} ({error})"),
    }
}

fn assert_one_yaml_error(name: &str, parsed: &ParsedFile, elapsed: Duration) {
    let codes: Vec<DiagnosticCode> = parsed.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(
        codes,
        [DiagnosticCode::FrontmatterYaml],
        "{name}: {:?}",
        parsed.diagnostics
    );
    assert!(elapsed <= LIMIT, "{name}: {elapsed:?}");
    let document = parsed.document().expect("the body is still read");
    assert_eq!(document.id, None, "{name}: no guessed ID");
    // The body after the block is still parsed.
    assert_eq!(
        parsed
            .sections()
            .iter()
            .map(|s| s.id.as_deref())
            .collect::<Vec<_>>(),
        [Some("X-2")],
        "{name}"
    );
}

fn wrap(yaml: &str) -> String {
    format!("---\nid: X-1\n{yaml}\n---\n\n## After {{#X-2}}\n")
}

/// Nine levels of ten aliases each: 10^9 nodes when expanded.
fn alias_bomb() -> String {
    let mut yaml = String::from(
        "l0: &l0 [\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\"]\n",
    );
    for level in 1..=9 {
        let previous = format!("*l{}", level - 1);
        let items = vec![previous; 10].join(",");
        yaml.push_str(&format!("l{level}: &l{level} [{items}]\n"));
    }
    yaml.push_str("bomb: *l9");
    wrap(&yaml)
}

#[test]
fn alias_bomb_is_one_yaml_error_within_a_second() {
    let (parsed, elapsed) = parse_within_limit("alias-bomb", alias_bomb());
    assert_one_yaml_error("alias-bomb", &parsed, elapsed);
}

/// Levels `l0`..`l{top}` of ten aliases each, then `uses` references of the
/// top level: the replayed events grow tenfold per level.
fn alias_ladder(top: usize, uses: usize) -> String {
    let mut yaml = String::from("x_l0: &l0 [a,a,a,a,a,a,a,a,a,a]\n");
    for level in 1..=top {
        let items = vec![format!("*l{}", level - 1); 10].join(",");
        yaml.push_str(&format!("x_l{level}: &l{level} [{items}]\n"));
    }
    for n in 0..uses {
        yaml.push_str(&format!("x_use{n}: *l{top}\n"));
    }
    wrap(yaml.trim_end())
}

/// The spec's boundary: alias expansion over 10 000 nodes is
/// `frontmatter-yaml` (`MAX_ALIAS_EXPANSION`), well below the library's own
/// default budgets (250 000 nodes, 1 000 000 events, the alias/anchor ratio
/// from 100 aliases), so only the parser's budget can stop it.
#[test]
fn alias_expansion_over_ten_thousand_is_an_error_and_under_is_not() {
    // l3 replays l2 ten times: about 11 000 events, 31 aliases.
    let (parsed, elapsed) = parse_within_limit("alias-11k", alias_ladder(3, 0));
    assert_one_yaml_error("alias-11k", &parsed, elapsed);
    // l2 used three times: about 4 500 events in all, 23 aliases.
    let (parsed, _) = parse_within_limit("alias-4k", alias_ladder(2, 3));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|d| d.code == DiagnosticCode::UnknownKey),
        "an expansion under the budget parses: {:?}",
        parsed.diagnostics
    );
    assert_eq!(parsed.document().unwrap().id.as_deref(), Some("X-1"));
}

#[test]
fn small_alias_use_within_budget_is_fine() {
    let text = "---\nid: X-1\nx_base: &b {a: 1}\nraised_by: *b\n---\n\n## After {#X-2}\n";
    let (parsed, _) = parse_within_limit("small-alias", text.to_owned());
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>(),
        [DiagnosticCode::UnknownKey],
        "only `x_base` is untyped"
    );
    let document = parsed.document().unwrap();
    assert_eq!(document.id.as_deref(), Some("X-1"));
    let raised_by = document.fields.as_ref().unwrap().raised_by.as_ref();
    assert_eq!(raised_by.map(|m| m.len()), Some(1), "the alias is expanded");
}

#[test]
fn hundred_thousand_nested_flow_sequences_are_one_yaml_error_within_a_second() {
    let yaml = format!("deep: {}", "[".repeat(100_000));
    let (parsed, elapsed) = parse_within_limit("nested-[", wrap(&yaml));
    assert_one_yaml_error("nested-[", &parsed, elapsed);
}

#[test]
fn hundred_thousand_nested_flow_mappings_are_one_yaml_error_within_a_second() {
    let yaml = format!("deep: {}", "{a: ".repeat(100_000));
    let (parsed, elapsed) = parse_within_limit("nested-{", wrap(&yaml));
    assert_one_yaml_error("nested-{", &parsed, elapsed);
}

#[test]
fn deep_block_nesting_is_one_yaml_error_within_a_second() {
    // 100 000 nested block sequences on one line: `- - - … x`.
    let yaml = format!("deep:\n  {}x", "- ".repeat(100_000));
    let (parsed, elapsed) = parse_within_limit("nested-block", wrap(&yaml));
    assert_one_yaml_error("nested-block", &parsed, elapsed);
}

// Nesting at the cap and one level over it. The root mapping is level 1,
// so the value of `x_deep` holds `MAX_DEPTH - 1` nested collections at the
// cap and `MAX_DEPTH` just over it.

/// Nested collections under the root mapping for a total nesting of `total`.
fn under_root(total: usize) -> usize {
    total - 1
}

/// `x_deep: [[…1…]]` with `n` flow sequences.
fn flow_sequences(n: usize) -> String {
    format!("x_deep: {}1{}", "[".repeat(n), "]".repeat(n))
}

/// `x_deep: {a: {a: …1…}}` with `n` flow mappings.
fn flow_mappings(n: usize) -> String {
    format!("x_deep: {}1{}", "{a: ".repeat(n), "}".repeat(n))
}

/// `x_deep: [{a: [{a: …1…}]}]`: `n` flow collections, sequences and
/// mappings alternating.
fn flow_mix(n: usize) -> String {
    let mut open = String::new();
    let mut close = String::new();
    for level in 0..n {
        if level % 2 == 0 {
            open.push('[');
            close.insert(0, ']');
        } else {
            open.push_str("{a: ");
            close.insert(0, '}');
        }
    }
    format!("x_deep: {open}1{close}")
}

/// `x_deep: {{…{a: 1}…: 1}: 1}`: `n` flow mappings, each the key of the next.
fn mappings_as_keys(n: usize) -> String {
    format!("x_deep: {}a{}", "{".repeat(n), ": 1}".repeat(n))
}

/// `n` block collections under `x_deep:`, one per line and two spaces
/// deeper each: level `i` (1-based) is a sequence entry `-` when
/// `is_sequence(i)`, else a mapping key `a:`; the innermost holds `1`.
fn block(n: usize, is_sequence: impl Fn(usize) -> bool) -> String {
    let mut yaml = String::from("x_deep:");
    for level in 1..=n {
        let token = if is_sequence(level) { "-" } else { "a:" };
        yaml.push('\n');
        yaml.push_str(&"  ".repeat(level));
        yaml.push_str(token);
    }
    yaml.push_str(" 1");
    yaml
}

fn block_mappings(n: usize) -> String {
    block(n, |_| false)
}

fn block_sequences(n: usize) -> String {
    block(n, |_| true)
}

fn block_mix(n: usize) -> String {
    block(n, |level| level % 2 == 1)
}

/// Block mappings for the outer half, flow mix for the inner half.
fn block_then_flow(n: usize) -> String {
    let outer = n / 2;
    let inner = flow_mix(n - outer);
    let inner = inner.strip_prefix("x_deep: ").expect("flow_mix prefix");
    let mut yaml = block(outer, |_| false);
    yaml.truncate(yaml.len() - " 1".len());
    format!("{yaml} {inner}")
}

/// Nested collections in a kept value.
fn nesting(value: &FmValue) -> usize {
    match value {
        FmValue::Seq(items) => 1 + items.iter().map(nesting).max().unwrap_or(0),
        FmValue::Map(map) => 1 + map.iter().map(|(_, v)| nesting(v)).max().unwrap_or(0),
        _ => 0,
    }
}

/// At the cap the block parses on a 2 MiB thread: only `x_deep` is
/// reported (`unknown-key`), the ID is read and, when `kept` is given, the
/// value in `extra` still holds that many nested collections.
fn assert_parses_at_cap(name: &str, build: fn(usize) -> String, kept: Option<usize>) {
    assert_parses_at_cap_with(name, build, kept, &[DiagnosticCode::UnknownKey]);
}

/// [`assert_parses_at_cap`] with the diagnostics expected at the cap.
fn assert_parses_at_cap_with(
    name: &str,
    build: fn(usize) -> String,
    kept: Option<usize>,
    codes: &[DiagnosticCode],
) {
    let n = under_root(MAX_DEPTH);
    let (parsed, _) = parse_within_limit(name, wrap(&build(n)));
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>(),
        codes,
        "{name}: nesting {MAX_DEPTH} (root + {n}) is within the cap: {:?}",
        parsed.diagnostics
    );
    let document = parsed.document().expect("document node");
    assert_eq!(document.id.as_deref(), Some("X-1"), "{name}");
    let extra = document.extra.as_ref().expect("x_deep is kept in extra");
    let entry = extra
        .iter()
        .find(|entry| entry.key == "x_deep")
        .expect("x_deep entry");
    if let Some(kept) = kept {
        assert_eq!(nesting(&entry.value), kept, "{name}: value kept whole");
    }
}

/// One level over the cap is exactly one `frontmatter-yaml`, within 1 s.
fn assert_rejected_over_cap(name: &str, build: fn(usize) -> String) {
    let n = under_root(MAX_DEPTH + 1);
    let (parsed, elapsed) = parse_within_limit(name, wrap(&build(n)));
    assert_one_yaml_error(name, &parsed, elapsed);
}

#[test]
fn flow_sequences_at_the_depth_cap_parse_and_one_more_level_is_one_yaml_error() {
    assert_parses_at_cap("flow-[-at-cap", flow_sequences, Some(under_root(MAX_DEPTH)));
    assert_rejected_over_cap("flow-[-over-cap", flow_sequences);
}

#[test]
fn flow_mappings_at_the_depth_cap_parse_and_one_more_level_is_one_yaml_error() {
    assert_parses_at_cap("flow-{-at-cap", flow_mappings, Some(under_root(MAX_DEPTH)));
    assert_rejected_over_cap("flow-{-over-cap", flow_mappings);
}

#[test]
fn flow_mix_at_the_depth_cap_parses_and_one_more_level_is_one_yaml_error() {
    assert_parses_at_cap("flow-mix-at-cap", flow_mix, Some(under_root(MAX_DEPTH)));
    assert_rejected_over_cap("flow-mix-over-cap", flow_mix);
}

#[test]
fn mappings_as_keys_at_the_depth_cap_parse_and_one_more_level_is_one_yaml_error() {
    // An entry whose key is a collection is dropped with one
    // `frontmatter-type` (P4 of docs/features/phase1-cleanup.md), so only
    // the outer mapping is left, empty.
    assert_parses_at_cap_with(
        "keys-at-cap",
        mappings_as_keys,
        Some(1),
        &[DiagnosticCode::UnknownKey, DiagnosticCode::FrontmatterType],
    );
    assert_rejected_over_cap("keys-over-cap", mappings_as_keys);
}

#[test]
fn block_mappings_at_the_depth_cap_parse_and_one_more_level_is_one_yaml_error() {
    assert_parses_at_cap(
        "block-map-at-cap",
        block_mappings,
        Some(under_root(MAX_DEPTH)),
    );
    assert_rejected_over_cap("block-map-over-cap", block_mappings);
}

#[test]
fn block_sequences_at_the_depth_cap_parse_and_one_more_level_is_one_yaml_error() {
    assert_parses_at_cap(
        "block-seq-at-cap",
        block_sequences,
        Some(under_root(MAX_DEPTH)),
    );
    assert_rejected_over_cap("block-seq-over-cap", block_sequences);
}

#[test]
fn block_mix_at_the_depth_cap_parses_and_one_more_level_is_one_yaml_error() {
    assert_parses_at_cap("block-mix-at-cap", block_mix, Some(under_root(MAX_DEPTH)));
    assert_rejected_over_cap("block-mix-over-cap", block_mix);
}

#[test]
fn block_then_flow_at_the_depth_cap_parses_and_one_more_level_is_one_yaml_error() {
    assert_parses_at_cap(
        "block-flow-at-cap",
        block_then_flow,
        Some(under_root(MAX_DEPTH)),
    );
    assert_rejected_over_cap("block-flow-over-cap", block_then_flow);
}

/// The builders produce what they claim: a text of `n` nested collections
/// has exactly `n` levels (checked one level below the cap, where every
/// shape must parse), so the boundary tests are not off by one.
#[test]
fn builders_nest_exactly_as_many_levels_as_asked() {
    let n = under_root(MAX_DEPTH) - 1;
    for (name, build) in [
        ("flow-[", flow_sequences as fn(usize) -> String),
        ("flow-{", flow_mappings),
        ("flow-mix", flow_mix),
        ("block-map", block_mappings),
        ("block-seq", block_sequences),
        ("block-mix", block_mix),
        ("block-flow", block_then_flow),
    ] {
        let (parsed, _) = parse_within_limit(name, wrap(&build(n)));
        let document = parsed.document().expect("document node");
        let entry = document
            .extra
            .as_ref()
            .and_then(|extra| extra.iter().find(|entry| entry.key == "x_deep"))
            .unwrap_or_else(|| panic!("{name}: x_deep kept: {:?}", parsed.diagnostics));
        assert_eq!(nesting(&entry.value), n, "{name}");
    }
}

/// A repository document, read by its repository-relative path.
fn document_text(relative: &str) -> String {
    let path = common::repository_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{relative}: {error}"))
}

/// The first line of `file` starting with `marker` once list and checkbox
/// marks (`- `, `[ ] `, `[x] `) are dropped.
fn line_starting<'t>(file: &str, text: &'t str, marker: &str) -> &'t str {
    text.lines()
        .find(|line| {
            let line = line.trim_start().trim_start_matches("- ");
            let line = line
                .strip_prefix("[ ] ")
                .or_else(|| line.strip_prefix("[x] "))
                .unwrap_or(line);
            line.starts_with(marker)
        })
        .unwrap_or_else(|| panic!("{file}: no line starting `{marker}`"))
}

/// The number right after `marker` in `line` of `file`.
fn number_after(file: &str, line: &str, marker: &str) -> usize {
    let at = line
        .find(marker)
        .unwrap_or_else(|| panic!("{file}: no `{marker}` in: {line}"))
        + marker.len();
    let digits: String = line[at..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("{file}: no number after `{marker}` in: {line}"))
}

/// The number right before `marker` in `line` of `file`.
fn number_before(file: &str, line: &str, marker: &str) -> usize {
    let at = line
        .find(marker)
        .unwrap_or_else(|| panic!("{file}: no `{marker}` in: {line}"));
    let head = &line[..at];
    let start = head
        .rfind(|c: char| !c.is_ascii_digit())
        .map_or(0, |index| index + 1);
    head[start..]
        .parse()
        .unwrap_or_else(|_| panic!("{file}: no number before `{marker}` in: {line}"))
}

/// The cap is the documented number, not just a number the stack survives:
/// the shapes above still fit a 2 MiB debug stack with a cap twice as high,
/// so this is the check that turns red when `MAX_DEPTH` drifts from the
/// documents. The crate README states the cap ("Nesting cap N: … the
/// (N+1)th is one `frontmatter-yaml`"); the spec's AC-07 repeats it
/// ("nesting ≤ N …; N+1 levels,").
#[test]
fn max_depth_is_the_cap_the_spec_names() {
    let readme = "crates/specengine-core/README.md";
    let text = document_text(readme);
    let cap = line_starting(readme, &text, "Nesting cap ");
    assert_eq!(
        number_after(readme, cap, "Nesting cap "),
        MAX_DEPTH,
        "{readme}: the stated cap vs `MAX_DEPTH`"
    );
    assert_eq!(
        number_after(readme, cap, " and the "),
        MAX_DEPTH + 1,
        "{readme}: the first rejected depth vs `MAX_DEPTH + 1`"
    );

    let spec = "docs/features/spec-parser.md";
    let text = document_text(spec);
    let ac07 = line_starting(spec, &text, "AC-07 ");
    assert_eq!(
        number_after(spec, ac07, "nesting ≤ "),
        MAX_DEPTH,
        "{spec}: AC-07's accepted nesting vs `MAX_DEPTH`"
    );
    assert_eq!(
        number_before(spec, ac07, " levels,"),
        MAX_DEPTH + 1,
        "{spec}: AC-07's first rejected nesting vs `MAX_DEPTH + 1`"
    );
}
