//! docs/features/round.md, the pure half (core `round.rs`; canon
//! `docs/canon/round.md` "Order", "Sheet", "Answers"):
//!
//! - the bounds: `ROUND_LIMIT_DEFAULT` 20, `ROUND_LIMIT_MAX` 200,
//!   `ROUND_FILE_MAX_BYTES` 8 388 608, the reason cap 4 096, the answer
//!   cap the intake's 2 048;
//! - [`order_key`]: severity `high`, `normal`, `low`, none; waiting tasks,
//!   most first; `created_at`, oldest first; the ID's number (by value, not
//!   by text);
//! - [`reading_minutes`]: `ceil(c / 1000)`, 0 for none;
//! - [`parse_answers`]: the tokens of the grammar's table, any ASCII case
//!   for `ok` and `reject:`; a leading U+FEFF dropped, CR LF lines; blank,
//!   indented and quoted lines ignored; a column-0 line that reads as an
//!   answer line only up to case, a look-alike, an invisible prefix or its
//!   colon refused with its Latin form; duplicates (`?` counts, a blank
//!   slot does not); the caps refused, never cut; a character the queue
//!   escapes refused; `reject` without a reason; an ID or an option past
//!   a `u64`;
//! - [`answers_text`]: the line of the first byte that is not UTF-8.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

use specengine_core::intake::{ANSWER_MAX, IntakeSeverity};
use specengine_core::round::{
    AnswerLine, PROJECT_LINE, ProjectLine, ROUND_FILE_MAX_BYTES, ROUND_LIMIT_DEFAULT,
    ROUND_LIMIT_MAX, ROUND_REASON_MAX_BYTES, Refusal, Token, answers_text, order_key,
    parse_answers, reading_minutes,
};

/// `(line, id, answer, token)` of every answer line of `text`.
fn lines(text: &str) -> Vec<(usize, String, String, Result<Token, Refusal>)> {
    parse_answers(text)
        .answers
        .into_iter()
        .map(
            |AnswerLine {
                 line,
                 id,
                 answer,
                 token,
             }| (line, id, answer, token),
        )
        .collect()
}

/// The token of the one answer line of `project: p\n<line>`.
fn token_of(line: &str) -> Result<Token, Refusal> {
    let parsed = parse_answers(&format!("project: p\n{line}\n"));
    assert_eq!(parsed.answers.len(), 1, "{line:?}: {parsed:?}");
    parsed.answers[0].token.clone()
}

/// The refusal of a column-0 line that is no answer line.
fn not_an_answer_line(written: &str, latin: &str, look_alike: bool) -> Result<Token, Refusal> {
    Err(Refusal::NotAnAnswerLine {
        written: written.to_owned(),
        latin: latin.to_owned(),
        look_alike,
    })
}

#[test]
fn the_bounds_are_the_canon_s() {
    assert_eq!(ROUND_LIMIT_DEFAULT, 20);
    assert_eq!(ROUND_LIMIT_MAX, 200);
    assert_eq!(ROUND_FILE_MAX_BYTES, 8_388_608);
    assert_eq!(ROUND_REASON_MAX_BYTES, 4096);
    assert_eq!(ANSWER_MAX, 2048);
    assert_eq!(PROJECT_LINE, "project:");
}

