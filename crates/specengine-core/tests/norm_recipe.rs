//! docs/features/norm-hash.md, the recipe `specengine-norm/v1` through core's
//! public `norm_input` (no hash here: core has no hash crate; the hashes are
//! pinned in `specengine-cli`'s `norm_show.rs` and the store's
//! `norm_index.rs`):
//!
//! - TE-1: the golden input bytes of MEC-STAMINA, RULE-STAM-REGEN,
//!   RULE-SPRINT-COST, MEC-SPRINT (spec-a) and FLAG-DRY-RUN (spec-b), built
//!   here by hand (lengths written out as little-endian bytes, the spec-b
//!   text in escapes). M: a final LF; links unsorted; big-endian lengths.
//! - TE-2: invariance, a row each, each naming the mutation it catches.
//! - TE-3: sensitivity.
//! - TE-4: null (core half): each of the five codes nulls the document, its
//!   sections keep theirs; a file that is not UTF-8 has no input; the other
//!   findings null nothing. M: the text alone hashed on a broken
//!   front-matter.
//! - TE-9: determinism (core half): another path, the same bytes.
//! - TE-10: genre: every node of spec-c (a Python web service) has its
//!   input; a scan of the recipe's source finds no fixture prefix, kind,
//!   front-matter key, link type, path or stack literal (`revision.rs`
//!   holds `"rev"` alone, by design). M: a `"mechanic"` literal.
//! - TE-11: performance: 50 000 sections within the store's `own_text`
//!   bound (2 s, debug, the loop alone). M: a quadratic join.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::time::{Duration, Instant};

use common::{corpus_scheme, fixture, repository_root, to_crlf, walked_md_files, with_bom};
use specengine_core::{IdSchemeToml, NORM_RECIPE, TYPED_KEYS, norm_input};
use specengine_model::{DiagnosticCode, IdScheme, LINK_TYPES, MENTIONS, ParsedFile};

// ------------------------------------------------------------------ helpers

fn spec_a() -> IdScheme {
    corpus_scheme(&fixture("spec-a"))
}

fn parse(path: &str, bytes: &[u8], scheme: &IdScheme) -> ParsedFile {
    specengine_core::parse(path, bytes, scheme)
}

/// The position of node `id` in `parsed` (0 the document).
fn ord_of(parsed: &ParsedFile, id: &str) -> usize {
    parsed
        .nodes
        .iter()
        .position(|node| node.id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} in the parse of {}", parsed.path))
}

/// `norm_input` of node `id` of `bytes`, parsed at `path`.
fn input_at(path: &str, bytes: &[u8], scheme: &IdScheme, id: &str) -> Option<Vec<u8>> {
    let parsed = parse(path, bytes, scheme);
    norm_input(&parsed, bytes, ord_of(&parsed, id))
}

fn input_of(bytes: &[u8], scheme: &IdScheme, id: &str) -> Option<Vec<u8>> {
    input_at("docs/spec/lantern.md", bytes, scheme, id)
}

/// `norm_input` of every node of `bytes`, by position.
fn inputs(bytes: &[u8], scheme: &IdScheme) -> Vec<Option<Vec<u8>>> {
    let parsed = parse("docs/spec/lantern.md", bytes, scheme);
    (0..parsed.nodes.len())
        .map(|ord| norm_input(&parsed, bytes, ord))
        .collect()
}

/// An input read back by hand (`u32le` lengths): the recipe frame, the
/// links, the text. Panics on anything else.
fn decode(input: &[u8]) -> (Vec<String>, String) {
    fn u32_at(input: &[u8], at: &mut usize) -> usize {
        let bytes: [u8; 4] = input[*at..*at + 4].try_into().expect("four bytes");
        *at += 4;
        u32::from_le_bytes(bytes) as usize
    }
    fn frame(input: &[u8], at: &mut usize) -> String {
        let len = u32_at(input, at);
        let text = std::str::from_utf8(&input[*at..*at + len]).expect("UTF-8 frame");
        *at += len;
        text.to_owned()
    }
    let mut at = 0;
    assert_eq!(frame(input, &mut at), NORM_RECIPE);
    let count = u32_at(input, &mut at);
    let links = (0..count).map(|_| frame(input, &mut at)).collect();
    let text = frame(input, &mut at);
    assert_eq!(at, input.len(), "nothing after the text frame");
    (links, text)
}

fn shown(input: &Option<Vec<u8>>) -> String {
    match input {
        Some(bytes) => format!("{:?}", decode(bytes)),
        None => "null".to_owned(),
    }
}

/// `text` with the one occurrence of `from` replaced by `to`.
fn with(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "{from:?} occurs exactly once in the base"
    );
    text.replacen(from, to, 1)
}

/// `bytes` framed by hand: a little-endian length the test spells out,
/// checked against the bytes it frames (a guard on the test's own typing).
fn framed(le: [u8; 4], bytes: &[u8]) -> Vec<u8> {
    assert_eq!(
        u32::from_le_bytes(le) as usize,
        bytes.len(),
        "the hand-written length of {:?}",
        String::from_utf8_lossy(bytes)
    );
    let mut out = le.to_vec();
    out.extend_from_slice(bytes);
    out
}

fn fixture_bytes(corpus: &str, path: &str) -> Vec<u8> {
    fs::read(fixture(corpus).join(path)).expect("fixture file")
}

// ------------------------------------------------------------------ TE-1

const RECIPE_FRAME: [u8; 4] = [0x12, 0x00, 0x00, 0x00];

fn golden(links: &[([u8; 4], &str)], text: ([u8; 4], &str)) -> Vec<u8> {
    let mut want = framed(RECIPE_FRAME, b"specengine-norm/v1");
    want.extend_from_slice(&[links.len() as u8, 0x00, 0x00, 0x00]);
    for (le, link) in links {
        want.extend(framed(*le, link.as_bytes()));
    }
    want.extend(framed(text.0, text.1.as_bytes()));
    want
}

