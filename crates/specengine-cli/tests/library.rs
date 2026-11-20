//! docs/features/doc-library.md (RM-0127) "Acceptance criteria", the CLI
//! half: the `spec` binary over the scene (`tests/library_scene`, a scratch
//! git copy of `fixtures/spec-a` with the spec's changes, every commit by a
//! named author and committer at a named date), over copies of it, and over
//! this repository (read only, its index under a scratch `HOME`).
//!
//! - DL1: 13 rows at C1 in path order (spec-a's 12 live documents and the
//!   generated one), the document's, the counts', each row's, each
//!   decision's and each last change's keys in "Data"'s order; every value
//!   the scene's files give; `title`, `kind`, `status`, `archived`,
//!   `used_by`, `mentioned_by`, `orphan` equal `spec map --json`'s node of
//!   that name; `--archive` 15 rows, DEC-0007 and DEC-0031 archived. M:
//!   `tags` dropped from the row; generated documents left out.
//! - DL3: stamina's decisions DEC-0023 (accepted, its section) then DEC-0007
//!   (superseded), sprint's DEC-0031 (rejected), every other row's none,
//!   with and without `--archive`, `counts.decisions` 3; `draft`, `""` and
//!   no status are `other` (the core unit test of `decision_group` is the
//!   black-box twin here). M: `superseded-by …` read as accepted; Tier 3
//!   sources dropped without `--archive`.
//! - DL4: `behind` false at C1, true at C2 (and with stamina.md edited but
//!   not committed), false at C3; the text row ends `| behind | Stamina`.
//!   M: `>=` for `>`; `date:` compared instead of commits.
//! - DL5: no `.git`: every `last_change` and `newer` null, stamina `behind`
//!   null, sprint's false, the note once.
//! - DL6: `tags` never counted (`norm_hash`, `rev` as on spec-a), typed (no
//!   `unknown-key`), read by a closed class contract (`key-extra`), and
//!   mistyped as a string (`frontmatter-type`, `[]`, `norm_hash` null). M:
//!   `tags` counted in `norm_input` (and core `tests/norm_recipe.rs`);
//!   `tags` out of `TYPED_KEYS`.
//! - DL7: 38 entries: exactly six `tag-invalid` warnings with "Data"'s
//!   messages, the exit unchanged, 32 kept. M: the repeat rule dropped.
//! - DL8: `movement`'s rows are the documents of `spec tree DOM-MOVEMENT`; a
//!   direction's rows are those of its kinds ("Data"); a root naming
//!   nothing holds no row, one note; a root that is an ID section holds the
//!   documents under it, not its own document. M: the root's own document
//!   left out.
//! - DL9: an index stamped 17 holding stamina's `tags` in `extra` with
//!   `unknown-key` (the row a format-17 build stores, written by a raw
//!   `sqlite3`) is read again: the tags are served, no `unknown-key`, the
//!   stamp reads 18. M: `INDEX_FORMAT` left at 17. The literal form (a
//!   build of the base commit writes the index) is `#[ignore]`d:
//!   `SPECENGINE_BASE_SPEC=<base spec> cargo nextest run -p specengine-cli
//!   --test library --run-ignored only dl9_`.
//! - DL12 is `specengine-http`'s `tests/library.rs`: on this repository the
//!   CLI's JSON is cut at the output cap, so the whole answer is the route's.
//! - DL16: two roots under two `HOME`s, files created in opposite orders
//!   (the second indexed half-way, so its index rows are not in path
//!   order): text and JSON byte-equal. M: rows in index row order.
//! - "Data", **Text**, **Exits**: the text is the JSON written in its
//!   grammar, line for line; the cut keeps rows whole and says so; another
//!   flag, a REF or no project exit 2.
//!
//! Every expected value is worked out from the spec and the scene's files,
//! never read back from a run of the read under test (but the uses, which
//! are `spec map`'s by DL1's own words).

#![cfg(unix)]

mod common;
mod library_scene;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::git;
use common::{Run, program_with, spec_with};
use library_scene::{
    ANN_NAME, C1_AT, C2_AT, C3_AT, DEC_0007, DEC_0023, DEC_0031, LIST, MOVEMENT, SLUG, SPRINT,
    STAMINA, Scene, TAGS_LINE, TAGS_LINE_NO, ann,
};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};

// ------------------------------------------------------------- the keys

/// "Data": the document's six keys.
const DOC_KEYS: [&str; 6] = ["archive", "notes", "counts", "rows", "truncated", "omitted"];
/// "Data", **Counts**.
const COUNT_KEYS: [&str; 5] = ["documents", "archived", "decisions", "behind", "orphans"];
/// "Data": the 22 keys of a row, in order; `disciplines` the 23rd, after
/// `tags`, since docs/features/department-practices.md "The library".
const ROW_KEYS: [&str; 23] = [
    "name",
    "id",
    "path",
    "title",
    "class",
    "kind",
    "status",
    "archived",
    "tier",
    "owner",
    "reviewed",
    "scope",
    "tags",
    "disciplines",
    "rev",
    "parent",
    "modules",
    "used_by",
    "mentioned_by",
    "orphan",
    "last_change",
    "decisions",
    "behind",
];
/// "Data": a decision's 10 keys.
const DECISION_KEYS: [&str; 10] = [
    "name",
    "id",
    "title",
    "path",
    "status",
    "group",
    "date",
    "section",
    "last_change",
    "newer",
];
/// `spec catalog`'s `last_change` without `of_file`.
const CHANGE_KEYS: [&str; 5] = ["at", "by", "by_name", "commit", "proposal"];

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

fn keys_of(json: &str) -> Vec<String> {
    serde_json::from_str::<Keys>(json)
        .unwrap_or_else(|error| panic!("{error}: {json}"))
        .0
}

/// The raw text of each item of `key`'s array in `json` (its first
/// occurrence), the order of their keys kept.
fn raw_items(json: &str, key: &str) -> Vec<String> {
    let at = json
        .find(&format!("\"{key}\":["))
        .unwrap_or_else(|| panic!("no `{key}` array: {json}"));
    let bytes = json.as_bytes();
    let mut index = at + key.len() + 4;
    let mut out = Vec::new();
    let (mut depth, mut start, mut in_string) = (0usize, None, false);
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'\\' {
                index += 1;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => {
                    if depth == 0 {
                        start = Some(index);
                    }
                    depth += 1;
                }
                b'}' | b']' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        out.push(json[start.take().expect("an item's start")..=index].to_owned());
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    out
}

/// The raw text of `key`'s object in `json` (its first occurrence).
fn raw_object(json: &str, key: &str) -> Option<String> {
    let at = json.find(&format!("\"{key}\":{{"))?;
    let start = at + key.len() + 3;
    let bytes = json.as_bytes();
    let (mut depth, mut in_string, mut index) = (0usize, false, start);
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'\\' {
                index += 1;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(json[start..=index].to_owned());
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    panic!("`{key}`'s object does not end: {json}")
}

/// Every object of the raw document in "Data"'s key order: the document,
/// its counts, each row, each decision, each last change.
fn assert_key_orders(raw: &str) {
    assert_eq!(keys_of(raw), DOC_KEYS, "the document's keys");
    assert_eq!(
        keys_of(&raw_object(raw, "counts").expect("counts")),
        COUNT_KEYS,
        "the counts' keys"
    );
    for row in raw_items(raw, "rows") {
        assert_eq!(keys_of(&row), ROW_KEYS, "a row's keys: {row}");
        if let Some(change) = raw_object(&row, "last_change") {
            assert_eq!(keys_of(&change), CHANGE_KEYS, "a row's last change: {row}");
        }
        for decision in raw_items(&row, "decisions") {
            assert_eq!(
                keys_of(&decision),
                DECISION_KEYS,
                "a decision's keys: {decision}"
            );
            if let Some(change) = raw_object(&decision, "last_change") {
                assert_eq!(keys_of(&change), CHANGE_KEYS, "a decision's last change");
            }
        }
    }
}

// ------------------------------------------------------------- the runs

/// `spec args` in `cwd` under `home` with the scene's variables and `extra`
/// over them.
fn spec_at(scene: &Scene, cwd: &Path, home: &Path, args: &[&str], extra: &[(&str, &OsStr)]) -> Run {
    let vars = scene.vars_with(home, extra);
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

/// `spec args` in the scene's root under its `HOME`.
fn spec(scene: &Scene, args: &[&str]) -> Run {
    spec_at(scene, &scene.root, &scene.home, args, &[])
}

/// `spec library [--archive] --json` in `cwd`, exit 0: the run and its
/// document.
fn library_in(scene: &Scene, cwd: &Path, home: &Path, archive: bool) -> (Run, Value) {
    let mut args = vec!["library", "--json"];
    if archive {
        args.push("--archive");
    }
    let run = spec_at(scene, cwd, home, &args, &[]);
    run.code(0);
    let document = run.json();
    (run, document)
}

fn library(scene: &Scene, archive: bool) -> (Run, Value) {
    library_in(scene, &scene.root, &scene.home, archive)
}

/// `spec map [--archive] --json`'s nodes by name.
fn map_nodes(scene: &Scene, archive: bool) -> BTreeMap<String, Value> {
    let mut args = vec!["map", "--json"];
    if archive {
        args.push("--archive");
    }
    let run = spec(scene, &args);
    run.code(0);
    run.json()["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| {
            (
                node["name"].as_str().expect("a name").to_owned(),
                node.clone(),
            )
        })
        .collect()
}

fn rows(document: &Value) -> &Vec<Value> {
    document["rows"].as_array().expect("rows")
}

/// The row named `name`.
fn row<'a>(document: &'a Value, name: &str) -> &'a Value {
    rows(document)
        .iter()
        .find(|row| row["name"] == name)
        .unwrap_or_else(|| panic!("no row {name}: {document}"))
}

