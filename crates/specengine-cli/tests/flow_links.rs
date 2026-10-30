//! docs/features/flow-links.md (RM-0042, ADR-0055), the `spec` binary over
//! `fixtures/flow-relay` (an invented game: a retry loop RUN, FALL, RUN and
//! a round loop DRAW, RUN, SCORE, DRAW) and `fixtures/flow-hire` (a hired
//! bicycle, no game word), each in its own scratch `HOME`; the fixtures
//! are only read:
//!
//! - AC-01: `spec check` clean on each, the follows walk ending at `nodes
//!   5, edges 6`, the same bytes from copies written in opposite file
//!   orders. M1 (`depends_cycle` also reading `follows`) turns
//!   `ac01_*_is_clean` red.
//! - AC-02: each item rule alone: exit 1, `blocked`, one `frontmatter-type`
//!   with its `<problem>` at the item's line, `norm_hash` null; a mapping
//!   under `uses` is today's message. M2, M3.
//! - AC-03: `spec show --json` gives the golden `norm_hash`; a changed
//!   label moves it, its spelling does not. M4, M5.
//! - AC-04: `spec graph --impact` and `spec impact`. M6, M7.
//! - AC-05: `spec bundle --json` neighbours. M8, M9.
//! - AC-06: the labels on `spec graph` and `spec show --links`, both as
//!   text and JSON; a plain edge has neither key; nor has any edge of
//!   spec-a, spec-b, spec-c. M10 is `daemon_keys.rs`.
//! - AC-07: both projects from `[ids]` alone; no source literal holds a
//!   word of either. M11, M12 are `norm_recipe.rs`, `no_domain.rs`.
//! - "Exits": the exit codes of the spec's table.
//!
//! AC-08 is the store's `format.rs` and this crate's `bundle_config.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::bundle::{blake3_hex, bundle_json, has_word, item, literals, names, via};
use common::graph::{distances, graph_edges, keys, link_lines, spec30, summary_line};
use common::{
    Run, Scratch, copy_dir, fixture, index, md_files, read, read_text, replace, repository_root,
    snapshot, write,
};
use serde_json::{Value, json};

const RELAY: &str = "flow-relay";
const HIRE: &str = "flow-hire";
const RUN_DOC: &str = "docs/spec/leg-run.md";

/// `follows[1]` of `leg-run.md`, whole line.
const SECOND_ITEM: &str = "    - {to: LEG-FALL, kind: repeat, when: \"the torch is relit\"}\n";

/// Its line in the fixture's header. The spec's "Outputs" sample prints
/// `:9`, an illustration: `hire.md`'s five keys put `links:` at 7.
const SECOND_ITEM_LINE: usize = 10;

/// The golden `norm_hash` of LEG-RUN: BLAKE3 of the input built by hand in
/// [`leg_run_input`], from a third implementation of the hash (a one-chunk
/// BLAKE3 outside the repository) and from [`blake3_hex`].
const LEG_RUN_HASH: &str = "b3:122ebf283a692b2a8577ba0829f67697e9d13eaa6e1a1d9daa2ea622409a9fb8";

/// The same for the other labelled shapes, on each project.
const GOLDEN: [(&str, &str, &str); 4] = [
    ("flow-relay", "LEG-RUN", LEG_RUN_HASH),
    (
        "flow-relay",
        "LEG-SCORE",
        "b3:fc4fa706e6bc534a9a8731e4d81445b0b63cb2782c8bdd093c93f903336dc2c4",
    ),
    (
        "flow-hire",
        "JRN-METER",
        "b3:ae4fb549a784ccfce79d0224370eb1d5635e5c5ac5fef63657f4336eeebfb3d2",
    ),
    (
        "flow-hire",
        "JRN-RETURN",
        "b3:0f82d821a89b134e9d129e074dd001817028f59b50b51988a765dc22e5d3ff56",
    ),
];

/// A `follows` item of `leg-run.md` under the spec's item rules: the item
/// and `<problem>` verbatim, the rule applied alone.
fn the_eight_rules() -> Vec<(String, &'static str)> {
    vec![
        (
            "{to: LEG-FALL, by: me}".to_owned(),
            "unknown key `by`: an item takes `to`, `when`, `kind`",
        ),
        ("{kind: repeat}".to_owned(), "no `to`"),
        ("{to: 5}".to_owned(), "`to` is not a string"),
        (
            "{to: LEG-FALL, kind: loop}".to_owned(),
            "`kind` `loop` is none of `alternative`, `parallel`, `repeat`",
        ),
        (
            "{to: LEG-FALL, when: 5}".to_owned(),
            "`when` is not a string",
        ),
        ("{to: LEG-FALL, when: \"  \"}".to_owned(), "`when` is blank"),
        (
            format!("{{to: LEG-FALL, when: \"{}\"}}", "x".repeat(257)),
            "`when` is over 256 bytes",
        ),
        (
            "{to: LEG-FALL, when: \"a\\tb\"}".to_owned(),
            "`when` holds a control character",
        ),
    ]
}

/// The lines of stdout that are findings: everything before the summary.
fn finding_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| !line.starts_with("spec check "))
        .collect()
}

/// The summary line of `spec check`.
fn check_summary(stdout: &str) -> &str {
    stdout
        .lines()
        .find(|line| line.starts_with("spec check "))
        .unwrap_or_else(|| panic!("no summary in {stdout:?}"))
}