/// TE-1: the five golden inputs, byte for byte. The spec's two examples
/// (RULE-SPRINT-COST, MEC-SPRINT) are 93 and 297 bytes.
#[test]
fn te1_golden_inputs_of_spec_a_and_spec_b() {
    let a = spec_a();
    let b = corpus_scheme(&fixture("spec-b"));
    let stamina = fixture_bytes("spec-a", "docs/spec/movement/stamina.md");
    let sprint = fixture_bytes("spec-a", "docs/spec/movement/sprint.md");
    let cli = fixture_bytes("spec-b", "docs/spec/cli.md");

    let cases: Vec<(&str, &[u8], &IdScheme, Vec<u8>)> = vec![
        (
            // Front-matter `links:` in written order derived_from, depends_on,
            // uses_term; byte-sorted here. `id`, `kind`, `class`, `tier`,
            // `title`, `parent`, `status`, `owner`, `reviewed` not counted;
            // `# Stamina` and both sections cut.
            "MEC-STAMINA",
            &stamina,
            &a,
            golden(
                &[
                    ([0x19, 0, 0, 0], "depends_on ref:MEC-SPRINT"),
                    ([0x16, 0, 0, 0], "derived_from ref:A-101"),
                    ([0x15, 0, 0, 0], "derived_from ref:R-12"),
                    ([0x1c, 0, 0, 0], "uses_term ref:TERM-exhausted"),
                ],
                (
                    [0x73, 0, 0, 0],
                    "Stamina limits sprinting. Drained while sprinting, restored at rest. \
                     <!-- summary: first paragraph, \u{2264} 3 lines -->",
                ),
            ),
        ),
        (
            // A section: no links frame, its inline R-12 is text.
            "RULE-STAM-REGEN",
            &stamina,
            &a,
            golden(
                &[],
                (
                    [0x5d, 0, 0, 0],
                    "- Base rate 10 units/s, delay after sprinting 1.5 s (R-12).\n\
                     - While `Exhausted`, rate \u{00d7} 0.5.",
                ),
            ),
        ),
        (
            // The spec's first example: `rev=3` and the nested
            // EDGE-SPRINT-EMPTY (with its fence) cut out.
            "RULE-SPRINT-COST",
            &sprint,
            &a,
            golden(
                &[],
                (
                    [0x3f, 0, 0, 0],
                    "Sprinting costs 12 units/s, set by R-12@2 and bounded by A-101.",
                ),
            ),
        ),
        (
            // The spec's second example.
            "MEC-SPRINT",
            &sprint,
            &a,
            golden(
                &[
                    ([0x1e, 0, 0, 0], "constrains ref:RULE-STAM-REGEN"),
                    ([0x1a, 0, 0, 0], "depends_on ref:MEC-STAMINA"),
                ],
                (
                    [0xcb, 0, 0, 0],
                    "Hold the sprint key to run; each second drains stamina (see\n\
                     [[MEC-STAMINA#RULE-STAM-REGEN|regeneration]]).\n\
                     \n\
                     ## Tuning notes\n\
                     \n\
                     Numbers come from the feature stamina-tuning/AC-07 and from shared:DEC-0023@1.",
                ),
            ),
        ),
        (
            // spec-b, a nested section with `rev=2`; its homoglyph R\u{0415}Q
            // is text and nulls nothing.
            "FLAG-DRY-RUN",
            &cli,
            &b,
            golden(
                &[],
                (
                    [0x5d, 0, 0, 0],
                    "\u{041f}\u{0435}\u{0447}\u{0430}\u{0442}\u{0430}\u{0435}\u{0442} \
                     \u{043f}\u{043b}\u{0430}\u{043d} \u{0438} \
                     \u{043d}\u{0438}\u{0447}\u{0435}\u{0433}\u{043e} \u{043d}\u{0435} \
                     \u{043f}\u{0438}\u{0448}\u{0435}\u{0442}; \u{0441}\u{043c}. \
                     dry-run/CRIT-01 \u{0438} R\u{0415}Q-003.",
                ),
            ),
        ),
    ];
    let mut sizes = Vec::new();
    for (id, bytes, scheme, want) in &cases {
        let got = input_of(bytes, scheme, id).unwrap_or_else(|| panic!("{id}: null"));
        assert!(
            got == *want,
            "{id}: the input differs from the golden bytes\n got  {:?}\n want {:?}\n got  {got:02x?}\n want {want:02x?}",
            decode(&got),
            decode(want)
        );
        sizes.push((*id, got.len()));
    }
    assert_eq!(
        sizes,
        [
            ("MEC-STAMINA", 257),
            ("RULE-STAM-REGEN", 123),
            ("RULE-SPRINT-COST", 93),
            ("MEC-SPRINT", 297),
            ("FLAG-DRY-RUN", 123),
        ]
    );
    // The spec's literal for RULE-SPRINT-COST, as printed in "The recipe".
    let mut spec_bytes =
        b"\x12\x00\x00\x00specengine-norm/v1\x00\x00\x00\x00\x3f\x00\x00\x00".to_vec();
    spec_bytes
        .extend_from_slice(b"Sprinting costs 12 units/s, set by R-12@2 and bounded by A-101.");
    assert_eq!(
        input_of(&sprint, &a, "RULE-SPRINT-COST").as_deref(),
        Some(spec_bytes.as_slice())
    );
}

// ------------------------------------------------------------------ the base

/// A document carrying every typed front-matter key (the six counted ones
/// and the 21 never counted), an untyped key, a non-ID subheading, two ID
/// sections and a nested one: the base of TE-2 and TE-3.
const BASE: &str = "\
---
id: MEC-LANTERN
kind: mechanic
class: canon
tier: 2
title: Lantern
parent: DOM-LIGHT
status: accepted
owner: owner
reviewed: 2026-09-20
rev: 2
scope: [light]
date: 2026-09-01
shipped: 2026-09-02
ref: Phase 1
to: everyone
severity: low
generator: hand
source: notes
acceptance: manual
aliases: [LAMP-1]
raised_by: {role: owner}
difficulty: hard
release: v1
priority: should
estimate: 2-4
links:
  derived_from: [R-12, A-101]
  depends_on: [MEC-SPRINT]
supersedes: [MEC-OLDLAMP]
working_answer: Q-031
canon: docs/canon/light.md#fuel
refs: [DEC-0007]
adrs: [DEC-0008]
---

# Lantern

The lantern burns oil.
It smokes a little.

- Fuel lasts 60 s.
  - Refill at a well.

## Burn {#RULE-LANTERN-BURN rev=3}

Burns 1 unit/s (R-12@2).

### Gutter {#EDGE-LANTERN-GUTTER}

At zero oil the light goes out.

## Smoke {#RULE-LANTERN-SMOKE}

Smoke rises.

## Notes

Plain notes.
";

const DOC: &str = "MEC-LANTERN";
const BURN: &str = "RULE-LANTERN-BURN";
const GUTTER: &str = "EDGE-LANTERN-GUTTER";
const SMOKE: &str = "RULE-LANTERN-SMOKE";

/// The base parses with no finding that nulls (its untyped `difficulty` is
/// `unknown-key`), and its inputs are what "The recipe" says: every link
/// form rendered, byte-sorted; the H1, the sections and the headings cut.
#[test]
fn the_base_renders_every_link_form_and_its_own_text() {
    let scheme = spec_a();
    let parsed = parse("docs/spec/lantern.md", BASE.as_bytes(), &scheme);
    let codes: Vec<_> = parsed
        .diagnostics
        .iter()
        .map(|d| (d.code, d.line))
        .collect();
    assert_eq!(codes, [(DiagnosticCode::UnknownKey, 23)], "{codes:?}");
    let all = inputs(BASE.as_bytes(), &scheme);
    assert_eq!(all.len(), 4, "the document and three sections");
    let (links, text) = decode(all[0].as_ref().expect("the document is hashed"));
    assert_eq!(
        links,
        [
            "canon path:docs/canon/light.md#fuel",
            "depends_on ref:MEC-SPRINT",
            "derived_from ref:A-101",
            "derived_from ref:R-12",
            "mentions ref:DEC-0007",
            "mentions ref:DEC-0008",
            "supersedes ref:MEC-OLDLAMP",
            "working_answer ref:Q-031",
        ]
    );
    assert_eq!(
        text,
        "The lantern burns oil.\nIt smokes a little.\n\n- Fuel lasts 60 s.\n  - Refill at a well.\n\n## Notes\n\nPlain notes."
    );
    let section = |id: &str| decode(&input_of(BASE.as_bytes(), &scheme, id).expect(id));
    assert_eq!(
        section(BURN),
        (vec![], "Burns 1 unit/s (R-12@2).".to_owned())
    );
    assert_eq!(
        section(GUTTER),
        (vec![], "At zero oil the light goes out.".to_owned())
    );
    assert_eq!(section(SMOKE), (vec![], "Smoke rises.".to_owned()));
}

// ------------------------------------------------------------------ TE-2

/// One invariance row: `variant` must give every node (or the nodes at
/// `ords`) the base's input.
struct Same {
    row: &'static str,
    mutation: &'static str,
    variant: Vec<u8>,
    ords: Option<Vec<usize>>,
}