fn names(document: &Value) -> Vec<String> {
    rows(document)
        .iter()
        .map(|row| row["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// A `last_change` of the scene's: Ann's commit `commit` at `at`.
fn change(at: &str, commit: &str) -> Value {
    json!({"at": at, "by": ann(), "by_name": ANN_NAME, "commit": commit, "proposal": null})
}

/// The `check --json` findings of `code`.
fn findings(run: &Run, code: &str) -> Vec<Value> {
    let report = run.json();
    report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| finding["code"] == code)
        .cloned()
        .collect()
}

/// `spec check --json` in the scene's root: exit 0 or 1, the run.
fn check(scene: &Scene) -> Run {
    let run = spec(scene, &["check", "--json"]);
    assert!(run.code == 0 || run.code == 1, "{}", run.show());
    run
}

// ------------------------------------------------------- the scene's rows

/// One row as the scene's files give it.
struct Want {
    path: &'static str,
    name: &'static str,
    id: Option<&'static str>,
    class: &'static str,
    tier: Option<i64>,
    /// `owner: owner` and `reviewed: 2026-09-20` written.
    owned: bool,
    scope: Option<&'static [&'static str]>,
    parent: Option<&'static str>,
    /// `None`: the row's served kind decides (A-102 writes `kind:
    /// requirement` in a record whose prefix says assumption).
    modules: Option<&'static [&'static str]>,
    tier3: bool,
}

const fn want(path: &'static str, name: &'static str, class: &'static str) -> Want {
    Want {
        path,
        name,
        id: None,
        class,
        tier: None,
        owned: false,
        scope: None,
        parent: None,
        modules: Some(&[]),
        tier3: false,
    }
}

/// Every row of the scene with `--archive`, in path order.
fn wanted() -> Vec<Want> {
    let record = |path: &'static str, id: &'static str| Want {
        id: Some(id),
        owned: true,
        ..want(path, id, "canon")
    };
    let decision =
        |path: &'static str, id: &'static str, scope: &'static [&'static str], tier3: bool| Want {
            id: Some(id),
            scope: Some(scope),
            tier3,
            ..want(path, id, "decision")
        };
    vec![
        Want {
            scope: Some(&["movement"]),
            ..want(
                "docs/features/stamina-tuning.md",
                "docs/features/stamina-tuning.md",
                "spec",
            )
        },
        want(LIST, LIST, "generated"),
        record("docs/records/A/A-101.md", "A-101"),
        Want {
            modules: None,
            ..record("docs/records/A/A-102.md", "A-102")
        },
        decision(DEC_0007, "DEC-0007", &["stamina"], true),
        decision(DEC_0023, "DEC-0023", &["stamina"], false),
        decision(DEC_0031, "DEC-0031", &["sprint"], true),
        Want {
            modules: Some(&["records"]),
            ..record("docs/records/Q/Q-031.md", "Q-031")
        },
        Want {
            modules: Some(&["records"]),
            ..record("docs/records/Q/Q-032.md", "Q-032")
        },
        Want {
            modules: Some(&["records"]),
            ..record("docs/records/R/R-12.md", "R-12")
        },
        record("docs/records/TERM/TERM-exhausted.md", "TERM-exhausted"),
        Want {
            tier: Some(0),
            ..record("docs/spec/game.md", "DOM-GAME")
        },
        Want {
            tier: Some(1),
            parent: Some("DOM-GAME"),
            modules: Some(&["movement"]),
            ..record(MOVEMENT, "DOM-MOVEMENT")
        },
        Want {
            tier: Some(2),
            parent: Some("DOM-MOVEMENT"),
            modules: Some(&["movement"]),
            ..record(SPRINT, "MEC-SPRINT")
        },
        Want {
            tier: Some(2),
            parent: Some("DOM-MOVEMENT"),
            modules: Some(&["movement"]),
            ..record(STAMINA, "MEC-STAMINA")
        },
    ]
}

/// The decisions of the scene's rows at C1 (or with DEC-0023 last changed
/// at `dec_0023`): stamina's two, sprint's one.
fn decisions_of(
    c1: &str,
    dec_0023: (&str, &str),
    newer_0023: Value,
) -> BTreeMap<&'static str, Value> {
    BTreeMap::from([
        (
            "MEC-STAMINA",
            json!([
                {"name": "DEC-0023", "id": "DEC-0023", "title": "Regeneration waits for rest",
                 "path": DEC_0023, "status": "accepted", "group": "accepted",
                 "date": "2026-09-01", "section": "RULE-STAM-REGEN",
                 "last_change": change(dec_0023.0, dec_0023.1), "newer": newer_0023},
                {"name": "DEC-0007", "id": "DEC-0007", "title": "Stamina regenerates while walking",
                 "path": DEC_0007, "status": "superseded-by DEC-0023", "group": "superseded",
                 "date": "2026-08-01", "section": null,
                 "last_change": change(C1_AT, c1), "newer": false}
            ]),
        ),
        (
            "MEC-SPRINT",
            json!([
                {"name": "DEC-0031", "id": "DEC-0031", "title": "Sprint drains the lantern",
                 "path": DEC_0031, "status": "rejected", "group": "rejected",
                 "date": "2026-09-10", "section": null,
                 "last_change": change(C1_AT, c1), "newer": false}
            ]),
        ),
    ])
}

