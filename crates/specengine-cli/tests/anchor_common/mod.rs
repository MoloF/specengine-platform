//! Helpers of docs/features/anchor-states.md's CLI tests (TE-1..TE-4, the
//! named RD-2 and RD-3 tests): `fixtures/anchor-states`, a small kite game
//! crate (`crates/sky`), a binary (`crates/tool`) and RON tuning data, one
//! node per scenario of the table in "Acceptance criteria" TE-1.
//!
//! [`base`] copies the fixture into a scratch repository (slice 3's
//! `LockRepo`: every git process sandboxed, fixed dates), commits it,
//! writes the two `KITE-TRIM` functions whose bodies sit one byte under and
//! at `specengine_code::TRIVIAL_BODY_BYTES` (sized here through
//! `item_levels`, never a literal), commits them, then accepts every node
//! but `KITE-DECOY` through the library's `lock_accept` (one lock commit
//! each, format 2): no digest is written by hand. [`change`] applies a
//! scenario to the worktree, uncommitted. The expected rows are the
//! fixture's `expected.json`, authored from the spec's table.
//!
//! Only the anchor test binaries include this module (`mod
//! anchor_common;`, after `mod common;`, `mod lock_common;` and `mod
//! verify_common;`).

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs;

use serde_json::{Value, json};
use specengine_code::TRIVIAL_BODY_BYTES;

use crate::common::fixture;
use crate::lock_common::{LockRepo, item_code};

/// The fixture.
pub const FIXTURE: &str = "anchor-states";

/// Every node the base accepts, in that order; `KITE-DECOY` never (its
/// marker, on the lift's body under another node, stays `unlocked`).
pub const NODES: [&str; 19] = [
    "KITE-STEADY",
    "KITE-RENAME",
    "KITE-CARRY",
    "KITE-SKETCH",
    "KITE-STRAY",
    "KITE-TRIM",
    "KITE-PAIR",
    "KITE-CLONE",
    "KITE-CONTEST",
    "KITE-MARK",
    "KITE-RIG",
    "KITE-GAUGE",
    "KITE-WIND",
    "KITE-BANNER",
    "KITE-TETHER",
    "KITE-TOOL",
    "KITE-LAG",
    "KITE-FOG",
    "WISP-01",
];

pub const LIB: &str = "crates/sky/src/lib.rs";
pub const RENAME: &str = "crates/sky/src/rename.rs";
pub const CARRY: &str = "crates/sky/src/carry.rs";
pub const HAUL: &str = "crates/sky/src/haul.rs";
pub const SKETCH: &str = "crates/sky/src/skeleton.rs";
pub const STRAY: &str = "crates/sky/src/stray.rs";
pub const SIZED: &str = "crates/sky/src/sized.rs";
pub const TWINS: &str = "crates/sky/src/twins.rs";
pub const CLONES: &str = "crates/sky/src/clones.rs";
pub const CONTEST: &str = "crates/sky/src/contest.rs";
pub const MARKER: &str = "crates/sky/src/marker.rs";
pub const RIG: &str = "crates/sky/src/rig.rs";
pub const GAUGE: &str = "crates/sky/src/gauge.rs";
pub const BANNER: &str = "crates/sky/src/banner.rs";
pub const PENNANT: &str = "crates/sky/src/pennant.rs";
pub const TETHER: &str = "crates/sky/src/tether.rs";
pub const SPOOL: &str = "crates/sky/src/spool.rs";
pub const LAG: &str = "crates/sky/src/lag.rs";
pub const FOG: &str = "crates/sky/src/fog.rs";
pub const BREEZE: &str = "crates/sky/src/breeze.rs";
pub const STEADY: &str = "crates/sky/src/steady.rs";
pub const TOOL_MAIN: &str = "crates/tool/src/main.rs";
pub const TOOL_LIB: &str = "crates/tool/src/lib.rs";
pub const WINDS: &str = "assets/winds.ron";
pub const WISP_MD: &str = "docs/records/WISP-01.md";
pub const KITES_MD: &str = "docs/spec/kites.md";

/// TE-1's scenarios, in the table's order; each a key of
/// `expected.json`'s `scenarios` and of [`change`].
pub const SCENARIOS: [&str; 18] = [
    "rename within the file",
    "move to another file, the old file kept",
    "rename plus a local renamed (skeleton step)",
    "move across files plus an edit",
    "bodies of T-1 and T body_bytes, renamed",
    "twins, distinct quotes, both renamed",
    "twins, equal quotes, both renamed",
    "a contested candidate",
    "a marker moved, the old item stays",
    "a locked impl gaining an unmarked in-file twin",
    "an ordinal shift",
    "a RON path renamed",
    "a //! file renamed",
    "a [path] binding moved",
    "a rename by the manifest only (a lib added)",
    "a lagging marker, renamed",
    "an ID renamed to an alias",
    "a symbol gone from a file with an orphan parse error",
];