fn same(row: &'static str, mutation: &'static str, variant: impl Into<Vec<u8>>) -> Same {
    Same {
        row,
        mutation,
        variant: variant.into(),
        ords: None,
    }
}

impl Same {
    /// Compare only the nodes at `ords` (the variant changes the others).
    fn only(mut self, ords: &[usize]) -> Self {
        self.ords = Some(ords.to_vec());
        self
    }
}

/// The rows whose variant has the same nodes in the same order: every
/// node's input equals the base's.
fn invariance_rows() -> Vec<Same> {
    let base = BASE;
    let mut rows = vec![
        same(
            "CRLF line endings",
            "CRLF kept (its CR neither made LF nor trimmed as White_Space)",
            to_crlf(base.as_bytes()),
        ),
        same(
            "a lone CR inside text (paragraph and list lines; the known limit: not at a heading or a fence)",
            "a lone CR not made LF",
            with(
                &with(
                    &with(base, "burns oil.\nIt smokes", "burns oil.\rIt smokes"),
                    "60 s.\n  - Refill",
                    "60 s.\r  - Refill",
                ),
                "Smoke rises.\n",
                "Smoke rises.\r",
            ),
        ),
        same("a BOM", "the BOM read as text", with_bom(base.as_bytes())),
        same(
            "a BOM and CRLF",
            "the BOM read as text, CRLF kept",
            with_bom(&to_crlf(base.as_bytes())),
        ),
        same(
            "trailing spaces, tabs and a no-break space",
            "trailing White_Space kept",
            with(
                &with(
                    &with(base, "burns oil.\n", "burns oil.  \t\n"),
                    "Smoke rises.\n",
                    "Smoke rises.\u{00a0}\u{3000} \n",
                ),
                "Plain notes.\n",
                "Plain notes.   ",
            ),
        ),
        same(
            "extra blank lines (between paragraphs, before a section, at the end, blank lines of spaces)",
            "runs of empty lines kept, leading or trailing ones kept",
            with(
                &with(
                    &with(base, "a little.\n\n- Fuel", "a little.\n\n\n  \n\n- Fuel"),
                    "Burns 1 unit/s (R-12@2).\n\n",
                    "\n\nBurns 1 unit/s (R-12@2).\n\n\n\n",
                ),
                "Plain notes.\n",
                "Plain notes.\n\n\n\t\n",
            ),
        ),
        same(
            "a section heading renamed (and its level of `#` kept)",
            "a heading line in the text",
            with(base, "## Burn {#", "## Burning brightly {#"),
        ),
        same(
            "a nested section heading renamed",
            "a heading line in the text",
            with(base, "### Gutter {#", "### Guttering out {#"),
        ),
        same(
            "a section `rev` bump (rev=3 -> rev=4)",
            "the revision counted",
            with(base, "rev=3}", "rev=4}"),
        ),
        same(
            "a section `rev` written for the first time",
            "the revision counted",
            with(base, "{#RULE-LANTERN-SMOKE}", "{#RULE-LANTERN-SMOKE rev=2}"),
        ),
        same(
            "the document's `rev` bump (rev: 2 -> rev: 9)",
            "the revision counted",
            with(base, "rev: 2\n", "rev: 9\n"),
        ),
        same(
            "the document's H1 renamed",
            "the H1 line kept in the text",
            with(base, "# Lantern\n", "# The oil lantern\n"),
        ),
        same(
            "a section's class or another attribute added",
            "heading attributes counted",
            with(
                base,
                "{#RULE-LANTERN-SMOKE}",
                "{#RULE-LANTERN-SMOKE .playtest key=val}",
            ),
        ),
        same(
            "the untyped key's value changed",
            "an `extra` key counted",
            with(base, "difficulty: hard\n", "difficulty: easy\n"),
        ),
        same(
            "the untyped key removed",
            "an `extra` key counted",
            with(base, "difficulty: hard\n", ""),
        ),
        same(
            "another untyped key added",
            "an `extra` key counted",
            with(base, "difficulty: hard\n", "difficulty: hard\nmood: calm\n"),
        ),
        same(
            "`links:` items reordered, keys reordered",
            "links unsorted",
            with(
                base,
                "  derived_from: [R-12, A-101]\n  depends_on: [MEC-SPRINT]\n",
                "  depends_on: [MEC-SPRINT]\n  derived_from: [A-101, R-12]\n",
            ),
        ),
        same(
            "the counted keys reordered",
            "links in written order",
            with(
                base,
                "supersedes: [MEC-OLDLAMP]\nworking_answer: Q-031\ncanon: docs/canon/light.md#fuel\nrefs: [DEC-0007]\nadrs: [DEC-0008]\n",
                "adrs: [DEC-0008]\nrefs: [DEC-0007]\ncanon: docs/canon/light.md#fuel\nworking_answer: Q-031\nsupersedes: [MEC-OLDLAMP]\n",
            ),
        ),
        same(
            "a link duplicated (in one list and across `links:` and `supersedes:`)",
            "duplicates kept",
            with(
                &with(
                    base,
                    "derived_from: [R-12, A-101]",
                    "derived_from: [R-12, A-101, R-12]",
                ),
                "  depends_on: [MEC-SPRINT]\n",
                "  depends_on: [MEC-SPRINT]\n  supersedes: [MEC-OLDLAMP]\n",
            ),
        ),
        same(
            "`supersedes` moved from `supersedes:` into `links:`",
            "the link's origin key counted",
            with(
                &with(base, "supersedes: [MEC-OLDLAMP]\n", ""),
                "  depends_on: [MEC-SPRINT]\n",
                "  depends_on: [MEC-SPRINT]\n  supersedes: [MEC-OLDLAMP]\n",
            ),
        ),
        same(
            "a nested section edited (its parent and the document unchanged)",
            "nested ID sections kept in the parent's text",
            with(
                base,
                "At zero oil the light goes out.",
                "At zero oil the light goes out at once.",
            ),
        )
        .only(&[0, 1, 3]),
        same(
            "`status: superseded-by X` (its `supersedes` link has `src` X)",
            "the status link counted",
            with(
                base,
                "status: accepted\n",
                "status: superseded-by MEC-NEWLAMP\n",
            ),
        ),
    ];
    // Each never-counted typed key: its value changed, and the key removed.
    for (key, from, to) in never_counted_changes() {
        rows.push(same(
            key,
            "a never-counted key counted",
            with(base, from, to),
        ));
        rows.push(same(
            key,
            "a never-counted key counted",
            with(base, from, ""),
        ));
    }
    rows
}

/// Every typed key the recipe never counts, with a line of the base and a
/// changed line of the same type.
fn never_counted_changes() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("id", "id: MEC-LANTERN\n", "id: MEC-LAMP\n"),
        ("kind", "kind: mechanic\n", "kind: rule\n"),
        ("class", "class: canon\n", "class: spec\n"),
        ("tier", "tier: 2\n", "tier: 3\n"),
        ("title", "title: Lantern\n", "title: Oil lamp\n"),
        ("parent", "parent: DOM-LIGHT\n", "parent: DOM-DARK\n"),
        ("status", "status: accepted\n", "status: draft\n"),
        ("owner", "owner: owner\n", "owner: someone\n"),
        (
            "reviewed",
            "reviewed: 2026-09-20\n",
            "reviewed: 2026-10-01\n",
        ),
        ("rev", "rev: 2\n", "rev: 5\n"),
        ("scope", "scope: [light]\n", "scope: [light, dark]\n"),
        ("date", "date: 2026-09-01\n", "date: 2026-10-01\n"),
        ("shipped", "shipped: 2026-09-02\n", "shipped: 2026-10-02\n"),
        ("ref", "ref: Phase 1\n", "ref: Phase 2\n"),
        ("to", "to: everyone\n", "to: nobody\n"),
        ("severity", "severity: low\n", "severity: high\n"),
        ("generator", "generator: hand\n", "generator: script\n"),
        ("source", "source: notes\n", "source: talk\n"),
        (
            "acceptance",
            "acceptance: manual\n",
            "acceptance: automatic\n",
        ),
        (
            "aliases",
            "aliases: [LAMP-1]\n",
            "aliases: [LAMP-1, LAMP-2]\n",
        ),
        (
            "raised_by",
            "raised_by: {role: owner}\n",
            "raised_by: {role: agent}\n",
        ),
        // docs/features/roadmap-items.md RD-2: moving an item in the plan
        // is no change of its meaning.
        ("release", "release: v1\n", "release: v2\n"),
        ("priority", "priority: should\n", "priority: must\n"),
        ("estimate", "estimate: 2-4\n", "estimate: 3\n"),
    ]
}

