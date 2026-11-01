//! docs/features/hub-team.md (RM-0071) A1, the pure half (core `team_toml.rs`,
//! "Data", "Reader", "A login key"): `TeamConfig::from_toml` errs at the line on
//! an unknown key, a role outside three, `lead` with no zones, `owner` or
//! `member` with zones, an undeclared zone, a bad email, a login git would
//! alter; a bad `[team]` leaves the project's and the gate's readers as they
//! were, and gate `off` keeps `[zones]` unread. M: dropping the zone lookup.
//!
//! `spec check` and "routing it through `load_config`" are `specengine-cli`'s
//! `tests/team_config.rs`.
//!
//! Two projects of different nature (`fixtures/spec-a`, a game's brief;
//! `fixtures/card-press`, a print shop) take the same table (ADR-0008).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::fs;

use common::fixture;
use specengine_core::gate::{GateConfig, GateMode};
use specengine_core::{ProjectConfig, TEAM_ENTRIES_MAX, TeamConfig};

/// `[team.<key>]` holding `lines`, under two zones: the header on line 8, its
/// first key on line 9.
fn table(key: &str, lines: &str) -> String {
    format!(
        "[project]\nslug = \"press\"\n\n[zones]\nbrief = [\"docs/spec/**\"]\nmaps = \
         [\"docs/spec/maps/**\"]\n\n[team.{key}]\n{lines}\n"
    )
}

/// A member's two keys, good.
const MEMBER: &str = "role = \"member\"\nemail = \"mira@example.org\"";

const EMAIL: &str = "email = \"mira@example.org\"";

fn error(text: &str) -> (Option<usize>, String) {
    let error = TeamConfig::from_toml(text).expect_err(text);
    (error.line, error.message)
}

fn read(text: &str) -> TeamConfig {
    TeamConfig::from_toml(text)
        .unwrap_or_else(|e| panic!("{}\n{text}", e.at("specengine.toml")))
        .unwrap_or_else(|| panic!("a [team] table:\n{text}"))
}

/// The 1-based line holding `needle`.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line holds {needle:?} in\n{text}"))
        + 1
}

/// The first line of `text` from the `[team` table on, and the last.
fn team_lines(text: &str) -> (usize, usize) {
    (line_of(text, "[team"), text.lines().count())
}

// --------------------------------------------------------------- reading

/// No `[team]`: `None`, on either fixture and with `[zones]` alone. M: an absent
/// table read as an empty team.
#[test]
fn no_team_table_is_none_on_both_fixtures() {
    for name in ["spec-a", "card-press"] {
        let path = fixture(name).join("specengine.toml");
        let text = fs::read_to_string(path).expect("the fixture's config");
        assert!(
            TeamConfig::from_toml(&text).expect(name).is_none(),
            "{name}"
        );
    }
    let zones = "[zones]\nbrief = [\"docs/**\"]\n";
    assert!(TeamConfig::from_toml(zones).expect("zones").is_none());
}

/// "Data": each key as written, on two projects of different nature: the key
/// is the login byte for byte, quoted when not bare; a `lead`'s zones are read
/// from `[zones]`, no `[gate]` in the file. M: a login folded.
#[test]
fn a_good_table_reads_on_both_fixtures() {
    let team = "\n[zones]\nbrief = [\"docs/spec/**\"]\nmaps = [\"docs/spec/maps/**\"]\n\
                \n[team.mira]\nname = \"Mira K\"\nrole = \"lead\"\nzones = [\"maps\"]\nemail = \
                \"mira@example.org\"\n\
                \n[team.olga]\nrole = \"owner\"\nemail = \"olga@example.org\"\n\
                \n[team.\"tomas b\"]\nname = \"Tomas B\"\nrole = \"member\"\nemail = \
                \"1234+tomas@users.noreply.example.org\"\n";
    for name in ["spec-a", "card-press"] {
        let path = fixture(name).join("specengine.toml");
        let mut text = fs::read_to_string(path).expect("the fixture's config");
        text.push_str(team);
        let read = read(&text);
        assert_eq!(read.entries().count(), 3, "{name}");

        let mira = read.entry("mira").expect("mira");
        assert_eq!(mira.login, "mira");
        assert_eq!(mira.name.as_deref(), Some("Mira K"));
        assert_eq!(mira.role.as_str(), "lead");
        assert_eq!(mira.email, "mira@example.org");
        let zones: Vec<&str> = mira.zones.iter().map(|zone| zone.name.as_str()).collect();
        assert_eq!(zones, ["maps"], "{name}");

        let olga = read.entry("olga").expect("olga");
        assert_eq!(olga.role.as_str(), "owner");
        assert_eq!(olga.name, None, "`name` is optional");
        assert!(olga.zones.is_empty(), "{name}");

        let tomas = read.entry("tomas b").expect("a quoted login");
        assert_eq!(tomas.role.as_str(), "member");

        for other in ["Mira", "MIRA", " mira", "tomas", "tomas  b"] {
            assert!(read.entry(other).is_none(), "{name}: {other:?}");
        }
    }
}

