//! AC-10 of docs/features/spec-cli.md: `spec show REF` resolves as the
//! check does (05 §1 principle 1): aliases, `aliases_from` legacy IDs,
//! `slug/ID`, `ID#SECTION`, `@rev` noted, several holders all shown with one
//! warning, dangling and unconfigured references exit 1, look-alikes,
//! mixed scripts and `project:` exit 2; and every form's holders equal
//! `Resolver::resolve_detached` over the walk-fed `check_input`.

#![cfg(unix)]

mod common;

use std::path::Path;

use common::{Scratch, index, read_text, spec, write};
use specengine_core::ProjectConfig;
use specengine_core::check::{Resolution, Resolver};
use specengine_model::grammar;
use specengine_store::WorkingTree;

/// The holder paths of `written` by the core, over a fresh walk of `root`.
fn core_holders(root: &Path, written: &str) -> Vec<String> {
    let config = ProjectConfig::from_toml(&read_text(root, "specengine.toml")).expect("config");
    let tree = WorkingTree::new(root, &config.paths).expect("tree");
    let input = specengine_store::check_input(&tree, &config.scheme);
    let resolver = Resolver::new(&input, &config.scheme, &config.paths);
    let found = grammar::parse_reference(written, 0, &config.scheme)
        .unwrap_or_else(|| panic!("{written} is a reference"));
    match resolver.resolve_detached(&found.reference, written) {
        Resolution::Resolved(files) => files
            .iter()
            .map(|&file| resolver.paths()[file].to_owned())
            .collect(),
        other => panic!("{written}: {other:?}"),
    }
}

/// The distinct holder paths `show --json` printed, in order.
fn shown_holders(json: &serde_json::Value) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for node in json["nodes"].as_array().unwrap() {
        let path = node["path"].as_str().unwrap().to_owned();
        if paths.last() != Some(&path) {
            paths.push(path);
        }
    }
    paths
}

/// `(REF, the ID of the first node shown, its path)`; exit 0, holders as
/// the core's.
fn check_found(home: &Path, root: &Path, written: &str, id: &str, path: &str) -> serde_json::Value {
    let run = spec(home, root, &["show", written]);
    run.code(0);
    assert!(
        run.stdout.starts_with(&format!("{id} | ")),
        "{written} → {id}:\n{}",
        run.show()
    );
    let json = spec(home, root, &["--json", "show", written]);
    json.code(0);
    let json = json.json();
    assert_eq!(json["nodes"][0]["id"], id, "{written}: {json}");
    assert_eq!(json["nodes"][0]["path"], path, "{written}");
    assert_eq!(
        shown_holders(&json),
        core_holders(root, written),
        "{written}: holders equal resolve_detached's"
    );
    json
}

#[test]
fn spec_a_forms_resolve_like_the_check() {
    let scratch = Scratch::new("resolve-a");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    for (written, id, path) in [
        (
            "TERM-tired",
            "TERM-exhausted",
            "docs/records/TERM/TERM-exhausted.md",
        ),
        (
            "TERM-exhausted",
            "TERM-exhausted",
            "docs/records/TERM/TERM-exhausted.md",
        ),
        ("QST-031", "Q-031", "docs/records/Q/Q-031.md"),
        ("Q-031", "Q-031", "docs/records/Q/Q-031.md"),
        (
            "stamina-tuning/AC-07",
            "AC-07",
            "docs/features/stamina-tuning.md",
        ),
        ("AC-07", "AC-07", "docs/features/stamina-tuning.md"),
        (
            "MEC-STAMINA#RULE-STAM-REGEN",
            "RULE-STAM-REGEN",
            "docs/spec/movement/stamina.md",
        ),
        (
            "MEC-STAMINA#RULE-STAM-REGEN@3",
            "RULE-STAM-REGEN",
            "docs/spec/movement/stamina.md",
        ),
        (
            "RULE-STAM-REGEN",
            "RULE-STAM-REGEN",
            "docs/spec/movement/stamina.md",
        ),
        ("R-12", "R-12", "docs/records/R/R-12.md"),
        ("DEC-0007", "DEC-0007", "docs/records/DEC/DEC-0007.md"),
    ] {
        let run = spec(&home, &root, &["show", written]);
        run.code(0);
        let with_rev = written.contains('@');
        assert_eq!(
            run.stderr_lines().len(),
            usize::from(with_rev),
            "{written}: stderr\n{}",
            run.show()
        );
        let json = check_found(&home, &root, written, id, path);
        assert_eq!(json["nodes"].as_array().unwrap().len(), 1, "{written}");
    }
    // A section form shows the section's span, not the document.
    let json = spec(&home, &root, &["--json", "show", "stamina-tuning/AC-07"]).json();
    assert_eq!(json["nodes"][0]["line"], 24);
}

