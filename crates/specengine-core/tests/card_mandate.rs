//! docs/features/task-mandate.md (RM-0104, v5 step 6d, ADR-0079)
//! "Acceptance criteria", the pure half: model's `Mandate` and core's three
//! refusal rules, the canon docs/canon/card-mandate.md "The rule".
//!
//! - M1 (the core part): `Mandate::parse` reads `binding` and `open` exactly
//!   and refuses `Binding`, ` open`, `-` and the empty string;
//!   `mandate_problem(Some(Binding), None)` is the first line of the
//!   messages, `(Some(Open), Some("Ana Ruiz"))` none, no mandate none.
//! - M3 (the core part): `mandate_change_problem` refuses a card with no
//!   reviewer and a name other than the stored reviewer, compared exactly,
//!   with the two lines; the reviewer passes.
//! - M2 (the core part): `split_mandates` pairs each `D=M` with its child by
//!   discipline, not by position; a child named by no entry holds none; the
//!   four refusals with their lines. M: mandates paired by position; a child
//!   without one taking its neighbour's.
//!
//! The probe words are the spec's own (a lighthouse game's `art`, `sound`,
//! `narrative`; Ana Ruiz the reviewer, Ben Ode another person). Every
//! expected value is worked out from the spec, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

use specengine_core::card::{
    MANDATE_ENTRY_ECHO_MAX, mandate_change_problem, mandate_problem, split_mandates,
};
use specengine_core::card_toml::name_problem;
use specengine_model::Mandate;

/// The reviewer of every probe.
const ANA: &str = "Ana Ruiz";

/// Another person.
const BEN: &str = "Ben Ode";

/// The first line of the messages: a mandate with no reviewer.
const NO_REVIEWER: &str =
    "mandate: a mandate goes with a reviewer, who alone changes it: name one with --reviewer";

fn strings(written: &[&str]) -> Vec<String> {
    written.iter().map(|text| (*text).to_owned()).collect()
}

// ------------------------------------------------------------------ M1

/// M1: `Mandate::parse` takes exactly the two stored words; `as_str` gives
/// them back; serde writes and reads them lower-case.
#[test]
fn m1_parse_reads_the_two_words_exactly() {
    assert_eq!(Mandate::parse("binding"), Some(Mandate::Binding));
    assert_eq!(Mandate::parse("open"), Some(Mandate::Open));
    for written in [
        "Binding", " open", "open ", "-", "", "OPEN", "bind", "null", "none",
    ] {
        assert_eq!(Mandate::parse(written), None, "{written:?}");
    }
    assert_eq!(Mandate::Binding.as_str(), "binding");
    assert_eq!(Mandate::Open.as_str(), "open");
    assert_eq!(
        serde_json::to_string(&Mandate::Binding).expect("serialise"),
        "\"binding\""
    );
    assert_eq!(
        serde_json::to_string(&Mandate::Open).expect("serialise"),
        "\"open\""
    );
    assert_eq!(
        serde_json::from_str::<Mandate>("\"open\"").expect("deserialise"),
        Mandate::Open
    );
    assert!(serde_json::from_str::<Mandate>("\"Open\"").is_err());
    assert!(serde_json::from_str::<Mandate>("\"maybe\"").is_err());
}

/// M1: a mandate needs a reviewer; no mandate needs nothing.
#[test]
fn m1_mandate_problem_asks_for_a_reviewer() {
    assert_eq!(
        mandate_problem(Some(Mandate::Binding), None).as_deref(),
        Some(NO_REVIEWER)
    );
    assert_eq!(
        mandate_problem(Some(Mandate::Open), None).as_deref(),
        Some(NO_REVIEWER)
    );
    assert_eq!(mandate_problem(Some(Mandate::Open), Some(ANA)), None);
    assert_eq!(mandate_problem(Some(Mandate::Binding), Some(BEN)), None);
    assert_eq!(mandate_problem(None, None), None);
    assert_eq!(mandate_problem(None, Some(ANA)), None);
}