/// The keys "The recipe" counts; every other typed key is never counted.
const COUNTED: [&str; 6] = [
    "links",
    "supersedes",
    "working_answer",
    "canon",
    "refs",
    "adrs",
];

/// The never-counted list of the spec ("Never counted") is exactly core's
/// typed keys minus the six counted ones: a typed key added later has to
/// be placed on one side.
#[test]
fn te2_the_never_counted_keys_are_the_typed_keys_minus_the_counted_six() {
    let typed: BTreeSet<&str> = TYPED_KEYS.iter().map(|(key, _)| *key).collect();
    let counted: BTreeSet<&str> = COUNTED.into_iter().collect();
    let never: BTreeSet<&str> = never_counted_changes()
        .into_iter()
        .map(|(key, _, _)| key)
        .collect();
    let spec_list: BTreeSet<&str> = [
        "id",
        "kind",
        "parent",
        "title",
        "status",
        "tier",
        "owner",
        "reviewed",
        "rev",
        "class",
        "scope",
        "date",
        "shipped",
        "ref",
        "to",
        "severity",
        "generator",
        "source",
        "acceptance",
        "aliases",
        "raised_by",
        "release",
        "priority",
        "estimate",
    ]
    .into_iter()
    .collect();
    assert_eq!(never, spec_list);
    assert!(counted.is_subset(&typed), "{counted:?}");
    assert_eq!(
        typed.difference(&counted).copied().collect::<BTreeSet<_>>(),
        never
    );
}