#[test]
fn a_revision_is_ignored_with_a_note() {
    let scratch = Scratch::new("resolve-rev");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    let plain = spec(&home, &root, &["show", "R-12"]);
    plain.code(0);
    let run = spec(&home, &root, &["show", "R-12@3"]);
    run.code(0);
    assert_eq!(run.stdout, plain.stdout, "@3 prints the current text");
    let lines = run.stderr_lines();
    assert_eq!(lines.len(), 1, "{}", run.show());
    assert!(
        lines[0].starts_with("note: ") && lines[0].contains("@3"),
        "{}",
        run.show()
    );
    let json = spec(&home, &root, &["--json", "show", "R-12@3"]).json();
    assert_eq!(json["notes"].as_array().unwrap().len(), 1);
    assert!(json["notes"][0].as_str().unwrap().contains("@3"));
}

/// A second feature document defining `{#AC-07}`: the bare ID prints both
/// by path, exit 0, one warning citing each as `slug/ID`; the scoped form
/// picks one.
#[test]
fn a_bare_feature_scoped_id_with_two_holders_prints_both_with_one_warning() {
    let scratch = Scratch::new("resolve-two");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    write(
        &root,
        "docs/features/lantern-fuel.md",
        "---\nclass: spec\nstatus: draft\n---\n\n# Lantern fuel\n\n## Acceptance criteria\n\n### Fuel lasts the night {#AC-07}\n\nMeasured.\n",
    );
    index(&home, &root);
    let run = spec(&home, &root, &["show", "AC-07"]);
    run.code(0);
    let headers: Vec<&str> = run
        .stdout
        .lines()
        .filter(|line| line.starts_with("AC-07 | "))
        .collect();
    assert_eq!(headers.len(), 2, "{}", run.show());
    assert!(
        headers[0].contains(" | docs/features/lantern-fuel.md:10 | "),
        "{}",
        run.show()
    );
    assert!(
        headers[1].contains(" | docs/features/stamina-tuning.md:24 | "),
        "{}",
        run.show()
    );
    // An empty line between the two nodes.
    assert!(
        run.stdout.contains("Measured.\n\nAC-07 | "),
        "{}",
        run.show()
    );
    let lines = run.stderr_lines();
    assert_eq!(lines.len(), 1, "one warning\n{}", run.show());
    assert!(
        lines[0].starts_with("warning: ")
            && lines[0].contains("lantern-fuel/AC-07")
            && lines[0].contains("stamina-tuning/AC-07"),
        "{}",
        run.show()
    );
    let json = spec(&home, &root, &["--json", "show", "AC-07"]).json();
    assert_eq!(
        shown_holders(&json),
        [
            "docs/features/lantern-fuel.md",
            "docs/features/stamina-tuning.md"
        ]
    );
    assert_eq!(shown_holders(&json), core_holders(&root, "AC-07"));
    assert_eq!(
        json["notes"],
        serde_json::json!([]),
        "warnings are not notes"
    );
    for (scoped, path) in [
        ("lantern-fuel/AC-07", "docs/features/lantern-fuel.md"),
        ("stamina-tuning/AC-07", "docs/features/stamina-tuning.md"),
    ] {
        let json = check_found(&home, &root, scoped, "AC-07", path);
        assert_eq!(json["nodes"].as_array().unwrap().len(), 1, "{scoped}");
    }
}