// ------------------------------------------------------------------ M3

/// M3: only the stored reviewer changes the mandate, compared exactly; no
/// reviewer, nobody.
#[test]
fn m3_mandate_change_problem_is_the_reviewers_alone() {
    assert_eq!(
        mandate_change_problem("C-0007", None, BEN).as_deref(),
        Some(
            "mandate: C-0007 has no reviewer, who alone changes its mandate: name one with `spec \
             card assign C-0007 --reviewer N` first; nothing changed"
        )
    );
    assert_eq!(
        mandate_change_problem("C-0007", Some(ANA), BEN).as_deref(),
        Some(
            "mandate: only C-0007's reviewer `Ana Ruiz` changes its mandate, not `Ben Ode`; \
             nothing changed"
        )
    );
    assert_eq!(mandate_change_problem("C-0007", Some(ANA), ANA), None);
    // Compared exactly, as `accept` compares one: no case fold, no trim.
    for actor in ["ana ruiz", " Ana Ruiz", "Ana Ruiz ", "Ana  Ruiz"] {
        assert_eq!(
            mandate_change_problem("C-0002", Some(ANA), actor),
            Some(format!(
                "mandate: only C-0002's reviewer `Ana Ruiz` changes its mandate, not `{actor}`; \
                 nothing changed"
            )),
            "{actor:?}"
        );
    }
    // `agent` and `local` are never a reviewer; the card's ID is named.
    for actor in ["agent", "local"] {
        assert_eq!(
            mandate_change_problem("C-0012", Some(ANA), actor),
            Some(format!(
                "mandate: only C-0012's reviewer `Ana Ruiz` changes its mandate, not `{actor}`; \
                 nothing changed"
            ))
        );
    }
}

// ------------------------------------------------------------------ M2

/// M2: paired by discipline, in the children's order; the criterion's
/// command names `sound` before `art`, the children are `art`, `sound`. M:
/// paired by position (this reads `[open, binding]`).
#[test]
fn m2_split_mandates_pair_by_discipline() {
    let children = strings(&["art", "sound"]);
    assert_eq!(
        split_mandates(&strings(&["sound=open", "art=binding"]), &children),
        Ok(vec![Some(Mandate::Binding), Some(Mandate::Open)])
    );
    assert_eq!(
        split_mandates(&strings(&["art=binding", "sound=open"]), &children),
        Ok(vec![Some(Mandate::Binding), Some(Mandate::Open)])
    );
    assert_eq!(split_mandates(&[], &children), Ok(vec![None, None]));
    assert_eq!(split_mandates(&[], &[]), Ok(Vec::new()));
}

/// M2: a child named by no `--mandate` holds none, wherever it stands. M: a
/// child without one taking its neighbour's.
#[test]
fn m2_a_child_named_by_none_holds_none() {
    let children = strings(&["art", "narrative", "sound"]);
    assert_eq!(
        split_mandates(&strings(&["sound=open"]), &children),
        Ok(vec![None, None, Some(Mandate::Open)])
    );
    assert_eq!(
        split_mandates(&strings(&["art=binding"]), &children),
        Ok(vec![Some(Mandate::Binding), None, None])
    );
    assert_eq!(
        split_mandates(&strings(&["narrative=open"]), &children),
        Ok(vec![None, Some(Mandate::Open), None])
    );
    assert_eq!(
        split_mandates(&strings(&["sound=binding", "art=open"]), &children),
        Ok(vec![Some(Mandate::Open), None, Some(Mandate::Binding)])
    );
}

