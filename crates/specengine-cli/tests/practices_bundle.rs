//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! the bundle's layer 10 through the `spec` binary over the scene
//! (`tests/practices_scene`) and over scratch copies of `fixtures/spec-a`
//! and `fixtures/spec-b`. The fixtures are only read.
//!
//! - DP5: `spec bundle SCN-ARRIVAL --discipline art --json --budget 10000`:
//!   `layers.practices` holds art.md's three rules then shared.md's one, by
//!   (path, ord), form `text`, `via` null; none of dev.md's, the superseded
//!   decision's or ghost.md's; `## Practices` is the last heading, after
//!   every nonempty layer of 2-9 (Ancestors and Neighbours here), its items
//!   each `spec show`'s header without its suffixes, no `via`, then its
//!   text. M: Tier 3 practices admitted; layer 10 printed before layer 4.
//! - DP6 (this file's half): with no `--discipline` no layer 10 is filled
//!   or printed on the scene, whatever the target and budget; on spec-a
//!   and spec-b, where no document is a practice, `--discipline art` leaves
//!   the text and the JSON byte-equal. M: layer 10 filled without
//!   `--discipline`. The base build's comparison is `practices_base.rs`.
//! - DP7: at the budget just below the one placing two rules, the rules of
//!   a practice of 25 long rules: layers 1-9 equal the bundle without
//!   `--discipline` at that budget (which places everything), one rule is
//!   placed, the next 20 are tail lines of layer `practices`, `more` 4. M:
//!   layer 10 fitted before layers 2-9.
//! - DP8: a rule linked from the target by `constrains` stands in
//!   Neighbours, `via constrains out`, and not in layer 10.
//! - DP11, the bundle's cap: a practice of 150 rules gives 128 candidates
//!   (the first by position), the other 22 in `more`. M: no
//!   `PRACTICE_RULES_MAX`.
//! - "The bundle", usage: `--discipline Art` exits 2 naming it; 17
//!   disciplines exit 2; 16 answer.
//!
//! Every expected value is worked out from the spec, the scene's files and
//! `spec show` of the same nodes (not the read under test).

#![cfg(unix)]

mod common;
mod practices_scene;

use std::ffi::OsStr;
use std::path::Path;

use common::bundle::{LAYERS, names, split_text, tail_names, via};
use common::{Run, spec_with};
use practices_scene::{
    ART, ART_TEXT, LOD, NAMING, NORMALS, REALISM, SHARED, SHARED_TEXT, Scene, line_of,
    practice_text, prose,
};
use serde_json::{Value, json};

#[allow(unused_imports)]
use common::git;

/// The headings of the layers, by key ("Layers").
const HEADINGS: [(&str, &str); 10] = [
    ("targets", "Targets"),
    ("open_questions", "Open questions"),
    ("ancestors", "Ancestors"),
    ("criteria", "Criteria"),
    ("bindings", "Bindings"),
    ("decisions", "Decisions"),
    ("neighbours", "Neighbours"),
    ("terms", "Terms"),
    ("tests", "Tests"),
    ("practices", "Practices"),
];

// ------------------------------------------------------------- the runs

fn spec_at(scene: &Scene, cwd: &Path, home: &Path, args: &[&str]) -> Run {
    let vars = scene.vars_with(home);
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

fn spec(scene: &Scene, args: &[&str]) -> Run {
    spec_at(scene, &scene.root, &scene.home, args)
}

/// `spec bundle args` in `cwd`: the text run (exit 0).
fn bundle_text_at(scene: &Scene, cwd: &Path, home: &Path, args: &[&str]) -> Run {
    let mut all = vec!["bundle"];
    all.extend_from_slice(args);
    let run = spec_at(scene, cwd, home, &all);
    run.code(0);
    run
}

/// `spec --json bundle args` in `cwd`: exit 0, the document and its raw
/// line.
fn bundle_doc_at(scene: &Scene, cwd: &Path, home: &Path, args: &[&str]) -> (Value, String) {
    let mut all = vec!["--json", "bundle"];
    all.extend_from_slice(args);
    let run = spec_at(scene, cwd, home, &all);
    run.code(0);
    (run.json(), run.stdout)
}

fn bundle_doc(scene: &Scene, args: &[&str]) -> Value {
    bundle_doc_at(scene, &scene.root, &scene.home, args).0
}

/// `spec --json bundle args`, any exit: the run.
fn bundle_any(scene: &Scene, args: &[&str]) -> Run {
    let mut all = vec!["--json", "bundle"];
    all.extend_from_slice(args);
    spec(scene, &all)
}

/// The layer headings of a body (`## <a layer's heading>` and `## Not
/// included`), in order; an item's own text may hold `## ` lines too.
fn headings(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| line.strip_prefix("## "))
        .filter(|heading| {
            *heading == "Not included" || HEADINGS.iter().any(|(_, name)| name == heading)
        })
        .map(str::to_owned)
        .collect()
}