/// TE-2: every row gives the base's inputs (all nodes, or the named ones).
#[test]
fn te2_invariance_rows_each_with_its_mutation() {
    let scheme = spec_a();
    let base = inputs(BASE.as_bytes(), &scheme);
    assert!(base.iter().all(Option::is_some), "the base is hashed");
    let rows = invariance_rows();
    let mut failures = Vec::new();
    for row in &rows {
        let got = inputs(&row.variant, &scheme);
        let ords: Vec<usize> = row
            .ords
            .clone()
            .unwrap_or_else(|| (0..base.len()).collect());
        if got.len() != base.len() {
            failures.push(format!(
                "{} (M: {}): {} nodes, not {}",
                row.row,
                row.mutation,
                got.len(),
                base.len()
            ));
            continue;
        }
        for ord in ords {
            if got[ord] != base[ord] {
                failures.push(format!(
                    "{} (M: {}): node {ord}\n  base    {}\n  variant {}",
                    row.row,
                    row.mutation,
                    shown(&base[ord]),
                    shown(&got[ord])
                ));
            }
        }
    }
    assert!(rows.len() >= 60, "{} rows", rows.len());
    assert!(
        failures.is_empty(),
        "{} invariance row(s) broken:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// TE-2, the H1 rows on documents of their own: a punctuation-only H1 (it
/// has no slug anchor), a setext H1, an H1 after text; renaming it keeps
/// the input, the text around it stays. M: the H1 found through slug
/// anchors; the H1 line kept.
#[test]
fn te2_a_renamed_h1_of_any_form_keeps_the_input() {
    let scheme = spec_a();
    let cases = [
        (
            "a punctuation-only H1",
            "---\nid: MEC-PUNCT\n---\n\n# ???\n\nBody text.\n",
            "# ???\n",
            "# !!! ...\n",
            "Body text.",
        ),
        (
            "a setext H1",
            "---\nid: MEC-SETEXT\n---\n\nLantern\n=======\n\nBody text.\n",
            "Lantern\n=======\n",
            "Oil lamp\n===\n",
            "Body text.",
        ),
        (
            "an H1 after a paragraph",
            "---\nid: MEC-LATE\n---\n\nLead line.\n\n# Title\n\nBody text.\n",
            "# Title\n",
            "# Another title\n",
            "Lead line.\n\nBody text.",
        ),
        (
            "an H1 with closing hashes and trailing spaces",
            "---\nid: MEC-CLOSED\n---\n# Title #   \nBody text.\n",
            "# Title #   \n",
            "# Renamed ##\n",
            "Body text.",
        ),
        (
            "a second H1 is text",
            "---\nid: MEC-TWO\n---\n\n# First\n\n# Second\n\nBody text.\n",
            "# First\n",
            "# Renamed\n",
            "# Second\n\nBody text.",
        ),
    ];
    for (case, text, from, to, want) in cases {
        let id = text
            .lines()
            .find_map(|line| line.strip_prefix("id: "))
            .unwrap();
        let before = input_of(text.as_bytes(), &scheme, id).expect(case);
        let after = input_of(with(text, from, to).as_bytes(), &scheme, id).expect(case);
        assert_eq!(decode(&before), (vec![], want.to_owned()), "{case}");
        assert_eq!(decode(&after), decode(&before), "{case}: renamed");
    }
    // An H1 that is an ID section is a node of its own, never cut from the
    // document: the document keeps only the text before it.
    let text = "---\nid: MEC-IDH1\n---\n\nLead.\n\n# Rule {#RULE-IDH1}\n\nRule text.\n";
    let doc = input_of(text.as_bytes(), &scheme, "MEC-IDH1").expect("doc");
    assert_eq!(decode(&doc), (vec![], "Lead.".to_owned()));
    let rule = input_of(text.as_bytes(), &scheme, "RULE-IDH1").expect("rule");
    assert_eq!(decode(&rule), (vec![], "Rule text.".to_owned()));
}

/// TE-1/TE-2 (review of iteration 2): the H1 cut leaves the text before
/// and after it as two pieces joined with LF, so `Intro\n# Title\nBody`
/// is `Intro\n\nBody` (one empty line where the H1 was), never
/// `Intro\nBody`. M: the pieces around the H1 glued into one line run.
#[test]
fn te2_the_h1_cut_joins_the_text_around_it_with_an_empty_line() {
    let scheme = spec_a();
    let doc = |body: &str| {
        let text = format!("---\nid: MEC-JOIN\n---\n{body}");
        input_of(text.as_bytes(), &scheme, "MEC-JOIN").expect("hashed")
    };
    let cut = doc("Intro\n# Title\nBody\n");
    let blank = doc("Intro\n\nBody\n");
    let glued = doc("Intro\nBody\n");
    assert_eq!(decode(&cut), (vec![], "Intro\n\nBody".to_owned()));
    assert_eq!(cut, blank, "the H1's place is one empty line");
    assert_ne!(cut, glued, "the text around the H1 is never glued");
    assert_eq!(decode(&glued), (vec![], "Intro\nBody".to_owned()));
    // The same with the H1 renamed or given trailing spaces.
    assert_eq!(doc("Intro\n# Another title   \nBody\n"), blank);
}

/// TE-2, the lone-CR row on its own: a lone CR ending a text line (in a
/// document's paragraph, a list and a section's body) gives the input the
/// LF form gives. Out of scope here, the known limit (developer's
/// iteration-1 report): a lone CR ending a heading line or a front-matter
/// fence, which the parser does not read as a line end, so the parse
/// itself differs. M: a lone CR not made LF.
#[test]
fn te2_a_lone_cr_inside_text_is_lf() {
    let scheme = spec_a();
    let lf = "---\nid: MEC-CR\n---\n\n# Lamp\n\nFirst line.\nSecond line.\n\n- one\n- two\n\n## Part {#RULE-CR-PART}\n\nPart one.\nPart two.\n";
    let cr = with(
        &with(
            &with(lf, "First line.\nSecond", "First line.\rSecond"),
            "- one\n- two",
            "- one\r- two",
        ),
        "Part one.\nPart two.",
        "Part one.\rPart two.",
    );
    assert_eq!(cr.matches('\r').count(), 3);
    for id in ["MEC-CR", "RULE-CR-PART"] {
        let want = input_of(lf.as_bytes(), &scheme, id).expect(id);
        assert_eq!(
            input_of(cr.as_bytes(), &scheme, id).as_ref(),
            Some(&want),
            "{id}: a lone CR inside text is LF"
        );
    }
    let (_, text) = decode(&input_of(lf.as_bytes(), &scheme, "MEC-CR").unwrap());
    assert_eq!(text, "First line.\nSecond line.\n\n- one\n- two");
}

/// TE-2, the nested-section rows: a nested section edited, added, removed
/// or moved to another parent leaves its parent's input (and the
/// document's) as it was. M: nested ID sections kept in the parent's text.
#[test]
fn te2_nested_sections_never_reach_the_parent() {
    let scheme = spec_a();
    let base = BASE.as_bytes();
    let of = |bytes: &[u8], id: &str| input_of(bytes, &scheme, id).expect(id);
    let gutter = "### Gutter {#EDGE-LANTERN-GUTTER}\n\nAt zero oil the light goes out.\n\n";
    let removed = with(BASE, gutter, "");
    let added = with(
        BASE,
        "## Smoke {#RULE-LANTERN-SMOKE}\n\nSmoke rises.\n\n",
        "## Smoke {#RULE-LANTERN-SMOKE}\n\nSmoke rises.\n\n### Soot {#EDGE-LANTERN-SOOT}\n\nSoot gathers.\n\n",
    );
    let moved = with(
        &removed,
        "Smoke rises.\n\n",
        &format!("Smoke rises.\n\n{gutter}"),
    );
    let edited = with(BASE, "the light goes out.", "the light dies.");
    for (row, variant) in [
        ("removed", removed.as_bytes()),
        ("added", added.as_bytes()),
        ("moved to another parent", moved.as_bytes()),
        ("edited", edited.as_bytes()),
    ] {
        for id in [DOC, BURN, SMOKE] {
            assert_eq!(
                decode(&of(variant, id)),
                decode(&of(base, id)),
                "a nested section {row}: {id} (M: nested sections kept in the parent's text)"
            );
        }
    }
    // The nested section's own input moves with it.
    assert_eq!(of(moved.as_bytes(), GUTTER), of(base, GUTTER));
    assert_ne!(of(edited.as_bytes(), GUTTER), of(base, GUTTER));
}

/// TE-2: an `[ids]` kind or width change re-parses the same bytes to the
/// same inputs (a kind is never counted, a width is what `spec new`
/// issues). M: the node's kind in the input.
#[test]
fn te2_an_ids_kind_or_width_change_keeps_every_input() {
    let config = fs::read_to_string(fixture("spec-a").join("specengine.toml")).unwrap();
    let base = inputs(BASE.as_bytes(), &spec_a());
    for (row, from, to) in [
        (
            "MEC's kind",
            "MEC  = { kind = \"mechanic\",",
            "MEC  = { kind = \"lamp-system\",",
        ),
        (
            "RULE's kind",
            "RULE = { kind = \"rule\",",
            "RULE = { kind = \"law\",",
        ),
        (
            "R's width",
            "R    = { kind = \"requirement\", width = 2,",
            "R    = { kind = \"requirement\", width = 5,",
        ),
        (
            "A's width",
            "A    = { kind = \"assumption\",  width = 3 }",
            "A    = { kind = \"assumption\",  width = 1 }",
        ),
    ] {
        let changed = with(&config, from, to);
        let scheme = IdScheme::from_toml(&changed).expect("a valid changed scheme");
        assert_eq!(
            inputs(BASE.as_bytes(), &scheme),
            base,
            "{row} changed (M: the kind in the input)"
        );
    }
}

// ------------------------------------------------------------------ TE-3

/// TE-3: each change of meaning changes the input of the node it belongs
/// to.
#[test]
fn te3_sensitivity() {
    let scheme = spec_a();
    let base = BASE;
    let rows: Vec<(&str, &str, String)> = vec![
        ("a word", DOC, with(base, "burns oil", "burns tallow")),
        (
            "a word in a section",
            BURN,
            with(base, "Burns 1 unit/s", "Burns 2 units/s"),
        ),
        (
            "list nesting by indentation",
            DOC,
            with(base, "  - Refill at a well.", "- Refill at a well."),
        ),
        (
            "indentation deepened",
            DOC,
            with(base, "  - Refill at a well.", "    - Refill at a well."),
        ),
        (
            "a link added",
            DOC,
            with(
                base,
                "depends_on: [MEC-SPRINT]",
                "depends_on: [MEC-SPRINT, MEC-STAMINA]",
            ),
        ),
        (
            "a link removed",
            DOC,
            with(base, "  depends_on: [MEC-SPRINT]\n", ""),
        ),
        (
            "a link retargeted",
            DOC,
            with(
                base,
                "depends_on: [MEC-SPRINT]",
                "depends_on: [MEC-STAMINA]",
            ),
        ),
        (
            "a link's type changed",
            DOC,
            with(
                base,
                "  depends_on: [MEC-SPRINT]",
                "  constrains: [MEC-SPRINT]",
            ),
        ),
        (
            "a link's @REV added",
            DOC,
            with(
                base,
                "depends_on: [MEC-SPRINT]",
                "depends_on: [MEC-SPRINT@2]",
            ),
        ),
        (
            "a link's section added",
            DOC,
            with(
                base,
                "depends_on: [MEC-SPRINT]",
                "depends_on: [MEC-SPRINT#RULE-SPRINT-COST]",
            ),
        ),
        (
            "an unknown link type added (as written)",
            DOC,
            with(
                base,
                "  depends_on: [MEC-SPRINT]\n",
                "  depends_on: [MEC-SPRINT]\n  lights: [MEC-SPRINT]\n",
            ),
        ),
        (
            "`working_answer` changed",
            DOC,
            with(base, "working_answer: Q-031", "working_answer: Q-032"),
        ),
        (
            "`canon` changed (anchor)",
            DOC,
            with(base, "light.md#fuel", "light.md#oil"),
        ),
        (
            "`canon` changed (path)",
            DOC,
            with(base, "docs/canon/light.md#fuel", "docs/canon/lamp.md#fuel"),
        ),
        (
            "`supersedes` changed",
            DOC,
            with(
                base,
                "supersedes: [MEC-OLDLAMP]",
                "supersedes: [MEC-OLDERLAMP]",
            ),
        ),
        (
            "`refs` changed",
            DOC,
            with(base, "refs: [DEC-0007]", "refs: [DEC-0009]"),
        ),
        (
            "`adrs` changed",
            DOC,
            with(base, "adrs: [DEC-0008]", "adrs: [DEC-0009]"),
        ),
        (
            "a subheading without ID renamed",
            DOC,
            with(base, "## Notes\n", "## Remarks\n"),
        ),
        (
            "a subheading's level changed",
            DOC,
            with(base, "## Notes\n", "### Notes\n"),
        ),
        (
            "an HTML comment edited",
            BURN,
            with(
                base,
                "Burns 1 unit/s (R-12@2).",
                "Burns 1 unit/s (R-12@2). <!-- tuned -->",
            ),
        ),
        (
            "an inline reference's revision in a section",
            BURN,
            with(base, "(R-12@2)", "(R-12@3)"),
        ),
        (
            "inner spaces",
            SMOKE,
            with(base, "Smoke rises.", "Smoke  rises."),
        ),
        (
            "a paragraph split by a blank line",
            DOC,
            with(base, "burns oil.\nIt smokes", "burns oil.\n\nIt smokes"),
        ),
    ];
    let mut failures = Vec::new();
    for (row, id, variant) in &rows {
        let before = input_of(BASE.as_bytes(), &scheme, id);
        let after = input_of(variant.as_bytes(), &scheme, id);
        assert!(after.is_some(), "{row}: the variant is hashed");
        if before == after {
            failures.push(format!("{row}: {id} unchanged: {}", shown(&after)));
        }
    }
    assert!(failures.is_empty(), "insensitive:\n{}", failures.join("\n"));

    // Text moved from a section into its parent changes both: SMOKE's
    // line into the document's own text (before its first section).
    let moved = with(
        &with(
            base,
            "## Smoke {#RULE-LANTERN-SMOKE}\n\nSmoke rises.\n",
            "## Smoke {#RULE-LANTERN-SMOKE}\n",
        ),
        "It smokes a little.\n",
        "It smokes a little.\nSmoke rises.\n",
    );
    for id in [DOC, SMOKE] {
        assert_ne!(
            input_of(moved.as_bytes(), &scheme, id),
            input_of(BASE.as_bytes(), &scheme, id),
            "text moved from a section into its parent: {id}"
        );
    }
    let moved_up = with(
        base,
        "## Burn {#RULE-LANTERN-BURN rev=3}\n\nBurns 1 unit/s (R-12@2).\n\n### Gutter {#EDGE-LANTERN-GUTTER}\n\nAt zero oil the light goes out.\n",
        "## Burn {#RULE-LANTERN-BURN rev=3}\n\nBurns 1 unit/s (R-12@2).\n\nAt zero oil the light goes out.\n\n### Gutter {#EDGE-LANTERN-GUTTER}\n",
    );
    for id in [BURN, GUTTER] {
        assert_ne!(
            input_of(moved_up.as_bytes(), &scheme, id),
            input_of(BASE.as_bytes(), &scheme, id),
            "text moved from a nested section into its parent section: {id}"
        );
    }
    // The document is not the section's parent in text: it stays.
    assert_eq!(
        input_of(moved_up.as_bytes(), &scheme, DOC),
        input_of(BASE.as_bytes(), &scheme, DOC)
    );
}

// ------------------------------------------------------------------ TE-4

/// A document after a front-matter whose `head` is given, with a section.
fn doc_with_front_matter(head: &str) -> String {
    format!("{head}\n# Title\n\nDocument text.\n\n## Part {{#RULE-NULL-PART}}\n\nSection text.\n")
}

/// TE-4: each of the five codes nulls the document's input; its section
/// keeps its own, the same as with a sound front-matter. M: the text alone
/// hashed on a broken front-matter.
#[test]
fn te4_the_five_codes_null_the_document_and_only_it() {
    let scheme = spec_a();
    let sound = doc_with_front_matter("---\nid: MEC-NULL\n---\n");
    let section = input_of(sound.as_bytes(), &scheme, "RULE-NULL-PART").expect("section");
    assert_eq!(decode(&section), (vec![], "Section text.".to_owned()));
    let cases = [
        (
            DiagnosticCode::FrontmatterUnclosed,
            "---\nid: MEC-NULL\ntitle: open\n\n",
        ),
        (
            DiagnosticCode::FrontmatterYaml,
            "---\nid: MEC-NULL\nlinks: [unclosed\n---\n",
        ),
        (
            DiagnosticCode::FrontmatterNotMapping,
            "---\n- one\n- two\n---\n",
        ),
        (
            DiagnosticCode::FrontmatterType,
            "---\nid: MEC-NULL\ntier: high\n---\n",
        ),
        (
            DiagnosticCode::UnparsedReference,
            "---\nid: MEC-NULL\nlinks:\n  depends_on: [not one reference]\n---\n",
        ),
    ];
    for (code, head) in cases {
        let text = doc_with_front_matter(head);
        let parsed = parse("docs/n.md", text.as_bytes(), &scheme);
        assert!(
            parsed.diagnostics.iter().any(|found| found.code == code),
            "{code}: {:?}",
            parsed.diagnostics
        );
        assert_eq!(
            norm_input(&parsed, text.as_bytes(), 0),
            None,
            "{code}: the document is null (M: the text alone hashed)"
        );
        let ord = ord_of(&parsed, "RULE-NULL-PART");
        assert_eq!(
            norm_input(&parsed, text.as_bytes(), ord).as_ref(),
            Some(&section),
            "{code}: the section keeps its input"
        );
    }
}

/// TE-4: the other findings null nothing.
#[test]
fn te4_other_findings_null_nothing() {
    let scheme = spec_a();
    let cases = [
        (DiagnosticCode::IdNotInScheme, "---\nid: ZZZ-1\n---\n"),
        (
            DiagnosticCode::Homoglyph,
            "---\nid: MEC-NULL\nlinks:\n  depends_on: [M\u{0415}C-SPRINT]\n---\n",
        ),
        (DiagnosticCode::BadRev, "---\nid: MEC-NULL\nrev: 0\n---\n"),
        (
            DiagnosticCode::UnknownKey,
            "---\nid: MEC-NULL\nflavour: smoky\n---\n",
        ),
        (
            DiagnosticCode::UnknownLinkType,
            "---\nid: MEC-NULL\nlinks:\n  lights: [MEC-SPRINT]\n---\n",
        ),
        (
            DiagnosticCode::KindMismatch,
            "---\nid: MEC-NULL\nkind: rule\n---\n",
        ),
    ];
    for (code, head) in cases {
        let text = doc_with_front_matter(head);
        let parsed = parse("docs/n.md", text.as_bytes(), &scheme);
        assert!(
            parsed.diagnostics.iter().any(|found| found.code == code),
            "{code}: {:?}",
            parsed.diagnostics
        );
        assert!(
            norm_input(&parsed, text.as_bytes(), 0).is_some(),
            "{code} nulls nothing: {:?}",
            parsed.diagnostics
        );
    }
    // duplicate-id: both sections kept, both hashed, the document too.
    let text = doc_with_front_matter("---\nid: MEC-NULL\n---\n")
        + "\n## Again {#RULE-NULL-PART}\n\nOther text.\n";
    let parsed = parse("docs/n.md", text.as_bytes(), &scheme);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|found| found.code == DiagnosticCode::DuplicateId),
        "{:?}",
        parsed.diagnostics
    );
    for ord in 0..parsed.nodes.len() {
        assert!(
            norm_input(&parsed, text.as_bytes(), ord).is_some(),
            "duplicate-id: node {ord}"
        );
    }
}

