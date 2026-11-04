//! docs/features/spec-ids.md (RM-0073), core's half.
//! "Data", "The constant and the side": `HUB_ID_START` is
//! `HUB_PROPOSAL_FLOOR`'s value, 100001; `IdSide::holds` and `IdSide::next`
//! split the numbers at it.
//! "Acceptance criteria" AC-3 (a): `highest_record_number` counts only the
//! side's numbers, defined or in `aliases:`, of the prefix and its
//! `aliases_from`.
//! AC-6: `check::run` over the examples of "Data", "`id-width`", gives
//! exactly the findings listed there.
//!
//! Mutations (docs/features/spec-ids.md "Acceptance criteria"): AC-3's M2
//! (the terminal's maximum counts numbers from 100001); AC-6's M10 (the rule
//! unchanged: `DEC-100001` reported), M11 (every over-wide number passes:
//! `R-123`), M15 (the value alone exempts: `R-0100001` passes), M16 (`>` for
//! `>=`: `DEC-100001` reported; a lower start: `DEC-100000` passes).
//!
//! Real parser and check, in memory; spec-a's scheme read from its own
//! `specengine.toml` (the fixture is only read).

mod common;

use std::collections::BTreeSet;

use common::check::{Config, show, with_code};
use common::{corpus_scheme, fixture};
use specengine_core::proposal::HUB_PROPOSAL_FLOOR;
use specengine_core::record::{HUB_ID_START, IdSide, highest_record_number, record_id};
use specengine_model::ParsedFile;

// ------------------------------------------------------------ the constant

/// docs/features/spec-ids.md "Data", "The constant and the side": one
/// start, the hub's proposal floor's value; `holds` splits at it, 0 on the
/// terminal's side; `next` gives the hub at least the start, the terminal
/// up to 100000 and then nothing, `None` on overflow.
#[test]
fn the_start_is_the_floor_and_the_side_splits_the_numbers_at_it() {
    assert_eq!(HUB_ID_START, HUB_PROPOSAL_FLOOR);
    assert_eq!(HUB_ID_START, 100_001);

    for number in [0, 1, 24, HUB_ID_START - 1] {
        assert!(IdSide::Terminal.holds(number), "terminal {number}");
        assert!(!IdSide::Hub.holds(number), "hub {number}");
    }
    for number in [HUB_ID_START, HUB_ID_START + 1, u64::MAX] {
        assert!(IdSide::Hub.holds(number), "hub {number}");
        assert!(!IdSide::Terminal.holds(number), "terminal {number}");
    }

    // AC-3 (a): Hub 0 -> 100001, Terminal 99999 -> 100000, Terminal
    // 100000 -> None.
    assert_eq!(IdSide::Hub.next(0), Some(HUB_ID_START));
    assert_eq!(IdSide::Terminal.next(99_999), Some(100_000));
    assert_eq!(IdSide::Terminal.next(100_000), None);
    // The rest of "Data": a hub's next after its own highest; a terminal's
    // from 1; a hub's highest below the start still gives the start;
    // overflow none.
    assert_eq!(IdSide::Hub.next(HUB_ID_START), Some(HUB_ID_START + 1));
    assert_eq!(IdSide::Hub.next(100_007), Some(100_008));
    assert_eq!(IdSide::Hub.next(24), Some(HUB_ID_START));
    assert_eq!(IdSide::Terminal.next(0), Some(1));
    assert_eq!(IdSide::Terminal.next(24), Some(25));
    assert_eq!(IdSide::Terminal.next(HUB_ID_START), None);
    assert_eq!(IdSide::Hub.next(u64::MAX), None);
    assert_eq!(IdSide::Hub.next(u64::MAX - 1), Some(u64::MAX));

    // "Data": a prefix wider than six digits pads the hub's numbers, one
    // with no `width` writes them unpadded (`record_id`, unchanged).
    assert_eq!(record_id("X", 8, HUB_ID_START), "X-00100001");
    assert_eq!(record_id("X", 1, HUB_ID_START), "X-100001");
    assert_eq!(record_id("R", 2, HUB_ID_START), "R-100001");
}

