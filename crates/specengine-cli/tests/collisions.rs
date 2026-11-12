//! docs/features/collision-pass.md (RM-0060) "Acceptance criteria", the CLI
//! half: the `spec` binary over scratch git copies of `fixtures/spec-a`
//! (`Pair`), each gaining invented lamp documents committed here.
//!
//! - The pinned example: the copy with `endurance.md` (`MEC-ENDURANCE`,
//!   `Stamina pool`, `depends_on: [MEC-SPRINT]`); `spec collisions --kind
//!   mechanic` text and `--json` as "The document" gives them, every number
//!   worked out here from the signals' rules.
//! - CP1: two documents of a new kind, titles with no trigram in common,
//!   both `uses: [MEC-SPRINT]`: one pair, `shared` 1, score 200; `mentions`
//!   instead: none; a shared `parent:`: one. M: signal (a) dropped;
//!   `mentions` counted.
//! - CP2: `Lantern oil reserve` and `lantern  OIL reserves` of one kind: one
//!   pair, `titles` 923 and its reason; on two kinds none, one with
//!   `--any-kind`; a document and its own section with one title none, a
//!   near title in another document's section pairs. M: kinds not compared;
//!   signal (b) dropped.
//! - CP3: marking a pair `distinct` hides it, the count and `left` drop; a
//!   one-word edit lists it again with `marked` and `changed since`; trailing
//!   spaces and a blank line keep it hidden, and so does a heading-only
//!   rename (a known limit). M: the ledger keyed by name alone; keyed by
//!   `span_hash`.
//! - CP4: 1 000 children of one parent, titled `Sibling <n>`: no pair through
//!   the parent (degree > 64); 50 pairs then `left`, in the order an oracle
//!   built here from the spec's rules gives; a rerun, a clone at another root
//!   and another `HOME` byte-equal; `--limit 200` lists 200, 201 and 0 exit
//!   2. M: the degree cap removed (10 s timeout or parent pairs); the sort
//!   removed.
//! - CP5: each mark rule refuses with nothing written (the ledger and the
//!   last event `seq` unchanged); a batch with one bad pair writes none; a
//!   second mark replaces; no mark adds an event. M: an event emitted; the
//!   batch written in part.
//! - CP6, the CLI half: a queue holding marks exports format 7 at schema 14
//!   with no ledger line; the dump imports with an empty ledger; a format 7
//!   dump at 13 imports; a 15 dump and a 15 queue are refused; `spec
//!   collisions` over a schema 13 queue reads no marks. M: the ledger dumped.
//! - Edge data: a node without a `norm_hash` is skipped with its note; two
//!   holders of one ID never pair.
//!
//! The names the tests add (a lamp's wick, hood, chimney, oil) are invented
//! here. Every expected value is worked out from the spec, never read back
//! from a run of the read under test; the `norm_hash`es are `spec show`'s.

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::proposal::Pair;
use common::{Run, SPEC, data_dir, one_json_document};
use serde_json::{Value, json};
use specengine_cli::{
    Env, Exit, ExportStateRequest, Globals, ImportStateRequest, STATE_FORMAT, export_state,
    import_state, utc_now,
};
use specengine_store::QUEUE_SCHEMA_VERSION;

/// `COLLISIONS_LIMIT_DEFAULT` and `COLLISIONS_LIMIT_MAX` as the spec fixes
/// them.
const LIMIT_DEFAULT: usize = 50;
const LIMIT_MAX: usize = 200;
/// `COLLISION_MARKS_MAX`.
const MARKS_MAX: usize = 200;
/// `COLLISION_TITLES_MIN`, `COLLISION_TRIGRAM_DF_MAX`,
/// `COLLISION_SHARED_WEIGHT`, `COLLISION_SHARED_SCORED_MAX`.
const TITLES_MIN: u64 = 500;
const TRIGRAM_DF_MAX: usize = 200;
const SHARED_WEIGHT: u64 = 200;
const SHARED_SCORED_MAX: u64 = 5;

/// The new kind of CP1 and CP3's lamp parts.
const PART: &str = "lamp-part";
/// CP2's second new kind.
const NOTE: &str = "lamp-note";

/// The document's keys, in order, a pair's and a side's.
const DOCUMENT_KEYS: [&str; 8] = [
    "project", "kinds", "any_kind", "limit", "pairs", "left", "skipped", "notes",
];
const PAIR_KEYS: [&str; 8] = [
    "a",
    "b",
    "score",
    "titles",
    "shared",
    "shared_targets",
    "marked",
    "reason",
];
const SIDE_KEYS: [&str; 6] = ["id", "path", "ord", "kind", "title", "norm_hash"];
const MARK_KEYS: [&str; 3] = ["marked", "replaced", "notes"];

/// The `skipped` note, as the spec words it.
const SKIPPED_NOTE: &str =
    "1 node(s) without a norm_hash not paired: their files do not parse whole";

// ------------------------------------------------------------- the scene

/// A scratch copy of spec-a (`Pair`): files are written in its main
/// worktree and committed; `spec` runs there with the pair's `HOME`.
struct Scene {
    pair: Pair,
}

impl Scene {
    fn new(label: &str) -> Self {
        Self {
            pair: Pair::new(label, "spec-a"),
        }
    }

    fn root(&self) -> &Path {
        &self.pair.main
    }

    /// Writes `relative` and commits every change.
    fn put(&self, relative: &str, text: &str) {
        self.write(relative, text);
        self.commit(&format!("Write {relative}"));
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root().join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
        fs::write(&path, text).expect("write");
    }

    fn commit(&self, message: &str) {
        self.pair.git.add_all(self.root());
        self.pair.git.commit(self.root(), message);
    }

    /// `spec args` in the root, `stdin` piped.
    fn spec(&self, args: &[&str], stdin: &[u8]) -> Run {
        run_spec(
            &self.pair,
            self.root(),
            &self.pair.home,
            args,
            stdin,
            Duration::from_secs(120),
        )
    }

    /// `spec collisions args… --json`, exit 0: the document.
    fn collisions(&self, args: &[&str]) -> Value {
        self.collisions_run(args, true).json()
    }

    /// `spec collisions args… [--json]`, exit 0.
    fn collisions_run(&self, args: &[&str], json: bool) -> Run {
        let mut all = vec!["collisions"];
        all.extend_from_slice(args);
        if json {
            all.push("--json");
        }
        let run = self.spec(&all, b"");
        run.code(0);
        run
    }

    /// `spec show REF --json`'s first node's `norm_hash`.
    fn norm(&self, reference: &str) -> String {
        let run = self.spec(&["show", reference, "--json"], b"");
        run.code(0);
        run.json()["nodes"][0]["norm_hash"]
            .as_str()
            .unwrap_or_else(|| panic!("{reference}: no norm_hash: {}", run.show()))
            .to_owned()
    }

    /// `spec collisions mark --input - [extra…] --json` with `input`.
    fn mark(&self, input: &Value, extra: &[&str]) -> Run {
        let mut all = vec!["collisions", "mark", "--input", "-"];
        all.extend_from_slice(extra);
        all.push("--json");
        self.spec(&all, input.to_string().as_bytes())
    }

    /// Whether the queue DB holds `table` (a queue nothing wrote to yet
    /// holds the index alone).
    fn has_table(&self, table: &str) -> bool {
        self.pair
            .sql(&format!(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = '{table}';"
            ))
            .trim()
            == "1"
    }

    /// The ledger's rows, `project|a|a_norm|b|b_norm|verdict|proposal|author|at`,
    /// ordered; empty when there is no ledger yet.
    fn ledger(&self) -> String {
        if !self.has_table("collision_marks") {
            return String::new();
        }
        self.pair.sql(
            "SELECT project, a, a_norm, b, b_norm, verdict, coalesce(proposal, '<null>'), \
             author, at FROM collision_marks ORDER BY project, a, b;",
        )
    }

    /// The last event's `seq`, 0 for none.
    fn last_seq(&self) -> String {
        if !self.has_table("events") {
            return "0".to_owned();
        }
        self.pair
            .sql("SELECT coalesce(max(seq), 0) FROM events;")
            .trim()
            .to_owned()
    }