/// `spec --json show ID`'s `norm_hash`.
fn norm_hash(home: &Path, root: &Path, id: &str) -> Value {
    let run = spec30(home, root, &["--json", "show", id]);
    run.code(0);
    run.json()["nodes"][0]["norm_hash"].clone()
}

/// A copy of `name` whose documents are written and indexed one at a time,
/// forward or reversed, so its database holds rows in that order.
fn written_in_order(
    scratch: &Scratch,
    name: &str,
    dir: &str,
    home: &Path,
    reverse: bool,
) -> PathBuf {
    let source = scratch.dir(&format!("source-{dir}"));
    copy_dir(&fixture(name), &source);
    let root = scratch.dir(dir);
    write(&root, "specengine.toml", read(&source, "specengine.toml"));
    let mut files: Vec<String> = md_files(&source.join("docs"))
        .into_iter()
        .map(|file| format!("docs/{file}"))
        .collect();
    if reverse {
        files.reverse();
    }
    index(home, &root);
    for file in files {
        write(&root, &file, read(&source, &file));
        index(home, &root);
    }
    root
}

/// `(code, stdout, stderr)`, for a byte comparison.
fn bytes_of(run: &Run) -> (i32, String, String) {
    (run.code, run.stdout.clone(), run.stderr.clone())
}

// ------------------------------------------------------------------- AC-01

/// AC-01, AC-07: one function, each project: exit 0, one line, `clean`,
/// not a finding even under `--debt`, on the fixture itself through
/// `--root`, the fixture untouched. M1.
fn corpus_is_clean(name: &str) {
    let scratch = Scratch::new("flow-check");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let root = fixture(name);
    let before = snapshot(&root);
    let root_text = root.to_str().expect("UTF-8 root");

    let run = spec30(&home, &cwd, &["check", "--root", root_text]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(
        finding_lines(&run.stdout),
        Vec::<&str>::new(),
        "{}",
        run.show()
    );
    let summary = check_summary(&run.stdout);
    assert!(summary.ends_with(" \u{2014} clean"), "{summary}");
    assert!(
        summary.contains(" 8 documents, 0 errors, 0 warnings,"),
        "{summary}"
    );

    let run = spec30(&home, &cwd, &["check", "--debt", "--root", root_text]);
    run.code(0);
    assert_eq!(
        finding_lines(&run.stdout),
        Vec::<&str>::new(),
        "{}",
        run.show()
    );

    let run = spec30(&home, &cwd, &["--json", "check", "--root", root_text]);
    run.code(0);
    let json = run.json();
    assert_eq!(json["verdict"], "clean", "{json}");
    assert_eq!(json["findings"], json!([]), "{json}");

    assert_eq!(snapshot(&root), before, "the fixture is only read");
}

#[test]
fn ac01_the_game_project_is_clean_with_its_two_loops() {
    corpus_is_clean(RELAY);
}

#[test]
fn ac01_the_hire_project_is_clean_with_its_retry() {
    corpus_is_clean(HIRE);
}

/// AC-01: the follows walk over both loops ends, exit 0, the spec's last
/// line, each node once, on the fixture through `--root`; the whole text
/// is pinned. M: no visited set (the deadline kills the run).
#[test]
fn ac01_the_follows_walk_ends_at_five_nodes_and_six_edges() {
    let scratch = Scratch::new("flow-walk");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let root = fixture(RELAY);
    let root_text = root.to_str().expect("UTF-8 root");
    let before = snapshot(&root);
    let run = spec30(
        &home,
        &cwd,
        &[
            "--root", root_text, "graph", "LEG-RUN", "--impact", "--type", "follows",
        ],
    );
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(
        run.stdout,
        "\
0 LEG-RUN | leg | The run | docs/spec/leg-run.md:1
1 LEG-FALL | leg | The fall | docs/spec/leg-fall.md:1
1 LEG-SCORE | leg | The score | docs/spec/leg-score.md:1
2 LEG-DRAW | leg | The draw | docs/spec/leg-draw.md:1
2 LEG-END | leg | The end | docs/spec/leg-end.md:1
LEG-DRAW --follows--> LEG-SCORE | docs/spec/leg-draw.md:10 | kind repeat | when rounds remain
LEG-END --follows--> LEG-SCORE | docs/spec/leg-end.md:9 | kind alternative | when no round remains
LEG-FALL --follows--> LEG-RUN | docs/spec/leg-fall.md:9 | kind alternative | when the torch goes out
LEG-RUN --follows--> LEG-DRAW | docs/spec/leg-run.md:9
LEG-RUN --follows--> LEG-FALL | docs/spec/leg-run.md:10 | kind repeat | when the torch is relit
LEG-SCORE --follows--> LEG-RUN | docs/spec/leg-score.md:9 | when the runner arrives
nodes 5, edges 6
",
        "{}",
        run.show()
    );
    assert_eq!(snapshot(&root), before, "the fixture is only read");
}

/// AC-01: the same bytes from copies written in opposite file orders, at
/// different roots under their own `HOME`, for the walk the spec names and
/// for every read a label reaches; text and JSON, stdout, stderr, exit.
/// M: `HashMap` or rowid order.
#[test]
fn ac01_opposite_file_orders_give_the_same_bytes() {
    for (name, id) in [(RELAY, "LEG-RUN"), (HIRE, "JRN-RETURN")] {
        let scratch = Scratch::new("flow-order");
        let (home_a, home_b) = (scratch.home("a"), scratch.home("b"));
        let forward = written_in_order(&scratch, name, "forward", &home_a, false);
        let reverse = written_in_order(&scratch, name, "other/reverse", &home_b, true);
        let reads: Vec<Vec<&str>> = vec![
            vec!["graph", id, "--impact", "--type", "follows"],
            vec!["--json", "graph", id, "--impact", "--type", "follows"],
            vec!["graph", id],
            vec!["--json", "graph", id],
            vec!["graph", id, "--impact"],
            vec!["show", id, "--links"],
            vec!["--json", "show", id, "--links"],
            vec!["impact", id],
            vec!["--json", "impact", id],
            vec!["bundle", id],
            vec!["--json", "bundle", id],
            vec!["check"],
            vec!["--json", "check"],
        ];
        for args in reads {
            let one = spec30(&home_a, &forward, &args);
            let other = spec30(&home_b, &reverse, &args);
            assert_eq!(bytes_of(&one), bytes_of(&other), "{name}: {args:?}");
            assert!(
                !one.stdout.contains(forward.to_str().unwrap()),
                "no absolute root: {}",
                one.show()
            );
            assert_eq!(
                bytes_of(&spec30(&home_a, &forward, &args)),
                bytes_of(&one),
                "{name}: a repeat run: {args:?}"
            );
        }
        if name == RELAY {
            let run = spec30(
                &home_b,
                &reverse,
                &["graph", id, "--impact", "--type", "follows"],
            );
            assert_eq!(run.stdout.lines().last(), Some("nodes 5, edges 6"));
        }
    }
}

/// Cycles included, the reads answer: a default walk over both loops ends
/// and lists each edge of a followed type; `--depth` bounds it.
#[test]
fn ac01_a_default_walk_over_the_loops_answers() {
    let scratch = Scratch::new("flow-default");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");
    let run = spec30(&home, &root, &["graph", "LEG-RUN"]);
    run.code(0);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 6, edges 8",
        "{}",
        run.show()
    );
    let run = spec30(&home, &root, &["graph", "LEG-RUN", "--depth", "1"]);
    run.code(0);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 4, edges 3",
        "{}",
        run.show()
    );
}