// ------------------------------------------------------------------ AC-3 (a)

/// docs/features/spec-ids.md "Acceptance criteria" AC-3 (a): over files
/// holding `DEC-0024`, `DEC-100003` and `aliases: [QST-100002]` (spec-a's
/// `Q` aliases `QST`), `DEC` is 24 for `Terminal` and 100003 for `Hub`, `Q`
/// 0 and 100002. A section's `{#ID}` and a document's `id:` both count. M2:
/// the terminal's maximum counts numbers from 100001.
#[test]
fn ac3a_the_highest_record_number_counts_the_sides_numbers_alone() {
    let scheme = corpus_scheme(&fixture("spec-a"));
    let parse = |path: &str, text: &str| specengine_core::parse(path, text.as_bytes(), &scheme);
    let files: Vec<ParsedFile> = vec![
        parse(
            "docs/records/DEC/DEC-0024.md",
            "---\nid: DEC-0024\nclass: decision\nstatus: accepted\n---\n\n# Low\n\nA body.\n",
        ),
        parse(
            "docs/records/DEC/DEC-100003.md",
            "---\nid: DEC-100003\nclass: decision\nstatus: accepted\naliases: [QST-100002]\n\
             ---\n\n# High\n\nA body.\n",
        ),
    ];
    let dec = scheme.prefix("DEC").expect("spec-a's DEC").clone();
    let q = scheme.prefix("Q").expect("spec-a's Q").clone();
    assert_eq!(q.aliases_from, ["QST"], "the fixture's premise");
    assert_eq!(highest_record_number(&files, &dec, IdSide::Terminal), 24);
    assert_eq!(highest_record_number(&files, &dec, IdSide::Hub), 100_003);
    assert_eq!(highest_record_number(&files, &q, IdSide::Terminal), 0);
    assert_eq!(highest_record_number(&files, &q, IdSide::Hub), 100_002);

    // Sections count too, on their side.
    let sections = vec![parse(
        "docs/spec/notes.md",
        "# Notes\n\n## Low {#DEC-0031}\n\nA.\n\n## High {#DEC-100009}\n\nB.\n",
    )];
    assert_eq!(highest_record_number(&sections, &dec, IdSide::Terminal), 31);
    assert_eq!(highest_record_number(&sections, &dec, IdSide::Hub), 100_009);

    // Over spec-a's own walk (no number from 100001): the terminal's as
    // before, the hub's none.
    let walked: Vec<ParsedFile> = common::walked_md_files(&fixture("spec-a"))
        .iter()
        .map(|(path, bytes)| specengine_core::parse(path, bytes, &scheme))
        .collect();
    assert_eq!(highest_record_number(&walked, &dec, IdSide::Terminal), 23);
    assert_eq!(highest_record_number(&walked, &dec, IdSide::Hub), 0);
    assert_eq!(highest_record_number(&walked, &q, IdSide::Terminal), 32);
    assert_eq!(highest_record_number(&walked, &q, IdSide::Hub), 0);
}

// ------------------------------------------------------------------ AC-6