/// The headings a body must print: each nonempty layer's, in print order,
/// then `Not included` when the tail or `more` holds anything.
fn expected_headings(document: &Value) -> Vec<String> {
    let mut want: Vec<String> = HEADINGS
        .iter()
        .filter(|(key, _)| {
            document["layers"][key]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        })
        .map(|(_, heading)| (*heading).to_owned())
        .collect();
    let more = document["more"].as_u64().unwrap_or(0);
    if !tail_names(document).is_empty() || more > 0 {
        want.push("Not included".to_owned());
    }
    want
}

/// The section of `body` under `## <heading>`, the heading line included,
/// up to the next layer heading (an item's own `## ` lines stay in it).
fn section(body: &str, heading: &str) -> String {
    let start = body
        .find(&format!("\n## {heading}\n"))
        .map(|at| at + 1)
        .unwrap_or_else(|| panic!("no ## {heading}: {body}"));
    let rest = &body[start + 3..];
    let end = HEADINGS
        .iter()
        .map(|(_, name)| *name)
        .chain(["Not included"])
        .filter_map(|name| rest.find(&format!("\n## {name}\n")))
        .min()
        .map_or(body.len(), |at| start + 3 + at + 1);
    body[start..end].to_owned()
}

/// `spec show <id>`'s text after its header line.
fn shown_text(scene: &Scene, id: &str) -> String {
    let run = spec(scene, &["show", id]);
    run.code(0);
    let (_, text) = run.stdout.split_once('\n').expect("a header line");
    let mut text = text.to_owned();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// A layer 10 item as the body prints it: the header (name, kind, title,
/// `path:line`, no `via`), then the text.
fn printed_item(scene: &Scene, id: &str, title: &str, path: &str, line: u64) -> String {
    format!(
        "{id} | practice | {title} | {path}:{line}\n{}",
        shown_text(scene, id)
    )
}

// ------------------------------------------------------------------ DP5

/// DP5: the four rules of art's practices in (path, ord); each item's keys'
/// values; the headings in print order with Practices last after
/// Neighbours (so a layer 10 printed before layer 4 is out of place); the
/// section is the four items. M: Tier 3 practices admitted (`PRA-ART-PHOTO`
/// would lead); layer 10 printed before layer 4.
#[test]
fn dp5_layer_ten_holds_the_rules_by_path_and_position() {
    let scene = Scene::files("dp5", false);
    let args = ["SCN-ARRIVAL", "--discipline", "art", "--budget", "10000"];
    let document = bundle_doc(&scene, &args);
    assert_eq!(
        names(&document, "practices"),
        [REALISM, NORMALS, LOD, NAMING],
        "M: Tier 3 practices admitted, or another discipline's: {document}"
    );
    let rules = [
        (
            REALISM,
            "Realism",
            ART,
            line_of(ART_TEXT, "## Realism {#PRA-ART-REALISM}"),
        ),
        (
            NORMALS,
            "Normal maps",
            ART,
            line_of(ART_TEXT, "## Normal maps {#PRA-ART-NORMALS}"),
        ),
        (
            LOD,
            "Level of detail",
            ART,
            line_of(ART_TEXT, "## Level of detail {#PRA-ART-LOD}"),
        ),
        (
            NAMING,
            "Asset names",
            SHARED,
            line_of(SHARED_TEXT, "## Asset names {#PRA-SHARED-NAMING}"),
        ),
    ];
    for (index, (id, title, path, line)) in rules.iter().enumerate() {
        let item = &document["layers"]["practices"][index];
        assert_eq!(item["name"], *id);
        assert_eq!(item["kind"], "practice", "{item}");
        assert_eq!(item["title"], *title, "{item}");
        assert_eq!(item["path"], *path, "{item}");
        assert_eq!(item["line"], json!(line), "{item}");
        assert_eq!(item["form"], "text", "{item}");
        assert_eq!(item["via"], Value::Null, "{item}");
        assert_eq!(item["working_answer"], Value::Null, "{item}");
        assert_eq!(item["archived"], false, "{item}");
    }
    assert_eq!(document["tail"], json!([]), "everything fits at 10000");
    assert!(
        !document["layers"]["neighbours"]
            .as_array()
            .expect("neighbours")
            .is_empty(),
        "premise: SCN-STORM follows the target, so layer 7 prints: {document}"
    );

    let text = bundle_text_at(&scene, &scene.root, &scene.home, &args);
    let parts = split_text(&text.stdout);
    assert_eq!(parts.body, document["body"].as_str().expect("a body"));
    assert_eq!(
        parts.hash,
        document["bundle_hash"].as_str().expect("a hash")
    );
    let printed = headings(&parts.body);
    assert_eq!(
        printed,
        expected_headings(&document),
        "M: layer 10 printed before layer 4"
    );
    assert_eq!(printed.last().map(String::as_str), Some("Practices"));
    let items: Vec<String> = rules
        .iter()
        .map(|(id, title, path, line)| printed_item(&scene, id, title, path, *line))
        .collect();
    assert_eq!(
        section(&parts.body, "Practices"),
        format!("## Practices\n{}", items.join("\n")),
        "{}",
        parts.body
    );
}

// --------------------------------------------------------------- DP6, here

/// The names `spec map --json` lists, every node of the parse.
fn map_names(scene: &Scene, cwd: &Path, home: &Path) -> Vec<String> {
    let run = spec_at(scene, cwd, home, &["map", "--json"]);
    run.code(0);
    run.json()["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// DP6 on the scene: for every node at 2000 and 10000 tokens, no
/// `--discipline`: `layers.practices` is `[]` and no `## Practices` line,
/// the ten layer keys in print order. M: layer 10 filled without
/// `--discipline`.
#[test]
fn dp6_without_a_discipline_layer_ten_stays_empty() {
    let scene = Scene::files("dp6-scene", false);
    let nodes = map_names(&scene, &scene.root, &scene.home);
    assert!(nodes.iter().any(|name| name == REALISM), "{nodes:?}");
    for name in &nodes {
        for budget in ["2000", "10000"] {
            let (document, raw) = bundle_doc_at(
                &scene,
                &scene.root,
                &scene.home,
                &[name, "--budget", budget],
            );
            assert_eq!(
                document["layers"]["practices"],
                json!([]),
                "{name} at {budget}: M: filled without --discipline"
            );
            let body = document["body"].as_str().expect("a body");
            assert!(
                !body.lines().any(|line| line == "## Practices"),
                "{name} at {budget}: {body}"
            );
            let keys: Vec<usize> = LAYERS
                .iter()
                .map(|layer| {
                    raw.find(&format!("\"{layer}\":["))
                        .unwrap_or_else(|| panic!("no {layer}: {raw}"))
                })
                .collect();
            assert!(
                keys.windows(2).all(|pair| pair[0] < pair[1]),
                "the ten layers in print order: {raw}"
            );
            assert!(
                tail_names(&document)
                    .iter()
                    .all(|tail| !tail.starts_with("PRA-") || name.starts_with("PRA-")),
                "{name}: no rule reaches the tail by discipline: {document}"
            );
        }
    }
}

/// DP6 on spec-a and spec-b, which hold no practice: every node at 2000
/// and 10000 tokens gives the same text (body, `bundle_hash`, totals) and
/// the same JSON with `--discipline art` as without.
#[test]
fn dp6_a_discipline_with_no_practice_changes_nothing() {
    let scene = Scene::files("dp6-fixtures", false);
    for fixture in ["spec-a", "spec-b"] {
        let root = scene.dir(fixture);
        common::copy_dir(&common::fixture(fixture), &root);
        let home = scene.dir(&format!("{fixture}-home"));
        let nodes = map_names(&scene, &root, &home);
        assert!(nodes.len() > 10, "{fixture}: {nodes:?}");
        for name in &nodes {
            for budget in ["2000", "10000"] {
                let plain = [name.as_str(), "--budget", budget];
                let asked = [name.as_str(), "--budget", budget, "--discipline", "art"];
                let mut all = vec!["--json", "bundle"];
                all.extend_from_slice(&plain);
                let first = spec_at(&scene, &root, &home, &all);
                if first.code != 0 {
                    // A REF that names nothing or several holders is the
                    // bundle's own business; the asked run must match it.
                    let mut again = vec!["--json", "bundle"];
                    again.extend_from_slice(&asked);
                    let second = spec_at(&scene, &root, &home, &again);
                    assert_eq!(
                        (second.code, &second.stdout, &second.stderr),
                        (first.code, &first.stdout, &first.stderr),
                        "{fixture} {name} {budget}"
                    );
                    continue;
                }
                let (json_plain, raw_plain) = bundle_doc_at(&scene, &root, &home, &plain);
                let (_, raw_asked) = bundle_doc_at(&scene, &root, &home, &asked);
                assert_eq!(raw_asked, raw_plain, "{fixture} {name} {budget}: JSON");
                assert_eq!(json_plain["layers"]["practices"], json!([]));
                let text_plain = bundle_text_at(&scene, &root, &home, &plain);
                let text_asked = bundle_text_at(&scene, &root, &home, &asked);
                assert_eq!(
                    text_asked.stdout, text_plain.stdout,
                    "{fixture} {name} {budget}: text"
                );
            }
        }
    }
}

// ------------------------------------------------------------------ DP7

/// The long practice of DP7: `PRA-LONG`, `sound`, 25 rules of about 2 400
/// characters each.
fn long_practice(scene: &Scene) -> Vec<String> {
    let rules: Vec<(String, String, String)> = (1..=25)
        .map(|n| {
            (
                format!("PRA-LONG-{n:02}"),
                format!("Long rule {n:02}"),
                prose(&format!("long-{n:02}"), 2_400),
            )
        })
        .collect();
    scene.put(
        "docs/spec/practices/sound/long.md",
        &practice_text("PRA-LONG", "Long rules", &["sound"], &rules),
    );
    rules.into_iter().map(|(id, _, _)| id).collect()
}

/// The smallest budget in `lo..=hi` for which `holds` (monotone), probing
/// with the binary.
fn smallest(lo: u64, hi: u64, mut holds: impl FnMut(u64) -> bool) -> u64 {
    assert!(holds(hi), "premise: {hi} holds");
    let (mut lo, mut hi) = (lo, hi);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if holds(mid) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

/// DP7: the budget `B` just below the one placing two rules: the bundle
/// without `--discipline` at `B` places everything, and with `--discipline
/// sound` layers 1-9 are its own (keys and body), one rule is placed, the
/// next 20 rule IDs are tail lines of layer `practices`, and `more` counts
/// the 4 past them. M: layer 10 fitted before layers 2-9 (a second rule
/// then fits at `B`, pushing an earlier item out).
#[test]
fn dp7_fitting_degrades_rules_to_ids() {
    let scene = Scene::files("dp7", false);
    let rules = long_practice(&scene);
    let target = "SCN-ARRIVAL";
    let complete = |budget: u64| -> bool {
        let run = bundle_any(&scene, &[target, "--budget", &budget.to_string()]);
        run.code == 0 && {
            let document = run.json();
            tail_names(&document).is_empty() && document["more"].as_u64().unwrap_or(0) == 0
        }
    };
    let full = smallest(1, 10_000, complete);
    let placed = |budget: u64| -> usize {
        let run = bundle_any(
            &scene,
            &[
                target,
                "--budget",
                &budget.to_string(),
                "--discipline",
                "sound",
            ],
        );
        if run.code != 0 {
            return 0;
        }
        names(&run.json(), "practices").len()
    };
    let two = smallest(full, full + 10_000, |budget| placed(budget) >= 2);
    let budget = (two - 1).to_string();
    let (plain, _) = bundle_doc_at(
        &scene,
        &scene.root,
        &scene.home,
        &[target, "--budget", &budget],
    );
    assert_eq!(
        plain["tail"],
        json!([]),
        "premise: everything placed at {budget}"
    );
    let (asked, _) = bundle_doc_at(
        &scene,
        &scene.root,
        &scene.home,
        &[target, "--budget", &budget, "--discipline", "sound"],
    );
    for layer in LAYERS.iter().filter(|layer| **layer != "practices") {
        assert_eq!(
            asked["layers"][layer], plain["layers"][layer],
            "{layer} at {budget}: M: layer 10 fitted before layers 2-9"
        );
    }
    assert_eq!(names(&asked, "practices"), [rules[0].clone()], "{asked}");
    let body_plain = plain["body"].as_str().expect("a body");
    let body_asked = asked["body"].as_str().expect("a body");
    assert!(
        body_asked.starts_with(body_plain)
            && body_asked[body_plain.len()..].starts_with("\n## Practices\n"),
        "layers 1-9 byte-equal, then Practices:\n--- plain\n{body_plain}--- asked\n{body_asked}"
    );
    assert_eq!(
        tail_names(&asked),
        rules[1..21].to_vec(),
        "the next 20 are tail lines: {asked}"
    );
    for entry in asked["tail"].as_array().expect("a tail") {
        assert_eq!(entry["layer"], "practices", "{entry}");
    }
    assert_eq!(asked["more"], json!(4), "the 4 past the 20th: {asked}");
    let text = bundle_text_at(
        &scene,
        &scene.root,
        &scene.home,
        &[target, "--budget", &budget, "--discipline", "sound"],
    );
    let body = split_text(&text.stdout).body;
    assert!(body.ends_with("- 4 more\n"), "{body}");
    assert!(
        body.contains(&format!("\n- {} | Long rule 02 | ", rules[1])),
        "{body}"
    );
}

// ------------------------------------------------------------------ DP8

/// DP8: `SCN-ARRIVAL` with `links: constrains: [PRA-ART-LOD]`: the rule
/// stands in Neighbours, `via constrains out`, once in the body; layer 10
/// holds the other three.
#[test]
fn dp8_a_rule_stands_once_in_its_first_layer() {
    let scene = Scene::files("dp8", false);
    scene.edit(
        "docs/spec/scn-arrival.md",
        "reviewed: 2026-11-08\n---\n",
        "reviewed: 2026-11-08\nlinks:\n  constrains: [PRA-ART-LOD]\n---\n",
    );
    let args = ["SCN-ARRIVAL", "--discipline", "art", "--budget", "10000"];
    let document = bundle_doc(&scene, &args);
    assert!(
        names(&document, "neighbours").contains(&LOD.to_owned()),
        "{document}"
    );
    let item = common::bundle::item(&document, "neighbours", LOD);
    assert_eq!(
        via(item),
        [("constrains".to_owned(), "out".to_owned())],
        "{item}"
    );
    assert_eq!(names(&document, "practices"), [REALISM, NORMALS, NAMING]);
    let body = document["body"].as_str().expect("a body");
    assert_eq!(
        body.lines()
            .filter(|line| line.starts_with(&format!("{LOD} | ")))
            .count(),
        1,
        "{body}"
    );
}

// ----------------------------------------------------------------- DP11

/// DP11: a practice of 150 short rules (`sound`): the first 128 by position
/// are its candidates, all placed at 20000 tokens; the 22 others count in
/// `more`, never a tail line. M: no `PRACTICE_RULES_MAX`.
#[test]
fn dp11_a_practice_gives_at_most_128_rules() {
    let scene = Scene::files("dp11-rules", false);
    let rules: Vec<(String, String, String)> = (1..=150)
        .map(|n| {
            (
                format!("PRA-BIG-{n:03}"),
                format!("Rule {n:03}"),
                format!("Wick {n} is trimmed at dusk.\n"),
            )
        })
        .collect();
    scene.put(
        "docs/spec/practices/sound/big.md",
        &practice_text("PRA-BIG", "Big rules", &["sound"], &rules),
    );
    let document = bundle_doc(
        &scene,
        &["SCN-ARRIVAL", "--discipline", "sound", "--budget", "20000"],
    );
    let want: Vec<String> = rules[..128].iter().map(|(id, _, _)| id.clone()).collect();
    assert_eq!(
        names(&document, "practices"),
        want,
        "M: no PRACTICE_RULES_MAX"
    );
    assert!(
        tail_names(&document)
            .iter()
            .all(|name| !name.starts_with("PRA-BIG-")),
        "{document}"
    );
    assert_eq!(document["more"], json!(22), "{document}");
}

// ------------------------------------------------------------- the usage

/// "The bundle": a name off the flows' grammar exits 2 naming it, nothing
/// on stdout; 17 disciplines exit 2; 16 answer (on spec-a, where no flow
/// is declared, so a name no flow lists is not what is judged).
#[test]
fn a_bad_or_seventeenth_discipline_exits_2() {
    let scene = Scene::files("dp-usage", false);
    for args in [
        &["bundle", "SCN-ARRIVAL", "--discipline", "Art"][..],
        &["--json", "bundle", "SCN-ARRIVAL", "--discipline", "Art"][..],
    ] {
        let run = spec(&scene, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
        assert!(
            run.stderr
                .contains("`--discipline`: `Art` is no discipline name"),
            "{}",
            run.show()
        );
    }
    // The cap, on spec-a (no `[card]`: any well-formed name is free, so
    // only the count is judged).
    let root = scene.dir("spec-a");
    common::copy_dir(&common::fixture("spec-a"), &root);
    let home = scene.dir("spec-a-home");
    let names: Vec<String> = (1..=17).map(|n| format!("d{n:02}")).collect();
    let mut args = vec!["bundle", "MEC-STAMINA"];
    for name in &names[..16] {
        args.extend(["--discipline", name.as_str()]);
    }
    spec_at(&scene, &root, &home, &args).code(0);
    args.extend(["--discipline", names[16].as_str()]);
    let run = spec_at(&scene, &root, &home, &args);
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
}

/// The orchestrator's ruling on the developer's iteration 2, item 1: the
/// not-included reserve counts what layers 1-9 leave out, so at every
/// budget around the one that fits layers 1-9 exactly (that budget, and
/// 12 below and above it) the bundle with `--discipline` holds layers 1-9
/// byte-equal to the bundle without: their keys, and the body up to its
/// `## Practices` or `## Not included`. A practice of 128 rules makes the
/// candidates' count cross two powers of ten (2 without, 130 with), so a
/// reserve that counts layer 10 is two characters longer. M: the reserve
/// counting layer 10 (the last item of layers 2-9 drops out at the edge).
#[test]
fn layers_one_to_nine_are_the_same_at_every_budget() {
    let scene = Scene::files("dp7-edge", false);
    let rules: Vec<(String, String, String)> = (1..=128)
        .map(|n| {
            (
                format!("PRA-EDGE-{n:03}"),
                format!("Edge {n:03}"),
                format!("Wick {n} is trimmed.\n"),
            )
        })
        .collect();
    scene.put(
        "docs/spec/practices/sound/edge.md",
        &practice_text("PRA-EDGE", "Edge rules", &["sound"], &rules),
    );
    let target = "SCN-ARRIVAL";
    let complete = |budget: u64| -> bool {
        let run = bundle_any(&scene, &[target, "--budget", &budget.to_string()]);
        run.code == 0 && {
            let document = run.json();
            tail_names(&document).is_empty() && document["more"].as_u64().unwrap_or(0) == 0
        }
    };
    let exact = smallest(1, 10_000, complete);
    let prefix = |body: &str| -> String {
        let end = ["\n## Practices\n", "\n## Not included\n"]
            .iter()
            .filter_map(|heading| body.find(heading))
            .min()
            .unwrap_or(body.len());
        body[..end].to_owned()
    };
    let mut compared = 0;
    for budget in exact.saturating_sub(12)..=exact + 12 {
        let budget = budget.to_string();
        let plain = bundle_any(&scene, &[target, "--budget", &budget]);
        let asked = bundle_any(
            &scene,
            &[target, "--budget", &budget, "--discipline", "sound"],
        );
        assert_eq!(asked.code, plain.code, "{budget}: {}", asked.show());
        if plain.code != 0 {
            continue;
        }
        compared += 1;
        let (plain, asked) = (plain.json(), asked.json());
        for layer in LAYERS.iter().filter(|layer| **layer != "practices") {
            assert_eq!(
                asked["layers"][layer], plain["layers"][layer],
                "{layer} at {budget}: M: the reserve counting layer 10"
            );
        }
        assert_eq!(
            prefix(asked["body"].as_str().expect("a body")),
            prefix(plain["body"].as_str().expect("a body")),
            "layers 1-9 byte-equal at {budget}"
        );
    }
    assert!(
        compared >= 13,
        "the budgets from the exact one up answer: {compared}"
    );
}
