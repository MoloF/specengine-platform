//! docs/features/proposal-apply.md, core's pure half: `patch` (locate, the
//! span's bytes, the text as spliced, the structure check, the creation
//! refusals, the target forms) and `proposal` (the `PR-NNNN` grammar and
//! its look-alikes, the `[ids]` clash, the author, the commit message, the
//! patch-hash input, the clock's time stamps). Each "Rules" case is checked
//! on a file of spec-a's or spec-b's scheme.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use common::{corpus_scheme, fixture};
use specengine_core::patch::{
    LocateError, Refusal, StructureError, TargetForm, check_structure, locate, span_bytes, splice,
    structure, target_form, update, update_refusal, update_text,
};
use specengine_core::proposal::{
    Author, CommitFacts, ProposalIdError, commit_message, is_utc_timestamp, parse_proposal_id,
    patch_hash_input, prefix_clash, proposal_id, proposal_number, utc_timestamp,
};
use specengine_model::{IdScheme, Span, grammar};

const FILE: &str = "---\nid: MEC-SPRINT\nclass: canon\n---\n\n# Sprint\n\nIntro.\n\n\
## Cost {#RULE-SPRINT-COST}\n\nCosts 12.\n\n### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds.\n\n\
## Notes\n\nPlain.\n";

fn spec_a() -> IdScheme {
    corpus_scheme(&fixture("spec-a"))
}

fn parse(text: &str, scheme: &IdScheme) -> specengine_model::ParsedFile {
    specengine_core::parse("docs/spec/sprint.md", text.as_bytes(), scheme)
}

#[test]
fn locate_spans_and_the_text_as_spliced() {
    let scheme = spec_a();
    let parsed = parse(FILE, &scheme);
    let cost = locate(&parsed, "RULE-SPRINT-COST").unwrap();
    let empty = locate(&parsed, "EDGE-SPRINT-EMPTY").unwrap();
    assert_eq!(locate(&parsed, "MEC-SPRINT").unwrap(), 0);
    assert_eq!(locate(&parsed, "RULE-NOPE"), Err(LocateError::Absent));
    // A section's span holds its subsections and ends before trailing
    // whitespace; a document's is the whole file.
    let span = span_bytes(FILE.as_bytes(), &parsed.nodes[cost]);
    assert_eq!(
        std::str::from_utf8(span).unwrap(),
        "## Cost {#RULE-SPRINT-COST}\n\nCosts 12.\n\n### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds."
    );
    assert_eq!(
        span_bytes(FILE.as_bytes(), &parsed.nodes[empty]),
        b"### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds."
    );
    assert_eq!(
        span_bytes(FILE.as_bytes(), &parsed.nodes[0]),
        FILE.as_bytes()
    );
    let twice = format!("{FILE}\n## Again {{#RULE-SPRINT-COST}}\n\nX.\n");
    assert_eq!(
        locate(&parse(&twice, &scheme), "RULE-SPRINT-COST"),
        Err(LocateError::Repeated(2))
    );

    assert_eq!(update_text("text \t\r\n\n", true), "text");
    assert_eq!(update_text("text \t\r\n\n", false), "text \t\r\n\n");
    assert_eq!(update_text("  lead\n", true), "  lead");
    assert_eq!(splice(b"abcdef", Span::new(2, 4), "XYZ"), b"abXYZef");
    assert_eq!(splice(b"abc", Span::new(1, 99), "Z"), b"aZ", "clamped");
}

