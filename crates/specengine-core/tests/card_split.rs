//! docs/features/department-split.md (RM-0100, v5 step 6c, ADR-0078)
//! "Acceptance criteria", the pure half: core's `card_from_toml` over
//! `[[card.split]]` rows (S1), `split_plan` on built configs and scopes (S2),
//! and the gamedev preset as committed with its `sound` flow, `sound`
//! workspace, header sentence and commented rows (S9). The canon is
//! docs/canon/card-split.md "The rule".
//!
//! - S1: card-press's two rows in config order; no key, `split = []`: no
//!   rows, every flow as before; each line of the error table at its line,
//!   from a probe config each; `disciplines`' list messages the flows'; 64
//!   rows read, the 65th refused; a row naming a discipline no flow lists is
//!   the listing error, under `PLAIN_FLOW` too; the listing check waits for
//!   every flow, so a flow's own error stands alone; the preset with its two
//!   rows uncommented reads six flows and two rows. M: the listing check
//!   dropped; a kind compared trimmed; `split` judged before every flow reads.
//! - S2: the criterion's scope `[(A, scene), (A-VO, voice), (A-X, None),
//!   (B, voice)]`; a row `Scene` matches no `scene` node, a `None` kind none;
//!   70 nodes of one kind take 64 and the note, 64 none; `chosen` in written
//!   order; each refusal of a chosen list with its line. M: kinds matched
//!   case-insensitively; `suggested` in discipline order; a child with no
//!   kind node given `[]`; the cap at 65.
//! - S9 (the core half): the preset's `sound` flow, `sound` workspace
//!   between `art` and `production`, commented rows and header sentence,
//!   verbatim; `studio-lamp`'s copy byte-equal. M: the header sentence
//!   dropped.
//!
//! The probe words are invented here or the fixtures' own (a print shop's
//! `copy` and `plates`, its `broadsheet` and `print-run` kinds); S2's are
//! the criterion's. The core names none (ADR-0008). Every expected value is
//! worked out from the spec, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::fs;

use common::{fixture, repository_root};
use specengine_core::card_split::{
    MAX_SPLIT_CHILDREN, PlannedChild, ScopeNode, SplitPlan, TargetsFrom, split_plan,
};
use specengine_core::card_toml::{CardConfig, CardSplit, MAX_SPLITS};
use specengine_core::{PLAIN_FLOW, card_from_toml};

/// How every message names the config.
const CONFIG: &str = "specengine.toml";

/// A print shop's two flows: `copy` listing `copy`, `plates` listing
/// `plates`.
const FLOWS: &str = "[[card.flow]]\nname = \"copy\"\ntitle = \"Copy desk\"\ndisciplines = \
                     [\"copy\"]\ncolumns = [\"draft\", \"edit\", \"proof\", \"filed\"]\naccept = \
                     { from = \"proof\", to = \"filed\" }\n\n[[card.flow]]\nname = \
                     \"plates\"\ndisciplines = [\"plates\"]\ncolumns = [\"queued\", \"blocked\", \
                     \"checked\", \"struck\"]\naccept = { from = \"checked\", to = \"struck\" }\n";

/// The project table every probe opens with.
const HEAD: &str = "[project]\nslug = \"press\"\n\n";

/// `[project]`, the two flows, then `rows`.
fn press(rows: &str) -> String {
    format!("{HEAD}{FLOWS}\n{rows}")
}

/// One `[[card.split]]` row of `kind` listing `disciplines`.
fn row(kind: &str, disciplines: &[&str]) -> String {
    let listed: Vec<String> = disciplines
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect();
    format!(
        "[[card.split]]\nkind = \"{kind}\"\ndisciplines = [{}]\n\n",
        listed.join(", ")
    )
}

/// The 1-based line of the `nth` line of `text` holding `needle`.
fn line_holding(text: &str, needle: &str, nth: usize) -> usize {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .nth(nth - 1)
        .map(|(index, _)| index + 1)
        .unwrap_or_else(|| panic!("no line {nth} holding {needle:?} in\n{text}"))
}

/// `<config>:<line>: <message>`.
fn note(line: usize, message: &str) -> String {
    format!("{CONFIG}:{line}: {message}")
}

/// `text` read, or the error in full.
fn read(text: &str) -> CardConfig {
    card_from_toml(text).unwrap_or_else(|error| panic!("{}\n{text}", error.at(CONFIG)))
}

/// `text`'s first error, `<config>:<line>: <message>`: a flow's, which the
/// reader returns as its `Err`.
fn refused(text: &str) -> String {
    match card_from_toml(text) {
        Ok(config) => panic!("read, not refused: {:?}\n{text}", config.splits),
        Err(error) => error.at(CONFIG),
    }
}

/// `text`'s split problem, `<config>:<line>: <message>`: since
/// docs/features/release-hardening.md "Acceptance criteria" RH-R4 a bad
/// `[card] split` is no `Err` but `split_problem`, splitting off whole (no
/// row read) and the flows served.
fn split_off(text: &str) -> String {
    match card_from_toml(text) {
        Ok(config) => {
            let problem = config
                .split_problem
                .unwrap_or_else(|| panic!("read, no split problem: {:?}\n{text}", config.splits));
            assert_eq!(config.splits, [], "splitting off whole\n{text}");
            problem.at(CONFIG)
        }
        Err(error) => panic!("refused, not split off: {}\n{text}", error.at(CONFIG)),
    }
}