// ------------------------------------------------------------------- AC-02

/// AC-02: on a copy, each rule alone on `follows[1]`: exit 1, `blocked`,
/// one `frontmatter-type` at the item's line, its `<problem>` verbatim in
/// the spec's line, `norm_hash` null. M3 (no closed set) turns rule 4 red.
#[test]
fn ac02_each_item_rule_blocks_with_its_problem_at_the_item_s_line() {
    let scratch = Scratch::new("flow-rules");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");
    let base = read_text(&root, RUN_DOC);
    assert_eq!(norm_hash(&home, &root, "LEG-RUN"), LEG_RUN_HASH);
    for (item, problem) in the_eight_rules() {
        write(
            &root,
            RUN_DOC,
            base.replacen(SECOND_ITEM, &format!("    - {item}\n"), 1),
        );
        let run = spec30(&home, &root, &["check"]);
        run.code(1);
        assert_eq!(
            finding_lines(&run.stdout),
            [format!(
                "error  docs/spec/leg-run.md:{SECOND_ITEM_LINE}: frontmatter-type: front-matter \
                 `links`: `follows[1]`: {problem}; kept in `extra`"
            )],
            "{}",
            run.show()
        );
        let summary = check_summary(&run.stdout);
        assert!(summary.ends_with(" \u{2014} blocked"), "{summary}");
        let run = spec30(&home, &root, &["--json", "check"]);
        run.code(1);
        let json = run.json();
        assert_eq!(json["verdict"], "blocked", "{json}");
        let findings = json["findings"].as_array().expect("findings");
        assert_eq!(findings.len(), 1, "{json}");
        assert_eq!(findings[0]["code"], "frontmatter-type", "{json}");
        assert_eq!(findings[0]["line"], SECOND_ITEM_LINE, "{json}");
        assert_eq!(findings[0]["path"], RUN_DOC, "{json}");
        assert!(
            norm_hash(&home, &root, "LEG-RUN").is_null(),
            "{problem}: the document's hash is null"
        );
    }
}

/// AC-02: `uses: [{to: KIT-TORCH}]` gives today's message, at today's
/// line, the first of the `links` value; exit 1; the hash null. M2 (a
/// mapping accepted under every type) turns this red.
#[test]
fn ac02_a_mapping_under_uses_gives_today_s_message() {
    let scratch = Scratch::new("flow-uses");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");
    replace(
        &root,
        RUN_DOC,
        "uses: [KIT-TORCH]",
        "uses: [{to: KIT-TORCH}]",
    );
    let run = spec30(&home, &root, &["check"]);
    run.code(1);
    assert_eq!(
        finding_lines(&run.stdout),
        [
            "error  docs/spec/leg-run.md:8: frontmatter-type: front-matter `links` is a mapping, \
             expected a mapping of link type to references; kept in `extra`"
        ],
        "{}",
        run.show()
    );
    assert!(check_summary(&run.stdout).ends_with(" \u{2014} blocked"));
    assert!(norm_hash(&home, &root, "LEG-RUN").is_null());
}

