//! docs/features/scope-rings.md (RM-0113) "Acceptance criteria", the CLI
//! half: `spec rings [--json]` over scratch copies of `fixtures/spec-a`
//! with "Data"'s config ("the copy"; the fixture only read), each under its
//! own `HOME`, with invented files and cards written here.
//!
//! - SR1, the CLI half: every read loads a `specengine.toml` holding
//!   `[map]`, good or bad, and answers as without it; a bad table is one
//!   `note:` line and one `notes` entry, `<config>:<line>: <problem>; no ring
//!   is served`, and no ring. M: `map` missing from `RawFile` (every command
//!   exits 2).
//! - SR2: "Data"'s text byte for byte; ring 1 the two mechanics and ring 2
//!   the four rules, each in `spec tree`'s order; a third ring rooted at
//!   DOM-GAME holds the rest of `spec tree DOM-GAME`; moved first, it takes
//!   all and the other two are empty; without `[map]` no ring. M: a node in
//!   two rings; sectors by path, not tree order.
//! - SR3: a card on MEC-STAMINA makes its sector `in work`, accepted
//!   `confirmed`, a cancelled one counts for nothing (`no card`), each the
//!   catalog's `state`. M: a cancelled card counted.
//! - SR4: a root of `DOM-NONE` leaves its ring empty with the note, exit 0;
//!   a bad table gives the note, exit 0, `rings: []`; the copy at two roots
//!   and two `HOME`s gives byte-identical answers; past `OUTPUT_CAP_CHARS`
//!   the text is cut at a line with its tail and the JSON follows the cut.
//!   M: an unresolved root exiting 1.
//!
//! Every expected value is worked out from the spec and the files here,
//! never read back from a run of the read under test.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fmt;

use common::{Run, Scratch, read_text, replace, spec, write};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};
use specengine_cli::{CardAcceptRequest, Env, Exit, Globals, card_accept};

/// "Data"'s config, appended to the copy's `specengine.toml`.
const RINGS: &str = "\n[[map.ring]]\ntitle = \"Movement\"\nroot  = \"DOM-MOVEMENT\"\nkinds = [\"mechanic\"]\n\n[[map.ring]]\ntitle = \"Rules\"\nkinds = [\"rule\"]\n";

/// The third ring of SR2.
const ALL_RING: &str = "\n[[map.ring]]\ntitle = \"All\"\nroot = \"DOM-GAME\"\n";

/// "Data"'s text on the copy.
const DATA_TEXT: &str = "\
rings 2 | sectors 6
ring 1 | Movement | root DOM-MOVEMENT | kinds mechanic | sectors 2
  MEC-SPRINT | mechanic | no card | Sprint | docs/spec/movement/sprint.md:1
  MEC-STAMINA | mechanic | no card | Stamina | docs/spec/movement/stamina.md:1 | status accepted
ring 2 | Rules | kinds rule | sectors 4
  RULE-CORE-LOOP | rule | no card | Core loop | docs/spec/game.md:21
  RULE-MOVE-SPEEDS | rule | no card | Speeds | docs/spec/movement/README.md:16
  RULE-SPRINT-COST | rule | no card | Cost | docs/spec/movement/sprint.md:18
  RULE-STAM-REGEN | rule | no card | Regeneration | docs/spec/movement/stamina.md:21
";

/// The key sets of "Data", in the order written there.
const RINGS_KEYS: [&str; 2] = ["notes", "rings"];
const RING_KEYS: [&str; 4] = ["title", "root", "kinds", "sectors"];
const SECTOR_KEYS: [&str; 8] = [
    "name", "id", "kind", "title", "path", "line", "status", "state",
];

/// The text's cut, as the library says it.
const CAP: usize = specengine_cli::OUTPUT_CAP_CHARS;

/// A JSON object's keys, in the order written.
struct Keys(Vec<String>);