#[test]
fn update_replaces_exactly_the_span_and_checks_the_structure() {
    let scheme = spec_a();
    let parsed = parse(FILE, &scheme);
    let ord = locate(&parsed, "EDGE-SPRINT-EMPTY").unwrap();
    let done = update(
        "docs/spec/sprint.md",
        FILE.as_bytes(),
        &parsed,
        ord,
        "### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds at once.\n\n\n",
        &scheme,
    )
    .unwrap();
    assert_eq!(done.text, "### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds at once.");
    assert_eq!(
        String::from_utf8(done.bytes.clone()).unwrap(),
        FILE.replace("\n\nEnds.\n", "\n\nEnds at once.\n")
    );
    assert_eq!(structure(&done.parsed), structure(&parsed));
    assert_eq!(done.ord, ord);

    // Each structure change is refused.
    for (label, text) in [
        ("{#ID} dropped", "### Empty\n\nEnds."),
        (
            "the level changed",
            "#### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds.",
        ),
        (
            "an ID section added",
            "### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds.\n\n#### More {#EDGE-SPRINT-MORE}\n\nX.",
        ),
        ("the ID renamed", "### Empty {#EDGE-SPRINT-VOID}\n\nEnds."),
    ] {
        match update(
            "docs/spec/sprint.md",
            FILE.as_bytes(),
            &parsed,
            ord,
            text,
            &scheme,
        ) {
            Err(StructureError::Changed { .. }) => {}
            other => panic!("{label}: {other:?}"),
        }
    }
    for (label, text) in [
        (
            "a same-level heading",
            "### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds.\n\n### Other\n\nX.",
        ),
        (
            "a higher-level heading",
            "### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds.\n\n## Other\n\nX.",
        ),
    ] {
        match update(
            "docs/spec/sprint.md",
            FILE.as_bytes(),
            &parsed,
            ord,
            text,
            &scheme,
        ) {
            Err(StructureError::SpanDiffers { .. }) => {}
            other => panic!("{label}: {other:?}"),
        }
    }
    // A lower-level heading without an ID stays inside the span.
    assert!(
        update(
            "docs/spec/sprint.md",
            FILE.as_bytes(),
            &parsed,
            ord,
            "### Empty {#EDGE-SPRINT-EMPTY}\n\nEnds.\n\n#### Detail\n\nX.",
            &scheme,
        )
        .is_ok()
    );
    // The document itself: verbatim, trailing whitespace kept.
    let whole = update(
        "docs/spec/sprint.md",
        FILE.as_bytes(),
        &parsed,
        0,
        &FILE.replace("Intro.", "Intro, longer."),
        &scheme,
    )
    .unwrap();
    assert!(whole.text.ends_with("Plain.\n"));
    let error =
        check_structure(&parsed, &parse("# Other\n", &scheme), 0, Span::new(0, 8)).unwrap_err();
    assert!(error.to_string().contains("structure"), "{error}");
}

#[test]
fn generated_and_immutable_targets_are_refused() {
    let scheme = spec_a();
    let generated = parse(
        "---\nid: MEC-X\nclass: generated\n---\n\n# X\n\n## A {#RULE-X}\n\nY.\n",
        &scheme,
    );
    assert_eq!(
        update_refusal(&generated, 0, &scheme),
        Some(Refusal::Generated)
    );
    assert_eq!(
        update_refusal(&generated, 1, &scheme),
        Some(Refusal::Generated)
    );
    let record = parse(
        "---\nid: R-12\nclass: canon\n---\n\n# R\n\n## Part {#RULE-PART}\n\nY.\n",
        &scheme,
    );
    assert!(matches!(
        update_refusal(&record, 0, &scheme),
        Some(Refusal::ImmutableText { ref prefix, .. }) if prefix == "R"
    ));
    // A section of an immutable record: its document's prefix decides.
    assert!(matches!(
        update_refusal(&record, 1, &scheme),
        Some(Refusal::ImmutableText { ref id, .. }) if id == "R-12"
    ));
    let canon = parse(FILE, &scheme);
    for ord in 0..canon.nodes.len() {
        assert_eq!(update_refusal(&canon, ord, &scheme), None);
    }
}

#[test]
fn target_forms_name_the_canonical_id() {
    let scheme = spec_a();
    let form = |written: &str| {
        let found = grammar::parse_reference(written, 0, &scheme)
            .unwrap_or_else(|| panic!("{written} parses"));
        target_form(&found.reference)
    };
    assert_eq!(form("EDGE-SPRINT-EMPTY"), TargetForm::Canonical);
    assert_eq!(form("stamina-tuning/AC-07"), TargetForm::Canonical);
    assert_eq!(
        form("QST-031"),
        TargetForm::Alias {
            canonical: "Q-031".to_owned()
        }
    );
    assert_eq!(
        form("MEC-SPRINT#EDGE-SPRINT-EMPTY"),
        TargetForm::Section {
            section: "EDGE-SPRINT-EMPTY".to_owned()
        }
    );
    assert_eq!(
        form("R-12@2"),
        TargetForm::Revision {
            canonical: "R-12".to_owned()
        }
    );
    assert_eq!(
        form("[[MEC-SPRINT]]"),
        TargetForm::Wiki {
            canonical: "MEC-SPRINT".to_owned()
        }
    );
    assert_eq!(form("shared:DEC-0023"), TargetForm::Project);
}