/// "Exits", "Rules": a dangling `to` is `ref-dangling` at its line, exit
/// 1; a `to` naming no reference is `unparsed-reference`, a warning, exit
/// 0 and a null hash; a read of what names nothing is exit 1; no config is
/// 2.
#[test]
fn exits_of_the_spec_s_table() {
    let scratch = Scratch::new("flow-exits");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");
    let base = read_text(&root, RUN_DOC);

    write(
        &root,
        RUN_DOC,
        base.replacen(SECOND_ITEM, "    - kind: repeat\n      to: LEG-NOPE\n", 1),
    );
    let run = spec30(&home, &root, &["check"]);
    run.code(1);
    let lines = finding_lines(&run.stdout);
    assert_eq!(lines.len(), 1, "{}", run.show());
    assert!(
        lines[0].starts_with(&format!(
            "error  docs/spec/leg-run.md:{}: ref-dangling: ",
            SECOND_ITEM_LINE + 1
        )),
        "{}",
        run.show()
    );
    assert!(check_summary(&run.stdout).ends_with(" \u{2014} blocked"));
    // The hash stands: a dangling target is no malformed item.
    assert!(norm_hash(&home, &root, "LEG-RUN").is_string());
    // Reads answer over it.
    spec30(&home, &root, &["graph", "LEG-RUN"]).code(0);

    write(
        &root,
        RUN_DOC,
        base.replacen(
            SECOND_ITEM,
            "    - {to: \"not a reference\", kind: repeat}\n",
            1,
        ),
    );
    let run = spec30(&home, &root, &["--json", "check"]);
    run.code(0);
    let json = run.json();
    let findings = json["findings"].as_array().expect("findings");
    assert_eq!(findings.len(), 1, "{json}");
    assert_eq!(findings[0]["code"], "unparsed-reference", "{json}");
    assert_eq!(findings[0]["severity"], "warning", "{json}");
    assert_eq!(findings[0]["line"], SECOND_ITEM_LINE, "{json}");
    assert!(norm_hash(&home, &root, "LEG-RUN").is_null());

    write(&root, RUN_DOC, base);
    spec30(&home, &root, &["graph", "LEG-NOPE"]).code(1);
    spec30(&home, &root, &["show", "LEG-NOPE", "--links"]).code(1);
    spec30(&home, &root, &["impact", "LEG-NOPE"]).code(1);
    spec30(&home, &root, &["bundle", "LEG-NOPE"]).code(1);

    let elsewhere = scratch.dir("no-project");
    spec30(&home, &elsewhere, &["check"]).code(2);
    spec30(&home, &elsewhere, &["graph", "LEG-RUN"]).code(2);
}

// ------------------------------------------------------------------- AC-03

/// The hand-built `norm_input` of LEG-RUN: links byte-sorted, lengths as
/// little-endian bytes written out.
fn leg_run_input() -> Vec<u8> {
    let frame = |le: [u8; 4], text: &str| {
        assert_eq!(u32::from_le_bytes(le) as usize, text.len(), "{text}");
        let mut out = le.to_vec();
        out.extend_from_slice(text.as_bytes());
        out
    };
    let mut input = frame([0x12, 0, 0, 0], "specengine-norm/v1");
    input.extend_from_slice(&[3, 0, 0, 0]);
    input.extend(frame([0x14, 0, 0, 0], "follows ref:LEG-DRAW"));
    input.extend(frame(
        [0x32, 0, 0, 0],
        "follows ref:LEG-FALL | repeat | the torch is relit",
    ));
    input.extend(frame([0x12, 0, 0, 0], "uses ref:KIT-TORCH"));
    input.extend(frame(
        [0x42, 0, 0, 0],
        "The runner carries the lit torch down the lane to the next beacon.",
    ));
    input
}

/// AC-03: the golden hashes, on the fixture itself, in the text header and
/// JSON, over a hash this test holds itself. M4 (`rendered` ignoring
/// `edge`), M5 (`specengine-norm/v2`).
#[test]
fn ac03_the_golden_norm_hashes() {
    assert_eq!(format!("b3:{}", blake3_hex(&leg_run_input())), LEG_RUN_HASH);
    let scratch = Scratch::new("flow-golden");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    for (corpus, id, hash) in GOLDEN {
        let root = fixture(corpus);
        let root_text = root.to_str().expect("UTF-8 root");
        let run = spec30(&home, &cwd, &["--root", root_text, "--json", "show", id]);
        run.code(0);
        assert_eq!(run.json()["nodes"][0]["norm_hash"], hash, "{corpus} {id}");
        let run = spec30(&home, &cwd, &["--root", root_text, "show", id]);
        run.code(0);
        let header = run.stdout.lines().next().expect("a header");
        assert!(
            header.contains(&format!(" | norm {hash} | ")),
            "{corpus} {id}: {header}"
        );
    }
}