/// Each row of `document` against the scene's files at C1, its uses
/// against the map's node of its name.
fn assert_rows_at_c1(scene: &Scene, document: &Value, archive: bool) {
    let wanted: Vec<Want> = wanted()
        .into_iter()
        .filter(|want| archive || !want.tier3)
        .collect();
    assert_eq!(
        names(document),
        wanted.iter().map(|want| want.name).collect::<Vec<_>>(),
        "the rows in path order"
    );
    let paths: Vec<&str> = rows(document)
        .iter()
        .map(|row| row["path"].as_str().expect("a path"))
        .collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted, "by path, in byte order");
    let decisions = decisions_of(&scene.c1, (C1_AT, &scene.c1), json!(false));
    let map = map_nodes(scene, archive);
    for want in &wanted {
        let row = row(document, want.name);
        let context = format!("{}: {row}", want.name);
        assert_eq!(row["path"], want.path, "{context}");
        assert_eq!(row["id"], json!(want.id), "{context}");
        assert_eq!(row["class"], want.class, "{context}");
        assert_eq!(row["archived"], want.tier3, "{context}");
        assert_eq!(row["tier"], json!(want.tier), "{context}");
        let (owner, reviewed) = if want.owned {
            (json!("owner"), json!("2026-09-20"))
        } else {
            (Value::Null, Value::Null)
        };
        assert_eq!(row["owner"], owner, "{context}");
        assert_eq!(row["reviewed"], reviewed, "{context}");
        assert_eq!(row["scope"], json!(want.scope), "{context}");
        assert_eq!(row["parent"], json!(want.parent), "{context}");
        assert_eq!(row["rev"], 1, "none is written: {context}");
        let tags = if want.name == "MEC-STAMINA" {
            json!(["stamina", "tuning"])
        } else {
            json!([])
        };
        assert_eq!(row["tags"], tags, "{context}");
        let modules = want.modules.map_or_else(
            || {
                let kind = row["kind"].as_str().unwrap_or_default();
                if ["question", "requirement"].contains(&kind) {
                    json!(["records"])
                } else {
                    json!([])
                }
            },
            |modules| json!(modules),
        );
        assert_eq!(row["modules"], modules, "{context}");
        assert_eq!(row["last_change"], change(C1_AT, &scene.c1), "{context}");
        assert_eq!(
            row["decisions"],
            decisions
                .get(want.name)
                .cloned()
                .unwrap_or_else(|| json!([])),
            "{context}"
        );
        assert_eq!(row["behind"], false, "{context}");
        if let Some(node) = map.get(want.name) {
            for key in [
                "title",
                "kind",
                "status",
                "archived",
                "used_by",
                "mentioned_by",
                "orphan",
            ] {
                assert_eq!(
                    row[key], node[key],
                    "{key} of {context} vs the map's {node}"
                );
            }
        }
    }
    let stamina = row(document, "MEC-STAMINA");
    assert_eq!(
        (&stamina["title"], &stamina["kind"], &stamina["status"]),
        (&json!("Stamina"), &json!("mechanic"), &json!("accepted"))
    );
    assert_eq!(row(document, "DOM-GAME")["title"], "Lantern Keep");
    assert_eq!(row(document, LIST)["title"], Value::Null, "no H1, no title");
    assert!(
        map.contains_key("MEC-STAMINA") && map.contains_key("A-101"),
        "premise: the map lists the scene's documents"
    );
    let orphans = rows(document)
        .iter()
        .filter(|row| row["orphan"] == true)
        .count();
    assert_eq!(
        document["counts"],
        json!({"documents": wanted.len(), "archived": if archive { 2 } else { 0 },
               "decisions": 3, "behind": 0, "orphans": orphans})
    );
    assert_eq!(document["archive"], archive);
    assert_eq!(document["notes"], json!([]));
    assert_eq!(document["truncated"], false);
    assert_eq!(document["omitted"], Value::Null);
}

// ------------------------------------------------------- the text grammar