#[test]
fn every_token_of_the_grammar_s_table() {
    let text = "project: game\n\
                PR-0001: ok\n\
                PR-0002: OK\n\
                PR-0003: Ok\n\
                PR-0004: 1\n\
                PR-0005: [2]\n\
                PR-0006: reject: dup\n\
                PR-0007: REJECT:  Dup reason  \n\
                PR-0008: Reject:x\n\
                PR-0009: ?\n\
                PR-0010:\n\
                PR-0011:   Regeneration waits 2 s, not 1.5  \n\
                PR-0012: [ 1 ]\n\
                PR-0013: ok.\n\
                PR-0014: 01\n\
                PR-0015: rejected\n\
                PR-0016: rejected : x\n\
                PR-0017: \u{ff11}\n\
                PR-0018:ok\n\
                PR-0019:: ok\n\
                PR-0020: []\n";
    let parsed = parse_answers(text);
    assert_eq!(
        parsed.projects,
        [ProjectLine {
            line: 1,
            slug: "game".to_owned()
        }]
    );
    let want: [(&str, &str, Token); 19] = [
        ("PR-0001", "ok", Token::Ok),
        ("PR-0002", "OK", Token::Ok),
        ("PR-0003", "Ok", Token::Ok),
        ("PR-0004", "1", Token::Option(1)),
        ("PR-0005", "[2]", Token::Option(2)),
        ("PR-0006", "reject: dup", Token::Reject("dup".to_owned())),
        (
            "PR-0007",
            "REJECT:  Dup reason",
            Token::Reject("Dup reason".to_owned()),
        ),
        ("PR-0008", "Reject:x", Token::Reject("x".to_owned())),
        ("PR-0009", "?", Token::Open),
        ("PR-0010", "", Token::Open),
        (
            "PR-0011",
            "Regeneration waits 2 s, not 1.5",
            Token::Text("Regeneration waits 2 s, not 1.5".to_owned()),
        ),
        // Deviation 5: option tokens are ASCII only.
        ("PR-0012", "[ 1 ]", Token::Text("[ 1 ]".to_owned())),
        ("PR-0013", "ok.", Token::Text("ok.".to_owned())),
        ("PR-0014", "01", Token::Option(1)),
        ("PR-0015", "rejected", Token::Text("rejected".to_owned())),
        (
            "PR-0016",
            "rejected : x",
            Token::Text("rejected : x".to_owned()),
        ),
        ("PR-0017", "\u{ff11}", Token::Text("\u{ff11}".to_owned())),
        ("PR-0018", "ok", Token::Ok),
        ("PR-0019", ": ok", Token::Text(": ok".to_owned())),
    ];
    let got = lines(text);
    assert_eq!(got.len(), want.len() + 1, "{got:?}");
    for (index, (id, answer, token)) in want.into_iter().enumerate() {
        assert_eq!(
            got[index],
            (index + 2, id.to_owned(), answer.to_owned(), Ok(token)),
            "line {}",
            index + 2
        );
    }
    assert_eq!(
        got[19],
        (
            21,
            "PR-0020".to_owned(),
            "[]".to_owned(),
            Ok(Token::Text("[]".to_owned()))
        )
    );
}

#[test]
fn reject_without_a_reason_is_refused_in_any_case() {
    for line in [
        "PR-0001: reject",
        "PR-0001: REJECT",
        "PR-0001: Reject",
        "PR-0001: reject:",
        "PR-0001: reject:   ",
        "PR-0001: rEjEcT:\t",
    ] {
        assert_eq!(
            token_of(line),
            Err(Refusal::RejectWithoutReason),
            "{line:?}"
        );
    }
}

#[test]
fn blank_indented_quoted_and_other_lines_are_ignored() {
    let text = "\n\
                \x20 PR-0001: ok\n\
                \tPR-0001: ok\n\
                > PR-0001: ok\n\
                >PR-0001: ok\n\
                \u{a0}PR-0001: ok\n\
                # Open questions: game\n\
                ## PR-0001 - question, high\n\
                Concerns: Stamina (MEC-STAMINA)\n\
                Staged 2026-10-07T17:49:50Z: spec approve PR-0001\n\
                PR-: ok\n\
                PRX-0001: ok\n\
                P-0001: ok\n\
                note: PR-0001: ok\n\
                \x20 project: other\n\
                > project: other\n\
                Project: other\n";
    let parsed = parse_answers(text);
    assert!(parsed.answers.is_empty(), "{:?}", parsed.answers);
    assert!(parsed.projects.is_empty(), "{:?}", parsed.projects);
}

#[test]
fn project_lines_are_column_0_and_trimmed() {
    let parsed = parse_answers("project:   spaced slug  \nproject:x\nproject:\n");
    assert_eq!(
        parsed.projects,
        [
            ProjectLine {
                line: 1,
                slug: "spaced slug".to_owned()
            },
            ProjectLine {
                line: 2,
                slug: "x".to_owned()
            },
            ProjectLine {
                line: 3,
                slug: String::new()
            },
        ]
    );
}