/// AC-03: a changed `when` or `kind` changes the hash, each row its own;
/// spelling does not: reordered keys, block style, a quoted `to`, `{to:
/// LEG-DRAW}` for `LEG-DRAW`. M4 turns the change rows red.
#[test]
fn ac03_a_changed_label_moves_the_hash_and_its_spelling_does_not() {
    let scratch = Scratch::new("flow-moves");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");
    let base = read_text(&root, RUN_DOC);
    assert_eq!(norm_hash(&home, &root, "LEG-RUN"), LEG_RUN_HASH);
    let with_item = |item: &str| base.replacen(SECOND_ITEM, &format!("    - {item}\n"), 1);

    let mut seen = BTreeSet::from([LEG_RUN_HASH.to_owned()]);
    for (row, text) in [
        (
            "a changed `when`",
            with_item("{to: LEG-FALL, kind: repeat, when: \"the torch is out\"}"),
        ),
        (
            "a changed `kind`",
            with_item("{to: LEG-FALL, kind: alternative, when: \"the torch is relit\"}"),
        ),
        ("no `when`", with_item("{to: LEG-FALL, kind: repeat}")),
        (
            "no `kind`",
            with_item("{to: LEG-FALL, when: \"the torch is relit\"}"),
        ),
        ("no label", with_item("{to: LEG-FALL}")),
    ] {
        write(&root, RUN_DOC, text);
        let hash = norm_hash(&home, &root, "LEG-RUN");
        let hash = hash.as_str().unwrap_or_else(|| panic!("{row}: null"));
        assert!(seen.insert(hash.to_owned()), "{row}: {hash} repeats");
    }

    for (row, text) in [
        (
            "reordered item keys",
            with_item("{when: \"the torch is relit\", to: LEG-FALL, kind: repeat}"),
        ),
        (
            "block style",
            with_item("to: LEG-FALL\n      kind: repeat\n      when: \"the torch is relit\""),
        ),
        (
            "a quoted `to`",
            with_item("{to: \"LEG-FALL\", kind: repeat, when: \"the torch is relit\"}"),
        ),
        (
            "`{to: LEG-DRAW}` for `LEG-DRAW`",
            base.replacen("    - LEG-DRAW\n", "    - {to: LEG-DRAW}\n", 1),
        ),
    ] {
        assert_ne!(text, base, "{row}");
        write(&root, RUN_DOC, text);
        assert_eq!(norm_hash(&home, &root, "LEG-RUN"), LEG_RUN_HASH, "{row}");
    }
}

// ------------------------------------------------------------------- AC-04

/// AC-04: `uses` incoming, `supplies` outgoing, from KIT-OIL the four
/// nodes and three edges of the spec; from LEG-RUN nothing: `follows` is
/// not followed. M6 turns the second red, M7 the first.
#[test]
fn ac04_impact_follows_uses_and_supplies_never_follows() {
    let scratch = Scratch::new("flow-impact");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let root = fixture(RELAY);
    let root_text = root.to_str().expect("UTF-8 root");
    let before = snapshot(&root);
    let graph = |args: &[&str]| {
        let mut all = vec!["--root", root_text, "graph"];
        all.extend_from_slice(args);
        let run = spec30(&home, &cwd, &all);
        run.code(0);
        run
    };

    let run = graph(&["KIT-OIL", "--impact"]);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 4, edges 3",
        "{}",
        run.show()
    );
    assert_eq!(
        distances(&run.stdout),
        [
            (0, "KIT-OIL".to_owned()),
            (1, "KIT-TORCH".to_owned()),
            (2, "LEG-FALL".to_owned()),
            (2, "LEG-RUN".to_owned()),
        ],
        "{}",
        run.show()
    );
    assert_eq!(
        graph_edges(&run.stdout),
        [
            "KIT-OIL --supplies--> KIT-TORCH | docs/spec/kit-oil.md:8",
            "LEG-FALL --uses--> KIT-TORCH | docs/spec/leg-fall.md:10",
            "LEG-RUN --uses--> KIT-TORCH | docs/spec/leg-run.md:11",
        ],
        "{}",
        run.show()
    );

    let run = graph(&["LEG-RUN", "--impact"]);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 1, edges 0",
        "{}",
        run.show()
    );

    // Asked for, `uses` and `supplies` walk their table direction; a step
    // edit reaches later steps only by `--type follows` (ADR-0055 "Cost").
    let run = graph(&["KIT-TORCH", "--impact", "--type", "uses"]);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 3, edges 2",
        "{}",
        run.show()
    );
    let run = graph(&["KIT-OIL", "--impact", "--type", "supplies"]);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 2, edges 1",
        "{}",
        run.show()
    );
    assert_eq!(snapshot(&root), before, "the fixture is only read");
}

/// AC-04: `spec impact KIT-OIL` lists those four, its own command, not
/// only the walk under it; `spec impact LEG-RUN` the step alone. M7.
#[test]
fn ac04_spec_impact_lists_the_four_nodes() {
    let scratch = Scratch::new("flow-spec-impact");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");
    let run = spec30(&home, &root, &["--json", "impact", "KIT-OIL"]);
    run.code(0);
    let json = run.json();
    let reached: Vec<(String, u64)> = json["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| {
            (
                node["id"].as_str().expect("an id").to_owned(),
                node["distance"].as_u64().expect("a distance"),
            )
        })
        .collect();
    assert_eq!(
        reached,
        [
            ("KIT-OIL".to_owned(), 0),
            ("KIT-TORCH".to_owned(), 1),
            ("LEG-FALL".to_owned(), 2),
            ("LEG-RUN".to_owned(), 2),
        ],
        "{json}"
    );
    let text = spec30(&home, &root, &["impact", "KIT-OIL"]);
    text.code(0);
    for id in ["KIT-OIL", "KIT-TORCH", "LEG-FALL", "LEG-RUN"] {
        assert!(text.stdout.contains(id), "{id}: {}", text.show());
    }
    assert!(!text.stdout.contains("LEG-DRAW"), "{}", text.show());

    let run = spec30(&home, &root, &["--json", "impact", "LEG-RUN"]);
    run.code(0);
    let json = run.json();
    assert_eq!(json["nodes"].as_array().expect("nodes").len(), 1, "{json}");
}

// ------------------------------------------------------------------- AC-05