fn or_dash(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// "Data", **Text**: `document` written in the grammar, line for line.
fn text_of(document: &Value) -> String {
    let counts = &document["counts"];
    let mut out = format!(
        "library | documents {}, archived {} | decisions {} | behind {} | orphans {}{}\n",
        counts["documents"],
        counts["archived"],
        counts["decisions"],
        counts["behind"],
        counts["orphans"],
        if document["archive"] == true {
            " [archive]"
        } else {
            ""
        }
    );
    for row in rows(document) {
        let tags: Vec<&str> = row["tags"]
            .as_array()
            .expect("tags")
            .iter()
            .map(|tag| tag.as_str().expect("a tag"))
            .collect();
        let decisions = row["decisions"].as_array().expect("decisions");
        let count = |group: &str| {
            decisions
                .iter()
                .filter(|decision| decision["group"] == group)
                .count()
        };
        let change = &row["last_change"];
        out.push_str(&format!(
            "{} | {} | {} | {} | version {} | tier {} | owner {} | reviewed {} | tags {} | used by \
             {}, mentioned by {}{} | {} | {} | decisions {} accepted, {} superseded, {} rejected, \
             {} other{} | {}\n",
            or_dash(&row["name"]),
            or_dash(&row["class"]),
            or_dash(&row["kind"]),
            or_dash(&row["status"]),
            or_dash(&row["rev"]),
            or_dash(&row["tier"]),
            or_dash(&row["owner"]),
            or_dash(&row["reviewed"]),
            if tags.is_empty() {
                "-".to_owned()
            } else {
                tags.join(", ")
            },
            row["used_by"],
            row["mentioned_by"],
            if row["orphan"] == true {
                ", orphan"
            } else {
                ""
            },
            or_dash(&change["at"]),
            or_dash(&change["by_name"]),
            count("accepted"),
            count("superseded"),
            count("rejected"),
            decisions.len() - count("accepted") - count("superseded") - count("rejected"),
            if row["behind"] == true {
                " | behind"
            } else {
                ""
            },
            or_dash(&row["title"]),
        ));
        for decision in decisions {
            out.push_str(&format!(
                "    decision {} | {} | {} | {} | {}{}\n",
                or_dash(&decision["name"]),
                or_dash(&decision["group"]),
                or_dash(&decision["date"]),
                or_dash(&decision["last_change"]["at"]),
                or_dash(&decision["section"]),
                if decision["newer"] == true {
                    " | newer"
                } else {
                    ""
                },
            ));
        }
    }
    out
}

/// `spec library [--archive]`'s text equals its JSON in the grammar.
fn assert_text_is_the_json(scene: &Scene, archive: bool) -> String {
    let (_, document) = library(scene, archive);
    let mut args = vec!["library"];
    if archive {
        args.push("--archive");
    }
    let text = spec(scene, &args);
    text.code(0);
    assert_eq!(text.stdout, text_of(&document), "{}", text.show());
    text.stdout
}

// ------------------------------------------------------------------ DL1

/// DL1: the scene at C1, plain and `--archive`. M: `tags` dropped from the
/// row; generated documents left out.
#[test]
fn dl1_rows_are_every_document_in_path_order() {
    let scene = Scene::new("dl1");
    let (run, document) = library(&scene, false);
    assert_key_orders(run.stdout.trim_end());
    assert_eq!(
        rows(&document).len(),
        13,
        "spec-a's 12 live and the generated one"
    );
    assert_rows_at_c1(&scene, &document, false);
    let generated = row(&document, LIST);
    assert_eq!(generated["class"], "generated");
    let stamina = row(&document, "MEC-STAMINA");
    assert_eq!(stamina["tags"], json!(["stamina", "tuning"]));
    assert_eq!(stamina["tier"], 2);
    assert_eq!(stamina["parent"], "DOM-MOVEMENT");

    let (run, archived) = library(&scene, true);
    assert_key_orders(run.stdout.trim_end());
    assert_eq!(rows(&archived).len(), 15);
    assert_rows_at_c1(&scene, &archived, true);
    for name in ["DEC-0007", "DEC-0031"] {
        assert_eq!(row(&archived, name)["archived"], true, "{name}");
    }
    assert!(
        !run.stderr.contains("note: library: "),
        "no note: {}",
        run.show()
    );
}

/// "Data", **Text**: the text is the JSON in the grammar, plain and with
/// `--archive`; the header's `[archive]`.
#[test]
fn text_is_the_json_in_its_grammar() {
    let scene = Scene::new("text");
    let plain = assert_text_is_the_json(&scene, false);
    assert!(
        plain.starts_with("library | documents 13, archived 0 | decisions 3 | behind 0 | orphans "),
        "{plain}"
    );
    let archived = assert_text_is_the_json(&scene, true);
    assert!(
        archived.lines().next().is_some_and(|line| line
            .starts_with("library | documents 15, archived 2 | decisions 3 | behind 0 | orphans ")
            && line.ends_with(" [archive]")),
        "{archived}"
    );
    assert!(
        plain.contains(&format!(
            "    decision DEC-0031 | rejected | 2026-09-10 | {C1_AT} | -\n"
        )),
        "{plain}"
    );
}

// ------------------------------------------------------------------ DL3

/// DL3: the decisions by status, with dates, with and without `--archive`.
/// M: `superseded-by …` read as accepted; Tier 3 sources dropped without
/// `--archive`.
#[test]
fn dl3_decisions_by_status_with_dates() {
    let scene = Scene::new("dl3");
    for archive in [false, true] {
        let (_, document) = library(&scene, archive);
        let want = decisions_of(&scene.c1, (C1_AT, &scene.c1), json!(false));
        for row in rows(&document) {
            let name = row["name"].as_str().expect("a name");
            assert_eq!(
                row["decisions"],
                want.get(name).cloned().unwrap_or_else(|| json!([])),
                "{name}, archive {archive}"
            );
        }
        let stamina = row(&document, "MEC-STAMINA")["decisions"]
            .as_array()
            .expect("decisions")
            .iter()
            .map(|decision| {
                (
                    decision["name"].as_str().unwrap().to_owned(),
                    decision["group"].as_str().unwrap().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            stamina,
            [
                ("DEC-0023".to_owned(), "accepted".to_owned()),
                ("DEC-0007".to_owned(), "superseded".to_owned())
            ],
            "archive {archive}"
        );
        assert_eq!(document["counts"]["decisions"], 3, "archive {archive}");
    }
}

/// DL3, "Data" `group`: `draft`, `""` and no status are `other` (core
/// `decision_group`'s cases, through the read); within a group `date`
/// descending as text, null last, then `name`.
#[test]
fn dl3_a_status_core_does_not_read_is_other() {
    let scene = Scene::new("dl3-other");
    let write = |id: &str, status: Option<&str>, date: Option<&str>| {
        let mut front = format!("---\nid: {id}\nclass: decision\n");
        if let Some(status) = status {
            front.push_str(&format!("status: {status}\n"));
        }
        if let Some(date) = date {
            front.push_str(&format!("date: {date}\n"));
        }
        front.push_str("scope: [sprint]\ncanon: MEC-SPRINT\n---\n\n");
        scene.put(
            &format!("docs/records/DEC/{id}.md"),
            &format!("{front}# Decision {id}\n\nA decision on the sprint.\n"),
        );
    };
    write("DEC-0040", Some("draft"), None);
    write("DEC-0041", Some("\"\""), Some("2026-10-01"));
    write("DEC-0042", None, None);
    write("DEC-0043", Some("proposed"), Some("2026-10-02"));
    scene.commit(C2_AT, "Four open decisions on the sprint\n");
    let (_, document) = library(&scene, false);
    let sprint: Vec<(String, String, Value)> = row(&document, "MEC-SPRINT")["decisions"]
        .as_array()
        .expect("decisions")
        .iter()
        .map(|decision| {
            (
                decision["name"].as_str().unwrap().to_owned(),
                decision["group"].as_str().unwrap().to_owned(),
                decision["date"].clone(),
            )
        })
        .collect();
    assert_eq!(
        sprint,
        [
            (
                "DEC-0031".to_owned(),
                "rejected".to_owned(),
                json!("2026-09-10")
            ),
            (
                "DEC-0043".to_owned(),
                "other".to_owned(),
                json!("2026-10-02")
            ),
            (
                "DEC-0041".to_owned(),
                "other".to_owned(),
                json!("2026-10-01")
            ),
            ("DEC-0040".to_owned(), "other".to_owned(), Value::Null),
            ("DEC-0042".to_owned(), "other".to_owned(), Value::Null),
        ],
        "{document}"
    );
    assert_eq!(document["counts"]["decisions"], 7);
    assert_eq!(
        row(&document, "MEC-SPRINT")["behind"],
        false,
        "no accepted one"
    );
}

// ------------------------------------------------------------------ DL4

/// Stamina's `behind`, DEC-0023's `newer` and `counts.behind`.
fn behind_state(document: &Value) -> (Value, Value, Value) {
    let stamina = row(document, "MEC-STAMINA");
    let dec = stamina["decisions"]
        .as_array()
        .expect("decisions")
        .iter()
        .find(|decision| decision["name"] == "DEC-0023")
        .cloned()
        .expect("DEC-0023 under stamina");
    (
        stamina["behind"].clone(),
        dec["newer"].clone(),
        document["counts"]["behind"].clone(),
    )
}

/// DL4: one commit for everything reads not behind; C2 (DEC-0023 alone, 60
/// s later) behind, and the text row ends `| behind | Stamina`; an
/// uncommitted edit of stamina.md keeps it; C3 (stamina.md, 60 s later) not
/// behind. M: `>=` for `>` (C1 reads behind); `date:` compared instead of
/// commits (C2 does not).
#[test]
fn dl4_behind_its_decision_follows_the_commits() {
    let scene = Scene::new("dl4");
    let (_, at_c1) = library(&scene, false);
    assert_eq!(
        behind_state(&at_c1),
        (json!(false), json!(false), json!(0)),
        "C1: one commit holds both"
    );

    let c2 = scene.c2();
    let (_, at_c2) = library(&scene, false);
    assert_eq!(
        behind_state(&at_c2),
        (json!(true), json!(true), json!(1)),
        "C2: DEC-0023 committed after stamina"
    );
    let decisions = decisions_of(&scene.c1, (C2_AT, &c2), json!(true));
    assert_eq!(
        row(&at_c2, "MEC-STAMINA")["decisions"],
        decisions["MEC-STAMINA"]
    );
    assert_eq!(
        row(&at_c2, "MEC-STAMINA")["last_change"],
        change(C1_AT, &scene.c1)
    );
    // The text: the grammar over the JSON, and the stamina row and its two
    // decision lines spelled out.
    let text = assert_text_is_the_json(&scene, false);
    let stamina = row(&at_c2, "MEC-STAMINA");
    assert_eq!(stamina["orphan"], false, "premise: stamina is used");
    let line = format!(
        "MEC-STAMINA | canon | mechanic | accepted | version 1 | tier 2 | owner owner | reviewed \
         2026-09-20 | tags stamina, tuning | used by {}, mentioned by {} | {C1_AT} | {ANN_NAME} | \
         decisions 1 accepted, 1 superseded, 0 rejected, 0 other | behind | Stamina\n    decision \
         DEC-0023 | accepted | 2026-09-01 | {C2_AT} | RULE-STAM-REGEN | newer\n    decision \
         DEC-0007 | superseded | 2026-08-01 | {C1_AT} | -\n",
        stamina["used_by"], stamina["mentioned_by"]
    );
    assert!(text.contains(&line), "{line}\nnot in\n{text}");
    assert!(
        text.lines()
            .any(|line| line.starts_with("MEC-STAMINA | ") && line.ends_with("| behind | Stamina")),
        "{text}"
    );
    assert!(
        text.starts_with("library | documents 13, archived 0 | decisions 3 | behind 1 | orphans "),
        "{text}"
    );

    scene.stamina_edit();
    let (_, edited) = library(&scene, false);
    assert_eq!(
        behind_state(&edited),
        (json!(true), json!(true), json!(1)),
        "an uncommitted edit: HEAD's last change"
    );
    assert_eq!(
        row(&edited, "MEC-STAMINA")["last_change"],
        change(C1_AT, &scene.c1)
    );

    let c3 = scene.c3();
    let (_, at_c3) = library(&scene, false);
    assert_eq!(
        behind_state(&at_c3),
        (json!(false), json!(false), json!(0)),
        "C3: stamina committed after DEC-0023"
    );
    assert_eq!(
        row(&at_c3, "MEC-STAMINA")["last_change"],
        change(C3_AT, &c3)
    );
    assert_text_is_the_json(&scene, false);
}

/// "Rules": a canon page new and uncommitted reads no last change, so its
/// accepted decision's `newer` and its `behind` are null.
#[test]
fn a_new_uncommitted_canon_page_with_an_accepted_decision_reads_behind_null() {
    let scene = Scene::new("dl4-new");
    scene.put(
        "docs/records/DEC/DEC-0050.md",
        "---\nid: DEC-0050\nclass: decision\nstatus: accepted\ndate: 2026-10-01\nscope: \
         [climb]\ncanon: MEC-CLIMB\n---\n\n# Climbing is allowed\n\nClimb at half speed.\n",
    );
    scene.commit(C2_AT, "Allow climbing\n");
    // The page the decision names, new and never committed.
    scene.put(
        "docs/spec/movement/climb.md",
        "---\nid: MEC-CLIMB\nclass: canon\nparent: DOM-MOVEMENT\nowner: owner\nreviewed: \
         2026-09-20\n---\n\n# Climb\n\nClimbing costs stamina.\n",
    );
    let (_, document) = library(&scene, false);
    let climb = row(&document, "MEC-CLIMB");
    assert_eq!(climb["last_change"], Value::Null, "{climb}");
    assert_eq!(climb["decisions"][0]["name"], "DEC-0050");
    assert_eq!(climb["decisions"][0]["newer"], Value::Null);
    assert_eq!(climb["behind"], Value::Null);
    assert_eq!(document["counts"]["behind"], 0);
}

// ------------------------------------------------------------------ DL5

/// DL5: the scene's files with no `.git`: no last change, `newer` null,
/// stamina's `behind` null, sprint's false, the note once, exit 0.
#[test]
fn dl5_with_no_git_nothing_reads_behind() {
    let scene = Scene::new("dl5");
    let bare = scene.dir("bare");
    scene.copy_without_git(&bare);
    let home = scene.dir("bare-home");
    let (run, document) = library_in(&scene, &bare, &home, false);
    let note = "library: last change not read: not a repository";
    let notes: Vec<&str> = run
        .stderr
        .lines()
        .filter_map(|line| line.strip_prefix("note: "))
        .collect();
    assert_eq!(notes, [note], "{}", run.show());
    assert_eq!(document["notes"], json!([note]));
    for row in rows(&document) {
        let name = row["name"].as_str().unwrap();
        assert_eq!(row["last_change"], Value::Null, "{name}");
        for decision in row["decisions"].as_array().expect("decisions") {
            assert_eq!(decision["last_change"], Value::Null, "{name}");
            assert_eq!(decision["newer"], Value::Null, "{name}");
        }
        let want = match name {
            "MEC-STAMINA" => Value::Null,
            _ => json!(false),
        };
        assert_eq!(row["behind"], want, "{name}");
    }
    assert_eq!(
        row(&document, "MEC-SPRINT")["decisions"][0]["group"],
        "rejected"
    );
    let text = spec_at(&scene, &bare, &home, &["library"], &[]);
    text.code(0);
    assert_eq!(text.stdout, text_of(&document));
    assert_eq!(
        text.stderr
            .lines()
            .filter(|line| line.starts_with("note: "))
            .count(),
        1,
        "{}",
        text.show()
    );
}

// ------------------------------------------------------------------ DL6

/// `spec show MEC-STAMINA --json`'s node.
fn shown_stamina(scene: &Scene, cwd: &Path, home: &Path) -> Value {
    let run = spec_at(scene, cwd, home, &["show", "MEC-STAMINA", "--json"], &[]);
    run.code(0);
    run.json()["nodes"][0].clone()
}

/// DL6: on spec-a and on the scene, stamina's `norm_hash` and `rev` are
/// equal; the scene checks with no `unknown-key` for `tags`. M: `tags`
/// counted in `norm_input`; `tags` out of `TYPED_KEYS`.
#[test]
fn dl6_tags_are_typed_and_never_counted() {
    let scene = Scene::new("dl6");
    let plain = scene.dir("spec-a");
    common::copy_dir(&common::fixture("spec-a"), &plain);
    let plain_home = scene.dir("spec-a-home");
    let before = shown_stamina(&scene, &plain, &plain_home);
    let after = shown_stamina(&scene, &scene.root, &scene.home);
    assert!(before["norm_hash"].is_string(), "premise: hashed: {before}");
    assert_eq!(after["norm_hash"], before["norm_hash"], "M: tags counted");
    assert_eq!(after["rev"], before["rev"]);
    let run = check(&scene);
    let unknown: Vec<Value> = findings(&run, "unknown-key")
        .into_iter()
        .filter(|finding| finding["subject"] == "tags")
        .collect();
    assert_eq!(unknown, Vec::<Value>::new(), "M: tags untyped");
    assert_eq!(findings(&run, "tag-invalid"), Vec::<Value>::new());
}

/// DL6: a closed canon contract that does not list `tags` reads it as any
/// other key: `key-extra`, subject `tags`, at stamina's `tags:` line.
#[test]
fn dl6_a_closed_contract_reads_tags_as_a_key() {
    let scene = Scene::new("dl6-closed");
    scene.append(
        "specengine.toml",
        "\n[classes]\ncanon = { required = [\"class\"], optional = [\"id\", \"kind\", \"tier\", \
         \"title\", \"parent\", \"status\", \"owner\", \"reviewed\", \"links\", \"aliases\", \
         \"to\", \"severity\", \"working_answer\", \"refs\", \"raised_by\"], closed = true }\n",
    );
    let run = check(&scene);
    let extra: Vec<Value> = findings(&run, "key-extra")
        .into_iter()
        .filter(|finding| finding["subject"] == "tags")
        .collect();
    assert_eq!(extra.len(), 1, "{}", run.show());
    assert_eq!(extra[0]["path"], STAMINA);
    assert_eq!(extra[0]["line"], TAGS_LINE_NO);
}

/// DL6: `tags: stamina` (a bare string): `frontmatter-type` subject `tags`,
/// the row's `tags` `[]`, stamina's `norm_hash` null.
#[test]
fn dl6_a_mistyped_tags_key_nulls_the_hash_and_serves_no_tag() {
    let scene = Scene::new("dl6-mistyped");
    scene.edit(STAMINA, TAGS_LINE, "tags: stamina\n");
    let run = check(&scene);
    let mistyped: Vec<Value> = findings(&run, "frontmatter-type")
        .into_iter()
        .filter(|finding| finding["subject"] == "tags")
        .collect();
    assert_eq!(mistyped.len(), 1, "{}", run.show());
    assert_eq!(mistyped[0]["path"], STAMINA);
    assert_eq!(mistyped[0]["line"], TAGS_LINE_NO);
    let (_, document) = library(&scene, false);
    assert_eq!(row(&document, "MEC-STAMINA")["tags"], json!([]));
    assert_eq!(
        shown_stamina(&scene, &scene.root, &scene.home)["norm_hash"],
        Value::Null
    );
    // A mapping and a list holding a non-string entry: the List rule too.
    for written in ["tags: {a: b}\n", "tags: [stamina, 3]\n"] {
        scene.edit(
            STAMINA,
            &scene
                .read(STAMINA)
                .lines()
                .find(|line| line.starts_with("tags:"))
                .map(|line| format!("{line}\n"))
                .expect("a tags line"),
            written,
        );
        let (_, document) = library(&scene, false);
        assert_eq!(
            row(&document, "MEC-STAMINA")["tags"],
            json!([]),
            "{written}"
        );
    }
}

// ------------------------------------------------------------------ DL7

/// DL7: 38 entries, six findings with "Data"'s messages, 32 kept, the exit
/// unchanged. M: the repeat rule dropped.
#[test]
fn dl7_tag_caps() {
    let scene = Scene::new("dl7");
    let base_exit = check(&scene).code;
    let long = "a".repeat(65);
    let valid: Vec<String> = (1..=32).map(|n| format!("t{n:02}")).collect();
    let mut entries = vec![
        "\"\"".to_owned(),
        "\"x \"".to_owned(),
        "\"a\\x07b\"".to_owned(),
        format!("\"{long}\""),
        "\"ok\"".to_owned(),
        "\"ok\"".to_owned(),
    ];
    entries.extend(valid.iter().map(|tag| format!("\"{tag}\"")));
    assert_eq!(entries.len(), 38);
    scene.edit(
        STAMINA,
        TAGS_LINE,
        &format!("tags: [{}]\n", entries.join(", ")),
    );
    let run = check(&scene);
    assert_eq!(
        run.code,
        base_exit,
        "a warning changes no exit: {}",
        run.show()
    );
    let found = findings(&run, "tag-invalid");
    let mut messages: Vec<String> = found
        .iter()
        .map(|finding| {
            assert_eq!(finding["severity"], "warning", "{finding}");
            assert_eq!(finding["path"], STAMINA, "{finding}");
            assert_eq!(finding["line"], TAGS_LINE_NO, "{finding}");
            assert_eq!(finding["subject"], "tags", "{finding}");
            finding["message"].as_str().expect("a message").to_owned()
        })
        .collect();
    messages.sort();
    let mut want = vec![
        "tags: entry 1 is empty or has a space at an end".to_owned(),
        "tags: entry 2 is empty or has a space at an end".to_owned(),
        "tags: entry 3 holds a control character".to_owned(),
        "tags: entry 4 is 65 bytes; a tag has at most 64".to_owned(),
        "tags: entry 6 repeats entry 5".to_owned(),
        "tags: entry 38 is past the 32nd kept tag; a document keeps 32".to_owned(),
    ];
    want.sort();
    assert_eq!(messages, want, "{}", run.show());
    let (_, document) = library(&scene, false);
    let mut kept = vec![json!("ok")];
    kept.extend(valid[..31].iter().map(|tag| json!(tag)));
    assert_eq!(row(&document, "MEC-STAMINA")["tags"], Value::Array(kept));
    assert_eq!(
        shown_stamina(&scene, &scene.root, &scene.home)["norm_hash"],
        shown_stamina_of_scene()["norm_hash"],
        "a bad entry nulls no hash"
    );
}

/// Stamina's node on a fresh scene.
fn shown_stamina_of_scene() -> Value {
    let scene = Scene::new("dl7-fresh");
    shown_stamina(&scene, &scene.root, &scene.home)
}

/// "Data": a tag is any script; the row's tags are compared byte for byte
/// (no case folding: `Tide` and `tide` both kept).
#[test]
fn tags_in_any_script_are_served_as_written() {
    let scene = Scene::new("tags-script");
    let cyrillic = "\u{0432}\u{044b}\u{043d}\u{043e}\u{0441}\u{043b}\u{0438}\
                    \u{0432}\u{043e}\u{0441}\u{0442}\u{044c}";
    scene.edit(
        STAMINA,
        TAGS_LINE,
        &format!("tags: [Tide, tide, {cyrillic}]\n"),
    );
    let run = check(&scene);
    assert_eq!(findings(&run, "tag-invalid"), Vec::<Value>::new());
    let (_, document) = library(&scene, false);
    assert_eq!(
        row(&document, "MEC-STAMINA")["tags"],
        json!(["Tide", "tide", cyrillic])
    );
}

// ------------------------------------------------------------------ DL8

/// The names of `spec tree args --json`'s nodes.
fn tree_names(scene: &Scene, args: &[&str]) -> BTreeSet<String> {
    let mut all = vec!["tree"];
    all.extend_from_slice(args);
    all.push("--json");
    let run = spec(scene, &all);
    run.code(0);
    run.json()["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["id"].as_str().expect("an id").to_owned())
        .collect()
}

/// The names of the rows holding `module`.
fn holding(document: &Value, module: &str) -> BTreeSet<String> {
    rows(document)
        .iter()
        .filter(|row| {
            row["modules"]
                .as_array()
                .expect("modules")
                .iter()
                .any(|held| held == module)
        })
        .map(|row| row["name"].as_str().unwrap().to_owned())
        .collect()
}

/// DL8: `movement` holds exactly the documents of `spec tree DOM-MOVEMENT`;
/// `records` holds the rows of its kinds ("Data": a direction holds a row
/// whose `kind` is in its `kinds`); a root naming nothing holds no row and
/// is named by one note. M: the root's own document left out.
#[test]
fn dl8_modules() {
    let scene = Scene::new("dl8");
    scene.append(
        "specengine.toml",
        "\n[[module]]\nname = \"nowhere\"\nroot = \"MEC-NOTHING\"\n",
    );
    let (run, document) = library(&scene, false);
    let all: BTreeSet<String> = names(&document).into_iter().collect();
    let tree: BTreeSet<String> = tree_names(&scene, &["DOM-MOVEMENT"])
        .intersection(&all)
        .cloned()
        .collect();
    let movement = holding(&document, "movement");
    assert_eq!(movement, tree, "the tree's documents");
    assert_eq!(
        movement,
        ["DOM-MOVEMENT", "MEC-SPRINT", "MEC-STAMINA"]
            .map(str::to_owned)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "M: the root's own document left out"
    );
    let records = holding(&document, "records");
    let of_kinds: BTreeSet<String> = rows(&document)
        .iter()
        .filter(|row| {
            ["question", "requirement"].contains(&row["kind"].as_str().unwrap_or_default())
        })
        .map(|row| row["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(records, of_kinds);
    for name in ["Q-031", "Q-032", "R-12"] {
        assert!(records.contains(name), "{name} in {records:?}");
    }
    // DL8 words the direction's half as `spec tree --kind question --kind
    // requirement`'s documents: on spec-a that tree, rooted under `[paths]
    // spec` (`docs/spec`), reaches no record, so it can only be a part.
    let kind_tree: BTreeSet<String> =
        tree_names(&scene, &["--kind", "question", "--kind", "requirement"])
            .intersection(&all)
            .cloned()
            .collect();
    assert!(
        kind_tree.is_subset(&records),
        "{kind_tree:?} within {records:?}"
    );
    assert!(holding(&document, "nowhere").is_empty());
    let note = "library: module `nowhere`: its root `MEC-NOTHING` names nothing; no row is in it";
    assert_eq!(document["notes"], json!([note]));
    let notes: Vec<&str> = run
        .stderr
        .lines()
        .filter_map(|line| line.strip_prefix("note: "))
        .collect();
    assert_eq!(notes, [note], "{}", run.show());
    for row in rows(&document) {
        let modules: Vec<&str> = row["modules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|module| module.as_str().unwrap())
            .collect();
        let mut in_config_order = modules.clone();
        in_config_order.sort_by_key(|module| {
            ["movement", "records", "nowhere"]
                .iter()
                .position(|m| m == module)
        });
        assert_eq!(modules, in_config_order, "{row}");
    }
}

/// "Rules": a rooted module whose root resolves to an ID section holds the
/// documents whose ancestors hold that section, never the document holding
/// it; the row's `parent` names the section.
#[test]
fn dl8_a_module_rooted_at_a_section() {
    let scene = Scene::new("dl8-section");
    scene.put(
        "docs/spec/movement/walk.md",
        "---\nid: MEC-WALK\nclass: canon\nparent: RULE-MOVE-SPEEDS\nowner: owner\nreviewed: \
         2026-09-20\n---\n\n# Walk\n\nWalking is the slow speed.\n",
    );
    scene.append(
        "specengine.toml",
        "\n[[module]]\nname = \"speeds\"\nroot = \"RULE-MOVE-SPEEDS\"\n",
    );
    scene.commit(C2_AT, "Walk under the speeds\n");
    let (_, document) = library(&scene, false);
    assert_eq!(
        holding(&document, "speeds"),
        BTreeSet::from(["MEC-WALK".to_owned()]),
        "{document}"
    );
    assert!(holding(&document, "movement").contains("MEC-WALK"));
    assert_eq!(row(&document, "MEC-WALK")["parent"], "RULE-MOVE-SPEEDS");
    assert_eq!(
        row(&document, "MEC-WALK")["modules"],
        json!(["movement", "speeds"])
    );
}

/// "Data", **Notes**: a module root written with
/// `@rev` is read at the current files, its `@rev` named by one note in the
/// words `spec show`'s `classify` uses; the root still holds its rows.
#[test]
fn a_module_roots_rev_is_ignored_with_a_note() {
    let scene = Scene::new("dl8-rev");
    scene.append(
        "specengine.toml",
        "\n[[module]]\nname = \"pinned\"\nroot = \"DOM-MOVEMENT@2\"\n",
    );
    let (run, document) = library(&scene, false);
    let note = "library: module `pinned`: its root `DOM-MOVEMENT@2`: `@2` is ignored: `spec \
                library` reads the current files";
    assert_eq!(document["notes"], json!([note]), "{}", run.show());
    let notes: Vec<&str> = run
        .stderr
        .lines()
        .filter_map(|line| line.strip_prefix("note: "))
        .collect();
    assert_eq!(notes, [note], "{}", run.show());
    assert_eq!(holding(&document, "pinned"), holding(&document, "movement"));
    assert!(holding(&document, "pinned").contains("DOM-MOVEMENT"));
}

/// The tree's warning without a line: a parent held
/// by two documents is named once per child, each holder as `path (ID)`,
/// no line (the index holds none), on stderr alone; the read answers.
#[test]
fn a_parent_with_two_holders_is_warned_without_a_line() {
    let scene = Scene::new("parent-twins");
    for twin in ["a", "b"] {
        scene.put(
            &format!("docs/spec/twin-{twin}.md"),
            &format!(
                "---\nid: DOM-TWIN\nclass: canon\nowner: owner\nreviewed: 2026-09-20\n---\n\n# \
                 Twin {twin}\n\nOne of two.\n"
            ),
        );
    }
    scene.put(
        "docs/spec/child.md",
        "---\nid: MEC-CHILD\nclass: canon\nparent: DOM-TWIN\nowner: owner\nreviewed: \
         2026-09-20\n---\n\n# Child\n\nUnder a twin.\n",
    );
    scene.commit(C2_AT, "Twins\n");
    let (run, document) = library(&scene, false);
    let warning = "warning: the parent of `MEC-CHILD` has 2 holders; listed under the first: \
                   docs/spec/twin-a.md (DOM-TWIN), docs/spec/twin-b.md (DOM-TWIN)";
    let warnings: Vec<&str> = run
        .stderr
        .lines()
        .filter(|line| line.contains("the parent of `MEC-CHILD`"))
        .collect();
    assert_eq!(warnings, [warning], "{}", run.show());
    assert!(
        !run.stdout.contains("holders"),
        "a warning goes to stderr alone"
    );
    assert_eq!(document["notes"], json!([]), "a warning is no note");
    assert!(names(&document).contains(&"MEC-CHILD".to_owned()));
}

// ------------------------------------------------------------------ DL9

/// `sqlite3 db statements`; its stdout.
fn sql(db: &Path, statements: &str) -> String {
    let output = Command::new(common::proposal::sqlite3())
        .env_clear()
        .args(["-cmd", ".timeout 5000"])
        .arg(db)
        .arg(statements)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {statements}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 sqlite3 output")
}

/// The scene's index database under `home`.
fn index_db(home: &Path) -> PathBuf {
    common::data_dir(home).join(format!("{SLUG}.db"))
}

/// What a library read and a check serve of stamina's tags after the format
/// step: the tags, no `unknown-key`, the stamp this build's.
fn assert_read_again(scene: &Scene) {
    let (_, document) = library(scene, false);
    assert_eq!(
        row(&document, "MEC-STAMINA")["tags"],
        json!(["stamina", "tuning"]),
        "M: INDEX_FORMAT left at 17: the stored row is served"
    );
    let unknown: Vec<Value> = findings(&check(scene), "unknown-key")
        .into_iter()
        .filter(|finding| finding["subject"] == "tags")
        .collect();
    assert_eq!(unknown, Vec::<Value>::new());
    assert_eq!(
        sql(
            &index_db(&scene.home),
            "SELECT value FROM index_meta WHERE key = 'format';"
        ),
        format!("{}\n", specengine_store::INDEX_FORMAT),
        "restamped"
    );
}

/// DL9: an index of this build, made what a format-17 build leaves over
/// the same tree (stamina's node without `fields.tags`, an `unknown-key`
/// row for it, the stamp 17), with no file touched: the next read serves
/// the tags. M: `INDEX_FORMAT` left at 17 (the stamp matches, nothing is
/// read again, the row stays).
#[test]
fn dl9_a_format_17_index_is_read_again() {
    // 18 at doc-library; 19 since docs/features/department-practices.md (the
    // `disciplines` table); 20 since docs/features/specengine-guide.md (the
    // typed `summarises`); 21 since docs/features/release-tail.md (rule (7)'s
    // citation tail): a format-17 index is still read again.
    assert_eq!(specengine_store::INDEX_FORMAT, 21);
    let scene = Scene::new("dl9");
    let run = spec(&scene, &["index"]);
    run.code(0);
    let db = index_db(&scene.home);
    let node = "SELECT json_extract(node, '$.fields.tags') FROM nodes WHERE ord = 0 AND file_id = \
                (SELECT file_id FROM files WHERE path = 'docs/spec/movement/stamina.md');";
    assert_eq!(
        sql(&db, node),
        "[\"stamina\",\"tuning\"]\n",
        "premise: this build stores the tags in `fields`"
    );
    let message = "front-matter key `tags` is not a typed key; kept in `extra`";
    sql(
        &db,
        &format!(
            "UPDATE nodes SET node = json_remove(node, '$.fields.tags') WHERE ord = 0 \
             AND file_id = (SELECT file_id FROM files WHERE path = '{STAMINA}'); \
             INSERT INTO diagnostics (file_id, ord, code, diagnostic) SELECT file_id, 99, \
             'unknown-key', '{{\"code\":\"unknown-key\",\"severity\":\"warning\",\
             \"line\":{TAGS_LINE_NO},\"message\":\"{message}\"}}' FROM files \
             WHERE path = '{STAMINA}'; \
             UPDATE index_meta SET value = '17' WHERE key = 'format';"
        ),
    );
    assert_eq!(
        sql(&db, node),
        "\n",
        "premise: the tags are gone from the row"
    );
    assert_eq!(
        sql(&db, "SELECT value FROM index_meta WHERE key = 'format';"),
        "17\n"
    );
    assert_read_again(&scene);
    assert_eq!(
        sql(&db, node),
        "[\"stamina\",\"tuning\"]\n",
        "the row is read again"
    );
}

/// DL9, the literal form: the base commit's `spec index` writes a format-17
/// index over the scene; this build's read serves the tags. Needs the base
/// build: `SPECENGINE_BASE_SPEC=<a spec built at the base commit> cargo
/// nextest run -p specengine-cli --test library --run-ignored only dl9_`.
#[test]
#[ignore = "needs SPECENGINE_BASE_SPEC, a `spec` built at the base commit"]
fn dl9_the_base_builds_index_is_read_again() {
    let base = std::env::var_os("SPECENGINE_BASE_SPEC")
        .expect("SPECENGINE_BASE_SPEC names the base commit's `spec`");
    let scene = Scene::new("dl9-base");
    let vars = scene.vars(&[]);
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    let run = program_with(&base, &scene.root, &["index"], &env);
    run.code(0);
    assert_eq!(
        sql(
            &index_db(&scene.home),
            "SELECT value FROM index_meta WHERE key = 'format';"
        ),
        "17\n",
        "premise: the base build writes format 17"
    );
    assert_read_again(&scene);
}

// ------------------------------------------------------------------ DL16

/// DL16: the scene built at two roots under two `HOME`s, its files created
/// in path order and in the reverse (the second root indexed after half of
/// them, so its index rows are not in path order): the text and the JSON,
/// plain and `--archive`, byte-equal. M: rows in index row order.
#[test]
fn dl16_one_scene_gives_one_answer_at_any_root_and_home() {
    let first = Scene::new("dl16-a");
    let mut indexed = false;
    let second = Scene::build("dl16-b", true, |root, home, n| {
        if n == 9 {
            common::index(home, root);
            indexed = true;
        }
    });
    assert!(indexed, "premise: the second root was indexed half-way");
    assert_eq!(first.c1, second.c1, "premise: one commit at both roots");
    for args in [
        &["library"][..],
        &["library", "--json"],
        &["library", "--archive"],
        &["library", "--archive", "--json"],
    ] {
        let a = spec(&first, args);
        let b = spec(&second, args);
        a.code(0);
        b.code(0);
        assert_eq!(a.stdout, b.stdout, "{args:?}");
        assert_eq!(a.stderr, b.stderr, "{args:?}");
    }
}

// ------------------------------------------------------- exits and the cut

/// "Data", **Exits**: another flag, a REF or no project is 2, never 1; a
/// project with no document answers 0.
#[test]
fn exits() {
    let scene = Scene::new("exits");
    for args in [
        &["library", "--kind", "mechanic"][..],
        &["library", "MEC-STAMINA"],
        &["library", "--depth", "1"],
        &["library", "--archive", "--json", "--brief"],
    ] {
        let run = spec(&scene, args);
        assert_eq!(run.code, 2, "{args:?}: {}", run.show());
        assert!(run.stdout.is_empty(), "{args:?}: {}", run.show());
    }
    let nowhere = scene.dir("nowhere");
    let run = spec_at(&scene, &nowhere, &scene.home, &["library"], &[]);
    assert_eq!(run.code, 2, "no project: {}", run.show());
    let empty = scene.dir("empty");
    fs::write(
        empty.join("specengine.toml"),
        "[project]\nname = \"empty\"\nslug = \"empty-shelf\"\n\n[ids]\nDOM = { kind = \
         \"domain\", shape = \"name\" }\n",
    )
    .expect("a config");
    let home = scene.dir("empty-home");
    let (_, document) = library_in(&scene, &empty, &home, false);
    assert_eq!(document["rows"], json!([]));
    assert_eq!(
        document["counts"],
        json!({"documents": 0, "archived": 0, "decisions": 0, "behind": 0, "orphans": 0})
    );
}

/// "Data", **Text** and **JSON**: past `OUTPUT_CAP_CHARS` the text keeps
/// its first rows whole, each with its decision lines, and ends with the
/// spec's line; the CLI's JSON is cut the same way, `truncated` and
/// `omitted` said.
#[test]
fn the_cut_keeps_rows_whole() {
    let scene = Scene::new("cut");
    let many = 400;
    for n in 0..many {
        scene.put(
            &format!("docs/spec/many/m-{n:03}.md"),
            &format!(
                "---\nid: MEC-MANY-{n:03}\nclass: canon\nparent: DOM-MOVEMENT\nowner: owner\n\
                 reviewed: 2026-09-20\ntags: [wick, stair]\n---\n\n# Lamp wick {n:03} {}\n\nSee \
                 MEC-STAMINA.\n",
                "of the long stair below the lamp room "
                    .repeat(2)
                    .trim_end()
            ),
        );
    }
    scene.commit(C2_AT, "Many wicks\n");
    // Every live row's name, in path order.
    let mut order: Vec<(String, String)> = wanted()
        .into_iter()
        .filter(|want| !want.tier3)
        .map(|want| (want.path.to_owned(), want.name.to_owned()))
        .chain((0..many).map(|n| {
            (
                format!("docs/spec/many/m-{n:03}.md"),
                format!("MEC-MANY-{n:03}"),
            )
        }))
        .collect();
    order.sort();
    let order: Vec<String> = order.into_iter().map(|(_, name)| name).collect();
    let total = order.len();
    assert_eq!(total, 13 + many);

    let text = spec(&scene, &["library"]);
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    let last = lines.last().expect("a line");
    let row_lines: Vec<&str> = lines[1..lines.len() - 1]
        .iter()
        .copied()
        .filter(|line| !line.starts_with("    "))
        .collect();
    let shown = row_lines.len();
    assert!(
        shown >= 1 && shown < total,
        "premise: the text is cut ({shown} of {total})"
    );
    assert_eq!(
        *last,
        format!(
            "[truncated: {} of {total} rows not shown; the daemon's library is whole]",
            total - shown
        )
    );
    assert!(
        text.stdout.chars().count() <= specengine_cli::OUTPUT_CAP_CHARS + 200,
        "{} chars",
        text.stdout.chars().count()
    );
    for line in &lines[1..lines.len() - 1] {
        assert!(
            line.starts_with("    decision ") || line.split(" | ").count() >= 14,
            "a whole row or decision line: {line:?}"
        );
    }
    let printed: Vec<&str> = row_lines
        .iter()
        .map(|line| line.split(" | ").next().expect("a name"))
        .collect();
    assert_eq!(printed, order[..shown], "the first rows, in path order");
    // A row printed is printed with every decision line it has.
    if let Some(at) = printed.iter().position(|name| *name == "MEC-STAMINA") {
        let from = lines
            .iter()
            .position(|line| line.starts_with("MEC-STAMINA | "))
            .expect("stamina's line");
        assert!(lines[from + 1].starts_with("    decision DEC-0023 | "));
        assert!(lines[from + 2].starts_with("    decision DEC-0007 | "));
        assert!(at < shown);
    }

    let json = spec(&scene, &["library", "--json"]);
    json.code(0);
    let document = json.json();
    let kept = rows(&document).len();
    assert_eq!(document["truncated"], true);
    assert_eq!(document["omitted"], json!(total - kept));
    assert!(kept >= 1 && kept < total, "{kept} of {total}");
    assert_eq!(
        names(&document),
        order[..kept],
        "the first rows, in path order"
    );
}

/// Nothing under the root is written by a read: the scene's files and git
/// state are as before every read of this file (ADR-0004).
#[test]
fn a_read_writes_no_spec_file() {
    let scene = Scene::new("read-only");
    let files = || {
        let mut all = common::snapshot(&scene.root);
        all.retain(|path, _| path != ".git" && !path.starts_with(".git/"));
        all
    };
    let before = files();
    let _ = library(&scene, false);
    let _ = library(&scene, true);
    let text = spec(&scene, &["library"]);
    text.code(0);
    assert_eq!(files(), before);
    let status = scene.git.git_text(&scene.root, &["status", "--porcelain"]);
    assert_eq!(status, "", "nothing under the root changed");
}

// ------------------------------------------------------------ edge data

/// Edge data: a document whose front-matter never closes, a file that is
/// no UTF-8 and an ID with a look-alike letter answer exit 0 with no panic;
/// the file with no document node is no row ("Data", **Rows**).
#[test]
fn edge_files_answer_without_a_panic() {
    let scene = Scene::new("edge-files");
    scene.put(
        "docs/spec/movement/broken.md",
        "---\nid: MEC-BROKEN\nclass: canon\n\n# Broken\n\nThe front-matter never closes.\n",
    );
    fs::write(
        scene.path("docs/spec/movement/binary.md"),
        [0xff_u8, 0xfe, b'#', b' ', 0xc3, b'\n'],
    )
    .expect("a file that is no UTF-8");
    // `MEC-SPR\u{0406}NT`: a Cyrillic I among Latin letters.
    scene.put(
        "docs/spec/movement/mixed.md",
        "---\nid: MEC-SPR\u{0406}NT\nclass: canon\nparent: DOM-MOVEMENT\n---\n\n# Mixed\n\nA \
         look-alike ID.\n",
    );
    scene.commit(C2_AT, "Three odd files\n");
    for args in [
        &["library"][..],
        &["library", "--json"],
        &["library", "--archive", "--json"],
    ] {
        let run = spec(&scene, args);
        run.code(0);
        assert!(!run.stderr.contains("panicked"), "{}", run.show());
    }
    let (_, document) = library(&scene, false);
    let paths: Vec<&str> = rows(&document)
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect();
    assert!(
        !paths.contains(&"docs/spec/movement/binary.md"),
        "no document node, no row: {paths:?}"
    );
    assert!(paths.contains(&STAMINA));
}

/// Edge data: an empty repository (`git init`, no commit) reads no last
/// change, exit 0, no panic; `behind` null where an accepted decision is,
/// `false` elsewhere.
#[test]
fn an_unborn_head_reads_no_last_change() {
    let scene = Scene::new("unborn");
    let unborn = scene.dir("unborn");
    scene.copy_without_git(&unborn);
    scene.git.init(&unborn);
    let home = scene.dir("unborn-home");
    let (run, document) = library_in(&scene, &unborn, &home, false);
    assert!(!run.stderr.contains("panicked"), "{}", run.show());
    for row in rows(&document) {
        assert_eq!(row["last_change"], Value::Null, "{row}");
    }
    assert_eq!(row(&document, "MEC-STAMINA")["behind"], Value::Null);
    assert_eq!(row(&document, "MEC-SPRINT")["behind"], false);
}
