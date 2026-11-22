//! docs/features/comments.md (RM-0136, ADR-0087) "Acceptance criteria", the
//! core's pure half: core `comment` asked directly, every expected value
//! worked out from the spec's "Mentions grammar", "Data" and D14, never read
//! back from a run.
//!
//! - CM17: the grammar's examples ("Mentions grammar"): `x@mira`,
//!   `` `@mira` ``, `Bob@mira.org` and `@Mira` mention nobody; `@mira.` and
//!   `(@mira)` mention `mira`; `@mira.k` mentions `mira.k`; nothing inside a
//!   fenced or indented code block; duplicates once; sorted. M: the boundary
//!   rule dropped (`x@mira` and `Bob@mira.org` then mention).
//! - CM16: the body's caps: 8 193 bytes, a blank body, U+0000, 17 distinct
//!   mentions refused with "Data"'s lines; 8 192 bytes and 16 mentions
//!   taken. M: the cap check removed.
//! - CM10's arithmetic: a context body is one line, cut at 1 024 characters
//!   ending ` [cut]`.
//! - D4: `CM-` and 4 or more digits; a look-alike named with its Latin form;
//!   `[ids]` claiming `CM` is a clash.
//!
//! The surfaces (CLI, MCP, HTTP) are each crate's `tests/comments.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

use specengine_core::IdSchemeToml as _;
use specengine_core::comment::{
    COMMENT_BODY_MAX, COMMENT_CONTEXT_BODY_CHARS, COMMENT_CONTEXT_CHARS, COMMENT_CONTEXT_MAX,
    COMMENT_MENTIONS_MAX, COMMENT_PAGE_MAX, COMMENT_PREFIX, CommentIdError, body_problem,
    comment_id, comment_prefix_clash, context_body, mentions, parse_comment_id,
};
use specengine_model::IdScheme;

fn logins(body: &str) -> Vec<String> {
    mentions(body)
}

fn none() -> Vec<String> {
    Vec::new()
}