// ------------------------------------------------------------- sizing

/// The 1-based line of the one line of `source` containing `needle`.
pub fn line_of(source: &str, needle: &str) -> usize {
    let found: Vec<usize> = source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(at, _)| at + 1)
        .collect();
    assert_eq!(found.len(), 1, "one line holds {needle:?}:\n{source}");
    found[0]
}

/// `body_bytes` of the item of kind `item_kind` whose first line holds
/// `needle`, through `specengine-code`'s `item_levels`
/// (`lock_common::item_code`).
pub fn body_bytes(source: &str, needle: &str, item_kind: &str) -> u32 {
    let line = line_of(source, needle);
    item_code(source.as_bytes(), line, item_kind, &["sig", "body"])
        .fingerprints
        .expect("an item's fingerprints")
        .2
}

/// The item's `(body, skeleton, quote, body_bytes)` as accept records them,
/// through `item_levels`.
pub fn prints(source: &str, needle: &str, item_kind: &str) -> (String, String, String, u32) {
    let line = line_of(source, needle);
    let code = item_code(source.as_bytes(), line, item_kind, &["sig", "body"]);
    let (skeleton, quote, bytes) = code.fingerprints.expect("an item's fingerprints");
    (code.body, skeleton, quote, bytes)
}

/// A `KITE-TRIM` function named `name`, one identifier of its body `pad`
/// bytes longer than the shortest.
fn trim_fn(name: &str, pad: usize) -> String {
    let slack = format!("slack{}", "x".repeat(pad));
    format!(
        "// @implements KITE-TRIM@1\n\
         pub fn {name}(rate: f32, drag: f32, {slack}: f32) -> f32 {{\n    \
         let lift = rate * 0.5 + drag * 0.25;\n    \
         let balance = (lift - drag).abs() * 0.125;\n    \
         let trimmed = lift - balance + {slack};\n    \
         trimmed.max(0.0)\n\
         }}\n"
    )
}

/// The pad that gives [`trim_fn`] a body of exactly `target` bytes.
fn pad_for(target: u32) -> usize {
    let bytes = |pad: usize| {
        body_bytes(
            &trim_fn("trim_probe", pad),
            "pub fn trim_probe(",
            "function_item",
        )
    };
    let shortest = bytes(0);
    assert!(
        shortest < target,
        "the shortest trim body ({shortest}) is under {target}"
    );
    let pad = (target - shortest) as usize;
    assert_eq!(bytes(pad), target, "one pad byte is one body byte");
    pad
}

/// `sized.rs` as the base writes it: its doc lines, `trim_short` with a
/// body one byte under the threshold, `trim_exact` with one at it, each
/// checked through `item_levels`.
pub fn sized_text() -> String {
    let under = TRIVIAL_BODY_BYTES - 1;
    let at = TRIVIAL_BODY_BYTES;
    let mut text = fs::read_to_string(fixture(FIXTURE).join(SIZED)).expect("sized.rs");
    text.push('\n');
    text.push_str(&trim_fn("trim_short", pad_for(under)));
    text.push('\n');
    text.push_str(&trim_fn("trim_exact", pad_for(at)));
    assert_eq!(
        body_bytes(&text, "pub fn trim_short(", "function_item"),
        under
    );
    assert_eq!(body_bytes(&text, "pub fn trim_exact(", "function_item"), at);
    text
}

// --------------------------------------------------------------- base

/// The fixture at `<scratch>/<dir>` committed, `sized.rs`'s functions
/// committed, `before_lock` applied and committed when it changes
/// anything, then every node of [`NODES`] accepted.
pub fn base_with(label: &str, dir: &str, before_lock: impl Fn(&LockRepo)) -> LockRepo {
    let repo = LockRepo::of(label, FIXTURE, dir, &[]);
    repo.write(SIZED, sized_text());
    repo.commit_all("the sized bodies");
    before_lock(&repo);
    if !repo.porcelain().is_empty() {
        repo.commit_all("before the lock");
    }
    for node in NODES {
        repo.accept_ok(node);
    }
    assert!(repo.porcelain().is_empty(), "the base is committed");
    repo
}

/// [`base_with`] at `<scratch>/repo`, nothing before the lock.
pub fn base(label: &str) -> LockRepo {
    base_with(label, "repo", |_| {})
}

// ------------------------------------------------------------ changes

