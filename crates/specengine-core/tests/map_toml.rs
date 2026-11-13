//! docs/features/scope-rings.md (RM-0113) "Acceptance criteria", SR1:
//! `MapConfig::from_toml` reads "Data"'s config as two rings; each problem
//! of "Data"'s list (neither key, a 13th ring, `[map] x = 1`, a bad title,
//! root or kinds, a wrong type, an unknown key, `title` missing) serves no
//! ring and gives one note, `<problem>; no ring is served`, at its line,
//! the first problem by line when there are several; without `[map]` no
//! ring and no note; `ProjectConfig::from_toml` loads a file holding
//! `[map]`, good or bad. M: a bad ring dropped alone, the rest served;
//! `map` missing from `RawFile` (every command exits 2).
//!
//! Every config is real TOML read by the real readers; nothing is mocked.

use specengine_core::{MAP_RINGS_MAX, MapConfig, MapNote, ProjectConfig, RingConfig};

/// "Data"'s config, lines 1 to 8.
const DATA: &str = "[[map.ring]]\ntitle = \"Movement\"\nroot  = \"DOM-MOVEMENT\"\nkinds = [\"mechanic\"]\n\n[[map.ring]]\ntitle = \"Rules\"\nkinds = [\"rule\"]\n";

/// What every note ends with.
const SERVED: &str = "; no ring is served";

/// "Data"'s two rings.
fn data_rings() -> Vec<RingConfig> {
    vec![
        RingConfig {
            title: "Movement".to_owned(),
            root: Some("DOM-MOVEMENT".to_owned()),
            kinds: Some(vec!["mechanic".to_owned()]),
            line: 1,
        },
        RingConfig {
            title: "Rules".to_owned(),
            root: None,
            kinds: Some(vec!["rule".to_owned()]),
            line: 6,
        },
    ]
}

/// The one note `text` gives, with no ring served.
fn refused(text: &str) -> MapNote {
    let config = MapConfig::from_toml(text);
    assert_eq!(config.rings, Vec::new(), "no ring is served:\n{text}");
    assert_eq!(
        config.notes.len(),
        1,
        "one note:\n{text}\n{:?}",
        config.notes
    );
    let note = config.notes[0].clone();
    assert!(
        note.message.ends_with(SERVED),
        "{:?}:\n{text}",
        note.message
    );
    note
}

/// `text`'s note is `problem` at `line`.
fn assert_note(text: &str, line: usize, problem: &str) {
    let note = refused(text);
    assert_eq!(
        (note.line, note.message.as_str()),
        (line, format!("{problem}{SERVED}").as_str()),
        "\n{text}"
    );
}

/// "Data"'s config after `before` lines of other tables (the project's),
/// then `after`.
fn with(before: &str, after: &str) -> String {
    format!("{before}{DATA}{after}")
}

/// SR1: "Data"'s config reads two rings, in order, each with its header's
/// line; no note.
#[test]
fn sr1_datas_config_reads_two_rings() {
    let config = MapConfig::from_toml(DATA);
    assert_eq!(config.rings, data_rings());
    assert_eq!(config.notes, Vec::new());
    // Behind the project's own tables: each line moves with it.
    let project = "[project]\nslug = \"lantern-keep\"\n\n[ids]\nMEC = { kind = \"mechanic\", shape = \"name\" }\n\n";
    let config = MapConfig::from_toml(&with(project, ""));
    let mut want = data_rings();
    for ring in &mut want {
        ring.line += 6;
    }
    assert_eq!(config.rings, want);
    // A root alone, kinds listed in their order, any script in a title.
    let config = MapConfig::from_toml(
        "[[map.ring]]\ntitle = \"All\"\nroot = \"DOM-GAME\"\n\n[[map.ring]]\ntitle = \"\u{0412}\u{0441}\u{0451}\"\nroot = \"docs/spec/game.md\"\nkinds = [\"rule\", \"mechanic\"]\n",
    );
    assert_eq!(
        config.rings,
        [
            RingConfig {
                title: "All".to_owned(),
                root: Some("DOM-GAME".to_owned()),
                kinds: None,
                line: 1,
            },
            RingConfig {
                title: "\u{0412}\u{0441}\u{0451}".to_owned(),
                root: Some("docs/spec/game.md".to_owned()),
                kinds: Some(vec!["rule".to_owned(), "mechanic".to_owned()]),
                line: 5,
            },
        ]
    );
    assert_eq!(config.notes, Vec::new());
}

/// SR1: without `[map]`, and with no text at all, no ring and no note.
#[test]
fn sr1_without_map_no_ring_and_no_note() {
    for text in [
        "",
        "[project]\nslug = \"lantern-keep\"\n",
        "[project]\nslug = \"lantern-keep\"\n\n[[module]]\nname = \"m\"\ntitle = \"M\"\nkinds = [\"rule\"]\n",
    ] {
        assert_eq!(MapConfig::from_toml(text), MapConfig::default(), "{text}");
    }
}