impl<'de> Deserialize<'de> for Keys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Seen;
        impl<'de> Visitor<'de> for Seen {
            type Value = Keys;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Keys, M::Error> {
                let mut keys = Vec::new();
                while let Some((key, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                    keys.push(key);
                }
                Ok(Keys(keys))
            }
        }
        deserializer.deserialize_map(Seen)
    }
}

/// One scratch copy and its `HOME`.
struct Copy {
    _scratch: Scratch,
    root: std::path::PathBuf,
    home: std::path::PathBuf,
    /// The fixture's own config.
    own: String,
}

impl Copy {
    /// A copy of `fixtures/spec-a` with `config` appended to its
    /// `specengine.toml`.
    fn new(label: &str, config: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.copy("spec-a", "copy");
        let own = read_text(&root, "specengine.toml");
        write(&root, "specengine.toml", format!("{own}{config}"));
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            root,
            home,
            own,
        }
    }

    fn run(&self, args: &[&str]) -> Run {
        spec(&self.home, &self.root, args)
    }

    /// `spec args… --json`, exit 0: the document.
    fn json(&self, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let run = self.run(&all);
        run.code(0);
        run.json()
    }

    /// `spec rings --json`, exit 0.
    fn rings(&self) -> Value {
        self.json(&["rings"])
    }

    /// The fixture's own config followed by `config`.
    fn set_rings(&self, config: &str) {
        write(
            &self.root,
            "specengine.toml",
            format!("{}{config}", self.own),
        );
    }

    fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }
}

/// The names of ring `index`'s sectors (0-based), in order.
fn sectors(document: &Value, index: usize) -> Vec<String> {
    document["rings"][index]["sectors"]
        .as_array()
        .unwrap_or_else(|| panic!("ring {index}: {document}"))
        .iter()
        .map(|sector| sector["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// The node names of `spec tree [ROOT] [--kind K] --json`, in its order.
fn tree(copy: &Copy, args: &[&str]) -> Vec<String> {
    let mut all = vec!["tree"];
    all.extend_from_slice(args);
    let document = copy.json(&all);
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["id"].as_str().expect("an ID").to_owned())
        .collect()
}

/// The sector named `name` in any ring.
fn sector<'a>(document: &'a Value, name: &str) -> &'a Value {
    document["rings"]
        .as_array()
        .expect("rings")
        .iter()
        .flat_map(|ring| ring["sectors"].as_array().expect("sectors"))
        .find(|sector| sector["name"] == name)
        .unwrap_or_else(|| panic!("{name}: {document}"))
}

// ------------------------------------------------------------------ SR2