/// `(kind, disciplines)` of every row, in order.
fn rows_of(config: &CardConfig) -> Vec<(String, Vec<String>)> {
    config
        .splits
        .iter()
        .map(|CardSplit { kind, disciplines }| (kind.clone(), disciplines.clone()))
        .collect()
}

/// `(kind, disciplines)` owned.
fn want(kind: &str, disciplines: &[&str]) -> (String, Vec<String>) {
    (
        kind.to_owned(),
        disciplines.iter().map(|name| (*name).to_owned()).collect(),
    )
}

// ------------------------------------------------------------------ S1

/// S1: card-press's `[card]` reads `splits` `[{broadsheet, [copy, plates]},
/// {print-run, [plates]}]`, config order; the same text without the rows
/// reads no row and the same flows, byte for byte.
#[test]
fn s1_card_press_reads_its_two_rows_in_config_order() {
    let text = fs::read_to_string(fixture("card-press").join(CONFIG)).expect("card-press");
    let config = read(&text);
    assert_eq!(
        rows_of(&config),
        [
            want("broadsheet", &["copy", "plates"]),
            want("print-run", &["plates"])
        ]
    );
    assert_eq!(MAX_SPLITS, 64);
    // The rows cut out: the flows as they were, no row.
    let start = text
        .find("\n[[card.split]]\n")
        .expect("a row in card-press");
    let end = text[start..]
        .find("\n[[workspace]]\n")
        .map_or(text.len(), |at| start + at);
    let without = format!("{}{}", &text[..start], &text[end..]);
    assert!(!without.contains("\n[[card.split]]"), "{without}");
    let plain = read(&without);
    assert!(plain.splits.is_empty(), "{:?}", plain.splits);
    assert_eq!(plain.flows, config.flows, "every flow as before");
    // An empty array is no key; `PLAIN_FLOW` reads no row.
    let empty = read(&format!("{HEAD}[card]\nsplit = []\n\n{FLOWS}"));
    assert!(empty.splits.is_empty());
    assert_eq!(empty.flows, read(&format!("{HEAD}{FLOWS}")).flows);
    assert!(read(PLAIN_FLOW).splits.is_empty());
    assert!(read(HEAD).splits.is_empty());
}

/// S1: a row's kind is any string within the bounds, an inner space and
/// any script included, compared as written: `Scene` and `scene` are two
/// rows. The bound counts bytes: 64 bytes read, 32 two-byte characters too.
#[test]
fn s1_a_kind_is_data_within_its_bounds() {
    let wide = "\u{e9}".repeat(32);
    let at_bound = "k".repeat(64);
    let text = press(&format!(
        "{}{}{}{}{}",
        row("big sheet", &["copy"]),
        row("Scene", &["copy"]),
        row("scene", &["plates"]),
        row(&wide, &["copy", "plates"]),
        row(&at_bound, &["plates"]),
    ));
    assert_eq!(
        rows_of(&read(&text)),
        [
            want("big sheet", &["copy"]),
            want("Scene", &["copy"]),
            want("scene", &["plates"]),
            want(&wide, &["copy", "plates"]),
            want(&at_bound, &["plates"]),
        ]
    );
}

/// One probe of the error table: its label, its text, the line its error
/// stands on and the message.
struct Probe {
    label: &'static str,
    text: String,
    line: usize,
    message: String,
}

