//! AC-08 … AC-11 of docs/features/spec-cli-graph.md: `spec show REF
//! --links [--archive]` prints `show` unchanged with each node's links
//! block before its bytes: outgoing links written in its span (every
//! state), incoming live links resolving to it or a nested section, each
//! resolved from its citing file as `spec check` resolves it (aliases,
//! legacy forms, path targets and anchors, the name-shape fallback); the
//! live rule leaves generated and Tier 3 sources out, counted. Scratch
//! copies of spec-a and spec-b under their own `HOME`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::path::Path;

use common::graph::{item_lines, keys, link_lines, links_block, spec30, summary_line, tree_depths};
use common::{Scratch, md_files, read, read_text, replace, write};
use serde_json::Value;

const STAMINA: &str = "docs/spec/movement/stamina.md";

/// `(direction rank, weak, type, path, line)` of a link line: the order
/// key the spec gives (outgoing first; strong before `mentions`, then
/// type, path, line).
fn order_key(line: &str) -> (u8, bool, String, String, usize) {
    let (head, rest) = line.split_once(" | ").expect("` | ` in a link line");
    let mut words = head.split_whitespace();
    let direction = words.next().unwrap();
    let link_type = words.next().unwrap().to_owned();
    let place = rest.split(" | ").next().unwrap();
    let (path, number) = place.rsplit_once(':').expect("path:line");
    (
        u8::from(direction == "in"),
        link_type == "mentions",
        link_type,
        path.to_owned(),
        number.parse().expect("a line number"),
    )
}

/// The links block is ordered, its summary counts its lines, and the rest
/// of stdout is the header plus `show`'s bytes unchanged.
fn assert_block_shape(stdout: &str, bytes: &[u8], context: &str) {
    let lines = link_lines(stdout);
    let order: Vec<_> = lines.iter().map(|line| order_key(line)).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted, "{context}: link order\n{stdout}");
    let out = lines
        .iter()
        .filter(|line| line.starts_with("  out "))
        .count();
    let incoming = lines.len() - out;
    let block = links_block(stdout);
    let summary = block.last().unwrap();
    assert!(
        summary.starts_with(&format!("  links {out} out, {incoming} in")),
        "{context}: {summary}"
    );
    let header_and_block: usize = stdout
        .split_inclusive('\n')
        .take(1 + block.len())
        .map(str::len)
        .sum();
    assert_eq!(
        &stdout.as_bytes()[header_and_block..],
        bytes,
        "{context}: the bytes after the links block"
    );
}