/// SR2: "Data"'s text and JSON on the copy, the keys in "Data"'s order,
/// each ring's sectors `spec tree`'s lines for its root and kinds.
#[test]
fn sr2_the_rings_on_the_copy() {
    let copy = Copy::new("sr2", RINGS);
    let run = copy.run(&["rings"]);
    run.code(0);
    assert_eq!(run.stdout, DATA_TEXT, "{}", run.show());
    assert_eq!(run.stderr, "", "{}", run.show());

    let run = copy.run(&["rings", "--json"]);
    run.code(0);
    let body = run.stdout.strip_suffix('\n').expect("a line end");
    assert_eq!(
        serde_json::from_str::<Keys>(body).expect("an object").0,
        RINGS_KEYS
    );
    let document = run.json();
    assert_eq!(document["notes"], json!([]));
    let raw_rings = body
        .split_once("\"rings\":")
        .map(|(_, rest)| rest)
        .expect("rings");
    let first_ring = &raw_rings[raw_rings.find('{').expect("a ring")..];
    let ring_end = matching_brace(first_ring);
    assert_eq!(
        serde_json::from_str::<Keys>(&first_ring[..=ring_end])
            .expect("a ring")
            .0,
        RING_KEYS
    );
    let first_sector = &first_ring[first_ring.find("\"sectors\":[").expect("sectors") + 11..];
    let end = matching_brace(first_sector);
    assert_eq!(
        serde_json::from_str::<Keys>(&first_sector[..=end])
            .expect("a sector")
            .0,
        SECTOR_KEYS
    );
    assert_eq!(
        document["rings"][0],
        json!({"title": "Movement", "root": "DOM-MOVEMENT", "kinds": ["mechanic"], "sectors": [
            {"name": "MEC-SPRINT", "id": "MEC-SPRINT", "kind": "mechanic", "title": "Sprint", "path": "docs/spec/movement/sprint.md", "line": 1, "status": null, "state": "no card"},
            {"name": "MEC-STAMINA", "id": "MEC-STAMINA", "kind": "mechanic", "title": "Stamina", "path": "docs/spec/movement/stamina.md", "line": 1, "status": "accepted", "state": "no card"}]})
    );
    assert_eq!(
        document["rings"][1],
        json!({"title": "Rules", "root": null, "kinds": ["rule"], "sectors": [
            {"name": "RULE-CORE-LOOP", "id": "RULE-CORE-LOOP", "kind": "rule", "title": "Core loop", "path": "docs/spec/game.md", "line": 21, "status": null, "state": "no card"},
            {"name": "RULE-MOVE-SPEEDS", "id": "RULE-MOVE-SPEEDS", "kind": "rule", "title": "Speeds", "path": "docs/spec/movement/README.md", "line": 16, "status": null, "state": "no card"},
            {"name": "RULE-SPRINT-COST", "id": "RULE-SPRINT-COST", "kind": "rule", "title": "Cost", "path": "docs/spec/movement/sprint.md", "line": 18, "status": null, "state": "no card"},
            {"name": "RULE-STAM-REGEN", "id": "RULE-STAM-REGEN", "kind": "rule", "title": "Regeneration", "path": "docs/spec/movement/stamina.md", "line": 21, "status": null, "state": "no card"}]})
    );
    assert_eq!(document["rings"].as_array().expect("rings").len(), 2);
    // The tree reads each ring is: its root's holders and its kinds.
    assert_eq!(
        sectors(&document, 0),
        tree(&copy, &["DOM-MOVEMENT", "--kind", "mechanic"])
    );
    assert_eq!(sectors(&document, 1), tree(&copy, &["--kind", "rule"]));
}

/// The index of the brace closing the object `text` opens.
fn matching_brace(text: &str) -> usize {
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, letter) in text.char_indices() {
        if quoted {
            match (escaped, letter) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => quoted = false,
                _ => {}
            }
            continue;
        }
        match letter {
            '"' => quoted = true,
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth == 0 {
                    return index;
                }
            }
            _ => {}
        }
    }
    panic!("no closed object in {text}");
}