#[test]
fn a_leading_bom_is_dropped_and_crlf_lines_read_as_lf() {
    let parsed = parse_answers("\u{feff}project: game\r\nPR-0001: ok\r\nPR-0002: reject: a \r\n");
    assert_eq!(
        parsed.projects,
        [ProjectLine {
            line: 1,
            slug: "game".to_owned()
        }]
    );
    assert_eq!(
        lines("\u{feff}project: game\r\nPR-0001: ok\r\nPR-0002: reject: a \r\n"),
        [
            (2, "PR-0001".to_owned(), "ok".to_owned(), Ok(Token::Ok)),
            (
                3,
                "PR-0002".to_owned(),
                "reject: a".to_owned(),
                Ok(Token::Reject("a".to_owned()))
            ),
        ]
    );
    // Only one trailing CR is dropped; one inside the answer is refused.
    assert_eq!(token_of("PR-0001: a\rb"), Err(Refusal::Escaped('\r')));
    // A BOM not at the start of the text is an invisible prefix.
    assert_eq!(
        lines("project: game\n\u{feff}PR-0001: ok\n")[0].3,
        not_an_answer_line("\u{feff}PR-0001:", "PR-0001", false)
    );
}

#[test]
fn look_alikes_case_and_a_missing_colon_are_refused_with_the_latin_form() {
    let cases: [(&str, &str, bool, &str); 8] = [
        // U+0420 for `P`.
        ("\u{0420}R-0003: ok", "\u{0420}R-0003:", true, "ok"),
        ("pr-0003: ok", "pr-0003:", false, "ok"),
        ("Pr-0003: 1", "Pr-0003:", false, "1"),
        // Fullwidth letters, digits and colon.
        (
            "\u{ff30}\u{ff32}-0003: ok",
            "\u{ff30}\u{ff32}-0003:",
            true,
            "ok",
        ),
        (
            "PR-\u{ff10}\u{ff10}\u{ff10}\u{ff13}: ok",
            "PR-\u{ff10}\u{ff10}\u{ff10}\u{ff13}:",
            true,
            "ok",
        ),
        ("PR-0003\u{ff1a} ok", "PR-0003\u{ff1a}", true, "ok"),
        // No colon.
        ("PR-0003 ok", "PR-0003 ", false, "ok"),
        ("PR-0003", "PR-0003", false, ""),
    ];
    for (line, written, look_alike, answer) in cases {
        let got = lines(&format!("project: p\n{line}\n"));
        assert_eq!(
            got,
            [(
                2,
                "PR-0003".to_owned(),
                answer.to_owned(),
                not_an_answer_line(written, "PR-0003", look_alike)
            )],
            "{line:?}"
        );
    }
}

#[test]
fn invisible_prefixes_are_refused_never_ignored() {
    for prefix in [
        "\u{200b}", "\u{200d}", "\u{2060}", "\u{202e}", "\u{200f}", "\u{7}",
    ] {
        let line = format!("{prefix}PR-0003: ok");
        assert_eq!(
            token_of(&line),
            not_an_answer_line(&format!("{prefix}PR-0003:"), "PR-0003", false),
            "{line:?}"
        );
    }
    // Invisible and look-alike together: a look-alike.
    assert_eq!(
        token_of("\u{200b}\u{0420}R-0003: ok"),
        not_an_answer_line("\u{200b}\u{0420}R-0003:", "PR-0003", true)
    );
}