/// AC-08: MEC-STAMINA's and RULE-STAM-REGEN's links on spec-a, path
/// targets (a `canon:` path with a slug anchor) included; the block comes
/// before the bytes, which are `show`'s unchanged. M: path targets
/// unresolved.
#[test]
fn spec_a_links_of_a_document_and_a_section() {
    let scratch = Scratch::new("links-a");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let run = spec30(&home, &root, &["show", "MEC-STAMINA", "--links"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    let plain = spec30(&home, &root, &["show", "MEC-STAMINA"]);
    let header = plain.stdout.lines().next().unwrap();
    assert_eq!(run.stdout.lines().next().unwrap(), header);
    assert_block_shape(&run.stdout, &read(&root, STAMINA), "MEC-STAMINA");
    let lines = link_lines(&run.stdout);
    for want in [
        "  out derived_from R-12 | docs/spec/movement/stamina.md:12",
        "  out derived_from A-101 | docs/spec/movement/stamina.md:12",
        "  out depends_on MEC-SPRINT | docs/spec/movement/stamina.md:13",
        "  out uses_term TERM-exhausted | docs/spec/movement/stamina.md:14",
        "  in depends_on MEC-SPRINT | docs/spec/movement/sprint.md:9",
        "  in mentions Q-031 | docs/records/Q/Q-031.md:9",
        "  in constrains MEC-SPRINT | docs/spec/movement/sprint.md:10 | at RULE-STAM-REGEN",
        "  in canon DEC-0023 | docs/records/DEC/DEC-0023.md:6 | at RULE-STAM-REGEN | as docs/spec/movement/stamina.md#regeneration",
    ] {
        assert!(lines.contains(&want), "missing {want:?}\n{}", run.show());
    }

    let run = spec30(&home, &root, &["show", "RULE-STAM-REGEN", "--links"]);
    run.code(0);
    let lines = link_lines(&run.stdout);
    for want in [
        "  in constrains MEC-SPRINT | docs/spec/movement/sprint.md:10",
        "  in canon DEC-0023 | docs/records/DEC/DEC-0023.md:6 | as docs/spec/movement/stamina.md#regeneration",
        "  out mentions R-12 | docs/spec/movement/stamina.md:22",
    ] {
        assert!(lines.contains(&want), "missing {want:?}\n{}", run.show());
    }
    // A section's own links only: nothing written outside its span.
    assert!(
        !lines.iter().any(|line| line.contains("derived_from")),
        "{}",
        run.show()
    );
    let section = read_text(&root, STAMINA)
        .lines()
        .skip(20)
        .take(3)
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    assert_block_shape(&run.stdout, section.as_bytes(), "RULE-STAM-REGEN");

    // The JSON: the same links, exactly the keys of "Data".
    let json = spec30(
        &home,
        &root,
        &["--json", "show", "RULE-STAM-REGEN", "--links"],
    )
    .json();
    let links = &json["nodes"][0]["links"];
    assert_eq!(
        keys(links),
        ["outgoing", "incoming", "left_out", "omitted"].into(),
        "{json}"
    );
    assert_eq!(links["omitted"], 0, "{json}");
    assert_eq!(
        links["left_out"],
        serde_json::json!({"generated": 0, "tier3": 0})
    );
    for link in links["outgoing"]
        .as_array()
        .unwrap()
        .iter()
        .chain(links["incoming"].as_array().unwrap())
    {
        assert_eq!(
            keys(link),
            [
                "type", "origin", "at", "name", "written", "path", "line", "state", "reason"
            ]
            .into(),
            "{link}"
        );
    }
    let canon = links["incoming"]
        .as_array()
        .unwrap()
        .iter()
        .find(|link| link["type"] == "canon")
        .expect("the canon link in JSON");
    assert_eq!(
        canon,
        &serde_json::json!({
            "type": "canon", "origin": "frontmatter", "at": null, "name": "DEC-0023",
            "written": "docs/spec/movement/stamina.md#regeneration",
            "path": "docs/records/DEC/DEC-0023.md", "line": 6, "state": "resolved",
            "reason": null
        })
    );
    let total =
        links["outgoing"].as_array().unwrap().len() + links["incoming"].as_array().unwrap().len();
    assert_eq!(total, link_lines(&run.stdout).len());
}

/// AC-09: spec-b's incoming links land through legacy Cyrillic forms, a
/// path-form `canon:` with a Cyrillic slug and a reference-form `canon:`.
/// ADR-0002 is Tier 3 (`superseded-by`), so its `canon` link is listed
/// only with `--archive` (deviation 5 of the developer's report; the AC's
/// "both decisions" holds with `--archive`). M: `dst_id` compared as text.
#[test]
fn spec_b_links_through_legacy_forms_and_slugs() {
    let scratch = Scratch::new("links-b");
    let home = scratch.home("h");
    let root = scratch.copy("spec-b", "copy");
    let run = spec30(&home, &root, &["show", "REQ-002", "--links"]);
    run.code(0);
    assert_block_shape(
        &run.stdout,
        &read(&root, "docs/records/REQ/REQ-002.md"),
        "REQ-002",
    );
    let lines = link_lines(&run.stdout);
    assert!(
        lines.contains(
            &"  in derived_from MOD-CLI | docs/spec/cli.md:10 | as \u{0422}\u{0420}\u{0411}-002"
        ),
        "{}",
        run.show()
    );
    for prefix in [
        "  in mentions docs/features/dry-run.md | docs/features/dry-run.md:6",
        "  in mentions docs/features/dry-run.md | docs/features/dry-run.md:12",
        "  in mentions CRIT-01 | docs/features/dry-run.md:18",
    ] {
        assert!(
            lines.iter().any(|line| line.starts_with(prefix)),
            "missing {prefix:?}\n{}",
            run.show()
        );
    }

    let slug = "docs/spec/cli.md#\u{043a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{0430}-sync";
    let adr1 = format!("  in canon ADR-0001 | docs/records/ADR/ADR-0001.md:7 | as {slug}");
    let adr2 = "  in canon ADR-0002 | docs/records/ADR/ADR-0002.md:8 | as MOD-CLI#CMD-SYNC";
    let run = spec30(&home, &root, &["show", "MOD-CLI#CMD-SYNC", "--links"]);
    run.code(0);
    let lines = link_lines(&run.stdout);
    assert!(lines.contains(&adr1.as_str()), "{}", run.show());
    assert!(!lines.contains(&adr2), "{}", run.show());
    assert!(
        links_block(&run.stdout)
            .last()
            .unwrap()
            .ends_with("; left out: 1 archived (--archive)"),
        "{}",
        run.show()
    );
    let run = spec30(
        &home,
        &root,
        &["show", "MOD-CLI#CMD-SYNC", "--links", "--archive"],
    );
    run.code(0);
    let lines = link_lines(&run.stdout);
    assert!(lines.contains(&adr1.as_str()), "{}", run.show());
    assert!(lines.contains(&adr2), "{}", run.show());
    // The legacy Cyrillic question mention, outgoing, resolved to QN-07.
    assert!(
        lines.contains(
            &"  out mentions QN-07 | docs/spec/cli.md:21 | as \u{0412}\u{041e}\u{041f}-7"
        ),
        "{}",
        run.show()
    );
    // The look-alike mention dangles, outgoing, in its nested section.
    assert!(
        lines.iter().any(|line| line.starts_with(
            "  out mentions R\u{0415}Q-003 | docs/spec/cli.md:25 | at FLAG-DRY-RUN | dangling: "
        )),
        "{}",
        run.show()
    );
}

/// AC-09, the "Endpoints" rule: `status: superseded-by X` in D is the edge
/// X `--supersedes-->` D written at D's `status:` line, its liveness by D's
/// file. On X (or the document holding X as a nested section, ` | at X`)
/// it is outgoing, listed only when D's file is admitted, else counted in
/// `left_out`; on D it is incoming. ` | as` compares the written form with
/// X. spec-a's DEC-0023/DEC-0007 and spec-b's ADR-0001/ADR-0002 (both
/// superseded decisions Tier 3). M: source-naming edges not listed on X.
#[test]
fn superseded_by_is_outgoing_on_the_superseding_node() {
    let scratch = Scratch::new("links-superseded");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let own = "  out supersedes DEC-0007 | docs/records/DEC/DEC-0023.md:7";
    let named = "  out supersedes DEC-0007 | docs/records/DEC/DEC-0007.md:4";

    // Without `--archive`: DEC-0007's file is Tier 3, so the edge written
    // there is counted (with DEC-0007's mention of DEC-0023), not listed.
    let run = spec30(&home, &root, &["show", "DEC-0023", "--links"]);
    run.code(0);
    let lines = link_lines(&run.stdout);
    assert!(lines.contains(&own), "{}", run.show());
    assert!(!lines.contains(&named), "{}", run.show());
    assert_eq!(
        *links_block(&run.stdout).last().unwrap(),
        "  links 4 out, 2 in; left out: 2 archived (--archive)",
        "{}",
        run.show()
    );
    let json = spec30(&home, &root, &["--json", "show", "DEC-0023", "--links"]).json();
    assert_eq!(
        json["nodes"][0]["links"]["left_out"],
        serde_json::json!({"generated": 0, "tier3": 2}),
        "{json}"
    );

    // With `--archive`: listed outgoing, written at DEC-0007's line 4.
    let run = spec30(&home, &root, &["show", "DEC-0023", "--links", "--archive"]);
    run.code(0);
    let lines = link_lines(&run.stdout);
    assert!(lines.contains(&own), "{}", run.show());
    assert!(lines.contains(&named), "{}", run.show());
    assert_eq!(
        *links_block(&run.stdout).last().unwrap(),
        "  links 5 out, 3 in",
        "{}",
        run.show()
    );
    assert_block_shape(
        &run.stdout,
        &read(&root, "docs/records/DEC/DEC-0023.md"),
        "DEC-0023 --archive",
    );
    let json = spec30(
        &home,
        &root,
        &["--json", "show", "DEC-0023", "--links", "--archive"],
    )
    .json();
    let links = &json["nodes"][0]["links"];
    assert_eq!(
        links["left_out"],
        serde_json::json!({"generated": 0, "tier3": 0})
    );
    let superseding: Vec<&Value> = links["outgoing"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|link| link["type"] == "supersedes")
        .collect();
    assert_eq!(
        superseding,
        [
            &serde_json::json!({
                "type": "supersedes", "origin": "frontmatter", "at": null,
                "name": "DEC-0007", "written": "DEC-0023",
                "path": "docs/records/DEC/DEC-0007.md", "line": 4,
                "state": "resolved", "reason": null
            }),
            &serde_json::json!({
                "type": "supersedes", "origin": "frontmatter", "at": null,
                "name": "DEC-0007", "written": "DEC-0007",
                "path": "docs/records/DEC/DEC-0023.md", "line": 7,
                "state": "resolved", "reason": null
            }),
        ],
        "{json}"
    );
    assert!(
        !links["incoming"]
            .as_array()
            .unwrap()
            .iter()
            .any(|link| link["type"] == "supersedes"),
        "{json}"
    );

    // On the superseded one (a non-live REF, its file admitted): incoming,
    // both the `status:` edge and DEC-0023's `supersedes:`.
    for args in [
        &["show", "DEC-0007", "--links"][..],
        &["show", "DEC-0007", "--links", "--archive"],
    ] {
        let run = spec30(&home, &root, args);
        run.code(0);
        let lines = link_lines(&run.stdout);
        for want in [
            "  in supersedes DEC-0023 | docs/records/DEC/DEC-0007.md:4",
            "  in supersedes DEC-0023 | docs/records/DEC/DEC-0023.md:7",
        ] {
            assert!(lines.contains(&want), "{want}\n{}", run.show());
        }
        assert!(
            !lines
                .iter()
                .any(|line| line.starts_with("  out supersedes")),
            "{}",
            run.show()
        );
    }

    // Scratch: X a nested section (` | at`), X through a legacy alias
    // (` | as`); both D files live (class canon).
    write(
        &root,
        "docs/records/old-regen.md",
        "---\nclass: canon\nstatus: superseded-by RULE-STAM-REGEN\n---\n\n# Old regen\n",
    );
    write(
        &root,
        "docs/records/old-q.md",
        "---\nclass: canon\nstatus: superseded-by QST-031\n---\n\n# Old q\n",
    );
    let run = spec30(&home, &root, &["show", "MEC-STAMINA", "--links"]);
    run.code(0);
    assert!(
        link_lines(&run.stdout).contains(
            &"  out supersedes docs/records/old-regen.md | docs/records/old-regen.md:3 | at RULE-STAM-REGEN"
        ),
        "{}",
        run.show()
    );
    let run = spec30(&home, &root, &["show", "RULE-STAM-REGEN", "--links"]);
    run.code(0);
    assert!(
        link_lines(&run.stdout)
            .contains(&"  out supersedes docs/records/old-regen.md | docs/records/old-regen.md:3"),
        "{}",
        run.show()
    );
    let run = spec30(&home, &root, &["show", "Q-031", "--links"]);
    run.code(0);
    assert!(
        link_lines(&run.stdout).contains(
            &"  out supersedes docs/records/old-q.md | docs/records/old-q.md:3 | as QST-031"
        ),
        "{}",
        run.show()
    );
    let run = spec30(&home, &root, &["show", "docs/records/old-q.md", "--links"]);
    run.code(0);
    assert_eq!(
        link_lines(&run.stdout),
        ["  in supersedes Q-031 | docs/records/old-q.md:3 | as QST-031"],
        "{}",
        run.show()
    );

    // spec-b: ADR-0002 is `superseded-by ADR-0001`.
    let root = scratch.copy("spec-b", "copy-b");
    let named = "  out supersedes ADR-0002 | docs/records/ADR/ADR-0002.md:4";
    let run = spec30(&home, &root, &["show", "ADR-0001", "--links"]);
    run.code(0);
    let lines = link_lines(&run.stdout);
    assert!(
        !lines.iter().any(|line| line.contains(" supersedes ")),
        "{}",
        run.show()
    );
    assert!(
        links_block(&run.stdout)
            .last()
            .unwrap()
            .ends_with("; left out: 2 archived (--archive)"),
        "{}",
        run.show()
    );
    let run = spec30(&home, &root, &["show", "ADR-0001", "--links", "--archive"]);
    run.code(0);
    let lines = link_lines(&run.stdout);
    assert!(lines.contains(&named), "{}", run.show());
    assert!(
        !links_block(&run.stdout)
            .last()
            .unwrap()
            .contains("left out"),
        "{}",
        run.show()
    );
    let run = spec30(&home, &root, &["show", "ADR-0002", "--links"]);
    run.code(0);
    assert!(
        link_lines(&run.stdout)
            .contains(&"  in supersedes ADR-0001 | docs/records/ADR/ADR-0002.md:4"),
        "{}",
        run.show()
    );
}

/// `(path, line)` of every dangling link `show --links` prints, outgoing
/// or incoming, for each live document of `root`, sorted (a multiset: a
/// link listed on two documents would count twice).
fn links_dangling(
    home: &Path,
    root: &Path,
    config: &specengine_core::ProjectConfig,
) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for file in md_files(&root.join("docs")) {
        let path = format!("docs/{file}");
        let parsed = specengine_core::parse(&path, &read(root, &path), &config.scheme);
        if !specengine_core::check::is_live(&parsed) {
            continue;
        }
        let run = spec30(home, root, &["--json", "show", &path, "--links"]);
        run.code(0);
        let json = run.json();
        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1, "{path}: {json}");
        let links = &nodes[0]["links"];
        for link in links["outgoing"]
            .as_array()
            .unwrap()
            .iter()
            .chain(links["incoming"].as_array().unwrap())
        {
            if link["state"] == "dangling" {
                out.push((
                    link["path"].as_str().unwrap().to_owned(),
                    link["line"].as_u64().unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}

/// `(path, line)` of `spec check --json`'s `ref-dangling`,
/// `mention-dangling`, `link-dangling` and `canon-file` findings, `parent:`
/// aside, sorted.
fn check_dangling(home: &Path, root: &Path) -> Vec<(String, u64)> {
    let json = spec30(home, root, &["--json", "check"]).json();
    let mut out: Vec<(String, u64)> = json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| {
            [
                "ref-dangling",
                "mention-dangling",
                "link-dangling",
                "canon-file",
            ]
            .contains(&finding["code"].as_str().unwrap())
                && !finding["message"].as_str().unwrap().starts_with("`parent`")
        })
        .map(|finding| {
            (
                finding["path"].as_str().unwrap().to_owned(),
                finding["line"].as_u64().unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

/// AC-10: over every live document of both fixtures (plus scratch
/// additions: an inline `MEC-STAMINA-based` / `GLS-worktree-based` that
/// resolves through the name-shape fallback, dangling references, file
/// links and a `canon:` at a non-canon file, a class-less `status:
/// superseded-by` naming nothing, a dangling `parent:` set aside), the
/// links `--links` marks dangling, out or in, are, by (path, line) where
/// written, exactly the check's dangling findings. M: no name-shape
/// fallback; outgoing only.
#[test]
fn dangling_links_agree_with_the_check() {
    for fixture in ["spec-a", "spec-b"] {
        let scratch = Scratch::new("links-parity");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let config =
            specengine_core::ProjectConfig::from_toml(&read_text(&root, "specengine.toml"))
                .expect("config");
        // The unmodified fixture first.
        assert_eq!(
            links_dangling(&home, &root, &config),
            check_dangling(&home, &root),
            "{fixture}"
        );
        let fallback = if fixture == "spec-a" {
            write(
                &root,
                "docs/records/DEC/DEC-0060.md",
                "---\nid: DEC-0060\nclass: decision\nstatus: accepted\ndate: 2026-09-02\ncanon: docs/features/stamina-tuning.md#acceptance\nlinks:\n  depends_on: [MEC-NOWHERE]\n---\n\n# Feature as canon\n\nBased on MEC-STAMINA-based tuning, R-98 and R-99, see [gone](../../spec/gone.md).\n",
            );
            replace(
                &root,
                "docs/spec/movement/sprint.md",
                "parent: DOM-MOVEMENT",
                "parent: DOM-NOWHERE",
            );
            (
                "docs/records/DEC/DEC-0060.md",
                "MEC-STAMINA-based",
                "MEC-STAMINA",
            )
        } else {
            write(
                &root,
                "docs/spec/pull.md",
                "---\nid: CMD-PULL\nclass: canon\nparent: MOD-NOWHERE\nlinks:\n  depends_on: [MOD-NOWHERE, \u{0422}\u{0420}\u{0411}-009]\n---\n\n# Pull\n\nUses the GLS-worktree-based layout, REQ-404 and [gone](gone.md).\n",
            );
            ("docs/spec/pull.md", "GLS-worktree-based", "GLS-worktree")
        };
        // A class-less document superseded by nothing: the edge dangles at
        // its source, written at D's `status:` line (incoming on D).
        let (note, missing) = if fixture == "spec-a" {
            ("docs/records/note.md", "DEC-0404")
        } else {
            ("docs/note.md", "ADR-0404")
        };
        write(
            &root,
            note,
            format!("---\nstatus: superseded-by {missing}\n---\n\n# Note\n\nPlain.\n"),
        );
        let from_links = links_dangling(&home, &root, &config);
        let from_check = check_dangling(&home, &root);
        assert!(from_check.len() >= 5, "{fixture}: {from_check:?}");
        assert!(
            from_check.contains(&(note.to_owned(), 2)),
            "{fixture}: {from_check:?}"
        );
        assert_eq!(from_links, from_check, "{fixture}");
        let run = spec30(&home, &root, &["show", note, "--links"]);
        run.code(0);
        assert!(
            link_lines(&run.stdout)
                .iter()
                .any(|line| line.starts_with(&format!(
                    "  in supersedes {missing} | {note}:2 | dangling: "
                ))),
            "{fixture}\n{}",
            run.show()
        );
        // The fallback form is resolved, named by the node it lands on.
        let (path, written, name) = fallback;
        let json = spec30(&home, &root, &["--json", "show", path, "--links"]).json();
        let link = json["nodes"][0]["links"]["outgoing"]
            .as_array()
            .unwrap()
            .iter()
            .find(|link| link["written"] == written)
            .unwrap_or_else(|| panic!("{fixture}: no {written} link: {json}"));
        assert_eq!(link["state"], "resolved", "{link}");
        assert_eq!(link["name"], name, "{link}");
    }
}

/// AC-11: a generated file and a Tier 3 file linking to MEC-STAMINA are
/// not incoming; `left_out {generated: 1, tier3: 1}` and the summary
/// suffix; `--archive` admits the Tier 3 one, never the generated one;
/// spec-b's generated glossary page alike; `show REQ-002 --archive` is
/// exit 2. M: no live filter.
#[test]
fn generated_and_archived_sources_are_left_out() {
    let scratch = Scratch::new("links-live");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    write(
        &root,
        "docs/records/gen.md",
        "---\nclass: generated\ngenerator: g\nsource: s\n---\n\n# Gen\n\nDepends on MEC-STAMINA.\n",
    );
    write(
        &root,
        "docs/records/DEC/DEC-0050.md",
        "---\nid: DEC-0050\nclass: decision\nstatus: rejected\ndate: 2026-01-01\nlinks:\n  depends_on: [MEC-STAMINA]\n---\n\n# Old\n",
    );
    let run = spec30(&home, &root, &["show", "MEC-STAMINA", "--links"]);
    run.code(0);
    let lines = link_lines(&run.stdout);
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("DEC-0050") || line.contains("docs/records/gen.md")),
        "{}",
        run.show()
    );
    assert!(
        links_block(&run.stdout)
            .last()
            .unwrap()
            .ends_with("; left out: 1 generated, 1 archived (--archive)"),
        "{}",
        run.show()
    );
    let json = spec30(&home, &root, &["--json", "show", "MEC-STAMINA", "--links"]).json();
    assert_eq!(
        json["nodes"][0]["links"]["left_out"],
        serde_json::json!({"generated": 1, "tier3": 1}),
        "{json}"
    );

    let run = spec30(
        &home,
        &root,
        &["show", "MEC-STAMINA", "--links", "--archive"],
    );
    run.code(0);
    let lines = link_lines(&run.stdout);
    assert!(
        lines.contains(&"  in depends_on DEC-0050 | docs/records/DEC/DEC-0050.md:7"),
        "{}",
        run.show()
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("docs/records/gen.md")),
        "{}",
        run.show()
    );
    let summary = *links_block(&run.stdout).last().unwrap();
    assert!(
        summary.ends_with("; left out: 1 generated") && !summary.contains("archived"),
        "{summary}"
    );
    let json = spec30(
        &home,
        &root,
        &["--json", "show", "MEC-STAMINA", "--links", "--archive"],
    )
    .json();
    assert_eq!(
        json["nodes"][0]["links"]["left_out"],
        serde_json::json!({"generated": 1, "tier3": 0}),
        "{json}"
    );
    // The impact walk: the archived edge counted, not followed, unless
    // `--archive`.
    let run = spec30(&home, &root, &["graph", "MEC-STAMINA", "--impact"]);
    run.code(0);
    assert!(!run.stdout.contains("DEC-0050"), "{}", run.show());
    assert!(
        summary_line(&run.stdout).ends_with("; left out: 1 archived (--archive)"),
        "{}",
        run.show()
    );
    let run = spec30(
        &home,
        &root,
        &["graph", "MEC-STAMINA", "--impact", "--archive"],
    );
    run.code(0);
    assert!(
        run.stdout
            .contains("1 DEC-0050 | decision | Old | docs/records/DEC/DEC-0050.md:1 | archived\n"),
        "{}",
        run.show()
    );

    // spec-b: its generated glossary page mentions GLS-worktree.
    let root = scratch.copy("spec-b", "copy-b");
    for archive in [false, true] {
        let mut args = vec!["show", "GLS-worktree", "--links"];
        if archive {
            args.push("--archive");
        }
        let run = spec30(&home, &root, &args);
        run.code(0);
        assert!(!run.stdout.contains("GLS-task-branch"), "{}", run.show());
        assert!(
            links_block(&run.stdout)
                .last()
                .unwrap()
                .ends_with("; left out: 1 generated"),
            "{}",
            run.show()
        );
    }
    for args in [
        &["show", "REQ-002", "--archive"][..],
        &["--json", "show", "REQ-002", "--archive"],
    ] {
        let run = spec30(&home, &root, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
        // One line naming the fix, not clap's usage block.
        assert_eq!(run.stderr.lines().count(), 1, "{}", run.show());
        assert!(
            run.stderr.starts_with("spec: ")
                && run.stderr.contains("--archive")
                && run.stderr.contains("--links"),
            "{}",
            run.show()
        );
    }
}

/// The live rule in the tree: a generated and a Tier 3 child document
/// are left out with their subtrees and counted; `--archive` lists the
/// Tier 3 one (` | archived`), never the generated one.
#[test]
fn the_tree_leaves_out_generated_and_archived_children() {
    let scratch = Scratch::new("links-live-tree");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    write(
        &root,
        "docs/spec/gen.md",
        "---\nclass: generated\ngenerator: g\nsource: s\nparent: DOM-GAME\n---\n\n# Gen\n",
    );
    write(
        &root,
        "docs/spec/movement/old.md",
        "---\nclass: spec\nstatus: shipped\nscope: [x]\nparent: DOM-MOVEMENT\n---\n\n# Old feature\n\n## Part {#RULE-OLD}\n\nx\n",
    );
    let run = spec30(&home, &root, &["tree"]);
    run.code(0);
    assert_eq!(item_lines(&run.stdout).len(), 10, "{}", run.show());
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 10, roots 1; left out: 1 generated, 1 archived (--archive)"
    );
    let json = spec30(&home, &root, &["--json", "tree"]).json();
    assert_eq!(
        json["left_out"],
        serde_json::json!({"generated": 1, "tier3": 1})
    );
    let run = spec30(&home, &root, &["tree", "--archive"]);
    run.code(0);
    let depths = tree_depths(&run.stdout);
    assert_eq!(
        depths[4],
        (2, "docs/spec/movement/old.md".to_owned()),
        "{}",
        run.show()
    );
    assert_eq!(depths[5], (3, "RULE-OLD".to_owned()), "{}", run.show());
    assert!(
        item_lines(&run.stdout)[4].ends_with(" | status shipped | archived"),
        "{}",
        run.show()
    );
    assert!(!run.stdout.contains("docs/spec/gen.md"), "{}", run.show());
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 12, roots 1; left out: 1 generated"
    );
    // A non-live ROOT is still answered.
    let run = spec30(&home, &root, &["tree", "docs/spec/movement/old.md"]);
    run.code(0);
    assert!(
        item_lines(&run.stdout)[0].ends_with(" | archived"),
        "{}",
        run.show()
    );
    let json: Value = spec30(
        &home,
        &root,
        &["--json", "tree", "docs/spec/movement/old.md"],
    )
    .json();
    assert_eq!(json["nodes"][0]["archived"], true, "{json}");
}