/// TE-4: a file that is not UTF-8 has no nodes and no input; a position
/// past the last node and the bytes of another file are null too.
#[test]
fn te4_not_utf8_and_foreign_bytes_are_null() {
    let scheme = spec_a();
    let bytes = b"---\nid: MEC-NULL\n---\n\n# T\n\nbad \xff byte\n";
    let parsed = parse("docs/n.md", bytes, &scheme);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|found| found.code == DiagnosticCode::NotUtf8)
    );
    assert!(parsed.nodes.is_empty(), "{:?}", parsed.nodes);
    assert_eq!(norm_input(&parsed, bytes, 0), None);
    let text = doc_with_front_matter("---\nid: MEC-NULL\n---\n");
    let parsed = parse("docs/n.md", text.as_bytes(), &scheme);
    assert_eq!(
        norm_input(&parsed, text.as_bytes(), parsed.nodes.len()),
        None
    );
    // A parse of other bytes: a span past their end is null, not a panic.
    assert_eq!(norm_input(&parsed, b"short", 1), None);
}

// ------------------------------------------------------------------ TE-9

/// TE-9, core half: the same bytes parsed under another path (root, depth,
/// file name) give the same inputs. M: the path in the input.
#[test]
fn te9_the_path_never_enters_the_input() {
    let scheme = spec_a();
    let sprint = fixture_bytes("spec-a", "docs/spec/movement/sprint.md");
    let at = |path: &str| {
        let parsed = parse(path, &sprint, &scheme);
        (0..parsed.nodes.len())
            .map(|ord| norm_input(&parsed, &sprint, ord))
            .collect::<Vec<_>>()
    };
    let here = at("docs/spec/movement/sprint.md");
    for path in ["sprint.md", "other/root/docs/x.md", "/abs/elsewhere/s.md"] {
        assert_eq!(at(path), here, "{path}");
    }
    // Twice in one process: the same bytes.
    assert_eq!(at("docs/spec/movement/sprint.md"), here);
}