/// SR2: a third ring `{title = "All", root = "DOM-GAME"}` holds every node
/// of `spec tree DOM-GAME` but the six, in its order; moved first it takes
/// all and the other two are empty. M: a node in two rings.
#[test]
fn sr2_a_node_sits_in_the_first_ring_listing_it() {
    let copy = Copy::new("sr2-third", &format!("{RINGS}{ALL_RING}"));
    let document = copy.rings();
    let whole = tree(&copy, &["DOM-GAME"]);
    assert_eq!(whole.len(), 10, "{whole:?}");
    let six: BTreeSet<String> = sectors(&document, 0)
        .into_iter()
        .chain(sectors(&document, 1))
        .collect();
    assert_eq!(six.len(), 6);
    let rest: Vec<String> = whole
        .iter()
        .filter(|name| !six.contains(*name))
        .cloned()
        .collect();
    assert_eq!(
        rest,
        [
            "DOM-GAME",
            "DOM-MOVEMENT",
            "EDGE-SPRINT-EMPTY",
            "EDGE-STAM-ZERO"
        ]
    );
    assert_eq!(sectors(&document, 2), rest, "the rest, in tree order");
    assert_eq!(
        document["rings"][2],
        json!({"title": "All", "root": "DOM-GAME", "kinds": null, "sectors": document["rings"][2]["sectors"]})
    );
    let text = copy.run(&["rings"]);
    text.code(0);
    assert!(
        text.stdout.starts_with("rings 3 | sectors 10\n"),
        "{}",
        text.stdout
    );
    assert!(
        text.stdout
            .lines()
            .any(|line| line == "ring 3 | All | root DOM-GAME | sectors 4"),
        "{}",
        text.stdout
    );

    copy.set_rings(&format!("{ALL_RING}{RINGS}"));
    let document = copy.rings();
    assert_eq!(sectors(&document, 0), whole, "the first ring takes all");
    assert_eq!(sectors(&document, 1), Vec::<String>::new(), "{document}");
    assert_eq!(sectors(&document, 2), Vec::<String>::new(), "{document}");
    let text = copy.run(&["rings"]);
    text.code(0);
    assert_eq!(
        text.stdout.lines().collect::<Vec<_>>()[..1],
        ["rings 3 | sectors 10"]
    );
    for line in [
        "ring 2 | Movement | root DOM-MOVEMENT | kinds mechanic | sectors 0",
        "ring 3 | Rules | kinds rule | sectors 0",
    ] {
        assert!(
            text.stdout.lines().any(|seen| seen == line),
            "{line}: {}",
            text.stdout
        );
    }
}

/// SR2: a rule filed under MEC-STAMINA by `parent:` at a path that sorts
/// first is listed where `spec tree` lists it, last. M: sectors by path,
/// not tree order.
#[test]
fn sr2_sectors_follow_the_tree_not_the_paths() {
    let copy = Copy::new("sr2-order", RINGS);
    write(
        &copy.root,
        "docs/spec/a-rule.md",
        "---\nid: RULE-AAA\nclass: canon\nparent: MEC-STAMINA\n---\n\n# Early rule\n",
    );
    let document = copy.rings();
    let want = tree(&copy, &["--kind", "rule"]);
    assert_eq!(
        want.last().map(String::as_str),
        Some("RULE-AAA"),
        "{want:?}"
    );
    assert_eq!(sectors(&document, 1), want);
}

/// SR2: without `[map]` the read answers no ring and no note.
#[test]
fn sr2_without_map_no_ring() {
    let copy = Copy::new("sr2-none", "");
    let run = copy.run(&["rings"]);
    run.code(0);
    assert_eq!(run.stdout, "rings 0 | sectors 0\n", "{}", run.show());
    assert_eq!(run.stderr, "");
    let run = copy.run(&["rings", "--json"]);
    run.code(0);
    assert_eq!(run.stdout, "{\"notes\":[],\"rings\":[]}\n");
}

// ------------------------------------------------------------------ SR3