/// `Zone::covers`, through an entry: the gate's glob, case-sensitive, on a
/// root-relative path. M: `covers` read as a prefix.
#[test]
fn a_zone_covers_by_its_globs() {
    let text = table(
        "mira",
        &format!("role = \"lead\"\nzones = [\"maps\"]\n{EMAIL}"),
    );
    let team = read(&text);
    let mira = team.entry("mira").expect("mira");
    assert!(mira.covers("docs/spec/maps/harbor.md"));
    assert!(mira.covers("docs/spec/maps/deep/harbor.md"));
    assert!(!mira.covers("docs/spec/maps.md"));
    assert!(!mira.covers("docs/spec/mapsmore/harbor.md"));
    assert!(!mira.covers("docs/spec/MAPS/harbor.md"));
    assert!(!mira.covers("other/docs/spec/maps/harbor.md"));
    assert!(!mira.covers("docs/spec/harbor.md"));
}

// ------------------------------------------------------------- the errors

/// A1: each defect is an error at the line it sits on, naming what is wrong.
/// M: dropping the zone lookup.
#[test]
fn a1_each_defect_errs_at_its_line() {
    let lead = |zones: &str| format!("role = \"lead\"\nzones = {zones}\n{EMAIL}");
    let cases: [(&str, String, &str, &str); 14] = [
        (
            "an unknown key",
            format!("{MEMBER}\nhat = \"red\""),
            "hat = ",
            "hat",
        ),
        (
            "a key of another spelling",
            format!("{MEMBER}\nZones = [\"maps\"]"),
            "Zones = ",
            "Zones",
        ),
        (
            "a role outside three",
            format!("role = \"boss\"\n{EMAIL}"),
            "role = ",
            "boss",
        ),
        (
            "a role in another case",
            format!("role = \"Lead\"\nzones = [\"maps\"]\n{EMAIL}"),
            "role = ",
            "Lead",
        ),
        (
            "a role no string",
            format!("role = 5\n{EMAIL}"),
            "role = ",
            "",
        ),
        ("an undeclared zone", lead("[\"nope\"]"), "zones = ", "nope"),
        (
            "one undeclared of two",
            lead("[\"maps\", \"nope\"]"),
            "zones = ",
            "nope",
        ),
        (
            "a zone named twice",
            lead("[\"maps\", \"maps\"]"),
            "zones = ",
            "maps",
        ),
        (
            "`owner` with an empty list",
            format!("role = \"owner\"\nzones = []\n{EMAIL}"),
            "zones = ",
            "zones",
        ),
        (
            "`member` with an empty list",
            format!("role = \"member\"\nzones = []\n{EMAIL}"),
            "zones = ",
            "zones",
        ),
        (
            "`owner` with zones",
            format!("role = \"owner\"\nzones = [\"maps\"]\n{EMAIL}"),
            "zones = ",
            "zones",
        ),
        (
            "`member` with zones",
            format!("role = \"member\"\nzones = [\"maps\"]\n{EMAIL}"),
            "zones = ",
            "zones",
        ),
        (
            "an email with no @",
            "role = \"member\"\nemail = \"mira\"".to_owned(),
            "email = ",
            "",
        ),
        (
            "a name off a person's grammar",
            format!("{MEMBER}\nname = \"Mira\\u0007\""),
            "name = ",
            "",
        ),
    ];
    for (label, lines, needle, named) in &cases {
        let text = table("mira", lines);
        let (line, message) = error(&text);
        assert_eq!(line, Some(line_of(&text, needle)), "{label}: {message}");
        assert!(message.contains(named), "{label}: {message}");
    }
}

/// A1: what a table lacks has no line of its own: the error sits inside the
/// entry. M: `zones` optional on a `lead`; `email` optional.
#[test]
fn a1_a_missing_key_errs_inside_its_entry() {
    for (label, lines, named) in [
        (
            "`lead` with no zones",
            format!("role = \"lead\"\n{EMAIL}"),
            "zones",
        ),
        (
            "`lead` with an empty list",
            format!("role = \"lead\"\nzones = []\n{EMAIL}"),
            "zones",
        ),
        ("no role", EMAIL.to_owned(), "role"),
        ("no email", "role = \"member\"".to_owned(), "email"),
    ] {
        let text = table("mira", &lines);
        let (line, message) = error(&text);
        let line = line.unwrap_or_else(|| panic!("{label}: no line: {message}"));
        let (first, last) = team_lines(&text);
        assert!((first..=last).contains(&line), "{label}: line {line}");
        assert!(message.contains(named), "{label}: {message}");
    }
}