    /// A question on `target`, stored (its hits named): its ID.
    fn question(&self, target: &str, text: &str) -> String {
        let args = |named: &[String]| {
            let mut args: Vec<String> = [
                "propose",
                "question",
                target,
                "--text",
                text,
                "--working-answer",
                "Yes, the wick is trimmed first.",
                "--price-of-other",
                "One lamp scene re-timed.",
                "--author-role",
                "developer",
            ]
            .iter()
            .map(|arg| (*arg).to_owned())
            .collect();
            for hit in named {
                args.extend(["--distinct-from".to_owned(), hit.clone()]);
            }
            args
        };
        let call = |extra: &[&str], named: &[String]| {
            let mut all = args(named);
            all.extend(extra.iter().map(|arg| (*arg).to_owned()));
            let all: Vec<&str> = all.iter().map(String::as_str).collect();
            let run = self.spec(&all, b"");
            run.code(0);
            run
        };
        let run = call(&[], &[]);
        if run.stdout.starts_with("PR-") {
            return run.stdout.lines().next().expect("an ID").to_owned();
        }
        let hits: Vec<String> = call(&["--json"], &[]).json()["hits"]
            .as_array()
            .expect("hits")
            .iter()
            .map(|hit| hit["id"].as_str().expect("a hit's id").to_owned())
            .collect();
        let run = call(&[], &hits);
        assert!(run.stdout.starts_with("PR-"), "not stored: {}", run.show());
        run.stdout.lines().next().expect("an ID").to_owned()
    }
}

/// `spec args` in `cwd` with the pair's git sandbox, `HOME=home`, `stdin`
/// piped, killed past `timeout`: `None` when killed.
fn run_spec_within(
    pair: &Pair,
    cwd: &Path,
    home: &Path,
    args: &[&str],
    stdin: &[u8],
    timeout: Duration,
) -> Option<Run> {
    let mut command = Command::new(SPEC);
    command
        .env_clear()
        .envs(pair.git.vars())
        .env("HOME", home)
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn spec");
    {
        let mut input = child.stdin.take().expect("stdin");
        let _ = input.write_all(stdin);
    }
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait on spec") {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Some(Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().expect("spec exited, not killed by a signal"),
        stdout: String::from_utf8(out.join().expect("stdout")).expect("UTF-8 stdout"),
        stderr: String::from_utf8(err.join().expect("stderr")).expect("UTF-8 stderr"),
    })
}

fn run_spec(
    pair: &Pair,
    cwd: &Path,
    home: &Path,
    args: &[&str],
    stdin: &[u8],
    timeout: Duration,
) -> Run {
    run_spec_within(pair, cwd, home, args, stdin, timeout)
        .unwrap_or_else(|| panic!("spec {args:?} ran longer than {timeout:?}; killed"))
}

/// An ID-less document of `kind`: `title` in its front-matter when given,
/// `extra` closing the front-matter, `heading` its H1, `body` after it.
fn doc(kind: &str, title: Option<&str>, extra: &str, heading: &str, body: &str) -> String {
    let title = title.map_or_else(String::new, |title| format!("title: {title}\n"));
    format!("---\nkind: {kind}\n{title}{extra}---\n\n# {heading}\n\n{body}\n")
}

/// `links:` declaring `uses: [target]`.
fn uses(target: &str) -> String {
    format!("links:\n  uses: [{target}]\n")
}