#[test]
fn duplicates_refuse_every_line_and_a_blank_slot_is_no_answer() {
    let text = "project: game\n\
                PR-0001: ok\n\
                PR-0001: no\n\
                PR-0002:\n\
                PR-0002: ok\n\
                PR-0003:\n\
                PR-0003:\n\
                PR-0004: ?\n\
                PR-0004: ok\n\
                PR-0005: 1\n\
                pr-0005: 2\n\
                PR-0006: a\n\
                PR-0006: b\n\
                PR-0006: c\n\
                PR-0007: ok\n\
                PR-0007:\n";
    let got: Vec<(usize, String, Result<Token, Refusal>)> = lines(text)
        .into_iter()
        .map(|(line, id, _, token)| (line, id, token))
        .collect();
    let dup = |lines: &[usize]| Err(Refusal::Duplicate(lines.into()));
    let want: Vec<(usize, String, Result<Token, Refusal>)> = vec![
        (2, "PR-0001".to_owned(), dup(&[2, 3])),
        (3, "PR-0001".to_owned(), dup(&[2, 3])),
        (5, "PR-0002".to_owned(), Ok(Token::Ok)),
        (6, "PR-0003".to_owned(), Ok(Token::Open)),
        (8, "PR-0004".to_owned(), dup(&[8, 9])),
        (9, "PR-0004".to_owned(), dup(&[8, 9])),
        (10, "PR-0005".to_owned(), dup(&[10, 11])),
        (
            11,
            "PR-0005".to_owned(),
            not_an_answer_line("pr-0005:", "PR-0005", false),
        ),
        (12, "PR-0006".to_owned(), dup(&[12, 13, 14])),
        (13, "PR-0006".to_owned(), dup(&[12, 13, 14])),
        (14, "PR-0006".to_owned(), dup(&[12, 13, 14])),
        (15, "PR-0007".to_owned(), Ok(Token::Ok)),
    ];
    assert_eq!(got, want);
    // The order of the pair does not matter: neither one wins.
    let reversed = lines("project: p\nPR-0001: no\nPR-0001: ok\n");
    assert!(
        reversed
            .iter()
            .all(|(_, _, _, token)| *token == dup(&[2, 3])),
        "{reversed:?}"
    );
    // Refused lines of other kinds count as answers of their ID too.
    let capped = format!("project: p\nPR-0001: {}\nPR-0001: ok\n", "a".repeat(2049));
    assert!(
        lines(&capped)
            .iter()
            .all(|(_, _, _, token)| *token == dup(&[2, 3])),
        "{:?}",
        lines(&capped)
    );
}

#[test]
fn caps_are_refused_with_the_byte_count_never_cut() {
    let at = "a".repeat(ANSWER_MAX);
    assert_eq!(
        token_of(&format!("PR-0001: {at}")),
        Ok(Token::Text(at.clone()))
    );
    let over = "a".repeat(ANSWER_MAX + 1);
    assert_eq!(
        token_of(&format!("PR-0001: {over}")),
        Err(Refusal::OverCap {
            reason: false,
            bytes: 2049,
            max: 2048
        })
    );
    // Bytes, not characters: 1 025 two-byte characters.
    let wide = "\u{e9}".repeat(1025);
    assert_eq!(
        token_of(&format!("PR-0001: {wide}")),
        Err(Refusal::OverCap {
            reason: false,
            bytes: 2050,
            max: 2048
        })
    );
    let reason = "r".repeat(ROUND_REASON_MAX_BYTES);
    assert_eq!(
        token_of(&format!("PR-0001: reject: {reason}")),
        Ok(Token::Reject(reason.clone()))
    );
    let long = "r".repeat(ROUND_REASON_MAX_BYTES + 1);
    assert_eq!(
        token_of(&format!("PR-0001: reject: {long}")),
        Err(Refusal::OverCap {
            reason: true,
            bytes: 4097,
            max: 4096
        })
    );
    // The whole answer stays one line of its own: the line's text is kept.
    let line = &lines(&format!("project: p\nPR-0001: {over}\n"))[0];
    assert_eq!(line.2, over);
}

#[test]
fn a_character_the_queue_escapes_is_refused_in_an_answer_or_a_reason() {
    for (line, c) in [
        ("PR-0001: a\u{202e}b", '\u{202e}'),
        ("PR-0001: a\u{7}", '\u{7}'),
        ("PR-0001: \u{1b}[31mred", '\u{1b}'),
        ("PR-0001: reject: x\u{200f}", '\u{200f}'),
        ("PR-0001: reject: \u{2066}x", '\u{2066}'),
        ("PR-0001: x\u{61c}", '\u{61c}'),
    ] {
        assert_eq!(token_of(line), Err(Refusal::Escaped(c)), "{line:?}");
    }
    // A tab is not escaped.
    assert_eq!(
        token_of("PR-0001: a\tb"),
        Ok(Token::Text("a\tb".to_owned()))
    );
}