/// A1: `team` no table of tables errs at its line, and `[zones]` missing leaves
/// a named zone undeclared. M: dropping the zone lookup.
#[test]
fn a1_the_table_and_its_zones_must_be_there() {
    let text = "team = 5\n\n[project]\nslug = \"press\"\n";
    let (line, _) = error(text);
    assert_eq!(line, Some(1), "{text}");

    let text = "[team]\nmira = 5\n";
    let (line, _) = error(text);
    assert_eq!(line, Some(2), "{text}");

    let text = format!("[team.mira]\nrole = \"lead\"\nzones = [\"maps\"]\n{EMAIL}\n");
    let (line, message) = error(&text);
    assert_eq!(line, Some(3), "{text}");
    assert!(message.contains("maps"), "{message}");
}

/// "A login key": the key passes `hub_login_problem` and holds no `<`, `>` or
/// character git trims at either end, which would change `Decided-by`; each
/// refused key errs inside its entry. A dot inside is a dot. M: the end rule read
/// as a character anywhere; the opening end unjudged.
#[test]
fn a1_a_login_git_would_alter_is_refused() {
    let refused = [
        "\"mira.\"",
        "\"mira,\"",
        "\"mira:\"",
        "\"mira;\"",
        "'mira\"'",
        "\"mira'\"",
        "'mira\\'",
        // Git trims both ends.
        "\".mira\"",
        "\",mira\"",
        "\":mira\"",
        "\";mira\"",
        "'\"mira'",
        "\"'mira\"",
        "'\\mira'",
        "\"mi<ra\"",
        "\"mi>ra\"",
        "\"<mira\"",
        "\"mira>\"",
        // `hub_login_problem`.
        "\"local\"",
        "\"Agent\"",
        "\"-\"",
        "\"a, b\"",
        "\" mira\"",
        "\"mira \"",
        "\"mira\\u200B\"",
        "\"mi\\u00ADra\"",
        "\"\\uFEFFmira\"",
        "\"mira\\u0007\"",
        "\"\"",
    ];
    for key in refused {
        let text = table(key, MEMBER);
        let (line, message) = error(&text);
        let line = line.unwrap_or_else(|| panic!("{key}: no line: {message}"));
        let (first, last) = team_lines(&text);
        assert!((first..=last).contains(&line), "{key}: line {line}");
    }
    let long = format!("\"{}\"", "n".repeat(129));
    assert!(TeamConfig::from_toml(&table(&long, MEMBER)).is_err());

    for key in [
        "mira",
        "\"mi.ra\"",
        "\"mira k\"",
        "\"o'brien\"",
        "\"a:b\"",
        "\"\\u0410\\u043D\\u043D\\u0430 \\u041F\"",
    ] {
        let team = read(&table(key, MEMBER));
        assert_eq!(team.entries().count(), 1, "{key}");
    }
    let team = read(&table(&format!("\"{}\"", "n".repeat(128)), MEMBER));
    assert_eq!(team.entries().count(), 1, "128 bytes");
}

/// "`email` is required": one `@` between two non-empty parts, printable ASCII
/// with no space, no `<` or `>`, and the end rule of a login, on both ends. A
/// forge's no-reply address is fine. M: `email` read as any string.
#[test]
fn a1_a_bad_email_is_refused() {
    let refused = [
        "\"\"",
        "\"mira\"",
        "\"@example.org\"",
        "\"mira@\"",
        "\"@\"",
        "\"mi ra@example.org\"",
        "\"mira@exam ple.org\"",
        "\" mira@example.org\"",
        "\"mira@example.org \"",
        "\"mira@a@example.org\"",
        "\"<mira@example.org>\"",
        "\"mira<@example.org\"",
        "\"mira>@example.org\"",
        "\"mira@example.org.\"",
        "\"mira@example.org,\"",
        "\"mira@example.org:\"",
        "\"mira@example.org;\"",
        "'mira@example.org\"'",
        "\"mira@example.org'\"",
        "'mira@example.org\\'",
        "\".mira@example.org\"",
        "\",mira@example.org\"",
        "\":mira@example.org\"",
        "\";mira@example.org\"",
        "'\"mira@example.org'",
        "\"'mira@example.org\"",
        "'\\mira@example.org'",
        "\"mira\\u00E9@example.org\"",
        "\"mira\\t@example.org\"",
        "\"mira\\n@example.org\"",
        "\"mira\\u007F@example.org\"",
    ];
    for email in refused {
        let text = table("mira", &format!("role = \"member\"\nemail = {email}"));
        let (line, message) = error(&text);
        assert_eq!(line, Some(line_of(&text, "email = ")), "{email}: {message}");
    }
    for email in [
        "mira@example.org",
        "mira.k+hub@example.org",
        "1234+mira@users.noreply.example.org",
    ] {
        let text = table("mira", &format!("role = \"member\"\nemail = \"{email}\""));
        assert_eq!(read(&text).entry("mira").expect("mira").email, email);
    }
}