/// A JSON value whose objects keep their keys in document order (the
/// tests' `serde_json` keeps no order).
#[derive(Debug)]
enum Ordered {
    Object(Vec<(String, Ordered)>),
    Array(Vec<Ordered>),
    Scalar,
}

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Ordered;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Ordered, A::Error> {
                let mut members = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Ordered>()? {
                    members.push((key, value));
                }
                Ok(Ordered::Object(members))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Ordered, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<Ordered>()? {
                    items.push(item);
                }
                Ok(Ordered::Array(items))
            }

            fn visit_bool<E>(self, _: bool) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_i64<E>(self, _: i64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_u64<E>(self, _: u64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_f64<E>(self, _: f64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_str<E>(self, _: &str) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_unit<E>(self) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

impl Ordered {
    fn parse(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{error}: {text}"))
    }

    fn keys(&self) -> Vec<&str> {
        match self {
            Self::Object(members) => members.iter().map(|(key, _)| key.as_str()).collect(),
            other => panic!("not an object: {other:?}"),
        }
    }

    fn get(&self, key: &str) -> &Self {
        match self {
            Self::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
                .unwrap_or_else(|| panic!("no key {key}")),
            other => panic!("not an object: {other:?}"),
        }
    }

    fn items(&self) -> &[Self] {
        match self {
            Self::Array(items) => items,
            other => panic!("not an array: {other:?}"),
        }
    }
}

/// The document's, every pair's and every side's keys are the spec's, in
/// its order.
fn assert_document_keys(stdout: &str) {
    let ordered = Ordered::parse(stdout);
    assert_eq!(ordered.keys(), DOCUMENT_KEYS, "{stdout}");
    for pair in ordered.get("pairs").items() {
        assert_eq!(pair.keys(), PAIR_KEYS, "{stdout}");
        assert_eq!(pair.get("a").keys(), SIDE_KEYS, "{stdout}");
        assert_eq!(pair.get("b").keys(), SIDE_KEYS, "{stdout}");
    }
}

/// The pairs of a document.
fn pairs(document: &Value) -> &Vec<Value> {
    document["pairs"]
        .as_array()
        .unwrap_or_else(|| panic!("pairs: {document}"))
}

/// A side's name: its ID, else its path.
fn name(side: &Value) -> String {
    side["id"]
        .as_str()
        .or_else(|| side["path"].as_str())
        .unwrap_or_else(|| panic!("a side without a name: {side}"))
        .to_owned()
}

/// Each pair's two names.
fn names(document: &Value) -> Vec<(String, String)> {
    pairs(document)
        .iter()
        .map(|pair| (name(&pair["a"]), name(&pair["b"])))
        .collect()
}

/// The pair of names `a` and `b`, in either order.
fn find<'a>(document: &'a Value, a: &str, b: &str) -> Option<&'a Value> {
    pairs(document).iter().find(|pair| {
        let (x, y) = (name(&pair["a"]), name(&pair["b"]));
        (x == a && y == b) || (x == b && y == a)
    })
}

/// A side of an ID-less document at `path`.
fn path_side(path: &str, kind: &str, title: &str, norm: &str) -> Value {
    json!({"id": null, "path": path, "ord": 0, "kind": kind, "title": title, "norm_hash": norm})
}

/// One mark's input item.
fn item(
    a: &str,
    a_norm: &str,
    b: &str,
    b_norm: &str,
    verdict: &str,
    proposal: Option<&str>,
) -> Value {
    json!({"a": a, "a_norm": a_norm, "b": b, "b_norm": b_norm, "verdict": verdict, "proposal": proposal})
}

fn batch(items: Vec<Value>) -> Value {
    json!({ "pairs": items })
}

/// A mark run's document (stdout), whatever its exit.
fn mark_document(run: &Run) -> Value {
    one_json_document(&run.stdout)
        .unwrap_or_else(|problem| panic!("mark: {problem}\n{}", run.show()))
}

/// The human author's JSON (core `Author::human()`).
const HUMAN: &str = r#"{"type":"human","role":null,"model":null,"run":null}"#;

// ------------------------------------------------------- the pinned example

const ENDURANCE: &str = "docs/spec/movement/endurance.md";
const STAMINA: &str = "docs/spec/movement/stamina.md";
const SPRINT: &str = "docs/spec/movement/sprint.md";

/// "The document"'s example file: `id: MEC-ENDURANCE`, `title: Stamina
/// pool`, `depends_on: [MEC-SPRINT]`.
const ENDURANCE_TEXT: &str = "---\nid: MEC-ENDURANCE\ntitle: Stamina pool\nlinks:\n  \
                              depends_on: [MEC-SPRINT]\n---\n\n# Stamina pool\n\nThe pool a \
                              keeper draws on to run.\n";

/// The copy with the example's `endurance.md` committed.
fn example(label: &str) -> Scene {
    let scene = Scene::new(label);
    scene.put(ENDURANCE, ENDURANCE_TEXT);
    scene
}

/// The pinned example: `--kind mechanic` over the copy with
/// `endurance.md`. The mechanics are MEC-SPRINT (`Sprint`, no front-matter
/// title: its H1), MEC-STAMINA (`Stamina`) and MEC-ENDURANCE (`Stamina
/// pool`). Targets: ENDURANCE {MEC-SPRINT}; STAMINA {R-12, A-101,
/// MEC-SPRINT, TERM-exhausted, DOM-MOVEMENT (its parent)}; SPRINT
/// {MEC-STAMINA, RULE-STAM-REGEN, DOM-MOVEMENT}. Titles: ` stamina pool `
/// holds 12 trigrams, ` stamina ` 7, all 7 shared: floor(2000 * 7 / 19) =
/// 736; `sprint` shares none with either. So ENDURANCE-STAMINA scores 736 +
/// 200 = 936, SPRINT-STAMINA (DOM-MOVEMENT) 200, ENDURANCE-SPRINT nothing
/// (no target, no trigram).
#[test]
fn the_pinned_example_text_and_json() {
    let scene = example("cp-example");
    let endurance_norm = scene.norm("MEC-ENDURANCE");
    let stamina_norm = scene.norm("MEC-STAMINA");
    let sprint_norm = scene.norm("MEC-SPRINT");

    // 736 + 200 * min(1, 5), and 0 + 200 * min(1, 5).
    let score = |titles: u64, shared: u64| titles + SHARED_WEIGHT * shared.min(SHARED_SCORED_MAX);
    assert_eq!((score(736, 1), score(0, 1)), (936, 200));
    let run = scene.collisions_run(&["--kind", "mechanic"], true);
    assert_document_keys(&run.stdout);
    let expected = json!({
        "project": "lantern-keep",
        "kinds": ["mechanic"],
        "any_kind": false,
        "limit": 50,
        "pairs": [
            {
                "a": {"id": "MEC-ENDURANCE", "path": ENDURANCE, "ord": 0, "kind": "mechanic",
                      "title": "Stamina pool", "norm_hash": endurance_norm},
                "b": {"id": "MEC-STAMINA", "path": STAMINA, "ord": 0, "kind": "mechanic",
                      "title": "Stamina", "norm_hash": stamina_norm},
                "score": 936, "titles": 736, "shared": 1, "shared_targets": ["MEC-SPRINT"],
                "marked": null,
                "reason": "titles alike 0.736; 1 link target(s) shared: `MEC-SPRINT`"
            },
            {
                "a": {"id": "MEC-SPRINT", "path": SPRINT, "ord": 0, "kind": "mechanic",
                      "title": "Sprint", "norm_hash": sprint_norm},
                "b": {"id": "MEC-STAMINA", "path": STAMINA, "ord": 0, "kind": "mechanic",
                      "title": "Stamina", "norm_hash": stamina_norm},
                "score": 200, "titles": 0, "shared": 1, "shared_targets": ["DOM-MOVEMENT"],
                "marked": null,
                "reason": "1 link target(s) shared: `DOM-MOVEMENT`"
            }
        ],
        "left": 0,
        "skipped": 0,
        "notes": []
    });
    assert_eq!(run.json(), expected, "{}", run.stdout);

    let text = scene.collisions_run(&["--kind", "mechanic"], false);
    assert_eq!(
        text.stdout,
        "collisions: 2 pair(s) listed, 0 left, 0 node(s) skipped\n\
         936 | MEC-ENDURANCE | MEC-STAMINA | mechanic | titles alike 0.736; 1 link target(s) \
         shared: `MEC-SPRINT`\n\
         200 | MEC-SPRINT | MEC-STAMINA | mechanic | 1 link target(s) shared: `DOM-MOVEMENT`\n",
        "{}",
        text.show()
    );
    assert_eq!(text.stderr, "", "nothing on stderr");

    // `--kind` never errs: a kind nobody holds lists nothing.
    let none = scene.collisions(&["--kind", "nothing-here"]);
    assert_eq!(none["pairs"], json!([]));
    assert_eq!(none["left"], json!(0));
    assert_eq!(none["kinds"], json!(["nothing-here"]));
}

// ------------------------------------------------------------------- CP1

const WICK: &str = "docs/spec/lamp/wick.md";
const HOOD: &str = "docs/spec/lamp/brass-hood.md";
const CHIMNEY: &str = "docs/spec/lamp/chimney.md";

/// CP1: two lamp parts (`Wick`, `Brass hood`: no trigram in common, see
/// the titles' sets ` wi`, `wic`, `ick`, `ck ` and ` br` … `od `), both
/// `uses: [MEC-SPRINT]`: one pair, `shared` 1, `shared_targets`
/// `["MEC-SPRINT"]`, `titles` 0, score 200; `a` the side first by path
/// (`brass-hood.md`). M: signal (a) dropped.
#[test]
fn cp1_two_parts_using_one_target_pair_with_score_200() {
    let scene = Scene::new("cp1-uses");
    scene.put(
        WICK,
        &doc(
            PART,
            Some("Wick"),
            &uses("MEC-SPRINT"),
            "Wick",
            "The wick draws oil to the flame.",
        ),
    );
    scene.put(
        HOOD,
        &doc(
            PART,
            Some("Brass hood"),
            &uses("MEC-SPRINT"),
            "Brass hood",
            "The hood keeps rain off the flame.",
        ),
    );
    let wick = scene.norm(WICK);
    let hood = scene.norm(HOOD);
    let run = scene.collisions_run(&["--kind", PART], true);
    assert_document_keys(&run.stdout);
    let document = run.json();
    assert_eq!(
        document["pairs"],
        json!([{
            "a": path_side(HOOD, PART, "Brass hood", &hood),
            "b": path_side(WICK, PART, "Wick", &wick),
            "score": 200, "titles": 0, "shared": 1, "shared_targets": ["MEC-SPRINT"],
            "marked": null, "reason": "1 link target(s) shared: `MEC-SPRINT`"
        }]),
        "{document}"
    );
    assert_eq!(document["left"], json!(0));
    assert_eq!(document["kinds"], json!([PART]));

    // The text names an ID-less document by its path.
    let text = scene.collisions_run(&["--kind", PART], false);
    assert_eq!(
        text.stdout,
        format!(
            "collisions: 1 pair(s) listed, 0 left, 0 node(s) skipped\n200 | {HOOD} | {WICK} | \
             {PART} | 1 link target(s) shared: `MEC-SPRINT`\n"
        )
    );
}

/// CP1: the same two parts citing MEC-SPRINT in their text (`mentions`)
/// and declaring nothing: no pair (`titles` 0, no strong target). M:
/// `mentions` counted.
#[test]
fn cp1_a_mention_is_no_shared_target() {
    let scene = Scene::new("cp1-mentions");
    scene.put(
        WICK,
        &doc(
            PART,
            Some("Wick"),
            "",
            "Wick",
            "The wick burns while MEC-SPRINT drains.",
        ),
    );
    scene.put(
        HOOD,
        &doc(
            PART,
            Some("Brass hood"),
            "",
            "Brass hood",
            "The hood rattles during MEC-SPRINT.",
        ),
    );
    let document = scene.collisions(&["--kind", PART]);
    assert_eq!(document["pairs"], json!([]), "{document}");
    assert_eq!(document["left"], json!(0));
}

/// CP1: both parts with `parent: MEC-SPRINT` and no `links`: one pair
/// through the parent's end, score 200.
#[test]
fn cp1_a_shared_parent_is_a_shared_target() {
    let scene = Scene::new("cp1-parent");
    scene.put(
        WICK,
        &doc(
            PART,
            Some("Wick"),
            "parent: MEC-SPRINT\n",
            "Wick",
            "The wick draws oil.",
        ),
    );
    scene.put(
        HOOD,
        &doc(
            PART,
            Some("Brass hood"),
            "parent: MEC-SPRINT\n",
            "Brass hood",
            "The hood keeps rain off.",
        ),
    );
    let document = scene.collisions(&["--kind", PART]);
    assert_eq!(
        names(&document),
        [(HOOD.to_owned(), WICK.to_owned())],
        "{document}"
    );
    let pair = &pairs(&document)[0];
    assert_eq!(pair["shared"], json!(1));
    assert_eq!(pair["shared_targets"], json!(["MEC-SPRINT"]));
    assert_eq!(pair["score"], json!(200));
    assert_eq!(pair["titles"], json!(0));
}

// ------------------------------------------------------------------- CP2

const OIL: &str = "docs/spec/lamp/oil.md";
const OILS: &str = "docs/spec/lamp/oils.md";
const SPARE: &str = "docs/spec/lamp/spare.md";

/// ` lantern oil reserve ` holds 19 trigrams, ` lantern oil reserves ` 20,
/// 18 shared: floor(2000 * 18 / 39) = 923.
const OIL_TITLES: u64 = 923;

fn oil_scene(label: &str, second_kind: &str) -> Scene {
    let scene = Scene::new(label);
    scene.put(
        OIL,
        &doc(
            NOTE,
            Some("Lantern oil reserve"),
            "",
            "Oil",
            "Kept in the cellar.",
        ),
    );
    scene.put(
        OILS,
        &doc(
            second_kind,
            Some("lantern  OIL reserves"),
            "",
            "Oils",
            "Counted every dusk.",
        ),
    );
    scene
}

/// CP2: two notes titled `Lantern oil reserve` and `lantern  OIL reserves`,
/// sharing no target: one pair, `titles` 923, `shared` 0, score 923, the
/// reason `titles alike 0.923`. M: signal (b) dropped.
#[test]
fn cp2_alike_titles_of_one_kind_pair() {
    let scene = oil_scene("cp2-titles", NOTE);
    let document = scene.collisions(&["--kind", NOTE]);
    assert_eq!(
        names(&document),
        [(OIL.to_owned(), OILS.to_owned())],
        "{document}"
    );
    let pair = &pairs(&document)[0];
    assert_eq!(pair["titles"], json!(OIL_TITLES));
    assert_eq!(pair["shared"], json!(0));
    assert_eq!(pair["shared_targets"], json!([]));
    assert_eq!(pair["score"], json!(OIL_TITLES));
    assert_eq!(pair["reason"], json!("titles alike 0.923"));
    assert_eq!(
        pair["b"]["title"],
        json!("lantern  OIL reserves"),
        "as written"
    );
}

/// CP2: the same titles on two kinds: no pair by default or with both
/// `--kind`s; one with `--any-kind`, its text kind column `a/b`. M: kinds
/// not compared.
#[test]
fn cp2_two_kinds_pair_only_with_any_kind() {
    let scene = oil_scene("cp2-kinds", PART);
    for args in [vec![], vec!["--kind", NOTE, "--kind", PART]] {
        let document = scene.collisions(&args);
        assert!(find(&document, OIL, OILS).is_none(), "{args:?}: {document}");
    }
    let any = scene.collisions(&["--any-kind"]);
    assert_eq!(any["any_kind"], json!(true));
    let pair = find(&any, OIL, OILS).unwrap_or_else(|| panic!("--any-kind: {any}"));
    assert_eq!(pair["titles"], json!(OIL_TITLES));
    assert_eq!(pair["score"], json!(OIL_TITLES));
    let text = scene.collisions_run(&["--any-kind"], false);
    let line = format!("{OIL_TITLES} | {OIL} | {OILS} | {NOTE}/{PART} | titles alike 0.923");
    assert!(
        text.stdout.lines().any(|row| row == line),
        "{line}\n{}",
        text.stdout
    );
}

/// CP2, W10: a `rule` document titled `Lantern oil reserve` holding its own
/// section of that title (`RULE-OIL-RESERVE`): never paired with it; a
/// section `Lantern oil reserves` of another document (`RULE-OIL-SPARE`)
/// pairs with both at 923, in `(path, ord)` order.
#[test]
fn cp2_a_document_never_pairs_with_its_own_section() {
    let scene = Scene::new("cp2-own");
    scene.put(
        OIL,
        "---\nkind: rule\ntitle: Lantern oil reserve\n---\n\n# Oil\n\nThe cellar holds it.\n\n\
         ## Lantern oil reserve {#RULE-OIL-RESERVE}\n\nTwo casks at most.\n",
    );
    scene.put(
        SPARE,
        "---\nkind: rule\ntitle: Cellar stores\n---\n\n# Spare\n\nWhat the cellar keeps.\n\n\
         ## Lantern oil reserves {#RULE-OIL-SPARE}\n\nOne cask aside.\n",
    );
    let document = scene.collisions(&["--kind", "rule"]);
    assert!(
        find(&document, OIL, "RULE-OIL-RESERVE").is_none(),
        "a document and its own section: {document}"
    );
    assert_eq!(
        names(&document),
        [
            (OIL.to_owned(), "RULE-OIL-SPARE".to_owned()),
            ("RULE-OIL-RESERVE".to_owned(), "RULE-OIL-SPARE".to_owned()),
        ],
        "{document}"
    );
    for pair in pairs(&document) {
        assert_eq!(pair["titles"], json!(OIL_TITLES), "{pair}");
    }
    assert_eq!(pairs(&document)[1]["a"]["ord"], json!(1));
    assert_eq!(pairs(&document)[1]["a"]["path"], json!(OIL));
}

// ------------------------------------------------------------------- CP3

/// CP3's three parts, no front-matter title (each title its H1), all
/// `uses: [MEC-SPRINT]`: `Brass hood`-`Glass chimney` 200 + 173 (they share
/// `ass` and `ss `, 2 of 10 + 13 trigrams: floor(4000 / 23) = 173, under
/// 500, so no `titles alike` part), `Brass hood`-`Wick` 200, `Glass
/// chimney`-`Wick` 200.
fn parts(label: &str) -> Scene {
    let scene = Scene::new(label);
    scene.put(
        WICK,
        &doc(
            PART,
            None,
            &uses("MEC-SPRINT"),
            "Wick",
            "The wick draws oil to the flame.",
        ),
    );
    scene.put(
        HOOD,
        &doc(
            PART,
            None,
            &uses("MEC-SPRINT"),
            "Brass hood",
            "The hood keeps rain off.",
        ),
    );
    scene.put(
        CHIMNEY,
        &doc(
            PART,
            None,
            &uses("MEC-SPRINT"),
            "Glass chimney",
            "The chimney draws the smoke up.",
        ),
    );
    scene
}

/// CP3: marking the hood-wick pair `distinct` hides it: three pairs become
/// two, and with `--limit 1` `left` drops from 2 to 1. A one-word edit of
/// the wick lists it again, `marked {verdict: distinct, proposal: null,
/// at}`, the reason's `changed since`; marked again with the new norm it
/// hides; trailing spaces and a blank line keep it hidden, and so does an
/// H1 rename (the title changes, no `norm_hash` does: a known limit). M: the
/// ledger keyed by name alone; keyed by `span_hash`.
#[test]
fn cp3_the_ledger_hides_a_pair_until_a_side_changes() {
    let scene = parts("cp3-ledger");
    let before = scene.collisions(&["--kind", PART]);
    assert_eq!(
        names(&before),
        [
            (HOOD.to_owned(), CHIMNEY.to_owned()),
            (HOOD.to_owned(), WICK.to_owned()),
            (CHIMNEY.to_owned(), WICK.to_owned()),
        ],
        "{before}"
    );
    let scores: Vec<&Value> = pairs(&before).iter().map(|pair| &pair["score"]).collect();
    assert_eq!(scores, [&json!(373), &json!(200), &json!(200)]);
    let limited = scene.collisions(&["--kind", PART, "--limit", "1"]);
    assert_eq!(pairs(&limited).len(), 1);
    assert_eq!(limited["left"], json!(2));

    // The norms as the read lists them, as an agent marks them.
    let listed_norm = |document: &Value, path: &str| -> String {
        pairs(document)
            .iter()
            .flat_map(|pair| [&pair["a"], &pair["b"]])
            .find(|side| side["path"] == path)
            .and_then(|side| side["norm_hash"].as_str())
            .unwrap_or_else(|| panic!("{path} listed: {document}"))
            .to_owned()
    };
    let hood = listed_norm(&before, HOOD);
    let wick = listed_norm(&before, WICK);
    let seq = scene.last_seq();
    let run = scene.mark(
        &batch(vec![item(HOOD, &hood, WICK, &wick, "distinct", None)]),
        &[],
    );
    run.code(0);
    assert_eq!(Ordered::parse(&run.stdout).keys(), MARK_KEYS);
    assert_eq!(
        mark_document(&run),
        json!({"marked": 1, "replaced": 0, "notes": []})
    );
    assert_eq!(scene.last_seq(), seq, "a mark adds no event");
    let ledger = scene.ledger();
    let row: Vec<&str> = ledger.trim().split('|').collect();
    assert_eq!(row.len(), 9, "{ledger}");
    assert_eq!(
        row[..8],
        [
            "lantern-keep",
            HOOD,
            hood.as_str(),
            WICK,
            wick.as_str(),
            "distinct",
            "<null>",
            HUMAN
        ],
        "{ledger}"
    );
    let at = row[8].to_owned();
    assert!(is_utc(&at), "at: {at}");

    let after = scene.collisions(&["--kind", PART]);
    assert_eq!(
        names(&after),
        [
            (HOOD.to_owned(), CHIMNEY.to_owned()),
            (CHIMNEY.to_owned(), WICK.to_owned()),
        ],
        "{after}"
    );
    let limited = scene.collisions(&["--kind", PART, "--limit", "1"]);
    assert_eq!(pairs(&limited).len(), 1);
    assert_eq!(limited["left"], json!(1), "{limited}");

    // A one-word edit of the wick: listed again, marked.
    scene.put(
        WICK,
        &doc(
            PART,
            None,
            &uses("MEC-SPRINT"),
            "Wick",
            "The wick draws lamp oil to the flame.",
        ),
    );
    let listed = scene.collisions(&["--kind", PART]);
    let pair = find(&listed, HOOD, WICK).unwrap_or_else(|| panic!("listed again: {listed}"));
    let edited = listed_norm(&listed, WICK);
    assert_ne!(edited, wick, "the word changes the meaning hash");
    // `spec show`'s norm after the word, for the premises below.
    let shown = scene.norm(WICK);
    assert_eq!(
        pair["marked"],
        json!({"verdict": "distinct", "proposal": null, "at": at}),
        "{listed}"
    );
    assert_eq!(
        pair["reason"],
        json!(format!(
            "1 link target(s) shared: `MEC-SPRINT`; marked distinct {at}, changed since"
        ))
    );
    assert_eq!(pair["b"]["norm_hash"], json!(edited));

    // Marked again with the new norm: hidden, the row replaced.
    let run = scene.mark(
        &batch(vec![item(WICK, &edited, HOOD, &hood, "distinct", None)]),
        &[],
    );
    run.code(0);
    assert_eq!(
        mark_document(&run),
        json!({"marked": 1, "replaced": 1, "notes": []})
    );
    assert!(find(&scene.collisions(&["--kind", PART]), HOOD, WICK).is_none());

    // Trailing spaces and a blank line: no `norm_hash` changes, hidden.
    scene.put(
        WICK,
        &doc(
            PART,
            None,
            &uses("MEC-SPRINT"),
            "Wick",
            "The wick draws lamp oil to the flame.   \n\n",
        ),
    );
    assert_eq!(
        scene.norm(WICK),
        shown,
        "premise: whitespace keeps the norm"
    );
    let kept = scene.collisions(&["--kind", PART]);
    assert!(find(&kept, HOOD, WICK).is_none(), "whitespace: {kept}");

    // An H1 rename: the title changes, the norm does not; hidden (pinned as
    // a known limit).
    scene.put(
        WICK,
        &doc(
            PART,
            None,
            &uses("MEC-SPRINT"),
            "Brass wick holder",
            "The wick draws lamp oil to the flame.   \n\n",
        ),
    );
    assert_eq!(scene.norm(WICK), shown, "premise: a heading keeps the norm");
    let renamed = scene.collisions(&["--kind", PART]);
    assert!(find(&renamed, HOOD, WICK).is_none(), "a rename: {renamed}");
    assert_eq!(
        find(&renamed, CHIMNEY, WICK).expect("the chimney pair")["b"]["title"],
        json!("Brass wick holder"),
        "the title is the H1's"
    );
}

/// `YYYY-MM-DDTHH:MM:SSZ`.
fn is_utc(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

// ------------------------------------------------------------------- CP4

const SIBLINGS: usize = 1_000;
const HALL: &str = "docs/spec/lamps/README.md";

fn sibling_path(n: usize) -> String {
    format!("docs/spec/lamps/s{n:04}.md")
}

/// The parent and its 1 000 children of one kind, committed.
fn flood(label: &str) -> Scene {
    let scene = Scene::new(label);
    scene.write(
        HALL,
        "---\nid: DOM-LAMPS\nclass: canon\ntier: 1\nparent: DOM-GAME\n---\n\n# Lamps\n\n\
         Every lamp of the keep.\n",
    );
    for n in 1..=SIBLINGS {
        scene.write(
            &sibling_path(n),
            &doc(
                PART,
                Some(&format!("Sibling {n}")),
                "parent: DOM-LAMPS\n",
                &format!("Sibling {n}"),
                &format!("Lamp number {n} of the hall."),
            ),
        );
    }
    scene.commit("A hall of lamps");
    scene
}

/// The title's trigram set as "Signal (b)" defines it: whitespace runs one
/// space, trimmed, lower-cased, padded with one space each side, every
/// window of three scalar values.
fn trigrams(title: &str) -> BTreeSet<String> {
    let normal = title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let chars: Vec<char> = format!(" {normal} ").chars().collect();
    chars
        .windows(3)
        .map(|window| window.iter().collect())
        .collect()
}

/// The title's words key (the orchestrator's ruling R1 on "Signal (b)"):
/// the normalized title split at whitespace, its words sorted, deduplicated
/// and joined.
fn words_key(title: &str) -> String {
    let lower = title.to_lowercase();
    let words: BTreeSet<&str> = lower.split_whitespace().collect();
    words.into_iter().collect::<Vec<_>>().join(" ")
}

/// The oracle of the flood under the orchestrator's ruling R1 (iteration
/// 2): a pair is a candidate when the two share a trigram held by at most
/// 200 titles of the group, or their words keys are equal in a bucket of at
/// most 200 titles; `titles` is Dice over the full trigram sets; it
/// qualifies at 500. Every qualifying pair `(score, a, b)` of the 1 000
/// siblings (no shared target: the parent's degree is over the cap), in the
/// spec's order (score descending, then the sides' paths, which sort as
/// their numbers).
fn sibling_oracle() -> Vec<(u64, usize, usize)> {
    let titles: Vec<String> = (1..=SIBLINGS).map(|n| format!("Sibling {n}")).collect();
    let sets: Vec<BTreeSet<String>> = titles.iter().map(|title| trigrams(title)).collect();
    let mut postings: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, set) in sets.iter().enumerate() {
        for gram in set {
            postings.entry(gram.as_str()).or_default().push(index);
        }
    }
    let mut candidates: BTreeSet<(usize, usize)> = BTreeSet::new();
    for holders in postings.values() {
        if holders.len() <= TRIGRAM_DF_MAX {
            for (at, &x) in holders.iter().enumerate() {
                for &y in &holders[at + 1..] {
                    candidates.insert((x, y));
                }
            }
        }
    }
    let mut buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, title) in titles.iter().enumerate() {
        buckets.entry(words_key(title)).or_default().push(index);
    }
    for members in buckets.values() {
        if members.len() <= TRIGRAM_DF_MAX {
            for (at, &x) in members.iter().enumerate() {
                for &y in &members[at + 1..] {
                    candidates.insert((x, y));
                }
            }
        }
    }
    let mut out = Vec::new();
    for (x, y) in candidates {
        let total = sets[x].len() + sets[y].len();
        let common = sets[x].intersection(&sets[y]).count();
        let titles = (2000 * common / total) as u64;
        if titles >= TITLES_MIN {
            // No shared target: the score is the titles'.
            out.push((titles, x + 1, y + 1));
        }
    }
    out.sort_by(|p, q| q.0.cmp(&p.0).then(p.1.cmp(&q.1)).then(p.2.cmp(&q.2)));
    out
}