/// The probes of the error table, one per line, each from its own config.
fn probes() -> Vec<Probe> {
    let mut probes = Vec::new();
    let text = format!("{HEAD}[card]\nbogus = 1\n\n{FLOWS}");
    probes.push(Probe {
        label: "another key in [card]",
        line: line_holding(&text, "bogus = 1", 1),
        message: "unknown field `bogus`, expected `flow` or `split`".to_owned(),
        text,
    });
    let text = format!("{HEAD}[card]\nsplit = 1\n\n{FLOWS}");
    probes.push(Probe {
        label: "split not an array",
        line: line_holding(&text, "split = 1", 1),
        message: "`[card] split` is an array of tables (`[[card.split]]`), not an integer"
            .to_owned(),
        text,
    });
    let text = format!("{HEAD}[card]\nsplit = {{ kind = \"x\" }}\n\n{FLOWS}");
    probes.push(Probe {
        label: "split a table",
        line: line_holding(&text, "split = {", 1),
        message: "`[card] split` is an array of tables (`[[card.split]]`), not a table".to_owned(),
        text,
    });
    let many: String = (1..=65)
        .map(|at| row(&format!("kind-{at:02}"), &["copy"]))
        .collect();
    let text = press(&many);
    probes.push(Probe {
        label: "65 rows",
        line: line_holding(&text, "[[card.split]]", 65),
        message: "at most 64 `[[card.split]]` tables".to_owned(),
        text,
    });
    let text = format!("{HEAD}[card]\nsplit = [\n  1,\n]\n\n{FLOWS}");
    probes.push(Probe {
        label: "an entry that is no table",
        line: line_holding(&text, "  1,", 1),
        message: "`[[card.split]]` is a table, not an integer".to_owned(),
        text,
    });
    let text = press(
        "[[card.split]]\nkind = \"broadsheet\"\ndisciplines = [\"copy\"]\ncolour = \"red\"\n",
    );
    probes.push(Probe {
        label: "another key in a row",
        line: line_holding(&text, "colour", 1),
        message: "unknown field `colour`, expected `kind` or `disciplines`".to_owned(),
        text,
    });
    let text = press("[[card.split]]\ndisciplines = [\"copy\"]\n");
    probes.push(Probe {
        label: "no kind",
        line: line_holding(&text, "[[card.split]]", 1),
        message: "`[[card.split]]` has no `kind`".to_owned(),
        text,
    });
    let text = press("[[card.split]]\nkind = \"broadsheet\"\n");
    probes.push(Probe {
        label: "no disciplines",
        line: line_holding(&text, "[[card.split]]", 1),
        message: "`[[card.split]]` `broadsheet` has no `disciplines`".to_owned(),
        text,
    });
    let text = press("[[card.split]]\nkind = 3\ndisciplines = [\"copy\"]\n");
    probes.push(Probe {
        label: "a kind that is no string",
        line: line_holding(&text, "kind = 3", 1),
        message: "`kind` is a string, not an integer".to_owned(),
        text,
    });
    let text = press(&row("", &["copy"]));
    probes.push(Probe {
        label: "an empty kind",
        line: line_holding(&text, "kind = \"\"", 1),
        message: "`kind`: `` is 0 bytes long; a kind has 1 to 64".to_owned(),
        text,
    });
    let long = "k".repeat(65);
    let text = press(&row(&long, &["copy"]));
    probes.push(Probe {
        label: "a kind of 65 bytes",
        line: line_holding(&text, &long, 1),
        message: format!("`kind`: `{long}` is 65 bytes long; a kind has 1 to 64"),
        text,
    });
    // 33 characters, 66 bytes: the bound counts bytes.
    let wide = "\u{e9}".repeat(33);
    let text = press(&row(&wide, &["copy"]));
    probes.push(Probe {
        label: "a kind of 33 two-byte characters",
        line: line_holding(&text, &wide, 1),
        message: format!("`kind`: `{wide}` is 66 bytes long; a kind has 1 to 64"),
        text,
    });
    let text = press("[[card.split]]\nkind = \"broad\\tsheet\"\ndisciplines = [\"copy\"]\n");
    probes.push(Probe {
        label: "a control character",
        line: line_holding(&text, "broad\\tsheet", 1),
        message: "`kind`: a kind holds a control character".to_owned(),
        text,
    });
    // M: a kind compared trimmed reads both of these as `broadsheet`.
    for (label, kind) in [
        ("a space at the end", "broadsheet "),
        ("a space at the start", " broadsheet"),
    ] {
        let text = press(&row(kind, &["copy"]));
        probes.push(Probe {
            label,
            line: line_holding(&text, "kind = ", 1),
            message: format!("`kind`: `{kind}` has a space at an end"),
            text,
        });
    }
    let text = press(&format!(
        "{}{}",
        row("broadsheet", &["copy"]),
        row("broadsheet", &["plates"])
    ));
    probes.push(Probe {
        label: "a kind written twice",
        line: line_holding(&text, "kind = \"broadsheet\"", 2),
        message: "`[[card.split]]` kind `broadsheet` is written twice".to_owned(),
        text,
    });
    let text = press(&row("broadsheet", &[]));
    probes.push(Probe {
        label: "no discipline listed",
        line: line_holding(&text, "disciplines = []", 1),
        message: "`disciplines` lists none; a row lists 1 to 32".to_owned(),
        text,
    });
    // The listing error, at the entry: `voice` on its own line.
    let text = press(
        "[[card.split]]\nkind = \"broadsheet\"\ndisciplines = [\n  \"copy\",\n  \"voice\",\n]\n",
    );
    probes.push(Probe {
        label: "a discipline no flow lists",
        line: line_holding(&text, "\"voice\",", 1),
        message: "`[[card.split]]` `broadsheet`: no `[[card.flow]]` lists the discipline `voice`"
            .to_owned(),
        text,
    });
    // `disciplines`' type, grammar and repeat: the flows' list reader's.
    let text = press("[[card.split]]\nkind = \"broadsheet\"\ndisciplines = \"copy\"\n");
    probes.push(Probe {
        label: "disciplines a string",
        line: line_holding(&text, "disciplines = \"copy\"", 1),
        message: "`disciplines` is a list of strings, not a string".to_owned(),
        text,
    });
    let text = press(
        "[[card.split]]\nkind = \"broadsheet\"\ndisciplines = [\n  \"copy\",\n  \"Plates\",\n]\n",
    );
    probes.push(Probe {
        label: "a discipline off the grammar",
        line: line_holding(&text, "\"Plates\",", 1),
        message: "`disciplines`: `Plates` is not a name: lower-case ASCII letters, digits, `_` \
                  and `-`"
            .to_owned(),
        text,
    });
    let text = press(
        "[[card.split]]\nkind = \"broadsheet\"\ndisciplines = [\n  \"copy\",\n  \"copy\",\n]\n",
    );
    probes.push(Probe {
        label: "a discipline listed twice",
        line: line_holding(&text, "\"copy\",", 2),
        message: "`disciplines`: `copy` is listed twice".to_owned(),
        text,
    });
    probes
}

