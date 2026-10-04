//! docs/features/spec-cli-bundle.md, the layers: AC-01 (spec-a), AC-02
//! (spec-b), AC-03 (open by links), AC-04 (criteria), AC-05 (kinds
//! renamed, no kind literal), AC-10 (tail prices), AC-12 (REFs and exits),
//! AC-14 (key sets), and the REF rules of "Description" (several holders,
//! one node named twice, a target within another, a Tier 3 target).
//! Scratch copies of `fixtures/spec-a` and `-b`, each run with its own
//! `HOME`; the fixtures are only read.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use common::bundle::{
    ITEM_KEYS, LAYERS, TAIL_KEYS, TOP_KEYS, WORKING_ANSWER_KEYS, all_layers, bundle, bundle_json,
    item, literals, names, split_text, tail_names, via,
};
use common::graph::{keys, spec30};
use common::{FIXTURES, Scratch, fixture, read_text, replace, repository_root, write};
use serde_json::Value;
use specengine_core::IdSchemeToml as _;
use specengine_model::IdScheme;

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

/// The `working_answer` link `spec show <question> --links` places, as
/// `{name, written, path, line, state}` (a bundle's `working_answer`).
fn links_working_answer(home: &Path, root: &Path, question: &str) -> Value {
    let json = spec30(home, root, &["--json", "show", question, "--links"]).json();
    let outgoing = json["nodes"][0]["links"]["outgoing"]
        .as_array()
        .unwrap_or_else(|| panic!("{question}: no outgoing links: {json}"));
    let link = outgoing
        .iter()
        .find(|link| link["type"] == "working_answer")
        .unwrap_or_else(|| panic!("{question}: no working_answer link: {json}"));
    serde_json::json!({
        "name": link["name"], "written": link["written"], "path": link["path"],
        "line": link["line"], "state": link["state"],
    })
}

/// Every layer but the listed ones is `[]`.
fn only_layers(json: &Value, filled: &[(&str, &[&str])]) {
    let wanted: BTreeMap<&str, Vec<String>> = filled
        .iter()
        .map(|(layer, items)| (*layer, items.iter().map(|s| (*s).to_owned()).collect()))
        .collect();
    for (layer, got) in all_layers(json) {
        let want = wanted.get(layer).cloned().unwrap_or_default();
        assert_eq!(got, want, "layer {layer}: {json}");
    }
}

/// Every name of every layer once, nowhere else in the tail.
fn each_once(json: &Value) {
    let mut seen = BTreeSet::new();
    for (layer, items) in all_layers(json) {
        for name in items {
            assert!(
                seen.insert(name.clone()),
                "{name} twice (again in {layer}): {json}"
            );
        }
    }
    for name in tail_names(json) {
        assert!(
            seen.insert(name.clone()),
            "{name} placed and listed: {json}"
        );
    }
}

/// The text bundle's body starts with the title, `## Targets` and exactly
/// `spec show`'s stdout of `reference`, then `next`.
fn target_text_is_show(home: &Path, root: &Path, reference: &str, title: &str, next: &str) {
    let show = spec30(home, root, &["show", reference]);
    show.code(0);
    let run = bundle(home, root, &[reference, "--budget", "10000"]);
    run.code(0);
    let text = split_text(&run.stdout);
    let expected = format!("# Bundle: {title}\n\n## Targets\n{}\n{next}", show.stdout);
    assert!(
        text.body.starts_with(&expected),
        "the target is not `spec show`'s text:\n--- bundle\n{}\n--- show\n{}",
        text.body,
        show.stdout
    );
}