/// CP4: 1 000 children of DOM-LAMPS titled `Sibling <n>`: no pair through
/// the parent (it is held by 1 000 > 64 of the group), every listed pair a
/// title pair; the first 50 and `left` as the oracle gives them (R1: 59 384
/// qualify: `Sibling 100`-`Sibling 1000` 956, nine `n`-`nnn` pairs 952, then
/// 857s and the rest in path order; iteration 1's capped sets gave 5 033,
/// 888 first); `--limit 200` lists the oracle's first 200;
/// the run (index warm) within 10 s. M: the degree cap removed; the sort
/// removed.
#[test]
fn cp4_a_flood_of_children_lists_title_pairs_in_order() {
    let scene = flood("cp4-flood");
    let oracle = sibling_oracle();
    assert_eq!(oracle.len(), 59_384, "the oracle's count (R1)");
    assert_eq!(oracle[0], (956, 100, 1000));

    // Warm the index, then the timed read.
    scene.spec(&["tree", "--json"], b"").code(0);
    let started = Instant::now();
    let run = run_spec_within(
        &scene.pair,
        scene.root(),
        &scene.pair.home,
        &["collisions", "--kind", PART, "--json"],
        b"",
        Duration::from_secs(10),
    )
    .unwrap_or_else(|| panic!("spec collisions ran past 10 s over the flood"));
    run.code(0);
    eprintln!("cp4: the flood's read took {:?}", started.elapsed());
    assert_document_keys(&run.stdout);
    let document = run.json();
    assert_eq!(document["limit"], json!(LIMIT_DEFAULT));
    let listed = pairs(&document);
    assert_eq!(listed.len(), LIMIT_DEFAULT);
    assert_eq!(document["left"], json!(oracle.len() - LIMIT_DEFAULT));
    assert_eq!(document["skipped"], json!(0));
    for (index, pair) in listed.iter().enumerate() {
        assert_eq!(
            pair["shared"],
            json!(0),
            "no pair through the parent: {pair}"
        );
        assert_eq!(pair["shared_targets"], json!([]), "{pair}");
        let (score, a, b) = oracle[index];
        assert_eq!(
            (
                pair["score"].clone(),
                pair["a"]["path"].clone(),
                pair["b"]["path"].clone()
            ),
            (json!(score), json!(sibling_path(a)), json!(sibling_path(b))),
            "pair {index}"
        );
        assert_eq!(pair["titles"], json!(score), "pair {index}");
    }

    let wide = scene.collisions(&["--kind", PART, "--limit", "200"]);
    assert_eq!(pairs(&wide).len(), LIMIT_MAX);
    assert_eq!(wide["left"], json!(oracle.len() - LIMIT_MAX));
    for (index, pair) in pairs(&wide).iter().enumerate() {
        let (score, a, b) = oracle[index];
        assert_eq!(
            (
                pair["a"]["path"].clone(),
                pair["b"]["path"].clone(),
                pair["score"].clone()
            ),
            (json!(sibling_path(a)), json!(sibling_path(b)), json!(score)),
            "--limit 200, pair {index}"
        );
    }

    for limit in ["201", "0"] {
        let run = scene.spec(&["collisions", "--kind", PART, "--limit", limit], b"");
        run.code(2);
        assert!(
            run.stderr
                .contains(&format!("spec: --limit {limit}: the limit is 1 to 200")),
            "{}",
            run.show()
        );
        assert_eq!(run.stdout, "", "{}", run.show());
    }
}