fn just(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// "Data"'s constants, as the spec fixes them.
#[test]
fn data_the_constants_are_the_specs() {
    assert_eq!(COMMENT_PREFIX, "CM");
    assert_eq!(COMMENT_BODY_MAX, 8192);
    assert_eq!(COMMENT_MENTIONS_MAX, 16);
    assert_eq!(COMMENT_PAGE_MAX, 100);
    assert_eq!(COMMENT_CONTEXT_MAX, 20);
    assert_eq!(COMMENT_CONTEXT_CHARS, 8192);
    assert_eq!(COMMENT_CONTEXT_BODY_CHARS, 1024);
}

/// CM17: the grammar's own examples. M: the boundary rule dropped.
#[test]
fn cm17_the_grammars_examples_hold() {
    for body in ["x@mira", "`@mira`", "Bob@mira.org", "@Mira"] {
        assert_eq!(logins(body), none(), "{body:?} mentions nobody");
    }
    for body in ["@mira.", "(@mira)", "@mira"] {
        assert_eq!(logins(body), just(&["mira"]), "{body:?}");
    }
    assert_eq!(logins("@mira.k"), just(&["mira.k"]));
}

/// CM17, D14: the boundary is a character other than an ASCII letter, a
/// digit, `.`, `_` or `-` before the `@`, or the body's start.
#[test]
fn cm17_the_boundary_before_the_at() {
    for glued in [
        "a@lev",
        "Z@lev",
        "7@lev",
        ".@lev",
        "_@lev",
        "-@lev",
        "mail.me@lev",
    ] {
        assert_eq!(logins(glued), none(), "{glued:?}");
    }
    for open in [
        " @lev",
        "\n@lev",
        "a\t@lev",
        ",@lev",
        ":@lev",
        "/@lev",
        "!@lev",
        "\"@lev\"",
        "[@lev]",
        "\u{00e9}@lev",
        "\u{043b}@lev",
        "@@lev",
    ] {
        assert_eq!(logins(open), just(&["lev"]), "{open:?}");
    }
    // The longest match of the login grammar, ending in a letter or digit.
    assert_eq!(logins("@lev-"), just(&["lev"]));
    assert_eq!(logins("@lev_"), just(&["lev"]));
    assert_eq!(logins("@lev..."), just(&["lev"]));
    assert_eq!(logins("@lev.k-2"), just(&["lev.k-2"]));
    assert_eq!(logins("@levK"), just(&["lev"]), "a capital ends the login");
    assert_eq!(logins("@a"), just(&["a"]), "one letter is a login");
    assert_eq!(logins("@1lev"), none(), "a login opens on a letter");
    assert_eq!(logins("@"), none());
    assert_eq!(logins("@ lev"), none());
    // The longest login is 64 characters; the 65th is not taken.
    let long = format!("l{}", "o".repeat(70));
    assert_eq!(logins(&format!("@{long}")), just(&[&long[..64]]));
}

/// CM17: nothing inside an inline code span, a fenced or an indented code
/// block (CommonMark's).
#[test]
fn cm17_never_inside_code() {
    for body in [
        "see `@mira` here",
        "``a @mira b``",
        "```\n@mira\n```\n",
        "~~~text\nask @mira\n~~~\n",
        "Before.\n\n    @mira in an indented block\n",
    ] {
        assert_eq!(logins(body), none(), "{body:?}");
    }
    assert_eq!(
        logins("`@ana` and @lev\n\n```\n@mira\n```\nthen @ira"),
        just(&["ira", "lev"])
    );
}

/// CM17: duplicates count once; the list is sorted.
#[test]
fn cm17_duplicates_once_and_sorted() {
    assert_eq!(
        logins("@lev, @ana and @lev again; (@mira) @ana."),
        just(&["ana", "lev", "mira"])
    );
}

/// CM16: "Data"'s refusal lines; the limits themselves taken. M: the cap
/// check removed.
#[test]
fn cm16_the_body_caps() {
    let at_cap = "\u{00e9}".repeat(COMMENT_BODY_MAX / 2);
    assert_eq!(at_cap.len(), 8192);
    assert_eq!(body_problem(&at_cap), None, "8 192 bytes are a body");
    let over = format!("{at_cap}x");
    assert_eq!(
        body_problem(&over).as_deref(),
        Some("body: 8193 bytes; at most 8192")
    );
    for blank in ["", " ", " \t\n\r\n", "\n\n"] {
        assert_eq!(
            body_problem(blank).as_deref(),
            Some("body: blank: give a value"),
            "{blank:?}"
        );
    }
    assert_eq!(
        body_problem("a\u{0}b").as_deref(),
        Some("body: holds U+0000")
    );
    let names: Vec<String> = (0..17).map(|n| format!("@p{n:02}")).collect();
    assert_eq!(
        body_problem(&names.join(" ")).as_deref(),
        Some("body: 17 mentions; at most 16")
    );
    assert_eq!(body_problem(&names[..16].join(" ")), None, "16 are taken");
    // Duplicates count once: 17 mentions of 16 logins are taken.
    let mut repeated = names[..16].join(" ");
    repeated.push_str(" @p00");
    assert_eq!(body_problem(&repeated), None);
    // A mention in code is no mention: 17 with one in code are taken.
    let mut coded = names[..16].join(" ");
    coded.push_str(" `@p16`");
    assert_eq!(body_problem(&coded), None);
}

/// CM10's arithmetic: one line, cut at 1 024 characters, the cut ending
/// ` [cut]`; a body of exactly 1 024 characters is not cut.
#[test]
fn cm10_a_context_body_is_one_line_cut_at_1024() {
    assert_eq!(
        context_body("Keep\nthe rail\r\nat 1.2 m?\r"),
        ("Keep the rail at 1.2 m? ".to_owned(), false)
    );
    let exact = "\u{0436}".repeat(1024);
    assert_eq!(context_body(&exact), (exact.clone(), false));
    let long = "r".repeat(3000);
    let (cut, was_cut) = context_body(&long);
    assert!(was_cut);
    assert_eq!(cut, format!("{} [cut]", "r".repeat(1024)));
}

/// D4: the ID's grammar; a look-alike names its Latin form; no other form.
#[test]
fn d4_comment_ids_and_their_look_alikes() {
    assert_eq!(comment_id(12), "CM-0012");
    assert_eq!(comment_id(12_345), "CM-12345");
    assert_eq!(parse_comment_id("CM-0012"), Ok("CM-0012".to_owned()));
    assert_eq!(parse_comment_id("CM-10000"), Ok("CM-10000".to_owned()));
    for not_one in [
        "CM-12", "CM-00012", "cm-0012", "C-0012", "CM0012", "PR-0012", "",
    ] {
        assert_eq!(
            parse_comment_id(not_one),
            Err(CommentIdError::NotAnId),
            "{not_one:?}"
        );
    }
    // A Cyrillic capital es (U+0421) and em (U+041C) for `C` and `M`.
    assert_eq!(
        parse_comment_id("\u{0421}\u{041c}-0012"),
        Err(CommentIdError::LookAlike {
            fix: "CM-0012".to_owned()
        })
    );
}

/// D4: `[ids]` claiming `CM` clashes; a project with no such prefix does not.
#[test]
fn d4_ids_claiming_cm_clash() {
    let clean = IdScheme::from_toml("[ids]\nMEC = { kind = \"mechanic\", shape = \"name\" }\n")
        .expect("a scheme");
    assert_eq!(comment_prefix_clash(&clean), None);
    let claiming = IdScheme::from_toml(
        "[ids]\nMEC = { kind = \"mechanic\", shape = \"name\" }\nCM = { kind = \"cue\", width = 4 }\n",
    )
    .expect("a scheme");
    let clash = comment_prefix_clash(&claiming).expect("a clash");
    assert!(clash.contains("`CM`"), "{clash}");
}