/// S1: each line of the error table, from a probe config each, at its line:
/// `<config>:<line>: <message>`, the first error by line, as the split
/// problem (RH-R4: splitting off, no row read, never the reader's `Err`).
/// M: the listing check dropped (`voice` reads); a kind compared trimmed
/// (the two space probes read); split errors joined to `[card]`'s again.
#[test]
fn s1_each_error_of_the_table_stands_at_its_line() {
    let mut wrong = Vec::new();
    for probe in probes() {
        let got = match card_from_toml(&probe.text) {
            Ok(config) => match config.split_problem {
                Some(problem) if config.splits.is_empty() => problem.at(CONFIG),
                _ => format!("read: {:?}", rows_of(&config)),
            },
            Err(error) => format!("refused whole: {}", error.at(CONFIG)),
        };
        // `[card]`'s own key is no split row: the whole table is refused.
        let expected = if probe.label == "another key in [card]" {
            format!("refused whole: {}", note(probe.line, &probe.message))
        } else {
            note(probe.line, &probe.message)
        };
        if got != expected {
            wrong.push(format!("{}:\n  want {expected}\n  got  {got}", probe.label));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// S1: 32 disciplines in a row read, 33 do not, `disciplines`' 33rd-entry
/// message the flows' list reader's (its words left to that reader: one
/// naming `disciplines` and 32).
#[test]
fn s1_a_row_lists_at_most_thirty_two_disciplines() {
    let names: Vec<String> = (1..=33).map(|at| format!("desk-{at:02}")).collect();
    let quoted = |count: usize| -> String {
        names[..count]
            .iter()
            .map(|name| format!("  \"{name}\",\n"))
            .collect()
    };
    let flow = format!(
        "[[card.flow]]\nname = \"desks\"\ndisciplines = [\n{}]\ncolumns = [\"open\", \
         \"shut\"]\naccept = {{ from = \"open\", to = \"shut\" }}\n\n",
        quoted(33)
    );
    let probe = |count: usize| {
        format!(
            "{HEAD}{flow}[[card.split]]\nkind = \"desk\"\ndisciplines = [\n{}]\n",
            quoted(count)
        )
    };
    let config = read(&probe(32));
    assert_eq!(config.splits.len(), 1);
    assert_eq!(config.splits[0].disciplines, names[..32]);
    let text = probe(33);
    let message = split_off(&text);
    assert!(
        message.contains("disciplines") && message.contains("32"),
        "{message}"
    );
    let rows_start = line_holding(&text, "[[card.split]]", 1);
    let line: usize = message[CONFIG.len() + 1..]
        .split(':')
        .next()
        .and_then(|digits| digits.parse().ok())
        .unwrap_or_else(|| panic!("a line: {message}"));
    assert!(line > rows_start, "at the row's list: {message}");
}

/// S1: 64 rows read in config order; the 65th is refused at its table
/// (the error table's probe), the first 64 never half-served.
#[test]
fn s1_sixty_four_rows_read() {
    let many: String = (1..=64)
        .map(|at| row(&format!("kind-{at:02}"), &["copy"]))
        .collect();
    let config = read(&press(&many));
    let kinds: Vec<String> = config.splits.iter().map(|row| row.kind.clone()).collect();
    let want: Vec<String> = (1..=64).map(|at| format!("kind-{at:02}")).collect();
    assert_eq!(kinds, want);
}

/// S1: the listing check runs once every flow reads, and a flow's own error
/// stands alone. Rows written before the flows read (the check waits for
/// them); a row naming the discipline of a flow that does not read leaves
/// that flow's error first, though the row stands earlier. M: `split`
/// judged before every flow reads (a flow's error masked).
#[test]
fn s1_the_listing_check_waits_for_every_flow() {
    let early = format!(
        "{HEAD}{}{}{FLOWS}",
        row("broadsheet", &["copy", "plates"]),
        row("print-run", &["plates"])
    );
    assert_eq!(
        rows_of(&read(&early)),
        [
            want("broadsheet", &["copy", "plates"]),
            want("print-run", &["plates"])
        ],
        "rows above the flows read"
    );
    let broken = FLOWS.replace(
        "accept = { from = \"checked\", to = \"struck\" }",
        "accept = { from = \"checked\", to = \"melted\" }",
    );
    let text = format!("{HEAD}{}{broken}", row("print-run", &["plates"]));
    assert_eq!(
        refused(&text),
        note(
            line_holding(&text, "melted", 1),
            "`accept.to`: `melted` is no column of this flow"
        ),
        "the flow's own error, alone"
    );
    // The same with the rows after the flows.
    let text = format!("{HEAD}{broken}\n{}", row("print-run", &["plates"]));
    assert_eq!(
        refused(&text),
        note(
            line_holding(&text, "melted", 1),
            "`accept.to`: `melted` is no column of this flow"
        )
    );
}

/// S1: a flow listing none takes no row's discipline: under `PLAIN_FLOW`
/// (no `[[card.flow]]`) a row is the listing error.
#[test]
fn s1_under_the_plain_flow_no_row_reads() {
    let text = format!("{HEAD}{}", row("broadsheet", &["copy"]));
    assert_eq!(
        split_off(&text),
        note(
            line_holding(&text, "disciplines", 1),
            "`[[card.split]]` `broadsheet`: no `[[card.flow]]` lists the discipline `copy`"
        )
    );
}

/// S1: the same text, read twice, gives the same rows (pure).
#[test]
fn s1_the_read_is_deterministic() {
    let text = fs::read_to_string(fixture("card-press").join(CONFIG)).expect("card-press");
    assert_eq!(read(&text), read(&text));
}

// ------------------------------------------------------------- the preset

/// `scripts/presets/gamedev.toml` as committed.
fn preset() -> String {
    let path = repository_root().join("scripts/presets/gamedev.toml");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The preset's `sound` flow, its lines in order.
const SOUND_FLOW: [&str; 8] = [
    "[[card.flow]]",
    "name = \"sound\"",
    "title = \"Sound\"",
    "disciplines = [\"sound\"]",
    "columns = [\"backlog\", \"sent-back\", \"design\", \"recording\", \"implementation\", \
     \"review\", \"in-game\"]",
    "titles = { backlog = \"Backlog\", sent-back = \"Returned\", design = \"Sound design\", \
     recording = \"Recording\", implementation = \"Implementation\", review = \"Review\", \
     in-game = \"In game\" }",
    "accept = { from = \"review\", to = \"in-game\" }",
    "return = \"sent-back\"",
];

/// The preset's `sound` workspace, its lines in order.
const SOUND_WORKSPACE: [&str; 6] = [
    "[[workspace]]",
    "name = \"sound\"",
    "title = \"Sound\"",
    "sections = [\"board\", \"flow\", \"tree\", \"questions\", \"inbox\"]",
    "landing = \"board\"",
    "flows = [\"sound\"]",
];

/// The commented split rows, their thirteen lines in order: five of prose,
/// then the eight of the two tables.
const COMMENTED_ROWS: [&str; 13] = [
    "# A split template: a task on a document of this kind, or holding nodes of",
    "# it, pre-selects these disciplines in the Split dialog, and each subtask",
    "# targets the nodes of its kinds. It never refuses a split. Use your project's",
    "# own kinds, and add it only once every terminal, plugin and hub runs a build",
    "# that reads `[[card.split]]`.",
    "# [[card.split]]",
    "# kind = \"character\"",
    "# disciplines = [\"narrative\", \"art\", \"sound\"]",
    "#",
    "# [[card.split]]",
    "# kind = \"scene\"",
    "# disciplines = [\"narrative\", \"level-design\", \"sound\", \"development\"]",
    "",
];

/// The header's version sentence, backticks aside.
const HEADER_SENTENCE: &str = "A flow's title and return, and a [[card.split]] row, need a build \
                               that reads them on every terminal, plugin and hub serving the \
                               project, an older one refusing the key.";

/// The index of the first run of `block` in `lines`, the whole of it in a
/// row.
fn block_at(lines: &[&str], block: &[&str], what: &str) -> usize {
    let start = lines
        .iter()
        .position(|line| *line == block[0])
        .unwrap_or_else(|| panic!("{what}: no line {:?}", block[0]));
    let mut at = start;
    // Several blocks open alike (`[[card.flow]]`): the one whose next line
    // is the block's second.
    while lines.get(at..at + block.len()) != Some(block) {
        at = lines[at + 1..]
            .iter()
            .position(|line| *line == block[0])
            .map(|next| at + 1 + next)
            .unwrap_or_else(|| {
                panic!(
                    "{what}: not verbatim; from line {}:\n{}",
                    start + 1,
                    lines[start..(start + block.len() + 2).min(lines.len())].join("\n")
                )
            });
    }
    at
}

/// The sentences of the preset's header, its leading `#` lines joined,
/// backticks dropped.
fn header(text: &str) -> String {
    let joined: Vec<&str> = text
        .lines()
        .take_while(|line| line.starts_with('#'))
        .map(|line| line.trim_start_matches('#').trim())
        .collect();
    joined.join(" ").replace('`', "")
}

/// S9 (D11): the preset holds the `sound` flow after `production`'s, the
/// `sound` workspace after `art`'s and before `production`'s, the commented
/// rows after the commented direction, and the header's version sentence,
/// each verbatim; the TOML reader finds six flows, five workspaces and no
/// live row. M: the header sentence dropped.
#[test]
fn s9_the_preset_holds_the_sound_lines_and_the_commented_rows() {
    let text = preset();
    let lines: Vec<&str> = text.lines().collect();
    let sound = block_at(&lines, &SOUND_FLOW, "the sound flow");
    let production_flow = lines
        .iter()
        .position(|line| *line == "name = \"production\"")
        .expect("production's flow");
    assert!(sound > production_flow, "after production's flow");
    let workspace = block_at(&lines, &SOUND_WORKSPACE, "the sound workspace");
    let named = |name: &str| {
        let line = format!("name = \"{name}\"");
        lines
            .iter()
            .enumerate()
            .filter(|(_, written)| **written == line)
            .map(|(at, _)| at)
            .collect::<Vec<usize>>()
    };
    let art = *named("art").last().expect("art's workspace");
    let production = *named("production").last().expect("production's workspace");
    assert!(
        art < workspace && workspace < production,
        "after art's workspace, before production's: {art} {workspace} {production}"
    );
    let direction = lines
        .iter()
        .position(|line| *line == "# kinds = [\"scene\"]")
        .expect("the commented direction");
    let rows = block_at(&lines, &COMMENTED_ROWS[..12], "the commented rows");
    assert!(rows > direction, "after the commented direction");
    assert!(
        header(&text).contains(HEADER_SENTENCE),
        "M: the header sentence dropped: {}",
        header(&text)
    );

    let table: toml::Table = text.parse().expect("the preset is TOML");
    let flows: Vec<&str> = table["card"]["flow"]
        .as_array()
        .expect("[[card.flow]]")
        .iter()
        .map(|flow| flow["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(
        flows,
        [
            "narrative",
            "development",
            "art",
            "level-design",
            "production",
            "sound"
        ]
    );
    let workspaces: Vec<&str> = table["workspace"]
        .as_array()
        .expect("[[workspace]]")
        .iter()
        .map(|workspace| workspace["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(
        workspaces,
        ["writing", "development", "art", "sound", "production"]
    );
    assert!(
        table["card"].get("split").is_none(),
        "the preset's rows stay commented (Rollout)"
    );
}

/// S1: the preset with its two rows uncommented reads: six flows, two rows
/// as written.
#[test]
fn s1_the_preset_with_its_rows_uncommented_reads() {
    let text = preset();
    let commented = COMMENTED_ROWS[5..12].join("\n");
    assert!(text.contains(&commented), "the commented rows");
    let live = commented
        .lines()
        .map(|line| line.trim_start_matches('#').trim_start())
        .collect::<Vec<&str>>()
        .join("\n");
    let opened = format!(
        "[project]\nslug = \"preset\"\n{}",
        text.replacen(&commented, &live, 1)
    );
    let config = read(&opened);
    assert_eq!(config.flows.len(), 6);
    assert_eq!(
        rows_of(&config),
        [
            want("character", &["narrative", "art", "sound"]),
            want(
                "scene",
                &["narrative", "level-design", "sound", "development"]
            ),
        ]
    );
    // As committed: no row.
    assert!(
        read(&format!("[project]\nslug = \"preset\"\n{text}"))
            .splits
            .is_empty()
    );
}

/// S9: `fixtures/studio-lamp/specengine.toml` holds the preset's bytes
/// verbatim between its two marker lines.
#[test]
fn s9_studio_lamp_holds_the_preset_byte_for_byte() {
    let lamp = fs::read_to_string(fixture("studio-lamp").join(CONFIG)).expect("studio-lamp");
    let open = "# --- scripts/presets/gamedev.toml ---\n";
    let close = "# --- end of the preset ---\n";
    let start = lamp.find(open).expect("the opening marker") + open.len();
    let end = lamp.find(close).expect("the closing marker");
    assert_eq!(&lamp[start..end], preset(), "the copy, byte for byte");
}

// ------------------------------------------------------------------ S2

/// S2's three flows: `narrative`, `sound` and a third, `art`; titled, so
/// `flow_title` is no name; first columns of their own.
const TRADES: &str = "[project]\nslug = \"trades\"\n\n[[card.flow]]\nname = \
                      \"narrative\"\ntitle = \"Story\"\ndisciplines = [\"narrative\"]\ncolumns = \
                      [\"outline\", \"draft\", \"final\"]\naccept = { from = \"draft\", to = \
                      \"final\" }\n\n[[card.flow]]\nname = \"sound\"\ntitle = \
                      \"Audio\"\ndisciplines = [\"sound\"]\ncolumns = [\"design\", \"mix\", \
                      \"shipped\"]\naccept = { from = \"mix\", to = \"shipped\" }\n\n[[card.flow]]\n\
                      name = \"art\"\ndisciplines = [\"art\"]\ncolumns = [\"sketch\", \
                      \"paint\", \"hung\"]\naccept = { from = \"paint\", to = \"hung\" }\n\n";

/// [`TRADES`] with `rows`.
fn trades(rows: &[(&str, &[&str])]) -> CardConfig {
    let written: String = rows
        .iter()
        .map(|(kind, disciplines)| row(kind, disciplines))
        .collect();
    read(&format!("{TRADES}{written}"))
}

/// S2's rows: `scene: [narrative, sound]`, `voice: [sound]`.
fn criterion_config() -> CardConfig {
    trades(&[("scene", &["narrative", "sound"]), ("voice", &["sound"])])
}

fn node(target: &str, kind: Option<&str>) -> ScopeNode {
    ScopeNode {
        target: target.to_owned(),
        kind: kind.map(str::to_owned),
    }
}

/// S2's scope: `[(A, scene), (A-VO, voice), (A-X, None), (B, voice)]`.
fn criterion_scope() -> Vec<ScopeNode> {
    vec![
        node("A", Some("scene")),
        node("A-VO", Some("voice")),
        node("A-X", None),
        node("B", Some("voice")),
    ]
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

/// A child as a row to compare: `(discipline, flow, flow_title, column,
/// targets, from, suggested)`, `from` as served.
type Row = (String, String, String, String, Vec<String>, String, bool);

fn served(from: &TargetsFrom) -> String {
    serde_json::to_value(from)
        .expect("`from` serializes")
        .as_str()
        .expect("`from` is a string")
        .to_owned()
}

fn child_row(child: &PlannedChild) -> Row {
    (
        child.discipline.clone(),
        child.flow.clone(),
        child.flow_title.clone(),
        child.column.clone(),
        child.targets.clone(),
        served(&child.from),
        child.suggested,
    )
}

fn want_row(
    discipline: &str,
    (flow, title, column): (&str, &str, &str),
    targets: &[&str],
    from: &str,
    suggested: bool,
) -> Row {
    (
        discipline.to_owned(),
        flow.to_owned(),
        title.to_owned(),
        column.to_owned(),
        strings(targets),
        from.to_owned(),
        suggested,
    )
}

const NARRATIVE: (&str, &str, &str) = ("narrative", "Story", "outline");
const SOUND: (&str, &str, &str) = ("sound", "Audio", "design");
const ART: (&str, &str, &str) = ("art", "art", "sketch");

fn plan(
    config: &CardConfig,
    scope: &[ScopeNode],
    parent: &[&str],
    chosen: Option<&[&str]>,
) -> Result<SplitPlan, String> {
    let chosen: Option<Vec<String>> = chosen.map(strings);
    split_plan(config, scope, &strings(parent), chosen.as_deref())
}

fn planned(
    config: &CardConfig,
    scope: &[ScopeNode],
    parent: &[&str],
    chosen: Option<&[&str]>,
) -> SplitPlan {
    plan(config, scope, parent, chosen).unwrap_or_else(|reason| panic!("refused: {reason}"))
}

fn rows(plan: &SplitPlan) -> Vec<Row> {
    plan.children.iter().map(child_row).collect()
}

/// S2, the criterion's case: chosen none, so every discipline the flows
/// list, flow order; `suggested` `[narrative, sound]`; `sound` targets
/// `[A, A-VO, B]` by kind; `narrative` `[A]`; `art`, in no matched row, the
/// parent's targets, not suggested; `A-X`, kindless, in no child. M: a child
/// with no kind node given `[]`.
#[test]
fn s2_the_criterions_plan() {
    let config = criterion_config();
    let scope = criterion_scope();
    let done = planned(&config, &scope, &["A", "B"], None);
    assert_eq!(done.suggested, strings(&["narrative", "sound"]));
    assert_eq!(
        rows(&done),
        [
            want_row("narrative", NARRATIVE, &["A"], "kinds", true),
            want_row("sound", SOUND, &["A", "A-VO", "B"], "kinds", true),
            want_row("art", ART, &["A", "B"], "parent", false),
        ]
    );
    assert!(done.notes.is_empty(), "{:?}", done.notes);
    assert!(matches!(done.children[0].from, TargetsFrom::Kinds));
    assert!(matches!(done.children[2].from, TargetsFrom::Parent));
    // Pure: one state, one result.
    assert_eq!(
        rows(&planned(&config, &scope, &["A", "B"], None)),
        rows(&done)
    );
}

/// S2: `suggested` is the matched rows' disciplines in row order, then list
/// order, each once; never the flows' order nor the alphabet. M:
/// `suggested` in discipline order instead of row order.
#[test]
fn s2_suggested_follows_the_rows() {
    let config = trades(&[
        ("voice", &["sound"]),
        ("scene", &["art", "narrative", "sound"]),
    ]);
    let done = planned(&config, &criterion_scope(), &["A"], None);
    assert_eq!(done.suggested, strings(&["sound", "art", "narrative"]));
    // Children stay in flow order; each marked by the list.
    assert_eq!(
        rows(&done),
        [
            want_row("narrative", NARRATIVE, &["A"], "kinds", true),
            want_row("sound", SOUND, &["A", "A-VO", "B"], "kinds", true),
            want_row("art", ART, &["A"], "kinds", true),
        ]
    );
    // A row whose kind no scope node holds suggests nothing.
    let config = trades(&[("island", &["art"]), ("voice", &["sound"])]);
    let done = planned(&config, &criterion_scope(), &["A"], None);
    assert_eq!(done.suggested, strings(&["sound"]));
    assert_eq!(
        rows(&done)[2],
        want_row("art", ART, &["A"], "parent", false),
        "an unmatched row refuses nothing and picks nothing"
    );
}

/// S2: a row `Scene` matches no `scene` node; a `None` kind matches none, a
/// scope of kindless nodes suggesting nothing. M: kinds matched
/// case-insensitively.
#[test]
fn s2_a_kind_matches_byte_for_byte() {
    let config = trades(&[("Scene", &["narrative"])]);
    let done = planned(&config, &criterion_scope(), &["A"], None);
    assert!(done.suggested.is_empty(), "{:?}", done.suggested);
    assert_eq!(
        rows(&done),
        [
            want_row("narrative", NARRATIVE, &["A"], "parent", false),
            want_row("sound", SOUND, &["A"], "parent", false),
            want_row("art", ART, &["A"], "parent", false),
        ]
    );
    let kindless = [node("A", None), node("A-X", None)];
    let done = planned(&criterion_config(), &kindless, &["A"], None);
    assert!(done.suggested.is_empty());
    assert!(
        done.children
            .iter()
            .all(|child| matches!(child.from, TargetsFrom::Parent))
    );
    // A kind is a project's word, inner spaces and all: matched as written.
    let config = trades(&[("big scene", &["narrative"])]);
    let spaced = [node("A", Some("big scene")), node("B", Some("big  scene"))];
    let done = planned(&config, &spaced, &["A"], None);
    assert_eq!(done.suggested, strings(&["narrative"]));
    assert_eq!(done.children[0].targets, strings(&["A"]));
}

/// The kind `scene`'s `count` scope nodes, `N-001` on.
fn many_scenes(count: usize) -> Vec<ScopeNode> {
    (1..=count)
        .map(|at| node(&format!("N-{at:03}"), Some("scene")))
        .collect()
}

/// S2: 70 scope nodes of one kind: the first 64 in scope order and the note
/// `` subtask `<d>`: <n> nodes of its kinds; the first 64 taken ``, never a
/// refusal; 64 take all with no note; 65 take 64 with the note. M: the cap
/// at 65.
#[test]
fn s2_a_child_takes_at_most_sixty_four_nodes() {
    let config = trades(&[("scene", &["narrative"])]);
    let chosen: &[&str] = &["narrative", "art"];
    let scope = many_scenes(70);
    let done = planned(&config, &scope, &["N-001"], Some(chosen));
    let first: Vec<String> = scope[..64].iter().map(|node| node.target.clone()).collect();
    assert_eq!(done.children[0].targets, first);
    assert!(matches!(done.children[0].from, TargetsFrom::Kinds));
    assert_eq!(
        done.notes,
        ["subtask `narrative`: 70 nodes of its kinds; the first 64 taken"]
    );
    assert_eq!(done.children[1].targets, strings(&["N-001"]));
    let at_cap = planned(&config, &many_scenes(64), &["N-001"], Some(chosen));
    assert_eq!(at_cap.children[0].targets.len(), 64);
    assert!(at_cap.notes.is_empty(), "{:?}", at_cap.notes);
    let over = planned(&config, &many_scenes(65), &["N-001"], Some(chosen));
    assert_eq!(over.children[0].targets.len(), 64);
    assert_eq!(
        over.notes,
        ["subtask `narrative`: 65 nodes of its kinds; the first 64 taken"]
    );
}

/// S2: `chosen` `[art, sound]` keeps that order, `suggested` marking each by
/// the rows; only the chosen are planned.
#[test]
fn s2_chosen_keeps_its_order() {
    let config = criterion_config();
    let chosen: &[&str] = &["art", "sound"];
    let done = planned(&config, &criterion_scope(), &["A"], Some(chosen));
    assert_eq!(
        rows(&done),
        [
            want_row("art", ART, &["A"], "parent", false),
            want_row("sound", SOUND, &["A", "A-VO", "B"], "kinds", true),
        ]
    );
    assert_eq!(done.suggested, strings(&["narrative", "sound"]));
}

/// S2: `[]`, a 33rd, a repeat and a name off the grammar each `Err` with
/// their line; `nowhere`, which no flow takes, `flow_for`'s `Err` prefixed
/// `child: `, the first in child order.
#[test]
fn s2_each_refusal_of_a_chosen_list_has_its_line() {
    assert_eq!(MAX_SPLIT_CHILDREN, 32);
    let config = criterion_config();
    let scope = criterion_scope();
    let refusal =
        |chosen: &[&str]| plan(&config, &scope, &["A"], Some(chosen)).expect_err("refused");
    assert_eq!(refusal(&[]), "child: none named; name 1 to 32");
    let names: Vec<String> = (1..=33).map(|at| format!("d{at}")).collect();
    let many: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_eq!(refusal(&many), "child: 33 named; a split makes 1 to 32");
    assert_eq!(
        refusal(&["sound", "art", "sound"]),
        "child: `sound` is named twice"
    );
    assert_eq!(
        refusal(&["sound", "Art"]),
        "child: `Art` is not a name: lower-case ASCII letters, digits, `_` and `-`"
    );
    let reason = config
        .flow_for(Some("nowhere"))
        .expect_err("no flow takes `nowhere`");
    assert_eq!(refusal(&["sound", "nowhere"]), format!("child: {reason}"));
    assert!(
        reason.starts_with("no flow takes the discipline `nowhere`"),
        "{reason}"
    );
    let other = config.flow_for(Some("elsewhere")).expect_err("none");
    assert_eq!(
        refusal(&["elsewhere", "nowhere"]),
        format!("child: {other}"),
        "the first in child order"
    );
    // 32 distinct names the flows do not list: past the count, `flow_for`.
    let names: Vec<&str> = many[..32].to_vec();
    let first = config.flow_for(Some("d1")).expect_err("none");
    assert_eq!(refusal(&names), format!("child: {first}"));
}

/// S2 (rules): no parent target, so no scope: nothing suggested, every
/// child a manual card (`[]`, `parent`).
#[test]
fn s2_no_parent_target_plans_manual_children() {
    let config = criterion_config();
    let done = planned(&config, &[], &[], None);
    assert!(done.suggested.is_empty());
    assert_eq!(
        rows(&done),
        [
            want_row("narrative", NARRATIVE, &[], "parent", false),
            want_row("sound", SOUND, &[], "parent", false),
            want_row("art", ART, &[], "parent", false),
        ]
    );
}

// ------------------------------------------------- release-hardening RH-R4

/// docs/features/release-hardening.md "Acceptance criteria" RH-R4, the pure
/// half, on Data's row (`kind = "mechanic"`, `disciplnes`): the reader is
/// `Ok`, its flows those of the text without the row, `splits` empty and
/// `split_problem` the unknown field at its line; a good row beside it is
/// not served either (splitting off whole); a bad flow with the row is the
/// flow's `Err`, as before. M: split errors joined to `[card]`'s again.
#[test]
fn rh_r4_a_bad_split_row_turns_splitting_off_alone() {
    let good = row("broadsheet", &["copy", "plates"]);
    let bad = "[[card.split]]\nkind = \"mechanic\"\ndisciplnes = [\"copy\"]\n";
    let text = press(&format!("{good}{bad}"));
    let config = card_from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at(CONFIG)));
    let plain = read(&press(""));
    assert_eq!(config.flows, plain.flows, "the flows served as without it");
    assert_eq!(config.splits, [], "no row served, the good one neither");
    assert_eq!(
        config.split_problem.map(|problem| problem.at(CONFIG)),
        Some(note(
            line_holding(&text, "disciplnes", 1),
            "unknown field `disciplnes`, expected `kind` or `disciplines`"
        ))
    );
    let broken = press(bad).replace(
        "accept = { from = \"checked\", to = \"struck\" }",
        "accept = { from = \"checked\", to = \"melted\" }",
    );
    assert_eq!(
        refused(&broken),
        note(
            line_holding(&broken, "melted", 1),
            "`accept.to`: `melted` is no column of this flow"
        ),
        "a bad flow with the row: the flow's error"
    );
}