/// SR1: twelve rings read; a 13th is "at most 12" at its header, no ring.
#[test]
fn sr1_at_most_twelve_rings() {
    assert_eq!(MAP_RINGS_MAX, 12);
    let ring = |n: usize| format!("[[map.ring]]\ntitle = \"Ring {n}\"\nkinds = [\"rule\"]\n\n");
    let twelve: String = (1..=12).map(ring).collect();
    let config = MapConfig::from_toml(&twelve);
    assert_eq!(config.rings.len(), 12);
    assert_eq!(config.notes, Vec::new());
    assert_eq!(config.rings[11].line, 45, "4 lines a ring");
    let thirteen: String = (1..=13).map(ring).collect();
    assert_eq!(
        MapConfig::from_toml(&thirteen).notes_at("specengine.toml"),
        [format!(
            "specengine.toml:49: at most 12 `[[map.ring]]` tables{SERVED}"
        )]
    );
    assert_note(&thirteen, 49, "at most 12 `[[map.ring]]` tables");
}

/// SR1: the table problems of "Data"'s list, each at its line, after a
/// good ring that is not served either. M: a bad ring dropped alone, the
/// rest served.
#[test]
fn sr1_each_problem_is_one_note_at_its_line_and_no_ring() {
    // Each bad table follows "Data"'s 8 lines and a blank: its header is
    // line 10.
    let cases: [(&str, usize, &str); 7] = [
        (
            "[[map.ring]]\ntitle = \"Movement\"\n",
            10,
            "`[[map.ring]]` `Movement` holds neither `root` nor `kinds`: a ring holds one or both",
        ),
        (
            "[[map.ring]]\nroot = \"DOM-GAME\"\n",
            10,
            "`[[map.ring]]` is required to hold `title`",
        ),
        ("[map]\nx = 1\n", 11, "`[map]` holds only `ring`"),
        (
            "[[map.ring]]\ntitle = \"\"\nkinds = [\"rule\"]\n",
            11,
            "`title` is 0 bytes long; a title has 1 to 64",
        ),
        (
            "[[map.ring]]\ntitle = \"Bell\\u0007tower\"\nkinds = [\"rule\"]\n",
            11,
            "`title` holds a control character",
        ),
        (
            "[[map.ring]]\ntitle = \"Rim\"\nroot = \"DOM-GAME \"\n",
            12,
            "`root` `DOM-GAME ` has a space at an end",
        ),
        (
            "[[map.ring]]\ntitle = \"Rim\"\nkinds = [\"rule\", \"rule\"]\n",
            12,
            "`kinds`: `rule` is listed twice",
        ),
    ];
    for (table, line, problem) in cases {
        let text = format!("{DATA}\n{table}");
        assert_note(&text, line, problem);
    }
    // The bounds of a title, a root and a kind.
    let long_title = "t".repeat(65);
    let long_root = "R".repeat(257);
    let long_kind = "k".repeat(65);
    let bounds: [(String, usize, String); 6] = [
        (
            format!("[[map.ring]]\ntitle = \"{long_title}\"\nkinds = [\"rule\"]\n"),
            2,
            "`title` is 65 bytes long; a title has 1 to 64".to_owned(),
        ),
        (
            "[[map.ring]]\ntitle = \"Rim\"\nroot = \"\"\n".to_owned(),
            3,
            "`root` is 0 bytes long; a root has 1 to 256".to_owned(),
        ),
        (
            format!("[[map.ring]]\ntitle = \"Rim\"\nroot = \"{long_root}\"\n"),
            3,
            "`root` is 257 bytes long; a root has 1 to 256".to_owned(),
        ),
        (
            "[[map.ring]]\ntitle = \"Rim\"\nroot = \"DOM\\tGAME\"\n".to_owned(),
            3,
            "`root` holds a control character".to_owned(),
        ),
        (
            "[[map.ring]]\ntitle = \"Rim\"\nkinds = []\n".to_owned(),
            3,
            "`kinds` lists no kind; a direction lists 1 to 32".to_owned(),
        ),
        (
            format!("[[map.ring]]\ntitle = \"Rim\"\nkinds = [\"rule\",\n  \"{long_kind}\"]\n"),
            4,
            format!("`kinds`: `{long_kind}` is 65 bytes long; a kind has 1 to 64"),
        ),
    ];
    for (text, line, problem) in &bounds {
        assert_note(text, *line, problem);
    }
    let mut kinds: Vec<String> = (0..33).map(|n| format!("\"k{n}\"")).collect();
    kinds[32] = format!("\n  {}", kinds[32]);
    assert_note(
        &format!(
            "[[map.ring]]\ntitle = \"Rim\"\nkinds = [{}]\n",
            kinds.join(", ")
        ),
        4,
        "a list holds at most 32 entries",
    );
    for (text, problem) in [
        (
            "[[map.ring]]\ntitle = \"Rim\"\nkinds = [\" rule\"]\n",
            "`kinds`: ` rule` has a space at an end",
        ),
        (
            "[[map.ring]]\ntitle = \"Rim\"\nkinds = [\"ru\\u0001le\"]\n",
            "`kinds`: a kind holds a control character",
        ),
    ] {
        assert_note(text, 3, problem);
    }
}