/// CP4: a rerun, a clone at another root with another `HOME`: the text and
/// the JSON byte-equal.
#[test]
fn cp4_the_flood_is_byte_equal_at_another_root_and_home() {
    let scene = flood("cp4-equal");
    let args_json = ["collisions", "--kind", PART, "--json"];
    let args_text = ["collisions", "--kind", PART];
    let first_json = scene.spec(&args_json, b"");
    first_json.code(0);
    let first_text = scene.spec(&args_text, b"");
    first_text.code(0);
    assert_eq!(
        scene.spec(&args_json, b"").stdout,
        first_json.stdout,
        "a rerun"
    );
    assert_eq!(
        scene.spec(&args_text, b"").stdout,
        first_text.stdout,
        "a rerun"
    );

    let other: PathBuf = scene.pair.scratch.join("elsewhere");
    scene.pair.git.git(
        scene.pair.scratch.path(),
        &[
            "clone",
            "-q",
            scene.root().to_str().expect("UTF-8"),
            other.to_str().expect("UTF-8"),
        ],
    );
    let home = scene.pair.scratch.home("other-home");
    let timeout = Duration::from_secs(120);
    let json_there = run_spec(&scene.pair, &other, &home, &args_json, b"", timeout);
    json_there.code(0);
    let text_there = run_spec(&scene.pair, &other, &home, &args_text, b"", timeout);
    text_there.code(0);
    assert_eq!(json_there.stdout, first_json.stdout, "JSON at another root");
    assert_eq!(text_there.stdout, first_text.stdout, "text at another root");
    assert!(
        first_text.stdout.starts_with(&format!(
            "collisions: 50 pair(s) listed, {} left, 0 node(s) skipped\n",
            59_384 - LIMIT_DEFAULT
        )),
        "{}",
        first_text.stdout
    );
}