/// The text of `relative` from the line holding `start` to the first
/// line `}` after it, inclusive, LF-ended.
pub fn block(repo: &LockRepo, relative: &str, start: &str) -> String {
    let text = repo.read(relative);
    assert_eq!(text.matches(start).count(), 1, "{relative}: {start:?}");
    let from = text.find(start).expect("the start");
    let length = text[from..].find("\n}\n").expect("the block's end") + 3;
    text[from..from + length].to_owned()
}

/// `block` cut from `from` and appended to `to` after a blank line.
fn move_block(repo: &LockRepo, from: &str, start: &str, to: &str, edit: impl Fn(&str) -> String) {
    let cut = block(repo, from, start);
    repo.replace(from, &cut, "");
    repo.append(to, &format!("\n{}", edit(&cut)));
}

/// Every `from` of `relative` replaced by `to` (at least one).
fn replace_all(repo: &LockRepo, relative: &str, from: &str, to: &str) {
    let text = repo.read(relative);
    assert!(text.contains(from), "{relative}: no {from:?}");
    repo.write(relative, text.replace(from, to));
}

/// The orphan parse error the last scenario appends: a stray closing
/// brace at file level, outside any item.
pub const ORPHAN: &str = "\n}\n";

/// Applies TE-1's scenario `name` to the worktree, uncommitted.
pub fn change(repo: &LockRepo, name: &str) {
    match name {
        "rename within the file" => {
            repo.replace(RENAME, "pub fn lift_force(", "pub fn lift_power(");
        }
        "move to another file, the old file kept" => {
            move_block(
                repo,
                CARRY,
                "// @implements KITE-CARRY@1",
                HAUL,
                str::to_owned,
            );
        }
        "rename plus a local renamed (skeleton step)" => {
            repo.replace(SKETCH, "pub fn tail_angle(", "pub fn tail_pitch(");
            replace_all(repo, SKETCH, "spread", "fan");
        }
        "move across files plus an edit" => {
            move_block(repo, STRAY, "// @implements KITE-STRAY@1", HAUL, |text| {
                text.replace("wave", "ripple")
            });
        }
        "bodies of T-1 and T body_bytes, renamed" => {
            repo.replace(SIZED, "pub fn trim_short(", "pub fn trim_brief(");
            repo.replace(SIZED, "pub fn trim_exact(", "pub fn trim_level(");
        }
        "twins, distinct quotes, both renamed" => {
            repo.replace(TWINS, "pub fn pull_north(", "pub fn tug_north(");
            repo.replace(TWINS, "pub fn pull_south(", "pub fn tug_south(");
        }
        "twins, equal quotes, both renamed" => {
            repo.replace(CLONES, "pub fn gust_left(", "pub fn gust_west(");
            repo.replace(CLONES, "pub fn gust_right(", "pub fn gust_east(");
        }
        "a contested candidate" => {
            let high = block(
                repo,
                CONTEST,
                "// @implements KITE-CONTEST@1\npub fn reel_high(",
            );
            repo.replace(CONTEST, &format!("{high}\n"), "");
            repo.replace(CONTEST, "pub fn reel_low(", "pub fn reel_mid(");
        }
        "a marker moved, the old item stays" => {
            repo.replace(
                MARKER,
                "// @implements KITE-MARK@1\npub fn spin_rate(",
                "pub fn spin_rate(",
            );
            repo.replace(
                MARKER,
                "pub fn spin_decay(",
                "// @implements KITE-MARK@1\npub fn spin_decay(",
            );
        }
        "a locked impl gaining an unmarked in-file twin" => {
            repo.append(
                RIG,
                "\nimpl Rig {\n    pub fn spare(&self) -> u32 {\n        self.lines + 1\n    }\n}\n",
            );
        }
        "an ordinal shift" => {
            repo.replace(
                GAUGE,
                "// @implements KITE-GAUGE@1\nimpl Gauge {\n    pub fn low(",
                "impl Gauge {\n    pub fn zero(&self) -> f32 {\n        0.0\n    }\n}\n\n\
                 // @implements KITE-GAUGE@1\nimpl Gauge {\n    pub fn low(",
            );
        }
        "a RON path renamed" => {
            repo.replace(WINDS, "gust_cap:", "gust_limit:");
        }
        "a //! file renamed" => {
            fs::rename(repo.root.join(BANNER), repo.root.join(PENNANT)).expect("renamed");
            repo.replace(LIB, "pub mod banner;\n", "");
            repo.replace(
                LIB,
                "pub mod marker;\n",
                "pub mod marker;\npub mod pennant;\n",
            );
        }
        "a [path] binding moved" => {
            move_block(
                repo,
                TETHER,
                "// @implements KITE-TETHER@1 [path]",
                SPOOL,
                str::to_owned,
            );
        }
        "a rename by the manifest only (a lib added)" => {
            repo.write(
                TOOL_LIB,
                "//! The planner's library: its arrival renames the binary's items.\n",
            );
        }
        "a lagging marker, renamed" => {
            // Accept locks no lagging marker: the node's revision is raised
            // after the lock, its `@1` marker left behind.
            repo.replace(KITES_MD, "{#KITE-LAG rev=1}", "{#KITE-LAG rev=2}");
            repo.replace(LAG, "pub fn reel_speed(", "pub fn reel_pace(");
        }
        "an ID renamed to an alias" => {
            repo.replace(
                WISP_MD,
                "id: WISP-01\n",
                "id: WISP-02\naliases: [WISP-01]\n",
            );
        }
        "a symbol gone from a file with an orphan parse error" => {
            repo.replace(FOG, "pub fn fog_density(", "pub fn fog_thickness(");
            repo.append(FOG, ORPHAN);
        }
        other => panic!("no scenario {other:?}"),
    }
}