#[test]
fn proposal_ids_and_their_look_alikes() {
    assert_eq!(proposal_id(1), "PR-0001");
    assert_eq!(proposal_id(10_000), "PR-10000");
    assert_eq!(proposal_number("PR-0001"), Some(1));
    assert_eq!(proposal_number("PR-10000"), Some(10_000));
    for not in [
        "PR-1", "PR-00001", "pr-0001", "PR-0001 ", "PR0001", "PR-", "PR-00a1", "QR-0001",
    ] {
        assert_eq!(proposal_number(not), None, "{not}");
    }
    assert_eq!(parse_proposal_id(" PR-0042\n"), Ok("PR-0042".to_owned()));
    for look_alike in [
        "\u{0420}R-0001",
        "\u{03a1}R-0001",
        "\u{ff30}\u{ff32}\u{ff0d}\u{ff10}\u{ff10}\u{ff10}\u{ff11}",
    ] {
        assert_eq!(
            parse_proposal_id(look_alike),
            Err(ProposalIdError::LookAlike {
                fix: "PR-0001".to_owned()
            }),
            "{look_alike}"
        );
    }
    // The Cyrillic Er stands for `P`, so in place of `R` it reads
    // `PP-0001`: no proposal ID at all.
    assert_eq!(
        parse_proposal_id("P\u{0420}-0001"),
        Err(ProposalIdError::NotAnId)
    );
    assert_eq!(parse_proposal_id("PR-1"), Err(ProposalIdError::NotAnId));
    assert_eq!(parse_proposal_id("R-12"), Err(ProposalIdError::NotAnId));
}

#[test]
fn pr_in_ids_clashes_as_a_prefix_or_an_alias() {
    assert_eq!(prefix_clash(&spec_a()), None);
    assert_eq!(prefix_clash(&corpus_scheme(&fixture("spec-b"))), None);
    let scheme = |toml: &str| {
        specengine_core::ProjectConfig::from_toml(toml)
            .expect("a valid config")
            .scheme
    };
    for toml in [
        "[ids]\nPR = { kind = \"pull\", width = 4 }\n",
        "[ids]\nPULL = { kind = \"pull\", width = 4, aliases_from = [\"PR\"] }\n",
        "[ids]\nPULL = { kind = \"pull\", width = 4, aliases_from = [\"\u{0420}R\"] }\n",
        "[ids]\nPULL = { kind = \"pull\", width = 4, aliases_from = [\"\u{0420}\u{0420}\", \"\u{0420}R\"] }\n",
    ] {
        let found = prefix_clash(&scheme(toml));
        assert!(
            found.as_deref().is_some_and(|text| text.contains("PR")),
            "{toml}: {found:?}"
        );
    }
    assert_eq!(
        prefix_clash(&scheme(
            "[ids]\nPRJ = { kind = \"project\", width = 4, aliases_from = [\"PRX\"] }\n"
        )),
        None
    );
}

#[test]
fn authors_commit_messages_hashes_and_time_stamps() {
    assert_eq!(Author::new(None, None, None).unwrap(), Author::human());
    let agent = Author::new(None, None, Some("run-1".to_owned())).unwrap();
    assert_eq!(
        agent.provenance(),
        "agent role=unknown model=unknown run=run-1"
    );
    for bad in ["", "two words", "caf\u{e9}", "tab\there"] {
        let error = Author::new(Some(bad.to_owned()), None, None).unwrap_err();
        assert!(error.starts_with("--author-role: "), "{bad:?}: {error}");
    }
    assert!(Author::new(None, Some("m".repeat(128)), None).is_ok());
    assert!(Author::new(None, Some("m".repeat(129)), None).is_err());
    assert_eq!(
        serde_json::to_value(&agent).unwrap(),
        serde_json::json!({"type": "agent", "role": null, "model": null, "run": "run-1"})
    );

    let message = commit_message(&CommitFacts {
        id: "PR-0001",
        rationale: "Line one.\n\n# kept",
        decided_by: "Ann Owner <ann@example.org>",
        author: &Author::new(
            Some("spec-writer".to_owned()),
            Some("claude-opus-5-5".to_owned()),
            None,
        )
        .unwrap(),
        base_commit: "0123456789012345678901234567890123456789",
    });
    assert_eq!(
        message,
        "spec: apply PR-0001\n\nLine one.\n\n# kept\n\nProposal: PR-0001\n\
         Decided-by: Ann Owner <ann@example.org>\n\
         Proposed-by: agent role=spec-writer model=claude-opus-5-5 run=unknown\n\
         Base-commit: 0123456789012345678901234567890123456789\n"
    );
    assert_eq!(patch_hash_input("R-1", "b3:00", "x\n"), b"R-1\nb3:00\nx\n");

    assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00Z");
    assert_eq!(utc_timestamp(1_791_235_043), "2026-10-05T21:17:23Z");
    assert!(is_utc_timestamp("2026-10-05T21:14:03Z"));
    for bad in [
        "2026-10-05T24:00:00Z",
        "2026-02-30T00:00:00Z",
        "2026-10-05 21:14:03Z",
        "2026-10-05T21:14:03",
        "2026-10-05T21:14:03+00:00",
        "2026-10-05T21:60:00Z",
    ] {
        assert!(!is_utc_timestamp(bad), "{bad}");
    }
}
