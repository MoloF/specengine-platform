//! docs/features/hub-team.md (RM-0071) A1 and A7, the CLI's half: `spec check`
//! on a bad `[team]` is one `team-config` error at its line, exit 1, every other
//! finding still listed, never `cannot-check`; `spec show`, `spec card` and the
//! gate stand as they were; and on a terminal `spec card accept --as` takes no
//! notice of the table. Over copies of `fixtures/spec-a` (a game's brief) and
//! `fixtures/card-press` (a print shop), so the code is tried on two projects of
//! different nature (ADR-0008). Every expected value is worked out from the
//! spec, never read back from a run.
//!
//! - A1: M: routing it through `load_config`.
//! - A7: M: judging `[team]` with no `Login`.
//!
//! The table's own errors are core's `tests/team_toml.rs`; the daemon's half is
//! `specengine-http`'s `tests/hub_team.rs`.

#![cfg(unix)]

mod common;

use std::path::Path;

use common::check::line_of;
use common::proposal::Pair;
use common::{Run, Scratch, read_text, replace, spec, write};
use serde_json::Value;

/// The keys of a good member.
const MEMBER: &str = "role = \"member\"\nemail = \"mira@example.org\"\n";

/// A good table: `mira` a member, `lena` leading `brief`.
const GOOD_TEAM: &str = "
[zones]
brief = [\"docs/**\"]

[team.mira]
name = \"Mira K\"
role = \"member\"
email = \"mira@example.org\"

[team.lena]
role = \"lead\"
zones = [\"brief\"]
email = \"lena@example.org\"
";

/// A table no reader takes.
const BAD_TEAM: &str = "
[team.mira]
role = \"editor\"
email = \"mira@example.org\"
";

/// `[zones]` and `[gate]` as a project that controls `src/**` holds them.
const GATE: &str = "
[zones]
code = [\"src/**\"]

[gate]
mode = \"selective\"
zones = [\"code\"]
";

/// Each defect of A1: its label, the tables, the start of the line it errs at.
fn defects() -> Vec<(&'static str, String, &'static str)> {
    vec![
        (
            "an unknown key",
            format!("[team.mira]\n{MEMBER}hat = \"red\"\n"),
            "hat = ",
        ),
        (
            "a role outside three",
            "[team.mira]\nrole = \"boss\"\nemail = \"mira@example.org\"\n".to_owned(),
            "role = ",
        ),
        (
            "`owner` with zones",
            "[zones]\nbrief = [\"docs/**\"]\n\n[team.mira]\nrole = \"owner\"\nzones = \
             [\"brief\"]\nemail = \"mira@example.org\"\n"
                .to_owned(),
            "zones = ",
        ),
        (
            "an undeclared zone",
            "[team.mira]\nrole = \"lead\"\nzones = [\"nope\"]\nemail = \"mira@example.org\"\n"
                .to_owned(),
            "zones = ",
        ),
        (
            "a bad email",
            "[team.mira]\nrole = \"member\"\nemail = \"mira\"\n".to_owned(),
            "email = ",
        ),
        (
            "a bad `[zones]`",
            format!("[zones]\nBad = [\"docs/**\"]\n\n[team.mira]\n{MEMBER}"),
            "Bad = ",
        ),
    ]
}

/// `extra` at the end of `root`'s config: the whole text.
fn append_config(root: &Path, extra: &str) -> String {
    let mut text = read_text(root, "specengine.toml");
    text.push_str(extra);
    write(root, "specengine.toml", &text);
    text
}

/// The findings of `spec check --json`, what a baseline matches on.
fn findings(report: &Value) -> Vec<(String, String, u64, String)> {
    report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|finding| {
            (
                finding["code"].as_str().expect("a code").to_owned(),
                finding["path"].as_str().expect("a path").to_owned(),
                finding["line"].as_u64().expect("a line"),
                finding["subject"].as_str().expect("a subject").to_owned(),
            )
        })
        .collect()
}

/// A1: on two fixtures, each defect is one `team-config`, an error at its line
/// in the config, exit 1, the verdict `blocked`, no cause of `cannot-check`,
/// and every other finding as it was: `spec-a` holds some, so the run went on
/// past the table. A second run is the same bytes. M: routing it through
/// `load_config`.
#[test]
fn a1_a_bad_team_is_one_finding_at_its_line_and_the_others_stay_listed() {
    for fixture in ["spec-a", "card-press"] {
        for (label, tables, needle) in defects() {
            let scratch = Scratch::new("team-check");
            let home = scratch.home("h");
            let root = scratch.copy(fixture, "copy");
            let plain = findings(&spec(&home, &root, &["check", "--json"]).json());
            if fixture == "spec-a" {
                assert!(!plain.is_empty(), "spec-a holds findings as copied");
            }

            let text = append_config(&root, &format!("\n{tables}"));
            let run = spec(&home, &root, &["check", "--json"]);
            run.code(1);
            let report = run.json();
            assert_eq!(report["verdict"], "blocked", "{fixture} {label}");
            assert_eq!(report["cannot_check"], serde_json::json!([]), "{label}");

            let all = findings(&report);
            let (team, others): (Vec<_>, Vec<_>) = all
                .into_iter()
                .partition(|(code, _, _, _)| code == "team-config");
            assert_eq!(team.len(), 1, "{fixture} {label}: {team:?}");
            assert_eq!(team[0].1, "specengine.toml", "{fixture} {label}");
            let line = u64::try_from(line_of(&text, needle)).expect("a line");
            assert_eq!(team[0].2, line, "{fixture} {label}");
            assert_eq!(others, plain, "{fixture} {label}: the others stay listed");

            let listed = report["findings"]
                .as_array()
                .expect("findings")
                .iter()
                .find(|finding| finding["code"] == "team-config")
                .expect("the finding");
            assert_eq!(listed["severity"], "error", "{fixture} {label}: {listed}");

            let again = spec(&home, &root, &["check", "--json"]);
            assert_eq!(again.stdout, run.stdout, "{fixture} {label}: a repeat");
        }
    }
}

/// A1, the text: one `error` line `specengine.toml:<line>: team-config: ...`,
/// and the summary's `blocked`, stderr empty: a terminal is told where to look.
/// M: routing it through `load_config`.
#[test]
fn a1_the_text_names_the_file_and_the_line() {
    let scratch = Scratch::new("team-check-text");
    let home = scratch.home("h");
    let root = scratch.copy("card-press", "copy");
    let text = append_config(&root, BAD_TEAM);
    let run = spec(&home, &root, &["check"]);
    run.code(1);
    assert_eq!(run.stderr, "", "{}", run.show());
    let opening = format!(
        "error  specengine.toml:{}: team-config: ",
        line_of(&text, "role = ")
    );
    assert!(
        run.stdout.lines().any(|line| line.starts_with(&opening)),
        "{opening}: {}",
        run.show()
    );
    let last = run.stdout.lines().last().unwrap_or_default();
    assert!(last.ends_with(" \u{2014} blocked"), "{last}");
}

/// The `team-config` findings of `report`.
fn team_findings(report: &Value) -> Vec<(String, String, u64, String)> {
    findings(report)
        .into_iter()
        .filter(|(code, _, _, _)| code == "team-config")
        .collect()
}

/// A1, `--staged`: the check against `HEAD` judges `team-config` on the checked
/// side: the staged config at its own line, whatever the worktree holds, and a
/// worktree's typo stays out of a clean index. The plain run reads the worktree.
/// M: `team-config` read off the worktree on a run against `HEAD`.
#[test]
fn a1_check_staged_judges_the_checked_side() {
    let pair = Pair::new("team-staged", "card-press");
    let good = read_text(&pair.main, "specengine.toml");
    let bad = append_config(&pair.main, BAD_TEAM);
    pair.git.add_all(&pair.main);
    write(&pair.main, "specengine.toml", &good);

    let staged = run(&pair, &["--json", "check", "--staged"]);
    staged.code(1);
    let report = staged.json();
    let team = team_findings(&report);
    assert_eq!(team.len(), 1, "{}", staged.show());
    assert_eq!(team[0].1, "specengine.toml");
    let line = u64::try_from(line_of(&bad, "role = ")).expect("a line");
    assert_eq!(team[0].2, line, "the staged file's line");
    assert_eq!(report["cannot_check"], serde_json::json!([]));
    assert!(
        team_findings(&run(&pair, &["--json", "check"]).json()).is_empty(),
        "the plain run reads the mended worktree"
    );

    pair.git.add_all(&pair.main);
    write(&pair.main, "specengine.toml", &bad);
    let staged = run(&pair, &["--json", "check", "--staged"]);
    assert!(
        team_findings(&staged.json()).is_empty(),
        "a clean index: {}",
        staged.show()
    );
    assert_eq!(
        team_findings(&run(&pair, &["--json", "check"]).json()).len(),
        1,
        "the plain run reads the worktree's typo"
    );
}

/// A1: a good `[team]` adds no finding, and no `[team]` none: the check judges
/// the table and nothing else of it. M: `team-config` raised on any `[team]`.
#[test]
fn a1_a_good_team_adds_no_finding() {
    for fixture in ["spec-a", "card-press"] {
        let scratch = Scratch::new("team-check-good");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let plain = spec(&home, &root, &["check", "--json"]);
        append_config(&root, GOOD_TEAM);
        let run = spec(&home, &root, &["check", "--json"]);
        assert_eq!(run.code, plain.code, "{fixture}: {}", run.show());
        assert_eq!(
            findings(&run.json()),
            findings(&plain.json()),
            "{fixture}: no finding"
        );
    }
}

/// `spec args` on `pair`'s main worktree, an empty pipe on stdin.
fn run(pair: &Pair, args: &[&str]) -> Run {
    pair.spec_piped(&pair.main, args, b"")
}

/// Exit 0: the document of `spec --json args`.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let done = run(pair, &all);
    done.code(0);
    done.json()
}

/// A `plates` card on SHT-FRONT, `ana` its assignee and `ben` its reviewer, at
/// `accept.from`: its ID. `ben` is in no table here.
fn in_checked(pair: &Pair) -> String {
    let made = doc(
        pair,
        &[
            "card",
            "new",
            "--title",
            "Lead story",
            "--discipline",
            "plates",
            "--target",
            "SHT-FRONT",
        ],
    );
    let id = made["id"].as_str().expect("a card's ID").to_owned();
    run(
        pair,
        &[
            "card",
            "assign",
            &id,
            "--assignee",
            "ana",
            "--reviewer",
            "ben",
        ],
    )
    .code(0);
    run(pair, &["card", "move", &id, "--to", "checked"]).code(0);
    id
}

/// A1: a bad `[team]` leaves `spec show`, `spec card` and the gate as they were:
/// the same exit and bytes on either side of it, a card's writes included, and
/// the gate's verdict on a controlled path and off one (ADR-0012, ADR-0039).
/// M: `ProjectConfig` reading the table; the gate reading `team-config`.
#[test]
fn a1_a_bad_team_leaves_show_card_and_gate_as_they_were() {
    let pair = Pair::new("team-bad-reads", "card-press");
    append_config(&pair.main, GATE);
    pair.commit_all(&pair.main, "the gate");
    let id = in_checked(&pair);
    let main = pair.main.to_str().expect("a UTF-8 path").to_owned();
    let reads: Vec<Vec<&str>> = vec![
        vec!["show", "SHT-FRONT"],
        vec!["--json", "show", "SHT-FRONT"],
        vec!["tree"],
        vec!["--json", "card", "list"],
        vec!["--json", "card", "show", &id],
        vec!["gate", "--worktree", &main, "--path", "src/a.rs", "--json"],
        vec![
            "gate",
            "--worktree",
            &main,
            "--path",
            "docs/spec/sht-front.md",
            "--json",
        ],
    ];
    let answers = |pair: &Pair| -> Vec<(i32, String, String)> {
        reads
            .iter()
            .map(|args| {
                let done = run(pair, args);
                (done.code, done.stdout.clone(), done.stderr.clone())
            })
            .collect()
    };
    let before = answers(&pair);
    // Each read says something: the gate's refusal of `src/a.rs` is its exit 2.
    for (args, (code, stdout, stderr)) in reads.iter().zip(&before) {
        assert!(!stdout.is_empty(), "{args:?}: {code}: {stderr}");
    }
    assert_eq!(before[5].0, 2, "the gate refuses a controlled path");
    assert_eq!(before[6].0, 0, "and passes one outside its zones");

    append_config(&pair.main, BAD_TEAM);
    assert_eq!(answers(&pair), before, "the reads, as they were");

    let made = doc(
        &pair,
        &[
            "card",
            "new",
            "--title",
            "Notices",
            "--discipline",
            "plates",
        ],
    );
    let other = made["id"].as_str().expect("a card's ID").to_owned();
    run(&pair, &["card", "move", &other, "--to", "checked"]).code(0);
    run(
        &pair,
        &[
            "card",
            "cancel",
            &other,
            "--reason",
            "Pulled for space.",
            "--as",
            "ana",
        ],
    )
    .code(0);
}

/// A7: with `[team]` in the file, good or bad, `spec card accept --as ben` on a
/// terminal accepts as ever, `ben` in no table: a shell on the host is the
/// operator's. M: judging `[team]` with no `Login`.
#[test]
fn a7_card_accept_on_a_terminal_takes_no_notice_of_team() {
    for (label, table) in [("good", GOOD_TEAM), ("bad", BAD_TEAM)] {
        let pair = Pair::new(&format!("team-terminal-{label}"), "card-press");
        append_config(&pair.main, table);
        pair.commit_all(&pair.main, "the team");
        let id = in_checked(&pair);
        let done = pair.spec_pty(
            &pair.main,
            &["card", "accept", &id, "--as", "ben", "--briefed"],
            Some("y"),
        );
        assert_eq!(done.code, 0, "{label}: {}", done.output);
        assert!(done.asked, "{label}: the terminal was asked");
        let card = doc(&pair, &["card", "show", &id]);
        assert_eq!(card["column"], "struck", "{label}: {card}");
        assert_eq!(card["acceptance"]["by"], "ben", "{label}: {card}");
    }
}

/// The security review's departed sequence, on a terminal: C-0001 accepted
/// departing SHT-FRONT, C-0002 left open on it, the node edited: `spec card
/// clear --group` clears C-0002 alone and says so of C-0001, which stands as it
/// was: a departed card is no mark to clear. The daemon's forms are
/// `specengine-http`'s `tests/hub_team.rs`; the terminal's harness is here.
/// M: `chosen` taking a departed card.
#[test]
fn a_departed_card_is_no_part_of_a_group_clear_on_a_terminal() {
    let pair = Pair::new("team-departed-terminal", "card-press");
    let departed = in_checked(&pair);
    assert_eq!(departed, "C-0001");
    let done = pair.spec_pty(
        &pair.main,
        &[
            "card",
            "accept",
            &departed,
            "--as",
            "ben",
            "--departs",
            "SHT-FRONT",
            "--summary",
            "The result left the brief.",
            "--observed",
            "The lead runs below the fold.",
            "--documented",
            "The lead runs above the fold.",
        ],
        Some("y"),
    );
    assert_eq!(done.code, 0, "{}", done.output);
    let open = doc(
        &pair,
        &[
            "card",
            "new",
            "--title",
            "Masthead",
            "--discipline",
            "plates",
            "--target",
            "SHT-FRONT",
        ],
    )["id"]
        .as_str()
        .expect("a card's ID")
        .to_owned();
    assert_eq!(open, "C-0002");
    replace(
        &pair.main,
        "docs/spec/sht-front.md",
        "the lead story.",
        "the lead story and the weather.",
    );
    pair.commit_all(&pair.main, "by hand");
    let held = doc(&pair, &["card", "show", &departed]);

    let done = pair.spec_pty(
        &pair.main,
        &[
            "card",
            "clear",
            "--group",
            "PRN-WEEKLY",
            "--no-effect",
            "--reason",
            "Only the weather.",
            "--as",
            "ben",
        ],
        Some("y"),
    );
    assert_eq!(done.code, 0, "{}", done.output);
    assert!(
        done.output.contains(&format!("cleared {open}: ")),
        "{}",
        done.output
    );
    assert!(
        !done.output.contains(&format!("cleared {departed}: ")),
        "{}",
        done.output
    );
    assert!(
        done.output.contains(
            "C-0001 reads departed PR-0001: passed over; clear it alone once that is settled"
        ),
        "the note: {}",
        done.output
    );
    assert_eq!(doc(&pair, &["card", "show", &departed]), held);
}