/// `(name, via)` of the `neighbours` layer, in order.
fn neighbours(json: &Value) -> Vec<(String, Vec<(String, String)>)> {
    names(json, "neighbours")
        .into_iter()
        .map(|name| {
            let pairs = via(item(json, "neighbours", &name));
            (name, pairs)
        })
        .collect()
}

fn row(name: &str, pairs: &[(&str, &str)]) -> (String, Vec<(String, String)>) {
    (
        name.to_owned(),
        pairs
            .iter()
            .map(|(link, way)| ((*link).to_owned(), (*way).to_owned()))
            .collect(),
    )
}

/// AC-05: exactly the spec's neighbours of LEG-RUN, in its order; KIT-TORCH
/// exactly KIT-OIL (incoming `uses` stays out); KIT-OIL exactly KIT-TORCH.
/// M8 loses LEG-SCORE, M9 adds two nodes to KIT-TORCH's.
#[test]
fn ac05_the_bundle_neighbours_are_the_spec_s() {
    let scratch = Scratch::new("flow-bundle");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");

    let json = bundle_json(&home, &root, &["LEG-RUN"]);
    assert_eq!(
        neighbours(&json),
        [
            row("KIT-TORCH", &[("uses", "out")]),
            row("LEG-DRAW", &[("follows", "out")]),
            row("LEG-FALL", &[("follows", "in"), ("follows", "out")]),
            row("LEG-SCORE", &[("follows", "in")]),
        ],
        "{json}"
    );

    let json = bundle_json(&home, &root, &["KIT-TORCH"]);
    assert_eq!(
        neighbours(&json),
        [row("KIT-OIL", &[("supplies", "in")])],
        "{json}"
    );

    let json = bundle_json(&home, &root, &["KIT-OIL"]);
    assert_eq!(
        neighbours(&json),
        [row("KIT-TORCH", &[("supplies", "out")])],
        "{json}"
    );
}

/// AC-05 on the other project: the table is one for both.
#[test]
fn ac05_the_same_table_on_the_other_project() {
    let scratch = Scratch::new("flow-bundle-hire");
    let home = scratch.home("h");
    let root = scratch.copy(HIRE, "copy");
    let json = bundle_json(&home, &root, &["JRN-RETURN"]);
    assert_eq!(
        neighbours(&json),
        [
            row("FEE-RIDE", &[("supplies", "out")]),
            row("JRN-NEAREST", &[("follows", "in"), ("follows", "out")]),
            row("JRN-RIDE", &[("follows", "out")]),
            row("JRN-SCAN", &[("uses", "out")]),
        ],
        "{json}"
    );
    let json = bundle_json(&home, &root, &["JRN-SCAN"]);
    assert_eq!(neighbours(&json), [], "{json}");
}

// ------------------------------------------------------------------- AC-06

/// The edge of `spec --json graph` from `src` to `dst`.
fn edge<'a>(json: &'a Value, src: &str, dst: &str) -> &'a Value {
    json["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .find(|edge| edge["src"] == src && edge["dst"] == dst)
        .unwrap_or_else(|| panic!("no edge {src} -> {dst}: {json}"))
}

/// The keys of a plain edge: `fixtures/daemon-keys.json`'s `GraphEdge`.
fn plain_edge_keys() -> BTreeSet<String> {
    let keys_file = read_text(&repository_root(), "fixtures/daemon-keys.json");
    let keys: Value = serde_json::from_str(&keys_file).expect("daemon-keys.json");
    keys["GraphEdge"]
        .as_array()
        .expect("GraphEdge")
        .iter()
        .map(|key| key.as_str().expect("a key").to_owned())
        .collect()
}