#[test]
fn ids_the_queue_never_writes_and_numbers_past_a_u64() {
    for line in [
        "PR-1: ok",
        "PR-001: ok",
        "PR-00001: ok",
        "PR-99999999999999999999999: ok",
    ] {
        let got = lines(&format!("project: p\n{line}\n"));
        assert_eq!(got.len(), 1, "{line:?}");
        assert_eq!(got[0].3, Err(Refusal::NoProposal), "{line:?}");
        assert_eq!(got[0].1, line.split(':').next().unwrap(), "{line:?}");
    }
    assert_eq!(
        token_of("PR-0001: 99999999999999999999999"),
        Ok(Token::Option(u64::MAX))
    );
    assert_eq!(
        token_of("PR-0001: [18446744073709551616]"),
        Ok(Token::Option(u64::MAX))
    );
    assert_eq!(
        token_of("PR-0001: 18446744073709551614"),
        Ok(Token::Option(18_446_744_073_709_551_614))
    );
    // A five-digit ID the queue writes past PR-9999.
    assert_eq!(token_of("PR-10000: ok"), Ok(Token::Ok));
}

#[test]
fn answers_text_names_the_line_of_the_first_byte_that_is_not_utf8() {
    assert_eq!(
        answers_text(b"project: a\nPR-0001: ok\n"),
        Ok("project: a\nPR-0001: ok\n")
    );
    assert_eq!(answers_text(b"\xff"), Err(1));
    assert_eq!(answers_text(b"project: a\nPR-0001: ok\n\xffx\n"), Err(3));
    assert_eq!(answers_text(b"project: a\r\nPR-0001: \xc3\n"), Err(2));
    assert_eq!(answers_text(b""), Ok(""));
}

#[test]
fn reading_minutes_round_up() {
    for (chars, minutes) in [(0, 0), (1, 1), (999, 1), (1000, 1), (1001, 2), (2500, 3)] {
        assert_eq!(reading_minutes(chars), minutes, "{chars}");
    }
}

#[test]
fn order_key_severity_then_waiting_then_age_then_number() {
    let at = "2026-10-05T12:00:00Z";
    let key =
        |severity, waiting, created: &str, id: &str| order_key(severity, waiting, created, id);
    // Severity first: high, normal, low, none, whatever the rest.
    let ranked = [
        key(
            Some(IntakeSeverity::High),
            0,
            "2026-10-07T00:00:00Z",
            "PR-0009",
        ),
        key(Some(IntakeSeverity::Normal), 9, at, "PR-0001"),
        key(Some(IntakeSeverity::Low), 9, at, "PR-0001"),
        key(None, 9, at, "PR-0001"),
    ];
    for pair in ranked.windows(2) {
        assert!(pair[0] < pair[1], "{pair:?}");
    }
    // Waiting tasks, most first, before the age.
    assert!(
        key(
            Some(IntakeSeverity::Normal),
            2,
            "2026-10-07T00:00:00Z",
            "PR-0009"
        ) < key(Some(IntakeSeverity::Normal), 1, at, "PR-0001")
    );
    // Then the oldest.
    assert!(
        key(Some(IntakeSeverity::Normal), 1, at, "PR-0009")
            < key(
                Some(IntakeSeverity::Normal),
                1,
                "2026-10-05T12:00:01Z",
                "PR-0001"
            )
    );
    // Then the ID's number by value: PR-9999 before PR-10000.
    assert!(
        key(Some(IntakeSeverity::Low), 0, at, "PR-9999")
            < key(Some(IntakeSeverity::Low), 0, at, "PR-10000")
    );
    assert!(
        key(Some(IntakeSeverity::Low), 0, at, "PR-0002")
            < key(Some(IntakeSeverity::Low), 0, at, "PR-0010")
    );
    // An ID the queue never wrote goes last.
    assert!(key(None, 0, at, "PR-10000") < key(None, 0, at, "PR-1"));
    // Equal inputs, equal keys.
    assert_eq!(key(None, 0, at, "PR-0001"), key(None, 0, at, "PR-0001"));
}