/// Dangling or unconfigured → exit 1 with the reason; look-alikes, mixed
/// scripts and `project:` → exit 2.
#[test]
fn not_found_is_exit_1_and_unusable_references_exit_2() {
    let scratch = Scratch::new("resolve-bad");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    for (written, needle) in [
        ("R-99", "R-99"),
        ("FOO-1", "prefixes: "),
        ("nonsense", "prefixes: "),
        ("MEC-STAMINA#RULE-NOPE", "RULE-NOPE"),
        ("nope/AC-07", "nope"),
        ("docs/records/R/R-99.md", "R-99.md"),
    ] {
        let run = spec(&home, &root, &["show", written]);
        run.code(1);
        assert_eq!(run.stdout, "", "{written}");
        let lines = run.stderr_lines();
        assert_eq!(lines.len(), 1, "{written}: {}", run.show());
        assert!(
            lines[0].starts_with("spec: ") && lines[0].contains(needle),
            "{written}: {}",
            run.show()
        );
        let json = spec(&home, &root, &["--json", "show", written]);
        json.code(1);
        let json = json.json();
        assert_eq!(json["nodes"], serde_json::json!([]));
        assert!(json["reason"].as_str().unwrap().contains(needle), "{json}");
    }
    let foo = spec(&home, &root, &["show", "FOO-1"]);
    assert!(
        foo.stderr.contains("QST"),
        "legacy prefixes listed: {}",
        foo.show()
    );
    for (written, needle) in [
        ("\u{0410}-101", "A-101"),
        ("M\u{0415}C-STAMINA", "MEC-STAMINA"),
        ("other:R-12", "project:"),
        ("other:stamina-tuning/AC-07", "project:"),
    ] {
        for json in [false, true] {
            let args: Vec<&str> = if json {
                vec!["--json", "show", written]
            } else {
                vec!["show", written]
            };
            let run = spec(&home, &root, &args);
            run.code(2);
            assert_eq!(run.stdout, "", "{written}");
            let lines = run.stderr_lines();
            assert_eq!(lines.len(), 1, "{written}: {}", run.show());
            assert!(
                lines[0].starts_with("spec: ") && lines[0].contains(needle),
                "{written}: want {needle:?}\n{}",
                run.show()
            );
        }
    }
    // A project with no `[ids]`: any ID is exit 1 naming the missing prefixes.
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"lantern-keep\"\n",
    );
    let run = spec(&home, &root, &["show", "R-12"]);
    run.code(1);
    assert!(run.stderr.contains("[ids]"), "{}", run.show());
}

#[test]
fn spec_b_legacy_cyrillic_ids_are_aliases() {
    let scratch = Scratch::new("resolve-b");
    let home = scratch.home("h");
    let root = scratch.copy("spec-b", "copy");
    index(&home, &root);
    for (written, id, path) in [
        (
            "\u{0422}\u{0420}\u{0411}-001",
            "REQ-001",
            "docs/records/REQ/REQ-001.md",
        ),
        (
            "\u{0412}\u{041e}\u{041f}-07",
            "QN-07",
            "docs/records/QN/QN-07.md",
        ),
        ("REQ-001", "REQ-001", "docs/records/REQ/REQ-001.md"),
        ("dry-run/CRIT-01", "CRIT-01", "docs/features/dry-run.md"),
        ("CRIT-01", "CRIT-01", "docs/features/dry-run.md"),
        ("MOD-CLI#CMD-SYNC", "CMD-SYNC", "docs/spec/cli.md"),
        ("ADR-0002", "ADR-0002", "docs/records/ADR/ADR-0002.md"),
    ] {
        check_found(&home, &root, written, id, path);
    }
    // A Latin prefix with one Cyrillic look-alike: exit 2 naming the fix.
    let run = spec(&home, &root, &["show", "R\u{0415}Q-001"]);
    run.code(2);
    assert!(run.stderr.contains("REQ-001"), "{}", run.show());
    let run = spec(&home, &root, &["show", "REQ-999"]);
    run.code(1);
}