/// SR1: a wrong type, and an unknown key, get the TOML reader's message at
/// the value or the key.
#[test]
fn sr1_a_wrong_type_or_an_unknown_key_is_the_readers_message() {
    for (text, line, opening) in [
        (
            "[[map.ring]]\ntitle = 3\nkinds = [\"rule\"]\n",
            2,
            "invalid type: integer `3`, expected a string",
        ),
        (
            "[[map.ring]]\ntitle = \"Rim\"\nroot = [\"DOM-GAME\"]\n",
            3,
            "invalid type: sequence, expected a string",
        ),
        (
            "[[map.ring]]\ntitle = \"Rim\"\nkinds = \"rule\"\n",
            3,
            "invalid type: string \"rule\", expected a sequence",
        ),
        (
            "[[map.ring]]\ntitle = \"Rim\"\nkinds = [\"rule\", 4]\n",
            3,
            "invalid type: integer `4`, expected a string",
        ),
        (
            "map = 3\n\n[project]\nslug = \"x\"\n",
            1,
            "invalid type: integer `3`",
        ),
        ("[map]\nring = 3\n", 2, "invalid type: integer `3`"),
    ] {
        let note = refused(text);
        assert_eq!(note.line, line, "{text}: {note:?}");
        assert!(
            note.message.starts_with(opening),
            "{text}: {:?} opens with {opening:?}",
            note.message
        );
    }
    // "Data": an unknown key names the three a ring holds.
    let note = refused("[[map.ring]]\ntitle = \"Rim\"\nkinds = [\"rule\"]\ncolour = \"red\"\n");
    assert_eq!(note.line, 4, "{note:?}");
    assert!(
        note.message
            .starts_with("unknown field `colour`, expected ")
            && ["`title`", "`root`", "`kinds`"]
                .iter()
                .all(|key| note.message.contains(key)),
        "{note:?}"
    );
}

/// SR1: two problems, the earlier line's is the note; written later in
/// the file, the earlier line still wins.
#[test]
fn sr1_the_first_problem_by_line() {
    let text = "[[map.ring]]\ntitle = \"\"\nkinds = [\"rule\"]\n\n[[map.ring]]\ntitle = \"Rim\"\n";
    assert_note(text, 2, "`title` is 0 bytes long; a title has 1 to 64");
    let text = "[[map.ring]]\ntitle = \"Rim\"\n\n[[map.ring]]\ntitle = \"Hub\"\nkinds = []\n";
    assert_note(
        text,
        1,
        "`[[map.ring]]` `Rim` holds neither `root` nor `kinds`: a ring holds one or both",
    );
    assert_eq!(
        MapConfig::from_toml(text).notes_at("specengine.toml"),
        [format!(
            "specengine.toml:1: `[[map.ring]]` `Rim` holds neither `root` nor `kinds`: a ring holds one or both{SERVED}"
        )]
    );
}

/// SR1: `ProjectConfig` loads a file holding `[map]`, good or bad, as it
/// loads it without (`map` name-checked only); a misspelt `[maps]` is still
/// an unknown key. M: `map` missing from `RawFile`.
#[test]
fn sr1_project_config_loads_a_file_holding_map() {
    let project = "[project]\nslug = \"lantern-keep\"\n\n[ids]\nMEC = { kind = \"mechanic\", shape = \"name\" }\n\n";
    let plain = ProjectConfig::from_toml(project).expect("the plain config");
    for text in [
        format!("{project}{DATA}"),
        format!("{project}{DATA}\n[map]\nx = 1\n"),
        format!("{project}[map]\nring = 3\n"),
        format!("{project}[[map.ring]]\ncolour = \"red\"\n"),
        format!("map = 3\n\n{project}"),
    ] {
        let config =
            ProjectConfig::from_toml(&text).unwrap_or_else(|error| panic!("{text}: {error:?}"));
        let mut want = plain.clone();
        want.project_line = config.project_line;
        assert_eq!(config, want, "{text}");
    }
    let error = ProjectConfig::from_toml(&format!("{project}[maps]\nring = 1\n"))
        .expect_err("an unknown table");
    assert!(error.message.contains("`maps`"), "{error:?}");
}