// ------------------------------------------------------------------ TE-10

/// TE-10: every node of spec-c (a Python web service, kinds and prefixes
/// of its own) has an input, the documents' with their declared links;
/// the inputs of a corpus are distinct per node.
#[test]
fn te10_spec_c_is_hashed_like_any_corpus() {
    for corpus in ["spec-c", "spec-a", "spec-b"] {
        let root = fixture(corpus);
        let scheme = corpus_scheme(&root);
        let mut nodes = 0;
        let mut nulls = Vec::new();
        let mut linked = 0;
        let mut seen = BTreeSet::new();
        for (path, bytes) in walked_md_files(&root) {
            let parsed = parse(&path, &bytes, &scheme);
            for ord in 0..parsed.nodes.len() {
                nodes += 1;
                match norm_input(&parsed, &bytes, ord) {
                    Some(input) => {
                        let (links, _) = decode(&input);
                        linked += usize::from(!links.is_empty());
                        seen.insert(input);
                    }
                    None => nulls.push(format!("{path}#{ord}")),
                }
            }
        }
        match corpus {
            "spec-c" => assert!(nulls.is_empty(), "spec-c: {nulls:?}"),
            // stamina-tuning.md: frontmatter-type.
            "spec-a" => assert_eq!(nulls, ["docs/features/stamina-tuning.md#0"]),
            // QN-08.md: frontmatter-yaml.
            _ => assert_eq!(nulls, ["docs/records/QN/QN-08.md#0"]),
        }
        assert!(nodes >= 8, "{corpus}: {nodes} nodes");
        assert!(linked >= 1, "{corpus}: {linked} documents with links");
        eprintln!(
            "{corpus}: {nodes} nodes, {} null, {} distinct inputs, {linked} with links",
            nulls.len(),
            seen.len()
        );
    }
}

/// The string literals of a Rust source (code only: comments skipped),
/// byte strings included; char literals separately.
fn literals(source: &str) -> (Vec<String>, Vec<char>) {
    let chars: Vec<char> = source.chars().collect();
    let mut strings = Vec::new();
    let mut singles = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        if c == '/' && next == Some('/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            at += 2;
            while at + 1 < chars.len() && !(chars[at] == '*' && chars[at + 1] == '/') {
                at += 1;
            }
            at += 2;
            continue;
        }
        if c == 'r' && (next == Some('"') || next == Some('#')) {
            let mut hashes = 0;
            let mut probe = at + 1;
            while chars.get(probe) == Some(&'#') {
                hashes += 1;
                probe += 1;
            }
            if chars.get(probe) == Some(&'"') {
                let close: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = chars[probe + 1..].iter().collect();
                let end = rest.find(&close).expect("raw string closes");
                strings.push(rest[..end].to_owned());
                at = probe + 1 + rest[..end].chars().count() + close.chars().count();
                continue;
            }
        }
        if c == '"' {
            let mut text = String::new();
            at += 1;
            while chars[at] != '"' {
                if chars[at] == '\\' {
                    at += 1;
                    match chars[at] {
                        'n' => text.push('\n'),
                        'r' => text.push('\r'),
                        't' => text.push('\t'),
                        '0' => text.push('\0'),
                        'u' => {
                            let close = (at..chars.len()).find(|&i| chars[i] == '}').unwrap();
                            let hex: String = chars[at + 2..close].iter().collect();
                            text.push(
                                char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap(),
                            );
                            at = close;
                        }
                        other => text.push(other),
                    }
                    at += 1;
                    continue;
                }
                text.push(chars[at]);
                at += 1;
            }
            strings.push(text);
            at += 1;
            continue;
        }
        if c == '\'' {
            // 'x', '\n', '\u{..}' are char literals; 'a without a close is a
            // lifetime.
            if next == Some('\\') {
                let close = (at + 2..chars.len()).find(|&i| chars[i] == '\'').unwrap();
                singles.push('\\');
                at = close + 1;
                continue;
            }
            if chars.get(at + 2) == Some(&'\'') {
                singles.push(next.unwrap());
                at += 3;
                continue;
            }
        }
        at += 1;
    }
    (strings, singles)
}

/// Every word a recipe literal must not hold: the prefixes and kinds of
/// every fixture corpus, core's typed front-matter keys, the model's link
/// types, and path and stack words.
fn forbidden_words() -> BTreeSet<String> {
    let mut words = BTreeSet::new();
    for entry in fs::read_dir(repository_root().join("fixtures")).unwrap() {
        let config = entry.unwrap().path().join("specengine.toml");
        let Ok(text) = fs::read_to_string(&config) else {
            continue;
        };
        let Ok(scheme) = IdScheme::from_toml(&text) else {
            continue;
        };
        for spec in scheme.prefixes() {
            words.insert(spec.prefix.clone());
            words.insert(spec.kind.clone());
        }
    }
    assert!(
        words.contains("MEC") && words.contains("mechanic"),
        "{words:?}"
    );
    assert!(
        words.contains("FEE") && words.contains("tariff"),
        "{words:?}"
    );
    words.extend(TYPED_KEYS.iter().map(|(key, _)| (*key).to_owned()));
    words.extend(LINK_TYPES.iter().map(|name| (*name).to_owned()));
    words.insert(MENTIONS.to_owned());
    for word in [
        "docs",
        "spec",
        "md",
        "toml",
        "specengine.toml",
        "bevy",
        "ecs",
        "rust",
        "cargo",
        "ron",
        "python",
        "system",
        "plugin",
        "game",
        "component",
        "resource",
        "src",
        "crates",
    ] {
        words.insert(word.to_owned());
    }
    words
}