/// AC-06: the two lines the spec prints and their JSON; a plain edge has
/// neither key, a label of one has that one. M10 is `daemon_keys.rs`.
#[test]
fn ac06_labels_on_graph_and_on_links() {
    let scratch = Scratch::new("flow-labels");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");
    let walk = ["graph", "LEG-RUN", "--impact", "--type", "follows"];
    let run = spec30(&home, &root, &walk);
    run.code(0);
    let lines = graph_edges(&run.stdout);
    assert!(
        lines.contains(
            &"LEG-RUN --follows--> LEG-FALL | docs/spec/leg-run.md:10 | kind repeat | when the \
              torch is relit"
        ),
        "{}",
        run.show()
    );

    let mut args = vec!["--json"];
    args.extend_from_slice(&walk);
    let json = spec30(&home, &root, &args).json();
    let plain_keys = plain_edge_keys();
    let labelled = edge(&json, "LEG-RUN", "LEG-FALL");
    assert_eq!(labelled["kind"], "repeat", "{json}");
    assert_eq!(labelled["when"], "the torch is relit", "{json}");
    let mut with_both = plain_keys.clone();
    with_both.extend(["kind".to_owned(), "when".to_owned()]);
    assert_eq!(
        keys(labelled)
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        with_both,
        "{json}"
    );
    let plain = edge(&json, "LEG-RUN", "LEG-DRAW");
    assert_eq!(
        keys(plain)
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        plain_keys,
        "the plain edge has neither key: {json}"
    );
    let when_only = edge(&json, "LEG-SCORE", "LEG-RUN");
    assert_eq!(when_only["when"], "the runner arrives", "{json}");
    assert!(when_only.get("kind").is_none(), "{json}");

    let run = spec30(&home, &root, &["show", "LEG-RUN", "--links"]);
    run.code(0);
    assert_eq!(
        link_lines(&run.stdout),
        [
            "  out follows LEG-DRAW | docs/spec/leg-run.md:9",
            "  out follows LEG-FALL | docs/spec/leg-run.md:10 | kind repeat | when the torch is \
             relit",
            "  out uses KIT-TORCH | docs/spec/leg-run.md:11",
            "  in follows LEG-FALL | docs/spec/leg-fall.md:9 | kind alternative | when the torch \
             goes out",
            "  in follows LEG-SCORE | docs/spec/leg-score.md:9 | when the runner arrives",
        ],
        "{}",
        run.show()
    );

    let json = spec30(&home, &root, &["--json", "show", "LEG-RUN", "--links"]).json();
    let links = &json["nodes"][0]["links"];
    let outgoing = links["outgoing"].as_array().expect("outgoing");
    let incoming = links["incoming"].as_array().expect("incoming");
    assert_eq!((outgoing.len(), incoming.len()), (3, 2), "{json}");
    assert!(outgoing[0].get("kind").is_none(), "{json}");
    assert!(outgoing[0].get("when").is_none(), "{json}");
    assert!(outgoing[2].get("kind").is_none(), "{json}");
    assert_eq!(outgoing[1]["kind"], "repeat", "{json}");
    assert_eq!(outgoing[1]["when"], "the torch is relit", "{json}");
    assert_eq!(incoming[1]["name"], "LEG-SCORE", "{json}");
    assert_eq!(incoming[1]["when"], "the runner arrives", "{json}");
    assert!(incoming[1].get("kind").is_none(), "{json}");

    // A kind with no `when`, on the other project.
    let hire = scratch.copy(HIRE, "hire");
    let run = spec30(&home, &hire, &["show", "JRN-UNLOCK", "--links"]);
    run.code(0);
    assert!(
        link_lines(&run.stdout)
            .contains(&"  in follows JRN-METER | docs/spec/jrn-meter.md:9 | kind parallel"),
        "{}",
        run.show()
    );
}

/// "Outputs": the label stands after `at` and `as` and before the state:
/// a section target, a dangling one, one of another project.
#[test]
fn ac06_a_label_stands_after_as_and_before_the_state() {
    let scratch = Scratch::new("flow-order-of-parts");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");
    let header = "---\nid: {ID}\nclass: canon\ntitle: A step\nowner: owner\n\
                  reviewed: 2026-10-01\n{LINKS}---\n\n# A step\n\n{BODY}";
    let doc = |id: &str, links: &str, body: &str| {
        header
            .replace("{ID}", id)
            .replace("{LINKS}", links)
            .replace("{BODY}", body)
    };
    write(
        &root,
        "docs/spec/leg-home.md",
        doc(
            "LEG-HOME",
            "",
            "Lead-in.\n\n## Part {#LEG-PART}\n\nThe part.\n",
        ),
    );
    write(
        &root,
        "docs/spec/leg-away.md",
        doc(
            "LEG-AWAY",
            "links:\n  follows:\n    - {to: LEG-HOME#LEG-PART, kind: repeat, when: \"x\"}\n    \
             - {to: LEG-NOPE, kind: parallel}\n    - {to: other:LEG-THERE, when: \"y\"}\n",
            "Away.\n",
        ),
    );
    let run = spec30(&home, &root, &["show", "LEG-AWAY", "--links"]);
    run.code(0);
    let lines = link_lines(&run.stdout);
    assert_eq!(lines.len(), 3, "{}", run.show());
    assert_eq!(
        lines[0],
        "  out follows LEG-PART | docs/spec/leg-away.md:9 | as LEG-HOME#LEG-PART | kind repeat | \
         when x",
        "{}",
        run.show()
    );
    assert!(
        lines[1].starts_with(
            "  out follows LEG-NOPE | docs/spec/leg-away.md:10 | kind parallel | dangling: "
        ),
        "{}",
        run.show()
    );
    assert_eq!(
        lines[2],
        "  out follows other:LEG-THERE | docs/spec/leg-away.md:11 | when y | skipped: another \
         project",
        "{}",
        run.show()
    );
    let run = spec30(&home, &root, &["show", "LEG-HOME", "--links"]);
    run.code(0);
    assert_eq!(
        link_lines(&run.stdout),
        [
            "  in follows LEG-AWAY | docs/spec/leg-away.md:9 | at LEG-PART | as \
             LEG-HOME#LEG-PART | kind repeat | when x"
        ],
        "{}",
        run.show()
    );

    let run = spec30(&home, &root, &["graph", "LEG-AWAY"]);
    run.code(0);
    let edges = graph_edges(&run.stdout);
    assert_eq!(edges.len(), 3, "{}", run.show());
    assert_eq!(
        edges[0],
        "LEG-AWAY --follows--> LEG-PART | docs/spec/leg-away.md:9 | kind repeat | when x"
    );
    assert!(
        edges[1].starts_with(
            "LEG-AWAY --follows--> LEG-NOPE | docs/spec/leg-away.md:10 | kind parallel | dangling: "
        ),
        "{}",
        run.show()
    );
    assert_eq!(
        edges[2],
        "LEG-AWAY --follows--> other:LEG-THERE | docs/spec/leg-away.md:11 | when y | skipped: \
         another project"
    );
}