// ----------------------------------------------------------- expected

/// The fixture's `expected.json`.
pub fn expected() -> Value {
    let path = fixture(FIXTURE).join("expected.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("expected.json"))
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The scenario `name` of `expected.json`.
pub fn scenario(name: &str) -> Value {
    let document = expected();
    let found: Vec<&Value> = document["scenarios"]
        .as_array()
        .expect("scenarios")
        .iter()
        .filter(|scenario| scenario["name"] == name)
        .collect();
    assert_eq!(found.len(), 1, "one scenario {name:?} in expected.json");
    found[0].clone()
}

/// The expected rows after the scenarios `names` applied together: the
/// baseline's, each scenario's nodes' rows replaced by its own.
pub fn expected_rows(names: &[&str]) -> Vec<Value> {
    let document = expected();
    let mut replaced: BTreeSet<String> = BTreeSet::new();
    let mut added: Vec<Value> = Vec::new();
    for name in names {
        let scenario = scenario(name);
        for node in scenario["nodes"].as_array().expect("nodes") {
            let node = node.as_str().expect("a node").to_owned();
            assert!(replaced.insert(node.clone()), "{node} changed twice");
        }
        added.extend(scenario["rows"].as_array().expect("rows").iter().cloned());
    }
    let mut rows: Vec<Value> = document["baseline"]
        .as_array()
        .expect("baseline")
        .iter()
        .filter(|row| !replaced.contains(row["node"].as_str().expect("node")))
        .cloned()
        .collect();
    rows.extend(added);
    rows
}

/// The six key fields of a row, as one string (for maps and messages).
pub fn key_of(row: &Value) -> String {
    ["node", "relation", "file", "kind", "symbol", "item_kind"]
        .iter()
        .map(|field| match &row[*field] {
            Value::Null => "-".to_owned(),
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The six key fields of a row, as an object.
pub fn key_json(row: &Value) -> Value {
    json!({
        "node": row["node"],
        "relation": row["relation"],
        "file": row["file"],
        "kind": row["kind"],
        "symbol": row["symbol"],
        "item_kind": row["item_kind"],
    })
}

/// Of `row`, the fields an expected row names (its key, state, category,
/// detail, class and anchor; `levels` and `compared` when given).
pub fn view_as(row: &Value, expected: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for (field, _) in expected.as_object().expect("an expected row") {
        out.insert(field.clone(), row[field.as_str()].clone());
    }
    Value::Object(out)
}

/// The anchor of a row with an entry and a marker binding at one key.
pub fn exact() -> Value {
    json!({"state": "exact", "from": null, "candidates": null})
}

/// The anchor of an entry row with no binding at its key, not followed.
pub fn lost() -> Value {
    json!({"state": "lost", "from": null, "candidates": null})
}

/// An item's place (`from`, a candidate): file, kind, symbol, item kind.
pub fn place(file: &str, kind: &str, symbol: &str, item_kind: Option<&str>) -> Value {
    json!({"file": file, "kind": kind, "symbol": symbol, "item_kind": item_kind})
}

/// The anchor of a merged row, moved from `from`.
pub fn moved(from: Value) -> Value {
    json!({"state": "moved", "from": from, "candidates": null})
}

/// The anchor of an entry row with several candidates.
pub fn ambiguous(candidates: Vec<Value>) -> Value {
    json!({"state": "ambiguous", "from": null, "candidates": candidates})
}

/// The text a place prints after `moved from ` or `candidate `:
/// `<file> | <kind> <symbol>[ <item_kind>]`.
pub fn place_text(place: &Value) -> String {
    let mut text = format!(
        "{} | {} {}",
        place["file"].as_str().expect("file"),
        place["kind"].as_str().expect("kind"),
        place["symbol"].as_str().expect("symbol")
    );
    if let Some(item_kind) = place["item_kind"].as_str() {
        text.push(' ');
        text.push_str(item_kind);
    }
    text
}