/// M2: each refusal with its line of the messages.
#[test]
fn m2_split_mandates_refuse_with_their_lines() {
    let children = strings(&["art", "sound"]);
    let refusal = |given: &[&str]| -> String {
        match split_mandates(&strings(given), &children) {
            Err(message) => message,
            Ok(read) => panic!("{given:?} read as {read:?}"),
        }
    };
    // An entry off `D=M`.
    assert_eq!(
        refusal(&["sound"]),
        "mandate: `sound` is not `<discipline>=binding` or `<discipline>=open`"
    );
    assert_eq!(
        refusal(&["art"]),
        "mandate: `art` is not `<discipline>=binding` or `<discipline>=open`"
    );
    assert_eq!(
        refusal(&["art=maybe"]),
        "mandate: `art=maybe` is not `<discipline>=binding` or `<discipline>=open`"
    );
    assert_eq!(
        refusal(&["art=Binding"]),
        "mandate: `art=Binding` is not `<discipline>=binding` or `<discipline>=open`"
    );
    // `D` off the name grammar: `mandate: ` then `name_problem`'s text.
    let grammar = name_problem("Art").expect("`Art` is no name");
    assert_eq!(refusal(&["Art=open"]), format!("mandate: {grammar}"));
    // `D` no `--child` of this split.
    assert_eq!(
        refusal(&["voice=open"]),
        "mandate: `voice` is no --child of this split"
    );
    // `D` twice, even with the same value.
    assert_eq!(
        refusal(&["art=open", "art=binding"]),
        "mandate: `art` is named twice"
    );
    assert_eq!(
        refusal(&["art=open", "sound=open", "art=open"]),
        "mandate: `art` is named twice"
    );
}

/// M2, the echo of an entry off `D=M` (the review's nit): on one line, CRLF,
/// LF and CR each one space; at most `MANDATE_ENTRY_ECHO_MAX` (72) bytes, a
/// longer entry cut to a 69-byte prefix on a char boundary and `\u{2026}`;
/// an entry of 72 bytes echoed whole. M: the entry echoed as given (a line
/// break in the refusal, or all 100 bytes).
#[test]
fn m2_an_entry_off_the_form_is_echoed_on_one_line_and_cut() {
    assert_eq!(MANDATE_ENTRY_ECHO_MAX, 72);
    let children = strings(&["art", "sound"]);
    let refusal = |entry: &str| -> String {
        match split_mandates(&[entry.to_owned()], &children) {
            Err(message) => message,
            Ok(read) => panic!("{entry:?} read as {read:?}"),
        }
    };
    let line = |shown: &str| {
        format!("mandate: `{shown}` is not `<discipline>=binding` or `<discipline>=open`")
    };
    // Off the form: no `=`, or a word that is no mandate.
    assert_eq!(refusal("art\r\nbinding"), line("art binding"));
    assert_eq!(refusal("art\nbinding"), line("art binding"));
    assert_eq!(refusal("art\rbinding"), line("art binding"));
    assert_eq!(refusal("art=open\r\n"), line("art=open "));
    assert_eq!(refusal("a\r\n\nb\rc"), line("a  b c"));
    assert!(!refusal("x\ny\r\nz").contains(['\n', '\r']));

    let long = "a".repeat(100);
    assert_eq!(refusal(&long), line(&format!("{}\u{2026}", "a".repeat(69))));
    let exact = "b".repeat(72);
    assert_eq!(refusal(&exact), line(&exact), "72 bytes echoed whole");
    let over = "c".repeat(73);
    assert_eq!(refusal(&over), line(&format!("{}\u{2026}", "c".repeat(69))));
    // The cut on a char boundary: a two-byte letter across byte 69.
    let wide = format!("{}\u{e9}{}", "d".repeat(68), "d".repeat(10));
    assert_eq!(
        refusal(&wide),
        line(&format!("{}\u{2026}", "d".repeat(68))),
        "never a split letter"
    );
    // Folded first, then cut: 50 CRLF, 100 bytes as given, are 50 spaces,
    // under the cap.
    let folded = "\r\n".repeat(50);
    assert_eq!(refusal(&folded), line(&" ".repeat(50)));
}