// ------------------------------------------------------------------- CP5

/// CP5: every refusal of "Mark" over the pinned example, each leaving the
/// ledger and the last event `seq` as they were; a batch with one bad pair
/// writes none; a valid mark, then the same again (`replaced` 1); a
/// `reported` mark naming a real proposal; a stale norm a note. M: an event
/// emitted; the batch written in part.
#[test]
fn cp5_mark_refuses_each_rule_and_writes_nothing() {
    let scene = example("cp5-rules");
    let e = scene.norm("MEC-ENDURANCE");
    let s = scene.norm("MEC-STAMINA");
    let real = scene.question("MEC-STAMINA", "Is the lamp wick trimmed before a sprint?");
    let seq = scene.last_seq();
    assert_ne!(seq, "0", "premise: the question's events");
    let valid = || item("MEC-ENDURANCE", &e, "MEC-STAMINA", &s, "distinct", None);
    let good = "b3:".to_owned() + &"0".repeat(64);
    let many: Vec<Value> = (0..=MARKS_MAX)
        .map(|n| {
            item(
                &format!("MEC-X{n:03}"),
                &e,
                "MEC-STAMINA",
                &s,
                "distinct",
                None,
            )
        })
        .collect();
    assert_eq!(many.len(), 201);

    // (input, the last note's start)
    let refusals: Vec<(&str, Value, String)> = vec![
        (
            "unknown name",
            batch(vec![item(
                "MEC-NOWHERE",
                &e,
                "MEC-STAMINA",
                &s,
                "distinct",
                None,
            )]),
            "pairs[0].a: no live node `MEC-NOWHERE`".to_owned(),
        ),
        (
            "a equal to b",
            batch(vec![item(
                "MEC-STAMINA",
                &s,
                "MEC-STAMINA",
                &s,
                "distinct",
                None,
            )]),
            "pairs[0]: a pair of one node".to_owned(),
        ),
        (
            "a bad norm",
            batch(vec![item(
                "MEC-ENDURANCE",
                "b3:5E1F",
                "MEC-STAMINA",
                &s,
                "distinct",
                None,
            )]),
            "pairs[0].a_norm: ".to_owned(),
        ),
        (
            "an upper-case norm",
            batch(vec![item(
                "MEC-ENDURANCE",
                &e,
                "MEC-STAMINA",
                &s.to_uppercase().replacen("B3:", "b3:", 1),
                "distinct",
                None,
            )]),
            "pairs[0].b_norm: ".to_owned(),
        ),
        (
            "reported without a proposal",
            batch(vec![item(
                "MEC-ENDURANCE",
                &e,
                "MEC-STAMINA",
                &s,
                "reported",
                None,
            )]),
            "pairs[0].proposal: ".to_owned(),
        ),
        (
            "distinct with a proposal",
            batch(vec![item(
                "MEC-ENDURANCE",
                &e,
                "MEC-STAMINA",
                &s,
                "distinct",
                Some(&real),
            )]),
            "pairs[0].proposal: ".to_owned(),
        ),
        (
            "an unknown proposal",
            batch(vec![item(
                "MEC-ENDURANCE",
                &e,
                "MEC-STAMINA",
                &s,
                "reported",
                Some("PR-0099"),
            )]),
            "pairs[0].proposal: no proposal PR-0099".to_owned(),
        ),
        ("201 pairs", batch(many), "pairs".to_owned()),
        ("no pairs", batch(vec![]), "pairs".to_owned()),
        (
            "a pair repeated reversed",
            batch(vec![
                valid(),
                item("MEC-STAMINA", &s, "MEC-ENDURANCE", &e, "distinct", None),
            ]),
            "pairs[1]: repeats pairs[0]".to_owned(),
        ),
        (
            "a control character in a name",
            batch(vec![item(
                "MEC-END\u{7}",
                &e,
                "MEC-STAMINA",
                &s,
                "distinct",
                None,
            )]),
            "pairs[0].a: ".to_owned(),
        ),
        (
            "a name over 256 bytes",
            batch(vec![item(
                &"M".repeat(257),
                &e,
                "MEC-STAMINA",
                &s,
                "distinct",
                None,
            )]),
            "pairs[0].a: ".to_owned(),
        ),
        (
            "one bad pair in a batch",
            batch(vec![
                valid(),
                item("MEC-SPRINT", &good, "MEC-NOWHERE", &good, "distinct", None),
            ]),
            "pairs[1].b: no live node `MEC-NOWHERE`".to_owned(),
        ),
    ];
    for (what, input, reason) in refusals {
        let run = scene.mark(&input, &[]);
        assert_eq!(run.code, 1, "{what}: {}", run.show());
        let document = mark_document(&run);
        assert_eq!(document["marked"], json!(0), "{what}: {document}");
        let last = document["notes"]
            .as_array()
            .and_then(|notes| notes.last())
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{what}: no note: {document}"));
        assert!(
            last.starts_with(&reason),
            "{what}: `{last}` is not `{reason}…`"
        );
        assert_eq!(scene.ledger(), "", "{what}: nothing written");
        assert_eq!(scene.last_seq(), seq, "{what}: no event");
    }

    // Form errors, exit 2: an unknown verdict, an unknown field, no JSON.
    let mut extra = valid();
    extra["why"] = json!("an unknown field");
    for (what, input) in [
        (
            "an unknown verdict",
            batch(vec![item(
                "MEC-ENDURANCE",
                &e,
                "MEC-STAMINA",
                &s,
                "maybe",
                None,
            )])
            .to_string(),
        ),
        ("an unknown field", batch(vec![extra]).to_string()),
        (
            "an unknown top key",
            json!({"pairs": [valid()], "more": 1}).to_string(),
        ),
        ("no JSON", "pairs: none".to_owned()),
    ] {
        let run = scene.spec(
            &["collisions", "mark", "--input", "-", "--json"],
            input.as_bytes(),
        );
        assert_eq!(run.code, 2, "{what}: {}", run.show());
        assert_eq!(scene.ledger(), "", "{what}: nothing written");
    }

    // A valid mark, then again: replaced; neither adds an event.
    let run = scene.mark(&batch(vec![valid()]), &["--author-role", "reviewer"]);
    run.code(0);
    assert_eq!(
        mark_document(&run),
        json!({"marked": 1, "replaced": 0, "notes": []})
    );
    let row = scene.ledger();
    assert!(
        row.contains(r#"{"type":"agent","role":"reviewer","model":null,"run":null}"#),
        "{row}"
    );
    let run = scene.mark(&batch(vec![valid()]), &[]);
    run.code(0);
    assert_eq!(
        mark_document(&run),
        json!({"marked": 1, "replaced": 1, "notes": []})
    );
    assert_eq!(scene.ledger().lines().count(), 1, "one row per pair");
    assert!(scene.ledger().contains(HUMAN), "the second mark's author");
    assert_eq!(scene.last_seq(), seq, "no mark adds an event");

    // A `reported` mark naming the real proposal, given reversed: the row
    // holds `a` < `b`, each norm with its name.
    let run = scene.mark(
        &batch(vec![item(
            "MEC-STAMINA",
            &s,
            "MEC-ENDURANCE",
            &e,
            "reported",
            Some(&real),
        )]),
        &[],
    );
    run.code(0);
    let rows = scene.ledger();
    let row: Vec<&str> = rows.trim().split('|').collect();
    assert_eq!(
        row[..7],
        [
            "lantern-keep",
            "MEC-ENDURANCE",
            e.as_str(),
            "MEC-STAMINA",
            s.as_str(),
            "reported",
            real.as_str()
        ],
        "{rows}"
    );

    // A stale norm: a note, never a refusal.
    let run = scene.mark(
        &batch(vec![item(
            "MEC-ENDURANCE",
            &good,
            "MEC-STAMINA",
            &s,
            "distinct",
            None,
        )]),
        &[],
    );
    run.code(0);
    assert_eq!(
        mark_document(&run),
        json!({"marked": 1, "replaced": 1, "notes": [
            "pairs[0]: MEC-ENDURANCE changed since it was listed; the pair will be listed again"
        ]})
    );
    // ... and the pair is listed again: the ledger's norm is not the index's.
    let listed = scene.collisions(&["--kind", "mechanic"]);
    assert!(
        find(&listed, "MEC-ENDURANCE", "MEC-STAMINA").is_some(),
        "{listed}"
    );
    assert_eq!(scene.last_seq(), seq, "no mark adds an event");

    // The text form.
    let text = scene.spec(
        &["collisions", "mark", "--input", "-"],
        batch(vec![valid()]).to_string().as_bytes(),
    );
    text.code(0);
    assert_eq!(
        text.stdout,
        "marked 1 pair(s), 1 replacing an earlier mark\n",
        "{}",
        text.show()
    );
}

/// CP5: a section's canonical ID and an ID-less document's path are names;
/// an alias (TERM-exhausted's `TERM-tired`) is not ("as `spec collisions`
/// names it"), refused with nothing written.
#[test]
fn cp5_names_are_the_reads_names() {
    let scene = parts("cp5-names");
    let hood = scene.norm(HOOD);
    let regen = scene.norm("RULE-STAM-REGEN");
    let term = scene.norm("TERM-exhausted");
    let run = scene.mark(
        &batch(vec![item(
            "TERM-tired",
            &term,
            HOOD,
            &hood,
            "distinct",
            None,
        )]),
        &[],
    );
    assert_eq!(run.code, 1, "{}", run.show());
    assert_eq!(
        mark_document(&run)["notes"]
            .as_array()
            .and_then(|notes| notes.last()),
        Some(&json!("pairs[0].a: no live node `TERM-tired`"))
    );
    assert_eq!(scene.ledger(), "");
    let run = scene.mark(
        &batch(vec![item(
            HOOD,
            &hood,
            "RULE-STAM-REGEN",
            &regen,
            "distinct",
            None,
        )]),
        &[],
    );
    run.code(0);
    assert_eq!(mark_document(&run)["marked"], json!(1));
    let rows = scene.ledger();
    // `a` < `b` byte-wise: an upper-case ID before a `docs/` path.
    assert!(
        rows.starts_with(&format!(
            "lantern-keep|RULE-STAM-REGEN|{regen}|{HOOD}|{hood}|distinct|<null>|"
        )),
        "{rows}"
    );
}

// ------------------------------------------------------------------- CP6

fn env_at(home: &Path, cwd: &Path) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

/// `spec export state --out <file>` of `home`'s queue: the dump's lines.
fn export(scene: &Scene, home: &Path, file: &Path) -> Vec<String> {
    export_state(
        &env_at(home, scene.root()),
        &Globals::default(),
        &ExportStateRequest {
            out: Some(file.to_path_buf()),
            now: utc_now(),
            git: scene.pair.git_env(scene.root()),
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"));
    let text = fs::read_to_string(file).expect("the dump");
    text.strip_suffix('\n')
        .expect("ends with LF")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

/// `spec import-state <file>` into `home`, consent yes: `Ok`, else the
/// exit-2 message.
fn import(scene: &Scene, home: &Path, file: &Path) -> Result<(), String> {
    let mut consent = |_: &str| true;
    match import_state(
        &env_at(home, scene.root()),
        &Globals::default(),
        &ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut consent,
    ) {
        Ok(outcome) => {
            assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
            Ok(())
        }
        Err(error) => {
            assert_eq!(error.exit, Exit::CannotRun, "{error}");
            Err(error.message)
        }
    }
}

fn sql_at(db: &Path, sql: &str) -> String {
    let output = Command::new(common::proposal::sqlite3())
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The example scene with one question and the pair marked `distinct`.
fn marked(label: &str) -> (Scene, String, String) {
    let scene = example(label);
    scene.question("MEC-STAMINA", "Is the lamp wick trimmed before a sprint?");
    let e = scene.norm("MEC-ENDURANCE");
    let s = scene.norm("MEC-STAMINA");
    scene
        .mark(
            &batch(vec![item(
                "MEC-ENDURANCE",
                &e,
                "MEC-STAMINA",
                &s,
                "distinct",
                None,
            )]),
            &[],
        )
        .code(0);
    assert_eq!(scene.ledger().lines().count(), 1, "premise: one mark");
    (scene, e, s)
}

/// CP6: a queue holding a mark exports format 7 at schema 14 with no ledger
/// line (no line names a norm or the table); the dump imports into a fresh
/// data directory with an empty ledger at 14; the same dump at schema 13
/// imports; at 15 it is refused (`upgrade SpecEngine`), nothing written. M:
/// the ledger dumped.
#[test]
fn cp6_the_ledger_is_never_dumped() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 14);
    assert_eq!(STATE_FORMAT, 7);
    let (scene, e, s) = marked("cp6-dump");
    let dumps = scene.pair.scratch.dir("dumps");
    let lines = export(&scene, &scene.pair.home, &dumps.join("q.jsonl"));
    assert!(
        lines[0].starts_with("{\"format\":7,\"queue_schema\":14,\"project\":\"lantern-keep\","),
        "{}",
        lines[0]
    );
    for line in &lines {
        assert!(
            !line.contains("collision") && !line.contains(&e) && !line.contains(&s),
            "a ledger line: {line}"
        );
    }
    let header: Value = serde_json::from_str(&lines[0]).expect("the header");
    let counted: u64 = ["proposals", "tasks", "runs", "cards", "events"]
        .iter()
        .map(|key| {
            header[key]
                .as_u64()
                .unwrap_or_else(|| panic!("{key}: {header}"))
        })
        .sum();
    assert_eq!(
        lines.len() as u64,
        counted + 1,
        "the header and its rows only"
    );

    let fresh = scene.pair.scratch.home("fresh");
    assert_eq!(import(&scene, &fresh, &dumps.join("q.jsonl")), Ok(()));
    let db = data_dir(&fresh).join("lantern-keep.db");
    assert_eq!(sql_at(&db, "PRAGMA user_version;"), "14");
    assert_eq!(sql_at(&db, "SELECT count(*) FROM collision_marks;"), "0");
    assert_eq!(
        export(&scene, &fresh, &dumps.join("again.jsonl")),
        lines,
        "the round trip byte-equal"
    );

    // Format 7 at schema 13 imports; at 15 it is refused.
    let mut thirteen = lines.clone();
    thirteen[0] = thirteen[0].replacen("\"queue_schema\":14,", "\"queue_schema\":13,", 1);
    let file = dumps.join("thirteen.jsonl");
    fs::write(&file, format!("{}\n", thirteen.join("\n"))).unwrap();
    let older = scene.pair.scratch.home("older");
    assert_eq!(import(&scene, &older, &file), Ok(()));
    let db = data_dir(&older).join("lantern-keep.db");
    assert_eq!(sql_at(&db, "PRAGMA user_version;"), "14");
    assert_eq!(sql_at(&db, "SELECT count(*) FROM collision_marks;"), "0");

    let mut fifteen = lines.clone();
    fifteen[0] = fifteen[0].replacen("\"queue_schema\":14,", "\"queue_schema\":15,", 1);
    let file = dumps.join("fifteen.jsonl");
    fs::write(&file, format!("{}\n", fifteen.join("\n"))).unwrap();
    let newer = scene.pair.scratch.home("newer");
    let message = import(&scene, &newer, &file).expect_err("schema 15");
    assert!(
        message.starts_with(&format!("{}:1: ", file.display())),
        "{message}"
    );
    assert!(message.contains("upgrade SpecEngine"), "{message}");
    assert!(!data_dir(&newer).exists(), "nothing written");
}

/// CP6: a queue a newer build stepped (15) is refused by `spec collisions`
/// and its mark, left at 15; a schema 13 queue (the table gone) reads no
/// marks: the hidden pair is listed, `marked` null.
#[test]
fn cp6_a_newer_queue_is_refused_and_a_13_queue_reads_no_marks() {
    let (scene, e, s) = marked("cp6-schema");
    let input = batch(vec![item(
        "MEC-ENDURANCE",
        &e,
        "MEC-STAMINA",
        &s,
        "distinct",
        None,
    )]);
    assert!(
        find(
            &scene.collisions(&["--kind", "mechanic"]),
            "MEC-ENDURANCE",
            "MEC-STAMINA"
        )
        .is_none(),
        "premise: the mark hides the pair"
    );
    scene.pair.sql("PRAGMA user_version = 15;");
    for args in [
        vec!["collisions", "--kind", "mechanic"],
        vec!["collisions", "mark", "--input", "-"],
    ] {
        let run = scene.spec(&args, input.to_string().as_bytes());
        assert_eq!(run.code, 2, "{args:?}: {}", run.show());
        assert!(
            run.stderr
                .contains("schema version 15, this build knows 14"),
            "{args:?}: {}",
            run.show()
        );
    }
    assert_eq!(
        scene.pair.sql("PRAGMA user_version;").trim(),
        "15",
        "left alone"
    );

    scene
        .pair
        .sql("DROP TABLE collision_marks; PRAGMA user_version = 13;");
    let document = scene.collisions(&["--kind", "mechanic"]);
    let pair = find(&document, "MEC-ENDURANCE", "MEC-STAMINA")
        .unwrap_or_else(|| panic!("a 13 queue reads no marks: {document}"));
    assert_eq!(pair["marked"], Value::Null);
}

// --------------------------------------------------------------- edge data

/// A document whose front-matter carries `frontmatter-type` (`rev: "3"`)
/// has no `norm_hash`: it is never paired (its title is MEC-STAMINA's) and
/// counts in `skipped`, with the one note; the text's head line and its
/// `note:` line say so. Two holders of one ID (`duplicate-id`) never pair with each
/// other. Nothing panics.
#[test]
fn edge_a_node_without_a_norm_is_skipped_and_twins_by_id_never_pair() {
    let scene = example("cp-edge");
    let clean = scene.collisions(&[]);
    assert_eq!(clean["skipped"], json!(0), "premise: spec-a parses whole");
    scene.put(
        "docs/spec/movement/lantern.md",
        "---\nid: MEC-LANTERN\nrev: \"3\"\ntitle: Stamina\n---\n\n# Stamina\n\nA second gauge.\n",
    );
    scene.put(
        "docs/spec/lamp/twin-a.md",
        "---\nid: MEC-TWIN\ntitle: Twin lamp\n---\n\n# Twin lamp\n\nThe left one.\n",
    );
    scene.put(
        "docs/spec/lamp/twin-b.md",
        "---\nid: MEC-TWIN\ntitle: Twin lamp\n---\n\n# Twin lamp\n\nThe right one.\n",
    );
    let document = scene.collisions(&["--kind", "mechanic"]);
    assert_eq!(document["skipped"], json!(1), "{document}");
    assert_eq!(document["notes"], json!([SKIPPED_NOTE]), "{document}");
    for (a, b) in names(&document) {
        assert_ne!(a, "MEC-LANTERN", "{document}");
        assert_ne!(b, "MEC-LANTERN", "{document}");
        assert!(
            !(a == "MEC-TWIN" && b == "MEC-TWIN"),
            "twins by ID: {document}"
        );
    }
    let text = scene.collisions_run(&["--kind", "mechanic"], false);
    let head = text.stdout.lines().next().expect("a head line");
    assert!(head.ends_with(", 1 node(s) skipped"), "{head}");
    // The note is a read's note: on stderr, as `note: ` (the spec lists it
    // after the pairs; `2>&1` puts stderr first, as every read does).
    assert!(
        text.stderr
            .lines()
            .any(|line| line == format!("note: {SKIPPED_NOTE}")),
        "{}",
        text.show()
    );
    assert!(!text.stdout.contains("note:"), "{}", text.show());
}

/// The read's cap (`docs/canon/spec-cli-read.md` "The cap", as the reads
/// cut): 21 parts under a 160-character directory, all `uses:
/// [MEC-SPRINT]`, qualify 210 pairs whose 200 lines pass 40 000
/// characters; the text stays within the cap, the JSON lists the same pairs
/// as the text, every pair cut is counted in `left` (listed + left = 210),
/// and a note says so. Nothing panics.
#[test]
fn edge_the_read_cuts_at_the_cap_and_counts_the_cut_in_left() {
    let scene = Scene::new("cp-cap");
    let dir = format!("docs/spec/{}", "lamp-room-".repeat(16));
    for n in 0..21 {
        scene.write(
            &format!("{dir}/part-{n:02}.md"),
            &doc(
                PART,
                Some(&format!("Q{n:02}")),
                &uses("MEC-SPRINT"),
                &format!("Q{n:02}"),
                "A lamp part.",
            ),
        );
    }
    scene.commit("Lamp parts with long paths");
    let args = ["collisions", "--kind", PART, "--limit", "200"];
    let text = scene.spec(&args, b"");
    text.code(0);
    let json = scene.spec(&[&args[..], &["--json"]].concat(), b"");
    json.code(0);
    let document = json.json();
    let listed = pairs(&document).len();
    assert!(
        listed < 200,
        "premise: 200 lines of over 200 characters pass the cap: {listed}"
    );
    assert!(
        text.stdout.chars().count() <= 40_000,
        "{} characters",
        text.stdout.chars().count()
    );
    assert_eq!(
        text.stdout.lines().count(),
        listed + 1,
        "the head and the pairs"
    );
    assert_eq!(
        document["left"],
        json!(210 - listed),
        "the cut counted in left"
    );
    assert!(
        text.stdout.starts_with(&format!(
            "collisions: {listed} pair(s) listed, {} left, 0 node(s) skipped\n",
            210 - listed
        )),
        "{}",
        text.stdout.lines().next().unwrap_or_default()
    );
    let notes = document["notes"].as_array().expect("notes");
    assert!(
        notes
            .iter()
            .any(|note| note.as_str().is_some_and(|note| note.contains("40000"))),
        "a note names the cap: {notes:?}"
    );
    assert!(text.stderr.contains("40000"), "{}", text.show());
}