/// AC-06: on spec-a, spec-b, spec-c no edge or link has either key, from
/// every ID each corpus holds. M: `kind: null` on a plain edge.
#[test]
fn ac06_no_earlier_corpus_prints_a_label_key() {
    for name in ["spec-a", "spec-b", "spec-c"] {
        let scratch = Scratch::new("flow-old");
        let home = scratch.home("h");
        let root = scratch.copy(name, "copy");
        let config =
            specengine_core::ProjectConfig::from_toml(&read_text(&root, "specengine.toml"))
                .expect("the config");
        let mut ids = BTreeSet::new();
        for file in md_files(&root.join("docs")) {
            let path = format!("docs/{file}");
            let parsed = specengine_core::parse(&path, &read(&root, &path), &config.scheme);
            ids.extend(parsed.nodes.iter().filter_map(|node| node.id.clone()));
        }
        assert!(ids.len() > 3, "{name}: {ids:?}");
        let mut links = 0;
        for id in &ids {
            for args in [
                vec!["--json", "graph", id],
                vec!["--json", "graph", id, "--impact"],
                vec!["--json", "show", id, "--links"],
            ] {
                let run = spec30(&home, &root, &args);
                if run.code != 0 {
                    continue;
                }
                let json = run.json();
                let mut items: Vec<Value> = json["edges"].as_array().cloned().unwrap_or_default();
                for node in json["nodes"].as_array().into_iter().flatten() {
                    for side in ["outgoing", "incoming"] {
                        if let Some(list) = node["links"][side].as_array() {
                            items.extend(list.iter().cloned());
                        }
                    }
                }
                links += items.len();
                for item in items {
                    // `show`'s node has its own `kind`; a link or edge none.
                    assert!(item.get("kind").is_none(), "{name} {args:?}: {item}");
                    assert!(item.get("when").is_none(), "{name} {args:?}: {item}");
                }
            }
        }
        assert!(links > 0, "{name}: no link met");
    }
}

/// "Outputs": `types` holds the 15 `LINK_TYPES` outgoing, 7 under
/// `--impact`, and an asked `follows` walks against the link.
#[test]
fn the_followed_types_are_fifteen_and_seven() {
    let scratch = Scratch::new("flow-types");
    let home = scratch.home("h");
    let root = scratch.copy(RELAY, "copy");
    let json = spec30(&home, &root, &["--json", "graph", "LEG-RUN"]).json();
    let types = json["types"].as_array().expect("types");
    assert_eq!(types.len(), 15, "{json}");
    assert!(
        types.iter().all(|item| item["direction"] == "out"),
        "{json}"
    );
    for name in ["follows", "uses", "supplies"] {
        assert!(
            types.contains(&json!({"type": name, "direction": "out"})),
            "{name}: {json}"
        );
    }
    let json = spec30(&home, &root, &["--json", "graph", "LEG-RUN", "--impact"]).json();
    assert_eq!(json["types"].as_array().expect("types").len(), 7, "{json}");
    let json = spec30(
        &home,
        &root,
        &[
            "--json", "graph", "LEG-RUN", "--impact", "--type", "follows",
        ],
    )
    .json();
    assert_eq!(
        json["types"],
        json!([{"type": "follows", "direction": "in"}]),
        "{json}"
    );
}

// ------------------------------------------------------------------- AC-07

/// AC-07: the same binary on the second project, from `[ids]` alone: its
/// flow, its impact, its labels.
#[test]
fn ac07_the_second_project_reads_from_ids_alone() {
    let scratch = Scratch::new("flow-genre");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let root = fixture(HIRE);
    let root_text = root.to_str().expect("UTF-8 root");
    let graph = |args: &[&str]| {
        let mut all = vec!["--root", root_text, "graph"];
        all.extend_from_slice(args);
        let run = spec30(&home, &cwd, &all);
        run.code(0);
        run
    };
    let run = graph(&["JRN-UNLOCK", "--impact", "--type", "follows"]);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 6, edges 6",
        "{}",
        run.show()
    );
    // The retry: RETURN and NEAREST reach each other, the walk ends.
    let run = graph(&["JRN-RETURN", "--impact", "--type", "follows"]);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 2, edges 2",
        "{}",
        run.show()
    );
    let run = graph(&["JRN-SCAN", "--impact"]);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 4, edges 3",
        "{}",
        run.show()
    );
    assert_eq!(
        distances(&run.stdout),
        [
            (0, "JRN-SCAN".to_owned()),
            (1, "JRN-RETURN".to_owned()),
            (1, "JRN-UNLOCK".to_owned()),
            (2, "FEE-RIDE".to_owned()),
        ],
        "{}",
        run.show()
    );
    let run = graph(&["JRN-RETURN", "--impact"]);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 2, edges 1",
        "{}",
        run.show()
    );
}

/// Every `.rs` under `dir`.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a source directory").flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// AC-07, ADR-0008: no source literal of any crate holds a prefix or a
/// kind only the flow fixtures use: the project's words stay in its
/// `specengine.toml`. M12 shows the daemon's half in `no_domain.rs`; this
/// reads every crate. M: a literal `"leg"` anywhere in `crates/*/src`.
#[test]
fn ac07_no_source_literal_holds_a_word_of_the_new_projects() {
    let root = repository_root();
    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("crates")).expect("crates").flatten() {
        let src = entry.path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    assert!(files.len() > 50, "{files:?}");
    let mut found = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("a source file");
        for (line, literal) in literals(&source) {
            for word in ["LEG", "KIT", "leg", "kit"] {
                if has_word(&literal, word) {
                    found.push(format!("{}:{line}: {literal}", file.display()));
                }
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}