/// SR3: a card on MEC-STAMINA reads `in work`; cancelled, `no card`; a new
/// one accepted, `confirmed`; each the catalog's `state`, the other
/// sectors untouched. M: a cancelled card counted.
#[test]
fn sr3_the_fill_is_the_work_state() {
    let copy = Copy::new("sr3", RINGS);
    let state = |copy: &Copy| {
        let document = copy.rings();
        let catalog = copy.json(&["catalog", "--kind", "mechanic"]);
        let row = catalog["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row["id"] == "MEC-STAMINA")
            .expect("MEC-STAMINA's row")
            .clone();
        let served = sector(&document, "MEC-STAMINA")["state"].clone();
        assert_eq!(served, row["state"], "the catalog's state");
        assert_eq!(sector(&document, "MEC-SPRINT")["state"], "no card");
        assert_eq!(sector(&document, "RULE-STAM-REGEN")["state"], "no card");
        served
    };
    assert_eq!(state(&copy), "no card");
    let new = |title: &str| {
        let run = copy.run(&[
            "card",
            "new",
            "--title",
            title,
            "--target",
            "MEC-STAMINA",
            "--reviewer",
            "Ann",
        ]);
        run.code(0);
    };
    new("Tune the stamina");
    assert_eq!(state(&copy), "in work");
    copy.run(&["card", "cancel", "C-0001", "--reason", "Not this release"])
        .code(0);
    assert_eq!(state(&copy), "no card", "M: a cancelled card counted");
    new("Tune the stamina again");
    assert_eq!(state(&copy), "in work");
    copy.run(&["card", "move", "C-0002", "--to", "review"])
        .code(0);
    let mut yes = |_: &str| true;
    let outcome = card_accept(
        &copy.env(),
        &Globals::default(),
        &CardAcceptRequest {
            id: "C-0002".to_owned(),
            actor: "Ann".to_owned(),
            briefed: true,
            now: "2026-11-13T10:00:00Z".to_owned(),
            ..CardAcceptRequest::default()
        },
        &mut yes,
    )
    .unwrap_or_else(|error| panic!("accept: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(state(&copy), "confirmed");
    let text = copy.run(&["rings"]);
    text.code(0);
    assert!(
        text.stdout.lines().any(|line| line
            == "  MEC-STAMINA | mechanic | confirmed | Stamina | docs/spec/movement/stamina.md:1 | status accepted"),
        "{}",
        text.stdout
    );
}

// ------------------------------------------------------------------ SR4

/// SR4, as ruled in iteration 2: a root the resolver refuses or finds no
/// node for leaves its ring empty with one note, `rings: ring <i>
/// `<title>`: <the resolver's reason>`, the wording `spec tree ROOT` gives;
/// an `@<rev>` is a note and the ring is served; the next ring is served,
/// exit 0, the note on stderr too. M: an unresolved root exiting 1.
#[test]
fn sr4_an_unresolved_root_is_a_note() {
    let lookalike = "DOM-MOVEM\u{0415}NT";
    let cases: [(&str, String, bool); 7] = [
        (
            "DOM-NONE",
            "`DOM-NONE` resolves to no ID and no alias".to_owned(),
            false,
        ),
        (
            lookalike,
            format!(
                "`{lookalike}` mixes scripts or uses look-alike letters; IDs are Latin only: \
                 write `DOM-MOVEMENT`"
            ),
            false,
        ),
        (
            "lantern-keep/DOM-GAME",
            "`lantern-keep/DOM-GAME` resolves to no feature document \
             `docs/features/lantern-keep.md`"
                .to_owned(),
            false,
        ),
        (
            "docs/../x.md",
            "`docs/../x.md` is no clean root-relative path: no leading `/`, no `.`, `..` or \
             empty component"
                .to_owned(),
            false,
        ),
        (
            "docs/spec/none.md",
            "`docs/spec/none.md` is no indexed document: not under the `[paths]` roots, \
             excluded, or missing"
                .to_owned(),
            false,
        ),
        (
            "other:DOM-GAME",
            "`other:DOM-GAME`: `project:` references are not supported yet; drop the \
             qualifier to read this project's node"
                .to_owned(),
            false,
        ),
        (
            "DOM-MOVEMENT@3",
            "`@3` is ignored: `spec rings` reads the current files".to_owned(),
            true,
        ),
    ];
    let copy = Copy::new("sr4-none", "");
    for (root, reason, served) in cases {
        copy.set_rings(&RINGS.replace("DOM-MOVEMENT", root));
        let note = format!("rings: ring 1 `Movement`: {reason}");
        let run = copy.run(&["rings", "--json"]);
        run.code(0);
        let document = run.json();
        assert_eq!(document["notes"], json!([note]), "{root}: {document}");
        assert_eq!(document["rings"][0]["root"], root, "{root}");
        let first = sectors(&document, 0);
        if served {
            assert_eq!(first, ["MEC-SPRINT", "MEC-STAMINA"], "{root}");
        } else {
            assert_eq!(first, Vec::<String>::new(), "{root}");
        }
        assert_eq!(sectors(&document, 1).len(), 4, "{root}: {document}");
        let run = copy.run(&["rings"]);
        run.code(0);
        assert!(
            run.stdout.lines().any(|line| line
                == format!(
                    "ring 1 | Movement | root {root} | kinds mechanic | sectors {}",
                    first.len()
                )),
            "{root}: {}",
            run.show()
        );
        assert!(
            run.stderr
                .lines()
                .any(|line| line == format!("note: {note}")),
            "{root}: the note on stderr: {}",
            run.show()
        );
    }
    // Not a REF: the resolver's reason names the prefixes.
    copy.set_rings(&RINGS.replace("DOM-MOVEMENT", "nope nope"));
    let document = copy.rings();
    let note = document["notes"][0].as_str().expect("a note");
    assert!(
        note.starts_with(
            "rings: ring 1 `Movement`: `nope nope` is no reference to an ID of this project; \
             prefixes: "
        ),
        "{document}"
    );
    assert_eq!(sectors(&document, 0), Vec::<String>::new());
}

/// The bad tables of SR4, each with the line its note names when written
/// right after the fixture's `config_lines` lines.
fn bad_tables(config_lines: usize) -> Vec<(String, usize, &'static str)> {
    // The first appended line is `config_lines + 1`, a blank one.
    let first = config_lines + 2;
    vec![
        (
            "\n[[map.ring]]\ntitle = \"Movement\"\n".to_owned(),
            first,
            "`[[map.ring]]` `Movement` holds neither `root` nor `kinds`: a ring holds one or both",
        ),
        (
            "\n[map]\nx = 1\n".to_owned(),
            first + 1,
            "`[map]` holds only `ring`",
        ),
        (
            "\n[[map.ring]]\nkinds = [\"rule\"]\n".to_owned(),
            first,
            "`[[map.ring]]` is required to hold `title`",
        ),
    ]
}

/// SR4: a bad table gives one note, `<config>:<line>: <problem>; no ring is
/// served`, on stderr and in `notes`, `rings: []`, exit 0; the good ring
/// before it is not served either.
#[test]
fn sr4_a_bad_table_serves_no_ring() {
    let copy = Copy::new("sr4-bad", "");
    for (table, line, problem) in bad_tables(copy.own.lines().count()) {
        let config = format!("{RINGS}{table}");
        copy.set_rings(&config);
        let line = line + RINGS.lines().count();
        let note = format!("specengine.toml:{line}: {problem}; no ring is served");
        let run = copy.run(&["rings", "--json"]);
        run.code(0);
        assert_eq!(
            run.json(),
            json!({"notes": [note], "rings": []}),
            "{table}: {}",
            run.show()
        );
        let run = copy.run(&["rings"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            "rings 0 | sectors 0\n",
            "{table}: {}",
            run.show()
        );
        assert_eq!(
            run.stderr_lines(),
            [format!("note: {note}")],
            "{table}: {}",
            run.show()
        );
    }
}

/// SR1, the CLI half: with "Data"'s `[map]`, and with a bad one, every
/// read answers as it does without `[map]` (code and stdout). M: `map`
/// missing from `RawFile` (every command exits 2).
#[test]
fn sr1_every_read_loads_a_config_holding_map() {
    let copy = Copy::new("sr1-load", "");
    let reads: [&[&str]; 14] = [
        &["check"],
        &["check", "--json"],
        &["tree"],
        &["tree", "--json"],
        &["show", "MEC-STAMINA"],
        &["show", "MEC-STAMINA", "--links", "--json"],
        &["graph", "MEC-STAMINA"],
        &["search", "stamina"],
        &["catalog", "--kind", "mechanic", "--json"],
        &["health", "--json"],
        &["bundle", "MEC-STAMINA"],
        &["impact", "MEC-STAMINA"],
        &["card", "list"],
        &["map", "--json"],
    ];
    // A first pass makes the index and the queue (`card list`), so the
    // compared passes all read one state.
    for args in reads {
        copy.run(args);
    }
    let before: Vec<(i32, String)> = reads
        .iter()
        .map(|args| {
            let run = copy.run(args);
            assert_ne!(run.code, 2, "{args:?}: {}", run.show());
            (run.code, run.stdout)
        })
        .collect();
    for config in [RINGS.to_owned(), format!("{RINGS}\n[map]\nx = 1\n")] {
        copy.set_rings(&config);
        for (args, (code, stdout)) in reads.iter().zip(&before) {
            let run = copy.run(args);
            assert_eq!(
                (run.code, &run.stdout),
                (*code, stdout),
                "{args:?} with {config:?}: {}",
                run.show()
            );
        }
        copy.run(&["rings"]).code(0);
    }
}

/// SR4: the copy at two roots and two `HOME`s, its files written in
/// opposite orders, a card on MEC-STAMINA in each, gives byte-identical
/// text and JSON, with and without a note.
#[test]
fn sr4_one_corpus_one_answer() {
    let scratch = Scratch::new("sr4-twice");
    let source = Copy::new("sr4-source", &format!("{RINGS}{ALL_RING}"));
    let mut files = files_of(&source.root);
    files.sort();
    let first = scratch.dir("one/deeper/copy");
    for (path, bytes) in &files {
        write(&first, path, bytes);
    }
    let second = scratch.dir("two");
    for (path, bytes) in files.iter().rev() {
        write(&second, path, bytes);
    }
    let homes = [scratch.home("first"), scratch.home("second")];
    for (root, home) in [(&first, &homes[0]), (&second, &homes[1])] {
        spec(
            home,
            root,
            &[
                "card",
                "new",
                "--title",
                "Tune the stamina",
                "--target",
                "MEC-STAMINA",
            ],
        )
        .code(0);
    }
    for round in 0..2 {
        if round == 1 {
            for root in [&first, &second] {
                replace(
                    root,
                    "specengine.toml",
                    "root = \"DOM-GAME\"",
                    "root = \"DOM-NONE\"",
                );
            }
        }
        for args in [&["rings"][..], &["rings", "--json"]] {
            let one = spec(&homes[0], &first, args);
            let two = spec(&homes[1], &second, args);
            one.code(0);
            assert_eq!(
                (two.code, &two.stdout, &two.stderr),
                (one.code, &one.stdout, &one.stderr),
                "{args:?}, round {round}"
            );
            assert!(one.stdout.contains("in work"), "{args:?}: {}", one.stdout);
        }
    }
}

/// The files under `root`, as (relative path, bytes).
fn files_of(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).expect("a readable directory") {
            let entry = entry.expect("an entry");
            let path = entry.path();
            if entry.file_type().expect("a type").is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).expect("under the root");
                out.push((
                    relative.to_str().expect("UTF-8").to_owned(),
                    std::fs::read(&path).expect("a file"),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

/// SR4 ("Data"): past `OUTPUT_CAP_CHARS` the text keeps whole lines, the
/// rings before the cut whole, then `[truncated: <k> of <n> sectors not
/// shown]`; the JSON holds exactly the printed sectors.
#[test]
fn sr4_the_cut_falls_at_a_line_and_the_json_follows_it() {
    let copy = Copy::new("sr4-cap", RINGS);
    let many = 450;
    for n in 0..many {
        write(
            &copy.root,
            &format!("docs/spec/wicks/w-{n:03}.md"),
            format!(
                "---\nid: MEC-WICK-{n:03}\nclass: canon\nparent: DOM-MOVEMENT\n---\n\n# Wick {n:03} {}\n",
                "of the long stair below the lamp room "
                    .repeat(2)
                    .trim_end()
            ),
        );
    }
    let total = 6 + many;
    let run = copy.run(&["rings"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(
        lines[0],
        format!("rings 2 | sectors {total}"),
        "{}",
        lines[0]
    );
    let tail = *lines.last().expect("a line");
    let shown: Vec<&str> = lines
        .iter()
        .filter(|line| line.starts_with("  "))
        .copied()
        .collect();
    assert!(!shown.is_empty() && shown.len() < total, "{}", shown.len());
    assert_eq!(
        tail,
        format!(
            "[truncated: {} of {total} sectors not shown]",
            total - shown.len()
        )
    );
    let body: usize = lines[..lines.len() - 1]
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    assert!(body <= CAP, "{body}");
    assert!(
        shown.iter().all(|line| line.starts_with("  MEC-")),
        "ring 1's sectors, whole lines"
    );
    let document = copy.rings();
    let printed: Vec<String> = document["rings"]
        .as_array()
        .expect("rings")
        .iter()
        .flat_map(|ring| ring["sectors"].as_array().expect("sectors"))
        .map(|sector| sector["name"].as_str().expect("a name").to_owned())
        .collect();
    let names: Vec<String> = shown
        .iter()
        .map(|line| {
            line.trim_start()
                .split(" | ")
                .next()
                .expect("a name")
                .to_owned()
        })
        .collect();
    assert_eq!(printed, names, "the JSON follows the cut");
}

/// "Rules and edge cases": a root with two holders takes each holder's
/// subtree, as `spec tree ROOT` lists them, with the tree's warning on
/// stderr alone; a ring of kinds alone sees `spec tree`'s roots (`[paths]
/// spec`), so a record outside them is in no such ring, and a root brings
/// it in.
#[test]
fn sr2_a_root_held_twice_and_a_record_outside_the_spec_roots() {
    let copy = Copy::new(
        "sr2-holders",
        "\n[[map.ring]]\ntitle = \"Twins\"\nroot = \"DOM-TWIN\"\n\n[[map.ring]]\ntitle = \"Asks\"\nkinds = [\"question\"]\n\n[[map.ring]]\ntitle = \"Asked\"\nroot = \"Q-031\"\n",
    );
    for (path, text) in [
        (
            "docs/spec/twins/a.md",
            "---\nid: DOM-TWIN\nclass: canon\n---\n\n# Twin one\n",
        ),
        (
            "docs/spec/twins/b.md",
            "---\nid: DOM-TWIN\nclass: canon\n---\n\n# Twin two\n\n## Gate {#RULE-TWIN-GATE}\n\nShut at dusk.\n",
        ),
        (
            "docs/spec/twins/c.md",
            "---\nid: MEC-UNDER\nclass: canon\nparent: DOM-TWIN\n---\n\n# Under\n",
        ),
    ] {
        write(&copy.root, path, text);
    }
    let tree = copy.json(&["tree", "DOM-TWIN"]);
    let want: Vec<(String, String)> = tree["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| {
            (
                node["id"].as_str().expect("an ID").to_owned(),
                node["path"].as_str().expect("a path").to_owned(),
            )
        })
        .collect();
    assert_eq!(want.len(), 4, "both holders' subtrees: {tree}");
    let run = copy.run(&["rings", "--json"]);
    run.code(0);
    let document = run.json();
    let twins: Vec<(String, String)> = document["rings"][0]["sectors"]
        .as_array()
        .expect("sectors")
        .iter()
        .map(|sector| {
            (
                sector["name"].as_str().expect("a name").to_owned(),
                sector["path"].as_str().expect("a path").to_owned(),
            )
        })
        .collect();
    assert_eq!(twins, want, "each holder's subtree, in the tree's order");
    assert_eq!(document["notes"], json!([]), "{document}");
    assert!(
        run.stderr.contains("DOM-TWIN"),
        "the tree's warning on stderr: {}",
        run.show()
    );
    assert_eq!(sectors(&document, 1), Vec::<String>::new(), "{document}");
    assert_eq!(sectors(&document, 2), ["Q-031"], "{document}");
}