/// AC-01: spec-a MEC-STAMINA at 10 000: the target as `spec show` prints
/// it; ancestors nearest first; DEC-0023 the one decision; A-101, R-12,
/// MEC-SPRINT neighbours once each, MEC-SPRINT by `constrains in` (a link
/// onto a section within the target) and `depends_on out`; the term;
/// every other layer `[]`. M: nested sections' links ignored; no live
/// filter; no de-duplication.
#[test]
fn ac01_spec_a_stamina_gets_its_layers() {
    let scratch = Scratch::new("bundle-ac01");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let json = bundle_json(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    only_layers(
        &json,
        &[
            ("targets", &["MEC-STAMINA"]),
            ("ancestors", &["DOM-MOVEMENT", "DOM-GAME"]),
            ("decisions", &["DEC-0023"]),
            ("neighbours", &["A-101", "R-12", "MEC-SPRINT"]),
            ("terms", &["TERM-exhausted"]),
        ],
    );
    each_once(&json);
    let sprint = item(&json, "neighbours", "MEC-SPRINT");
    assert_eq!(
        via(sprint),
        pairs(&[("constrains", "in"), ("depends_on", "out")]),
        "{sprint}"
    );
    assert_eq!(
        via(item(&json, "neighbours", "R-12")),
        pairs(&[("derived_from", "out")])
    );
    assert_eq!(
        via(item(&json, "decisions", "DEC-0023")),
        pairs(&[("canon", "in")])
    );
    assert_eq!(
        via(item(&json, "terms", "TERM-exhausted")),
        pairs(&[("uses_term", "out")])
    );
    let target = item(&json, "targets", "MEC-STAMINA");
    assert_eq!(target["form"], "text");
    assert!(
        target["via"].is_null() && target["working_answer"].is_null(),
        "{target}"
    );
    assert_eq!(target["status"], "accepted");
    assert!(item(&json, "ancestors", "DOM-GAME")["via"].is_null());
    assert_eq!(json["tail"], serde_json::json!([]));
    assert_eq!(json["more"], 0);

    // The text: `spec show`'s target, then the layers with their headers.
    target_text_is_show(&home, &root, "MEC-STAMINA", "MEC-STAMINA", "## Ancestors\n");
    let run = bundle(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    let body = split_text(&run.stdout).body;
    for line in [
        "## Ancestors\nDOM-MOVEMENT | domain | Movement | docs/spec/movement/README.md:1\nWalking, sprinting and resting.",
        "\nDOM-GAME | domain | Lantern Keep | docs/spec/game.md:1\n",
        "## Decisions\nDEC-0023 | decision | Regeneration waits for rest | docs/records/DEC/DEC-0023.md:1 | via canon in\nStamina regenerates only at rest (A-101 becomes the rule).\n",
        "\nMEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1 | via constrains in, depends_on out\nHold the sprint key",
        "## Terms\nTERM-exhausted | term | Exhausted | docs/records/TERM/TERM-exhausted.md:1 | via uses_term out\n",
    ] {
        assert!(body.contains(line), "{line:?} not in\n{body}");
    }
    for absent in [
        "## Open questions",
        "## Criteria",
        "## Bindings",
        "## Tests",
        "## Not included",
    ] {
        assert!(!body.contains(absent), "{absent} printed:\n{body}");
    }
    assert!(
        split_text(&run.stdout).totals.ends_with(", not included 0"),
        "{}",
        run.stdout
    );
}

/// AC-01, the live filter: a Tier 3 decision with `canon:` and a
/// `constrains` onto the target, and a generated record constraining it,
/// add nothing; the bundle stays byte-identical.
#[test]
fn ac01_tier3_and_generated_sources_add_nothing() {
    let scratch = Scratch::new("bundle-ac01-live");
    let home = scratch.home("h");
    let plain = scratch.copy("spec-a", "plain");
    let root = scratch.copy("spec-a", "copy");
    write(
        &root,
        "docs/records/DEC/DEC-0005.md",
        "---\nid: DEC-0005\nclass: decision\nstatus: superseded-by DEC-0023\ndate: 2026-07-01\n\
         canon: docs/spec/movement/stamina.md#regeneration\nscope: [stamina]\nlinks:\n  \
         constrains: [MEC-STAMINA]\n  derived_from: [MEC-STAMINA]\n---\n\n# Old regeneration\n\n\
         Superseded by DEC-0023.\n",
    );
    write(
        &root,
        "docs/records/TERM/TERM-generated.md",
        "---\nid: TERM-generated\nclass: generated\ngenerator: glossary tool\nsource: docs/spec/game.md\n\
         links:\n  constrains: [MEC-STAMINA]\n---\n\n# Generated\n\nA generated term.\n",
    );
    let base = bundle(&home, &plain, &["MEC-STAMINA", "--budget", "10000"]);
    let run = bundle(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    base.code(0);
    run.code(0);
    assert_eq!(
        run.stdout, base.stdout,
        "a Tier 3 or generated source entered the bundle"
    );
    // Named, the Tier 3 decision is admitted, marked archived.
    let json = bundle_json(&home, &root, &["DEC-0005", "--budget", "10000"]);
    let target = item(&json, "targets", "DEC-0005");
    assert_eq!(target["archived"], true, "{json}");
    let text = bundle(&home, &root, &["DEC-0005", "--budget", "10000"]);
    let header = text.stdout.lines().nth(3).unwrap_or_default();
    assert!(header.contains(" | archived"), "{}", text.show());
    // Its own links are followed (its file is a target's): MEC-STAMINA by
    // `constrains out`, `derived_from out`.
    let stamina = item(&json, "neighbours", "MEC-STAMINA");
    assert_eq!(
        via(stamina),
        pairs(&[("constrains", "out"), ("derived_from", "out")])
    );
}

/// AC-01, de-duplication: two targets sharing an ancestor; a node linked
/// by several qualifying links; an ancestor that is also a neighbour; a
/// criterion that also constrains — each an item once, in its first
/// layer.
#[test]
fn ac01_a_node_is_an_item_once_in_its_first_layer() {
    let scratch = Scratch::new("bundle-ac01-dedup");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    replace(
        &root,
        "docs/spec/movement/README.md",
        "parent: DOM-GAME\n",
        "parent: DOM-GAME\nlinks:\n  constrains: [MEC-STAMINA]\n",
    );
    write(
        &root,
        "docs/records/A/A-103.md",
        "---\nid: A-103\nclass: canon\nstatus: open\nowner: owner\nreviewed: 2026-09-20\nlinks:\n  \
         verifies: [MEC-STAMINA]\n  constrains: [MEC-STAMINA]\n---\n\n# Checked in a playtest\n\n\
         The stamina rules hold in a playtest.\n",
    );
    let json = bundle_json(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    each_once(&json);
    assert_eq!(names(&json, "ancestors"), ["DOM-MOVEMENT", "DOM-GAME"]);
    assert_eq!(names(&json, "criteria"), ["A-103"]);
    assert_eq!(
        via(item(&json, "criteria", "A-103")),
        pairs(&[("verifies", "in")])
    );
    assert_eq!(names(&json, "neighbours"), ["A-101", "R-12", "MEC-SPRINT"]);

    // Two targets: the shared ancestors once; neither target a candidate.
    let json = bundle_json(
        &home,
        &root,
        &["MEC-STAMINA", "MEC-SPRINT", "--budget", "10000"],
    );
    each_once(&json);
    assert_eq!(names(&json, "targets"), ["MEC-SPRINT", "MEC-STAMINA"]);
    assert_eq!(names(&json, "ancestors"), ["DOM-MOVEMENT", "DOM-GAME"]);
    let run = bundle(
        &home,
        &root,
        &["MEC-STAMINA", "MEC-SPRINT", "--budget", "10000"],
    );
    assert!(
        run.stdout
            .starts_with("# Bundle: MEC-SPRINT, MEC-STAMINA\n\n## Targets\n"),
        "{}",
        run.show()
    );
}

/// AC-02: spec-b. REQ-001: QN-07 an open question (a `refs:` mention)
/// with its working answer ASM-01; its Russian text as `spec show` prints
/// it. MOD-CLI: ADR-0001 the one decision (ADR-0002 is Tier 3). REQ-002:
/// CRIT-01 a criterion. M: weak links not followed in layer 2; Tier 3
/// decisions admitted.
#[test]
fn ac02_spec_b_questions_decisions_and_criteria() {
    let scratch = Scratch::new("bundle-ac02");
    let home = scratch.home("h");
    let root = scratch.copy("spec-b", "copy");

    let json = bundle_json(&home, &root, &["REQ-001", "--budget", "10000"]);
    only_layers(
        &json,
        &[("targets", &["REQ-001"]), ("open_questions", &["QN-07"])],
    );
    let question = item(&json, "open_questions", "QN-07");
    assert_eq!(question["status"], "open");
    assert_eq!(via(question), pairs(&[("mentions", "in")]));
    // `path`/`line`: where `working_answer:` is written (QN-07's line 8),
    // as `--links` places the link, not the answer's own place.
    let answer = &question["working_answer"];
    assert_eq!(answer["name"], "ASM-01", "{question}");
    assert_eq!(answer["written"], "ASM-01");
    assert_eq!(answer["path"], "docs/records/QN/QN-07.md");
    assert_eq!(answer["line"], 8);
    assert_eq!(answer["state"], "resolved");
    assert_eq!(*answer, links_working_answer(&home, &root, "QN-07"));
    let body = split_text(&bundle(&home, &root, &["REQ-001", "--budget", "10000"]).stdout).body;
    assert!(
        body.contains(
            "## Open questions\nQN-07 | question | - | docs/records/QN/QN-07.md:1 | status open | via mentions in\n"
        ),
        "{body}"
    );
    // The text line stays the answer's own header.
    let answer_line = "working answer: ASM-01 | assumption | \u{041f}\u{043e}\u{043b}\u{044c}\u{0437}\u{043e}\u{0432}\u{0430}\u{0442}\u{0435}\u{043b}\u{044c} \u{0440}\u{0430}\u{0431}\u{043e}\u{0442}\u{0430}\u{0435}\u{0442} \u{0432} \u{043e}\u{0434}\u{043d}\u{043e}\u{0439} \u{0432}\u{0435}\u{0442}\u{043a}\u{0435} | docs/records/ASM/ASM-01.md:1\n";
    assert!(body.contains(answer_line), "{body}");
    target_text_is_show(&home, &root, "REQ-001", "REQ-001", "## Open questions\n");

    // Written through an alias: `name` the resolved answer, `written` as
    // typed, the place still QN-07's line 8; the text line unchanged.
    let alias = scratch.copy("spec-b", "alias");
    replace(
        &alias,
        "docs/records/ASM/ASM-01.md",
        "id: ASM-01\n",
        "id: ASM-01\naliases: [ASM-1]\n",
    );
    replace(
        &alias,
        "docs/records/QN/QN-07.md",
        "working_answer: ASM-01\n",
        "working_answer: ASM-1\n",
    );
    let json = bundle_json(&home, &alias, &["REQ-001", "--budget", "10000"]);
    let answer = &item(&json, "open_questions", "QN-07")["working_answer"];
    assert_eq!(
        *answer,
        serde_json::json!({
            "name": "ASM-01", "written": "ASM-1", "path": "docs/records/QN/QN-07.md", "line": 8,
            "state": "resolved"
        })
    );
    assert_eq!(*answer, links_working_answer(&home, &alias, "QN-07"));
    let body = split_text(&bundle(&home, &alias, &["REQ-001", "--budget", "10000"]).stdout).body;
    assert!(body.contains(answer_line), "{body}");

    let json = bundle_json(&home, &root, &["MOD-CLI", "--budget", "10000"]);
    assert_eq!(names(&json, "decisions"), ["ADR-0001"], "{json}");
    assert_eq!(
        via(item(&json, "decisions", "ADR-0001")),
        pairs(&[("canon", "in")])
    );
    each_once(&json);
    let everywhere: Vec<String> = all_layers(&json).into_iter().flat_map(|(_, n)| n).collect();
    assert!(!everywhere.contains(&"ADR-0002".to_owned()), "{json}");
    assert!(
        !tail_names(&json).contains(&"ADR-0002".to_owned()),
        "{json}"
    );

    let json = bundle_json(&home, &root, &["REQ-002", "--budget", "10000"]);
    only_layers(
        &json,
        &[("targets", &["REQ-002"]), ("criteria", &["CRIT-01"])],
    );
    let criterion = item(&json, "criteria", "CRIT-01");
    assert_eq!(criterion["form"], "text");
    assert_eq!(criterion["path"], "docs/features/dry-run.md");
    assert_eq!(criterion["line"], 16);
}

/// AC-03: "open" is read from links, not `status:`. Without DEC-0023's
/// `answers:`, MEC-STAMINA's open question is Q-031 (`status open`,
/// working answer A-101); with it, none. DEC-0023's bundle holds Q-032 with
/// `status answered` verbatim (its working answer is DEC-0023, nothing
/// answers it). An `answers:` written in a Tier 3 file closes nothing.
/// M: "open" read from `status:`.
#[test]
fn ac03_open_questions_are_open_by_links() {
    let scratch = Scratch::new("bundle-ac03");
    let home = scratch.home("h");
    let answered = scratch.copy("spec-a", "answered");
    let json = bundle_json(&home, &answered, &["MEC-STAMINA", "--budget", "10000"]);
    assert_eq!(
        names(&json, "open_questions"),
        Vec::<String>::new(),
        "{json}"
    );

    let unanswered = scratch.copy("spec-a", "unanswered");
    replace(
        &unanswered,
        "docs/records/DEC/DEC-0023.md",
        "links:\n  answers: [Q-031]\n",
        "",
    );
    let json = bundle_json(&home, &unanswered, &["MEC-STAMINA", "--budget", "10000"]);
    assert_eq!(names(&json, "open_questions"), ["Q-031"], "{json}");
    let question = item(&json, "open_questions", "Q-031");
    assert_eq!(question["status"], "open");
    assert_eq!(question["working_answer"]["name"], "A-101");
    assert_eq!(
        question["working_answer"],
        serde_json::json!({
            "name": "A-101", "written": "A-101", "path": "docs/records/Q/Q-031.md", "line": 8,
            "state": "resolved"
        })
    );
    assert_eq!(
        question["working_answer"],
        links_working_answer(&home, &unanswered, "Q-031")
    );
    each_once(&json);
    let body =
        split_text(&bundle(&home, &unanswered, &["MEC-STAMINA", "--budget", "10000"]).stdout).body;
    assert!(
        body.contains(
            "## Open questions\nQ-031 | question | - | docs/records/Q/Q-031.md:1 | status open | via mentions in\n\
             **Question (verbatim for the customer):** Does stamina regenerate while walking, or only at rest?\n\
             **Working answer:** only at rest; walking does not reset the delay (A-101).\n\
             **Cost of the other answer:** a new case in RULE-STAM-REGEN and in the delay test, ~1 work item.\n\
             working answer: A-101 | assumption | Walking does not reset the regeneration delay | docs/records/A/A-101.md:1\n"
        ),
        "{body}"
    );

    let json = bundle_json(&home, &answered, &["DEC-0023", "--budget", "10000"]);
    assert_eq!(names(&json, "open_questions"), ["Q-032"], "{json}");
    let question = item(&json, "open_questions", "Q-032");
    assert_eq!(question["status"], "answered");
    assert_eq!(question["working_answer"]["name"], "DEC-0023");
    assert_eq!(
        question["working_answer"]["path"],
        "docs/records/Q/Q-032.md"
    );
    assert_eq!(question["working_answer"]["line"], 7);
    assert_eq!(
        question["working_answer"],
        links_working_answer(&home, &answered, "Q-032")
    );
    assert_eq!(via(question), pairs(&[("working_answer", "in")]));
    each_once(&json);

    // A superseded (Tier 3) decision's `answers:` does not close Q-031.
    let tier3 = scratch.copy("spec-a", "tier3");
    replace(
        &tier3,
        "docs/records/DEC/DEC-0023.md",
        "status: accepted\n",
        "status: superseded-by DEC-0007\n",
    );
    let json = bundle_json(&home, &tier3, &["MEC-STAMINA", "--budget", "10000"]);
    assert_eq!(names(&json, "open_questions"), ["Q-031"], "{json}");
    assert_eq!(names(&json, "decisions"), Vec::<String>::new(), "{json}");
}

/// AC-04: R-12's criteria: AC-07 (an ID section of a feature-scoped
/// prefix that mentions R-12), not RULE-STAM-REGEN (it mentions R-12 too);
/// a record with `links: {verifies: [MEC-STAMINA]}` enters MEC-STAMINA's.
/// M: any node mentioning a target a criterion; criteria from `verifies`
/// only.
#[test]
fn ac04_criteria_are_verifies_sources_and_feature_sections() {
    let scratch = Scratch::new("bundle-ac04");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let json = bundle_json(&home, &root, &["R-12", "--budget", "10000"]);
    assert_eq!(names(&json, "criteria"), ["AC-07"], "{json}");
    let criterion = item(&json, "criteria", "AC-07");
    assert_eq!(via(criterion), pairs(&[("mentions", "in")]));
    assert_eq!(criterion["form"], "text");
    let everywhere: Vec<String> = all_layers(&json).into_iter().flat_map(|(_, n)| n).collect();
    assert!(
        !everywhere.contains(&"RULE-STAM-REGEN".to_owned()),
        "{json}"
    );
    let body = split_text(&bundle(&home, &root, &["R-12", "--budget", "10000"]).stdout).body;
    assert!(
        body.contains(
            "## Criteria\nAC-07 | criterion | Regeneration starts 1.5 s after the last sprint | docs/features/stamina-tuning.md:24 | via mentions in\n### Regeneration starts 1.5 s after the last sprint {#AC-07}\n\nVerifies R-12."
        ),
        "{body}"
    );

    write(
        &root,
        "docs/records/A/A-103.md",
        "---\nid: A-103\nclass: canon\nstatus: open\nowner: owner\nreviewed: 2026-09-20\nlinks:\n  \
         verifies: [MEC-STAMINA]\n---\n\n# Checked in a playtest\n\nThe stamina rules hold in a playtest.\n",
    );
    let json = bundle_json(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    assert_eq!(names(&json, "criteria"), ["A-103"], "{json}");
    assert_eq!(
        via(item(&json, "criteria", "A-103")),
        pairs(&[("verifies", "in")])
    );
    let body = split_text(&bundle(&home, &root, &["MEC-STAMINA", "--budget", "10000"]).stdout).body;
    assert!(
        body.contains("## Criteria\nA-103 | assumption | Checked in a playtest | docs/records/A/A-103.md:1 | via verifies in\n---\nid: A-103\n"),
        "{body}"
    );
}

/// The kinds of a fixture: its `[ids]` and every `kind:` written in its
/// front-matter.
fn fixture_kinds(name: &str) -> BTreeSet<String> {
    let root = fixture(name);
    let toml = read_text(&root, "specengine.toml");
    let scheme = IdScheme::from_toml(&toml).expect("fixture scheme");
    let mut kinds: BTreeSet<String> = scheme
        .prefixes()
        .iter()
        .map(|spec| spec.kind.clone())
        .collect();
    for path in common::md_files(&root) {
        for line in read_text(&root, &path).lines() {
            if let Some(kind) = line.strip_prefix("kind: ") {
                kinds.insert(kind.trim().to_owned());
            }
        }
    }
    kinds
}

/// A kind renamed at equal cost: ROT13 of its ASCII letters keeps its
/// length and the class of every character, so every estimate, and with it
/// every budget decision, is the original's whatever the estimator weights.
fn renamed_kind(kind: &str) -> String {
    kind.chars()
        .map(|c| match c {
            'a'..='z' => char::from(b'a' + (c as u8 - b'a' + 13) % 26),
            'A'..='Z' => char::from(b'A' + (c as u8 - b'A' + 13) % 26),
            _ => c,
        })
        .collect()
}

/// `spec-a` at `dir` with every kind renamed (`[ids]` and front-matter),
/// each to a string that is no kind of the fixture.
fn renamed_copy(scratch: &Scratch, dir: &str) -> std::path::PathBuf {
    let root = scratch.copy("spec-a", dir);
    let kinds = fixture_kinds("spec-a");
    let mut toml = read_text(&root, "specengine.toml");
    for kind in &kinds {
        let renamed = renamed_kind(kind);
        assert!(
            !kinds.contains(&renamed),
            "{kind} renames to a kind: {renamed}"
        );
        toml = toml.replace(
            &format!("kind = \"{kind}\""),
            &format!("kind = \"{renamed}\""),
        );
    }
    assert!(!toml.contains("kind = \"question\""), "{toml}");
    write(&root, "specengine.toml", toml);
    for path in common::md_files(&root) {
        let text = read_text(&root, &path);
        let renamed: String = text
            .split_inclusive('\n')
            .map(|line| match line.strip_prefix("kind: ") {
                Some(kind) => format!("kind: {}", renamed_kind(kind)),
                None => line.to_owned(),
            })
            .collect();
        if renamed != text {
            write(&root, &path, renamed);
        }
    }
    root
}

/// AC-05: every kind of spec-a renamed → MEC-STAMINA, R-12,
/// EDGE-SPRINT-EMPTY (and DEC-0023, MEC-STAMINA without DEC-0023's
/// `answers:`, so layer 2 is not empty) give the same names per layer in
/// the same order, the same tail and `more`, at a roomy and a tight budget.
/// The renaming keeps every estimate ([`renamed_kind`]), so the tight
/// budget compares like with like under any token weights. M: a
/// `kind == "…"` in a layer rule.
#[test]
fn ac05_renamed_kinds_give_the_same_layers() {
    let scratch = Scratch::new("bundle-ac05");
    let home = scratch.home("h");
    let original = scratch.copy("spec-a", "original");
    let renamed = renamed_copy(&scratch, "renamed");
    let renamed_kinds: BTreeSet<String> = fixture_kinds("spec-a")
        .iter()
        .map(|kind| renamed_kind(kind))
        .collect();
    let mut filled = BTreeSet::new();
    let mut compare = |targets: &[&str], round: &str| {
        for &target in targets {
            for budget in ["10000", "150"] {
                let before = bundle_json(&home, &original, &[target, "--budget", budget]);
                let after = bundle_json(&home, &renamed, &[target, "--budget", budget]);
                assert_eq!(
                    all_layers(&after),
                    all_layers(&before),
                    "{round} {target} {budget}"
                );
                assert_eq!(
                    tail_names(&after),
                    tail_names(&before),
                    "{round} {target} {budget}"
                );
                assert_eq!(after["more"], before["more"], "{round} {target} {budget}");
                let kinds: Vec<String> = all_layers(&after)
                    .into_iter()
                    .flat_map(|(layer, _)| common::graph::field(&after["layers"][layer], "kind"))
                    .collect();
                assert!(
                    kinds
                        .iter()
                        .all(|kind| kind == "null" || renamed_kinds.contains(kind)),
                    "{round} {target}: kinds not renamed: {kinds:?}"
                );
                for (layer, items) in all_layers(&after) {
                    if !items.is_empty() {
                        filled.insert(layer);
                    }
                }
            }
        }
    };
    compare(
        &["MEC-STAMINA", "R-12", "EDGE-SPRINT-EMPTY", "DEC-0023"],
        "as is",
    );
    // Q-031 open (DEC-0023 answers nothing that resolves): layer 2 filled.
    for root in [&original, &renamed] {
        replace(
            root,
            "docs/records/DEC/DEC-0023.md",
            "links:\n  answers: [Q-031]\n",
            "links:\n  answers: [Q-099]\n",
        );
    }
    compare(&["MEC-STAMINA", "R-12", "EDGE-SPRINT-EMPTY"], "Q-031 open");
    for layer in [
        "targets",
        "open_questions",
        "ancestors",
        "criteria",
        "decisions",
        "neighbours",
        "terms",
    ] {
        assert!(
            filled.contains(layer),
            "layer {layer} never filled: {filled:?}"
        );
    }
}

/// AC-05: no string literal of the bundle sources (core `check/bundle.rs`,
/// CLI `src/bundle.rs`) equals a kind of either fixture (whole-literal
/// match: "term" is in "determinism", R1).
#[test]
fn ac05_the_bundle_sources_hold_no_kind_literal() {
    let mut kinds = fixture_kinds("spec-a");
    kinds.extend(fixture_kinds("spec-b"));
    assert!(
        kinds.contains("question") && kinds.contains("module"),
        "{kinds:?}"
    );
    let repository = repository_root();
    let mut offenders = Vec::new();
    for source in [
        "crates/specengine-core/src/check/bundle.rs",
        "crates/specengine-cli/src/bundle.rs",
    ] {
        let text = read_text(&repository, source);
        let found = literals(&text);
        assert!(
            found.len() >= 5,
            "{source}: the scan found {} literals",
            found.len()
        );
        for (line, literal) in found {
            if kinds.contains(&literal) {
                offenders.push(format!("{source}:{line}: {literal:?}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "kind literals:\n{}",
        offenders.join("\n")
    );
}

/// AC-10: every tail entry's tokens are `spec show <name> --json`'s
/// `tokens_est` (the node's own), the outlined target's sections and the
/// candidates alike. M: the tail priced by the summary.
#[test]
fn ac10_tail_entries_carry_the_nodes_own_estimate() {
    let scratch = Scratch::new("bundle-ac10");
    let home = scratch.home("h");
    let mut checked = 0;
    for (corpus, targets) in [
        (
            "spec-a",
            &["MEC-STAMINA", "DOM-MOVEMENT", "R-12", "DEC-0023"][..],
        ),
        ("spec-b", &["MOD-CLI", "REQ-001", "REQ-002"][..]),
    ] {
        let root = scratch.copy(corpus, corpus);
        for target in targets {
            let minimum = common::bundle::minimum(&home, &root, &[target]);
            for budget in [minimum, minimum + 40, minimum + 120] {
                let json = bundle_json(&home, &root, &[target, "--budget", &budget.to_string()]);
                for entry in json["tail"].as_array().unwrap() {
                    let name = entry["name"].as_str().unwrap();
                    let show = spec30(&home, &root, &["--json", "show", name]).json();
                    let nodes = show["nodes"].as_array().unwrap();
                    assert_eq!(nodes.len(), 1, "{name}: {show}");
                    assert_eq!(
                        entry["tokens_est"], nodes[0]["tokens_est"],
                        "{corpus} {target} at {budget}: {entry}"
                    );
                    assert_eq!(entry["path"], nodes[0]["path"], "{entry}");
                    assert_eq!(entry["line"], nodes[0]["line"], "{entry}");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked >= 10, "only {checked} tail entries checked");
}

/// AC-12: REFs and exits. `MEC-NOPE` → exit 1 with a JSON `reason`;
/// `MEC-STAMINA MEC-NOPE` → exit 1, no bundle; a look-alike ID → exit 2
/// naming the Latin fix; `project:` → exit 2; `--budget 0`, `x`, `-5`,
/// 2^32 and no REF → exit 2, no JSON; `QST-031` → Q-031's hash; a `.md`
/// path accepted. M: an unresolvable REF silently dropped.
#[test]
fn ac12_refs_resolve_or_the_bundle_is_refused() {
    let scratch = Scratch::new("bundle-ac12");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");

    let run = spec30(&home, &root, &["--json", "bundle", "MEC-NOPE"]);
    run.code(1);
    let json = run.json();
    assert_eq!(json["refs"], serde_json::json!(["MEC-NOPE"]));
    assert!(
        json["reason"]
            .as_str()
            .is_some_and(|r| r.contains("MEC-NOPE")),
        "{json}"
    );
    for key in TOP_KEYS {
        if !["refs", "reason", "notes"].contains(&key) {
            assert!(json[key].is_null(), "{key} set on exit 1: {json}");
        }
    }
    assert_eq!(
        run.stderr_lines().last().copied(),
        Some(&*format!("spec: {}", json["reason"].as_str().unwrap()))
    );

    for args in [
        &["bundle", "MEC-STAMINA", "MEC-NOPE"][..],
        &["bundle", "MEC-NOPE", "MEC-STAMINA"][..],
        &["--json", "bundle", "MEC-STAMINA", "MEC-NOPE"][..],
    ] {
        let run = spec30(&home, &root, args);
        run.code(1);
        assert!(!run.stdout.contains("# Bundle"), "{}", run.show());
        assert!(!run.stdout.contains("bundle_hash b3:"), "{}", run.show());
        if args[0] == "--json" {
            let json = run.json();
            assert!(
                json["body"].is_null() && json["bundle_hash"].is_null(),
                "{json}"
            );
            assert_eq!(json["refs"], serde_json::json!(["MEC-STAMINA", "MEC-NOPE"]));
        } else {
            assert!(run.stdout.is_empty(), "{}", run.show());
        }
        assert!(run.stderr.contains("spec: `MEC-NOPE`"), "{}", run.show());
    }

    // Exit 2, nothing on stdout, also with --json.
    let lookalike = "\u{0410}-101";
    for (args, needle) in [
        (vec!["--json", "bundle", lookalike], "write `A-101`"),
        (vec!["bundle", "MEC-STAMINA", lookalike], "write `A-101`"),
        (vec!["--json", "bundle", "other:R-12"], "project:"),
        (
            vec!["--json", "bundle", "MEC-STAMINA", "--budget", "0"],
            "--budget 0",
        ),
        (
            vec!["--json", "bundle", "MEC-STAMINA", "--budget", "-5"],
            "--budget -5",
        ),
        (
            vec!["--json", "bundle", "MEC-STAMINA", "--budget", "4294967296"],
            "4294967296",
        ),
        (
            vec!["--json", "bundle", "MEC-STAMINA", "--budget", "x"],
            "'x'",
        ),
        (vec!["--json", "bundle"], "<REF>"),
    ] {
        let run = spec30(&home, &root, &args);
        run.code(2);
        assert!(run.stdout.is_empty(), "{args:?}: {}", run.show());
        assert!(
            run.stderr.contains(needle),
            "{args:?}: {needle:?} not in {}",
            run.show()
        );
    }
    // The largest budget answers.
    bundle_json(&home, &root, &["MEC-STAMINA", "--budget", "4294967295"]);

    // An alias and the ID: one node, one hash; the title names the node.
    let alias = bundle_json(&home, &root, &["QST-031"]);
    let id = bundle_json(&home, &root, &["Q-031"]);
    assert_eq!(alias["bundle_hash"], id["bundle_hash"]);
    assert_eq!(alias["body"], id["body"]);
    assert_eq!(alias["refs"], serde_json::json!(["QST-031"]));
    assert!(
        alias["body"]
            .as_str()
            .unwrap()
            .starts_with("# Bundle: Q-031\n"),
        "{alias}"
    );

    // A root-relative `.md` path.
    let json = bundle_json(&home, &root, &["docs/features/stamina-tuning.md"]);
    assert_eq!(names(&json, "targets"), ["docs/features/stamina-tuning.md"]);
    assert!(
        json["body"]
            .as_str()
            .unwrap()
            .starts_with("# Bundle: docs/features/stamina-tuning.md\n"),
        "{json}"
    );
}

/// Description, REF: REFs naming one node count once; a target within
/// another merges into it with one note; several holders → all, one
/// warning.
#[test]
fn refs_naming_one_node_merge_and_holders_all_count() {
    let scratch = Scratch::new("bundle-refs");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let single = bundle_json(&home, &root, &["MEC-STAMINA"]);
    let twice = bundle_json(
        &home,
        &root,
        &[
            "MEC-STAMINA",
            "docs/spec/movement/stamina.md",
            "MEC-STAMINA",
        ],
    );
    assert_eq!(names(&twice, "targets"), ["MEC-STAMINA"]);
    assert_eq!(twice["bundle_hash"], single["bundle_hash"]);
    assert_eq!(twice["notes"], serde_json::json!([]));

    let run = spec30(
        &home,
        &root,
        &[
            "--json",
            "bundle",
            "RULE-STAM-REGEN",
            "MEC-STAMINA",
            "--budget",
            "10000",
        ],
    );
    run.code(0);
    let json = run.json();
    assert_eq!(names(&json, "targets"), ["MEC-STAMINA"]);
    assert_eq!(
        json["notes"],
        serde_json::json!(["`RULE-STAM-REGEN` is within `MEC-STAMINA`: bundled as part of it"]),
        "{json}"
    );
    assert_eq!(
        run.stderr,
        "note: `RULE-STAM-REGEN` is within `MEC-STAMINA`: bundled as part of it\n"
    );
    let plain = bundle_json(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    assert_eq!(json["body"], plain["body"]);

    // A second holder of R-12.
    write(
        &root,
        "docs/records/R/R-12-copy.md",
        "---\nid: R-12\nclass: canon\nstatus: draft\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
         # A second R-12\n\nHeld twice.\n",
    );
    let run = spec30(
        &home,
        &root,
        &["--json", "bundle", "R-12", "--budget", "10000"],
    );
    run.code(0);
    let json = run.json();
    assert_eq!(names(&json, "targets"), ["R-12", "R-12"], "{json}");
    let warnings: Vec<&str> = run
        .stderr_lines()
        .into_iter()
        .filter(|line| line.starts_with("warning: "))
        .collect();
    assert_eq!(warnings.len(), 1, "{}", run.show());
    assert!(
        warnings[0].contains("`R-12` has 2 holders"),
        "{}",
        run.show()
    );
    assert!(
        json["body"]
            .as_str()
            .unwrap()
            .starts_with("# Bundle: R-12, R-12\n"),
        "{json}"
    );
}

/// The key sets of one answered bundle (top level, layers, items, tail
/// entries, working answers); every key present.
fn assert_key_sets(json: &Value, context: &str) -> (usize, usize, usize) {
    assert_eq!(keys(json), BTreeSet::from(TOP_KEYS), "{context}: {json}");
    assert!(json["task"].is_null(), "{context}: task {json}");
    assert_eq!(keys(&json["layers"]), BTreeSet::from(LAYERS), "{context}");
    let (mut items, mut answers, mut entries) = (0, 0, 0);
    for layer in LAYERS {
        for value in json["layers"][layer].as_array().unwrap() {
            assert_eq!(
                keys(value),
                BTreeSet::from(ITEM_KEYS),
                "{context} {layer}: {value}"
            );
            items += 1;
            if !value["working_answer"].is_null() {
                assert_eq!(
                    keys(&value["working_answer"]),
                    BTreeSet::from(WORKING_ANSWER_KEYS),
                    "{context}"
                );
                answers += 1;
            }
            let via_null = value["via"].is_null();
            assert_eq!(
                via_null,
                ["targets", "ancestors"].contains(&layer),
                "{context} {layer}: via {value}"
            );
            for pair in value["via"].as_array().into_iter().flatten() {
                assert_eq!(
                    keys(pair),
                    BTreeSet::from(["type", "direction"]),
                    "{context}"
                );
            }
        }
    }
    for entry in json["tail"].as_array().unwrap() {
        assert_eq!(keys(entry), BTreeSet::from(TAIL_KEYS), "{context}: {entry}");
        assert!(
            LAYERS.contains(&entry["layer"].as_str().unwrap()),
            "{context}: {entry}"
        );
        entries += 1;
    }
    assert!(json["more"].is_u64(), "{context}");
    (items, answers, entries)
}

/// The layer keys of a raw `--json` stdout appear in print order (the
/// parsed `Value` sorts its keys).
fn layers_in_print_order(stdout: &str, context: &str) {
    let start = stdout
        .find("\"layers\":{")
        .unwrap_or_else(|| panic!("{context}: no layers object: {stdout}"));
    let mut last = start;
    for layer in LAYERS {
        let at = stdout[start..]
            .find(&format!("\"{layer}\":["))
            .map(|at| at + start)
            .unwrap_or_else(|| panic!("{context}: no {layer} key: {stdout}"));
        assert!(
            at > last || (layer == LAYERS[0] && at >= last),
            "{context}: {layer} out of order"
        );
        last = at;
    }
}

/// AC-14: both fixtures give the same key sets — top level, `layers` (all
/// nine, in print order, `[]` when empty), item, working answer, tail
/// entry — every key present, `task: null`; an exit 1 the same top-level
/// keys. `show`, `search`, `tree` and `graph` keep their HEAD key sets.
/// M: a key omitted when empty.
#[test]
fn ac14_both_fixtures_give_one_key_set() {
    let scratch = Scratch::new("bundle-ac14");
    let home = scratch.home("h");
    let mut totals = (0, 0, 0);
    for ((corpus, _), target) in FIXTURES.iter().zip(["MEC-STAMINA", "MOD-CLI"]) {
        let root = scratch.copy(corpus, corpus);
        let minimum = common::bundle::minimum(&home, &root, &[target]);
        for budget in [minimum, minimum + 60, 10_000] {
            let run = spec30(
                &home,
                &root,
                &["--json", "bundle", target, "--budget", &budget.to_string()],
            );
            run.code(0);
            layers_in_print_order(&run.stdout, corpus);
            let json = run.json();
            let (items, answers, entries) = assert_key_sets(&json, corpus);
            totals.0 += items;
            totals.1 += answers;
            totals.2 += entries;
            // Empty layers and an empty tail are `[]`, not absent.
            assert_eq!(json["layers"]["bindings"], serde_json::json!([]));
            assert_eq!(json["layers"]["tests"], serde_json::json!([]));
        }
        let ten = bundle_json(&home, &root, &[target, "--budget", "10000"]);
        assert_eq!(ten["tail"], serde_json::json!([]), "{corpus}: {ten}");
        let run = spec30(&home, &root, &["--json", "bundle", "NOPE-1"]);
        run.code(1);
        assert_eq!(keys(&run.json()), BTreeSet::from(TOP_KEYS));
    }
    let open = scratch.copy("spec-b", "spec-b-open");
    let json = bundle_json(&home, &open, &["REQ-001", "--budget", "10000"]);
    let (_, answers, _) = assert_key_sets(&json, "spec-b REQ-001");
    totals.1 += answers;
    assert!(
        totals.0 > 10 && totals.1 >= 1 && totals.2 >= 1,
        "{totals:?}"
    );

    // The other read commands keep the key sets their own tests pin.
    let root = scratch.copy("spec-a", "reads");
    let show = spec30(&home, &root, &["--json", "show", "MEC-STAMINA"]).json();
    assert_eq!(
        keys(&show),
        BTreeSet::from(["ref", "reason", "notes", "nodes"])
    );
    assert_eq!(
        keys(&show["nodes"][0]),
        BTreeSet::from([
            "id",
            "kind",
            "title",
            "path",
            "line",
            "end_line",
            "status",
            "rev",
            "tokens_est",
            "archived",
            "utf8",
            "sections",
            "text",
            "truncated",
            "omitted",
            "links",
        ])
    );
    let search = spec30(&home, &root, &["--json", "search", "stamina"]).json();
    assert_eq!(
        keys(&search),
        BTreeSet::from([
            "archive",
            "hits",
            "kinds",
            "limit",
            "notes",
            "query",
            "tier3_left_out",
            "truncated",
        ])
    );
    let tree = spec30(&home, &root, &["--json", "tree"]).json();
    assert_eq!(
        keys(&tree),
        BTreeSet::from([
            "ref",
            "reason",
            "notes",
            "depth",
            "kinds",
            "archive",
            "left_out",
            "truncated",
            "nodes",
        ])
    );
    let graph = spec30(&home, &root, &["--json", "graph", "MEC-STAMINA"]).json();
    assert_eq!(
        keys(&graph),
        BTreeSet::from([
            "ref",
            "reason",
            "impact",
            "types",
            "depth",
            "archive",
            "notes",
            "left_out",
            "truncated",
            "nodes",
            "edges",
        ])
    );
}

/// Description and Rules: a text without a line end gains one; a section
/// candidate shows its header alone (`form: header`); a dangling working
/// answer shows as written, where written, with its state; a dangling
/// parent and a parent cycle still answer, each ancestor once; no `HOME`
/// is exit 2 without stdout.
#[test]
fn line_ends_sections_dangling_answers_cycles_and_home() {
    let scratch = Scratch::new("bundle-edges");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    for path in ["docs/spec/movement/stamina.md", "docs/records/A/A-101.md"] {
        let text = read_text(&root, path);
        write(&root, path, text.trim_end_matches('\n'));
    }
    replace(
        &root,
        "docs/records/DEC/DEC-0023.md",
        "links:\n  answers: [Q-031]\n",
        "",
    );
    replace(
        &root,
        "docs/records/Q/Q-031.md",
        "working_answer: A-101",
        "working_answer: A-999",
    );
    let run = bundle(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    run.code(0);
    let body = split_text(&run.stdout).body;
    assert!(
        body.contains("is applied immediately.\n\n## Open questions\n"),
        "{body}"
    );
    assert!(
        body.contains("work item.\nworking answer: A-999 | docs/records/Q/Q-031.md:8 | dangling"),
        "{body}"
    );
    assert!(
        body.contains("Assumed until Q-031 is answered.\n\nR-12 | "),
        "{body}"
    );
    let json = bundle_json(&home, &root, &["MEC-STAMINA", "--budget", "10000"]);
    assert_eq!(
        item(&json, "open_questions", "Q-031")["working_answer"],
        serde_json::json!({
            "name": null, "written": "A-999", "path": "docs/records/Q/Q-031.md", "line": 8,
            "state": "dangling"
        })
    );
    assert_eq!(
        item(&json, "open_questions", "Q-031")["working_answer"],
        links_working_answer(&home, &root, "Q-031")
    );

    // A section candidate: its header alone.
    let json = bundle_json(&home, &root, &["MEC-SPRINT", "--budget", "10000"]);
    let section = item(&json, "neighbours", "RULE-STAM-REGEN");
    assert_eq!(section["form"], "header");
    assert_eq!(via(section), pairs(&[("constrains", "out")]));
    assert_eq!(item(&json, "neighbours", "MEC-STAMINA")["form"], "summary");
    let body = json["body"].as_str().unwrap();
    assert!(
        body.ends_with(
            "\nRULE-STAM-REGEN | rule | Regeneration | docs/spec/movement/stamina.md:21 | via constrains out\n"
        ),
        "{body}"
    );

    // A dangling parent, a parent cycle.
    replace(
        &root,
        "docs/spec/movement/sprint.md",
        "parent: DOM-MOVEMENT\n",
        "parent: DOM-NOPE\n",
    );
    let json = bundle_json(&home, &root, &["MEC-SPRINT"]);
    assert_eq!(names(&json, "ancestors"), Vec::<String>::new(), "{json}");
    replace(
        &root,
        "docs/spec/game.md",
        "tier: 0\n",
        "tier: 0\nparent: MEC-STAMINA\n",
    );
    let json = bundle_json(&home, &root, &["MEC-STAMINA"]);
    assert_eq!(
        names(&json, "ancestors"),
        ["DOM-MOVEMENT", "DOM-GAME"],
        "{json}"
    );
    // The cycle is broken at its first member in path order, DOM-GAME
    // (docs/canon/spec-cli-graph.md): it has no ancestor.
    let json = bundle_json(&home, &root, &["DOM-GAME"]);
    assert_eq!(names(&json, "ancestors"), Vec::<String>::new(), "{json}");
    let json = bundle_json(&home, &root, &["DOM-MOVEMENT"]);
    assert_eq!(names(&json, "ancestors"), ["DOM-GAME"], "{json}");

    // No HOME.
    let run = common::spec_with(&root, &["--json", "bundle", "MEC-STAMINA"], &[]);
    run.code(2);
    assert!(run.stdout.is_empty(), "{}", run.show());
    assert!(run.stderr.contains("HOME"), "{}", run.show());
}