/// The three widths of docs/features/spec-ids.md "Data", "`id-width`":
/// `DEC` 4, `R` 2, `X` 8.
const WIDTHS: &str = "\
[ids]
DEC = { kind = \"decision\", width = 4 }
R   = { kind = \"requirement\", width = 2 }
X   = { kind = \"extra\", width = 8 }
";

/// docs/features/spec-ids.md "Acceptance criteria" AC-6: `check::run` over
/// the examples of "Data", "`id-width`" gives exactly the findings listed
/// there: none on `DEC-100001`, `R-100001`, `X-00100001`; `id-width` on
/// `DEC-100000`, `R-123`, `R-7`, `R-0100001`, `X-100001`. Each as a
/// section's `{#ID}` and as a document's `id:`, at its line. M10, M11, M15,
/// M16.
#[test]
fn ac6_id_width_passes_a_record_id_form_number_from_the_start_alone() {
    let config = Config::from_toml(WIDTHS);
    let passing = ["DEC-100001", "R-100001", "X-00100001"];
    let reported = ["DEC-100000", "R-123", "R-7", "R-0100001", "X-100001"];
    let all: Vec<&str> = passing.iter().chain(reported.iter()).copied().collect();

    // As sections of one document: line 3 + 4 * index.
    let mut sections = String::from("# Examples\n");
    for id in &all {
        sections.push_str(&format!("\n## Example {{#{id}}}\n\nA body.\n"));
    }
    let report = config.check(&[("docs/spec/examples.md", sections.as_str())]);
    let found: BTreeSet<(usize, String)> = with_code(&report, "id-width")
        .iter()
        .map(|finding| {
            assert_eq!(finding.path, "docs/spec/examples.md", "{}", show(&report));
            (finding.line, finding.subject.clone())
        })
        .collect();
    let want: BTreeSet<(usize, String)> = all
        .iter()
        .enumerate()
        .filter(|(_, id)| reported.contains(id))
        .map(|(index, id)| (3 + 4 * index, (*id).to_owned()))
        .collect();
    assert_eq!(found, want, "{}", show(&report));

    // As documents' `id:`, one file each, line 2.
    let texts: Vec<(String, String)> = all
        .iter()
        .map(|id| {
            (
                format!("docs/spec/{}.md", id.to_ascii_lowercase()),
                format!("---\nid: {id}\n---\n# Example\n\nA body.\n"),
            )
        })
        .collect();
    let files: Vec<(&str, &str)> = texts
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let report = config.check(&files);
    let found: BTreeSet<(String, usize, String)> = with_code(&report, "id-width")
        .iter()
        .map(|finding| (finding.path.clone(), finding.line, finding.subject.clone()))
        .collect();
    let want: BTreeSet<(String, usize, String)> = reported
        .iter()
        .map(|id| {
            (
                format!("docs/spec/{}.md", id.to_ascii_lowercase()),
                2,
                (*id).to_owned(),
            )
        })
        .collect();
    assert_eq!(found, want, "{}", show(&report));
}

/// docs/features/spec-ids.md "Data", "`id-width`" and I6: the exemption is
/// the rule's, on any machine (the check takes no side), and only for a
/// number written as `record_id` writes it: a width at or above six digits
/// never needs it, and a reference is never judged. M15: the value alone.
#[test]
fn the_exemption_is_the_record_id_form_and_the_same_everywhere() {
    let config = Config::from_toml(
        "[ids]\nDEC = { kind = \"decision\", width = 4 }\nW = { kind = \"wide\", width = 6 }\n",
    );
    let text = "# Examples\n\n\
                ## A {#DEC-100002}\n\nSee DEC-100000 and DEC-0100002.\n\n\
                ## B {#DEC-00100003}\n\nA body.\n\n\
                ## C {#DEC-18446744073709551615}\n\nA body.\n\n\
                ## D {#W-100001}\n\nA body.\n\n\
                ## E {#W-0100002}\n\nA body.\n";
    let report = config.check(&[("docs/spec/more.md", text)]);
    let found: BTreeSet<String> = with_code(&report, "id-width")
        .iter()
        .map(|finding| finding.subject.clone())
        .collect();
    // `DEC-00100003`: a zero past the padding; `W-0100002`: seven digits
    // under a width of six. `DEC-18446744073709551615` is `u64::MAX`
    // written as `record_id` writes it.
    let want: BTreeSet<String> = ["DEC-00100003", "W-0100002"]
        .iter()
        .map(|id| (*id).to_owned())
        .collect();
    assert_eq!(found, want, "{}", show(&report));
    // The same input, the same report: one state, one result.
    let again = config.check(&[("docs/spec/more.md", text)]);
    assert_eq!(show(&again), show(&report));
}