/// "Reader": at most 256 entries, the 257th refused with a line. M: no cap.
#[test]
fn at_most_256_entries() {
    assert_eq!(TEAM_ENTRIES_MAX, 256);
    let entries = |count: usize| -> String {
        (0..count)
            .map(|index| {
                format!(
                    "[team.login{index}]\nrole = \"member\"\nemail = \"a{index}@example.org\"\n"
                )
            })
            .collect()
    };
    assert_eq!(read(&entries(256)).entries().count(), 256);
    let text = entries(257);
    let (line, message) = error(&text);
    assert!(line.is_some(), "{message}");
    assert!(message.contains("256"), "{message}");
}

// --------------------------------------------------------- beside the table

/// "Reader": `[zones]` read here takes the gate's rules and messages, whether
/// or not `[gate]` is on: the same line, the same words. M: a second reader of
/// `[zones]`, its messages drifting.
#[test]
fn zones_err_here_with_the_gates_own_messages() {
    for zones in [
        "Bad = [\"docs/**\"]",
        "brief = []",
        "brief = [\"/docs/**\"]",
        "brief = [\"../docs\"]",
    ] {
        let head = format!("[project]\nslug = \"press\"\n\n[zones]\n{zones}\n");
        let team =
            format!("{head}\n[team.mira]\nrole = \"member\"\nemail = \"mira@example.org\"\n");
        let gated = format!("{head}\n[gate]\nmode = \"observe\"\nzones = [\"brief\"]\n");
        let (team_line, team_message) = error(&team);
        let gate_error = GateConfig::from_toml(&gated).expect_err(&gated);
        assert_eq!(team_line, gate_error.line, "{zones}");
        assert_eq!(team_message, gate_error.message, "{zones}");
    }
}

/// "Reader": gate `off` keeps `[zones]` unread, a broken one included, while a
/// `[team]` beside it errs at it. A `lead` may name a zone `[gate] zones` does
/// not list: the table is `[zones]`, whatever the gate controls. M: the gate
/// reading `[zones]` when off; the lookup going through `[gate] zones`.
#[test]
fn gate_off_keeps_zones_unread_and_a_team_reads_all_of_them() {
    let broken = "[project]\nslug = \"press\"\n\n[zones]\nBad = []\n\n[gate]\nmode = \"off\"\n";
    let gate = GateConfig::from_toml(broken)
        .expect("gate off")
        .expect("a [gate] table");
    assert_eq!(gate.mode, GateMode::Off);
    assert!(gate.zones.is_empty());
    assert!(TeamConfig::from_toml(broken).expect("no [team]").is_none());

    let team = format!("{broken}\n[team.mira]\nrole = \"member\"\n{EMAIL}\n");
    let (line, _) = error(&team);
    assert_eq!(line, Some(line_of(&team, "Bad = ")));

    let listed = format!(
        "[project]\nslug = \"press\"\n\n[zones]\nbrief = [\"docs/**\"]\nmaps = [\"maps/**\"]\n\n\
         [gate]\nmode = \"selective\"\nzones = [\"brief\"]\n\n[team.mira]\nrole = \"lead\"\nzones \
         = [\"maps\"]\n{EMAIL}\n"
    );
    assert_eq!(read(&listed).entry("mira").expect("mira").zones.len(), 1);
}

/// A1: a bad `[team]` leaves the project's reader and the gate's as they were:
/// `RawFile` only name-checks the table. M: `ProjectConfig` reading it.
#[test]
fn a_bad_team_leaves_the_project_and_the_gate_readers() {
    let bad = table("mira", "role = \"boss\"\nhat = 5");
    assert!(TeamConfig::from_toml(&bad).is_err());
    let plain = bad
        .replace("\nhat = 5", "")
        .replace("\"boss\"", "\"member\"");

    let read_project = |text: &str| {
        let project = ProjectConfig::from_toml(text).expect("the project reads");
        format!("{project:?}")
    };
    assert_eq!(read_project(&bad), read_project(&plain));

    let gate = "\n[gate]\nmode = \"observe\"\nzones = [\"brief\"]\n";
    let gated = GateConfig::from_toml(&format!("{bad}{gate}"))
        .expect("the gate reads")
        .expect("a [gate] table");
    assert_eq!(gated.mode, GateMode::Observe);
}