/// The literal's words (letters, digits, `_` and `-`).
fn words_of(literal: &str) -> Vec<String> {
    literal
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The literals of `source` that break the recipe's rule: any string
/// literal holding a letter but the recipe's own three (its name and the
/// two target tags "The recipe" spells, ` ref:` and ` path:`), with the
/// forbidden words it holds; any alphabetic char literal.
fn offending_literals(source: &str) -> Vec<String> {
    let allowed = [NORM_RECIPE, " ref:", " path:"];
    let forbidden = forbidden_words();
    let (strings, singles) = literals(source);
    let mut out = Vec::new();
    for literal in strings {
        // The recipe's own literals, and those of no letter (the line
        // endings the normalisation rewrites).
        if allowed.contains(&literal.as_str()) || !literal.chars().any(char::is_alphabetic) {
            continue;
        }
        let hits: Vec<String> = words_of(&literal)
            .into_iter()
            .filter(|word| forbidden.contains(word) || forbidden.contains(&word.to_lowercase()))
            .collect();
        out.push(format!("{literal:?} (forbidden words: {hits:?})"));
    }
    for single in singles {
        if single.is_alphabetic() {
            out.push(format!("'{single}'"));
        }
    }
    out
}

/// TE-10: the recipe's source names no fixture prefix, kind, key, link
/// type, path or stack: its only string literals are its name and its two
/// target tags. `revision.rs` holds exactly one, `"rev"`, the key ADR-0040
/// names (by design). M: a `"mechanic"` literal.
#[test]
fn te10_the_recipe_source_holds_no_domain_literal() {
    let src = repository_root().join("crates/specengine-core/src");
    let norm = fs::read_to_string(src.join("norm.rs")).expect("norm.rs");
    let offending = offending_literals(&norm);
    assert!(
        offending.is_empty(),
        "norm.rs literals outside the recipe's own:\n{}",
        offending.join("\n")
    );
    let (strings, _) = literals(&norm);
    assert!(
        strings.iter().any(|literal| literal == NORM_RECIPE),
        "the scan sees the recipe's name: {strings:?}"
    );
    let revision = fs::read_to_string(src.join("revision.rs")).expect("revision.rs");
    let (strings, singles) = literals(&revision);
    assert_eq!(strings, ["rev"], "revision.rs: its one key literal");
    assert!(singles.is_empty(), "{singles:?}");
}

/// The scan itself: it sees a kind, a prefix, a key and a path literal,
/// skips comments, and tells char literals from lifetimes.
#[test]
fn te10_the_scan_sees_what_it_must() {
    let source = "// \"mechanic\" in a comment\n/* \"MEC\" */\nfn f<'a>(x: &'a str) -> bool {\n    let _ = \"mechanic\";\n    let _ = b\"docs/spec\";\n    let _ = 'k';\n    let _ = '\\n';\n    x == \"specengine-norm/v1\" || x == \"status\" || x == r#\"MEC\"#\n}\n";
    let offending = offending_literals(source);
    assert_eq!(
        offending,
        [
            "\"mechanic\" (forbidden words: [\"mechanic\"])",
            "\"docs/spec\" (forbidden words: [\"docs\", \"spec\"])",
            "\"status\" (forbidden words: [\"status\"])",
            "\"MEC\" (forbidden words: [\"MEC\"])",
            "'k'",
        ]
    );
}

// ------------------------------------------------------------------ TE-11

fn x_scheme() -> IdScheme {
    IdScheme::from_toml("[ids]\nX = { kind = \"x\", width = 5 }\nY = { kind = \"y\", width = 2 }\n")
        .expect("scheme")
}

/// One file of `sections` top-level ID sections in the shape of the
/// store's `own_text` test (`rows.rs`), each also citing a reference (one
/// inline link per section), the document declaring two links.
fn big_file(sections: usize) -> String {
    let mut text = String::from(
        "---\nid: Y-01\nlinks:\n  depends_on: [X-00001, X-00002]\n---\n# Top\n\nIntro.\n\n",
    );
    for n in 1..=sections {
        text.push_str(&format!("# Section {n} {{#X-{n:05}}}\n\nword{n} Y-01\n\n"));
    }
    text
}

/// `norm_input` of every node of `parsed`, timed, giving up past `bound`.
fn timed_inputs(parsed: &ParsedFile, bytes: &[u8], bound: Duration) -> (Duration, usize) {
    let started = Instant::now();
    for ord in 0..parsed.nodes.len() {
        assert!(
            norm_input(parsed, bytes, ord).is_some(),
            "node {ord} hashed"
        );
        if ord % 1_000 == 0 && started.elapsed() > bound {
            return (started.elapsed(), ord);
        }
    }
    (started.elapsed(), parsed.nodes.len())
}

/// TE-11: `norm_input` of every node of one file of 50 000 sections: the
/// loop alone (the parse untimed) within the store's `own_text` bound, 2 s
/// in a debug build; it gives up at the bound, so a quadratic cost fails
/// fast. Then the cost per node does not grow with the file: twice the
/// sections, at most three times the time (linear about 2, quadratic about
/// 4; the best of three runs each). M: a quadratic join (each node
/// filtering every link of the file).
#[test]
fn te11_fifty_thousand_sections_within_the_own_text_bound() {
    const SECTIONS: usize = 50_000;
    const BOUND: Duration = Duration::from_secs(2);
    let text = big_file(SECTIONS);
    let parsed = specengine_core::parse("big.md", text.as_bytes(), &x_scheme());
    assert_eq!(parsed.sections().len(), SECTIONS);
    assert!(
        parsed.links.len() > SECTIONS,
        "{} links",
        parsed.links.len()
    );
    let bytes = text.as_bytes();
    let (elapsed, done) = timed_inputs(&parsed, bytes, BOUND);
    eprintln!(
        "norm_input of {done} of {} nodes ({} bytes): {elapsed:?}",
        parsed.nodes.len(),
        bytes.len()
    );
    assert!(
        done == parsed.nodes.len() && elapsed <= BOUND,
        "norm_input of {done} of {} nodes took {elapsed:?} (> {BOUND:?})",
        parsed.nodes.len()
    );
    let (links, doc) = decode(&norm_input(&parsed, bytes, 0).unwrap());
    assert_eq!(links, ["depends_on ref:X-00001", "depends_on ref:X-00002"]);
    assert_eq!(doc, "Intro.");
    let (links, last) = decode(&norm_input(&parsed, bytes, SECTIONS).unwrap());
    assert!(links.is_empty());
    assert_eq!(last, format!("word{SECTIONS} Y-01"));
}

/// TE-11, the growth: per-node cost independent of the file's size.
#[test]
fn te11_twice_the_sections_at_most_three_times_the_time() {
    let best = |sections: usize| {
        let text = big_file(sections);
        let parsed = specengine_core::parse("big.md", text.as_bytes(), &x_scheme());
        (0..3)
            .map(|_| timed_inputs(&parsed, text.as_bytes(), Duration::from_secs(60)).0)
            .min()
            .unwrap()
    };
    let half = best(25_000);
    let full = best(50_000);
    let ratio = full.as_secs_f64() / half.as_secs_f64().max(1e-9);
    eprintln!("norm_input: 25 000 sections {half:?}, 50 000 sections {full:?}, ratio {ratio:.2}");
    // Below 50 ms for the whole file the ratio is noise.
    assert!(
        full < Duration::from_millis(50) || ratio < 3.0,
        "twice the sections took {ratio:.2} times as long ({half:?} -> {full:?}): \
         the cost per node grows with the file"
    );
}