#[test]
fn the_lines_of_one_id_share_one_list() {
    let got = lines("project: p\nPR-0001: a\nPR-0002: x\nPR-0001: b\nPR-0002: y\nPR-0001: c\n");
    let lists: Vec<std::sync::Arc<[usize]>> = got
        .iter()
        .map(|(_, _, _, token)| match token {
            Err(Refusal::Duplicate(found)) => std::sync::Arc::clone(found),
            other => panic!("not a duplicate: {other:?}"),
        })
        .collect();
    assert_eq!(&*lists[0], [2, 4, 6]);
    assert_eq!(&*lists[1], [3, 5]);
    // PR-0001's three lines hold the same allocation; PR-0002's another.
    assert!(std::sync::Arc::ptr_eq(&lists[0], &lists[2]));
    assert!(std::sync::Arc::ptr_eq(&lists[0], &lists[4]));
    assert!(std::sync::Arc::ptr_eq(&lists[1], &lists[3]));
    assert!(!std::sync::Arc::ptr_eq(&lists[0], &lists[1]));
}

#[test]
fn a_malformed_line_naming_a_canonical_id_counts_toward_its_duplicates() {
    let dup = |found: &[usize]| Err(Refusal::Duplicate(found.into()));
    // Case and look-alike forms of PR-0001, with an answer: both lines of
    // the ID refused; the malformed one keeps its own refusal.
    for (malformed, written, look_alike) in [
        ("pr-0001: no", "pr-0001:", false),
        ("\u{0420}R-0001: no", "\u{0420}R-0001:", true),
        ("PR-0001 no", "PR-0001 ", false),
    ] {
        let got = lines(&format!("project: p\nPR-0001: ok\n{malformed}\n"));
        assert_eq!(
            got,
            [
                (2, "PR-0001".to_owned(), "ok".to_owned(), dup(&[2, 3])),
                (
                    3,
                    "PR-0001".to_owned(),
                    "no".to_owned(),
                    not_an_answer_line(written, "PR-0001", look_alike)
                ),
            ],
            "{malformed:?}"
        );
    }
    // A malformed blank one does not count.
    let blank = lines("project: p\nPR-0001: ok\npr-0001:\n");
    assert_eq!(blank[0].3, Ok(Token::Ok), "{blank:?}");
    assert_eq!(blank[1].3, not_an_answer_line("pr-0001:", "PR-0001", false));
    // Nor one whose Latin form is no ID the queue writes.
    let other = lines("project: p\nPR-1: ok\npr-1: no\nPR-0001: ok\npr-00001: no\n");
    assert_eq!(other[0].3, Err(Refusal::NoProposal), "{other:?}");
    assert_eq!(other[1].3, not_an_answer_line("pr-1:", "PR-1", false));
    assert_eq!(other[2].3, Ok(Token::Ok), "{other:?}");
    assert_eq!(
        other[3].3,
        not_an_answer_line("pr-00001:", "PR-00001", false)
    );
    // Two malformed lines alone refuse nothing more than themselves.
    let alone = lines("project: p\npr-0001: a\npr-0001: b\n");
    assert!(
        alone
            .iter()
            .all(|line| matches!(line.3, Err(Refusal::NotAnAnswerLine { .. }))),
        "{alone:?}"
    );
}

#[test]
fn reject_not_followed_by_its_colon_is_refused() {
    for answer in [
        "reject : x",
        "reject x",
        "REJECT-x",
        "Reject\t: dup",
        "reject;x",
        "reject.",
        "reject !",
    ] {
        assert_eq!(
            token_of(&format!("PR-0001: {answer}")),
            Err(Refusal::RejectWithoutColon),
            "{answer:?}"
        );
    }
    // Another word, or the word with a non-ASCII letter: other text.
    for answer in [
        "rejected",
        "rejected : x",
        "rejects: x",
        "reject1: x",
        "reject\u{e9}: x",
        "reject\u{430}: x",
        "unreject: x",
    ] {
        assert_eq!(
            token_of(&format!("PR-0001: {answer}")),
            Ok(Token::Text(answer.to_owned())),
            "{answer:?}"
        );
    }
    // The forms with the colon right behind the word are unchanged.
    assert_eq!(
        token_of("PR-0001: REJECT:dup"),
        Ok(Token::Reject("dup".to_owned()))
    );
    assert_eq!(
        token_of("PR-0001: reject :"),
        Err(Refusal::RejectWithoutColon)
    );
    assert_eq!(
        token_of("PR-0001: reject"),
        Err(Refusal::RejectWithoutReason)
    );
}
