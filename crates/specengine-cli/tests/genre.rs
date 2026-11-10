//! AC-17 of docs/features/spec-cli.md (ADR-0008): the CLI knows no subject
//! domain. Its sources hold none of the words of one project's layout or
//! scheme, and the read loop answers alike on spec-a (English game design,
//! `[paths]` defaults) and spec-b (Russian CLI tooling, `roots`, legacy
//! Cyrillic aliases). The per-criterion files run both fixtures; this one
//! adds the scan and one battery across both.
//!
//! docs/features/flow-read.md AC-06: `src/flow.rs` is scanned with the rest,
//! the words of the flow fixtures' schemes join the list, and `spec flow`
//! joins the battery.
//!
//! docs/features/card-store.md AC-10: `src/card.rs` is scanned with the rest,
//! the words of `fixtures/card-press` join the list.
//!
//! docs/features/card-trace.md: `src/trace.rs` is scanned with the rest, the
//! ruling of that fixture joins the list, and `spec trace` joins the battery.
//!
//! docs/features/trace-views.md: `--stage`, `--trace` and `--marks` are read
//! here, the words of `fixtures/trace-almanac` join the list. `PART-` stays
//! out: `src/show.rs` tests a made-up scheme of its own under that prefix.
//!
//! docs/features/flow-scenario.md: `--in-flow` and `ancestors` are read in
//! `src/flow.rs`, `src/show.rs` and `src/subset.rs`, and the words of
//! `fixtures/flow-aside` join the list. `CONE-` stays out: `src/node_set.rs`
//! tests a made-up ID of its own under that prefix.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::PathBuf;

use common::{FIXTURES, Scratch, data_dir, md_files, read_text, snapshot, spec, write};

/// The words no CLI source may hold (the spec's list; pass 2a.1 adds
/// `cargo xtask`, docs/features/spec-cli-check.md AC-13; pass 2b this
/// repository's registered command prefix, docs/features/spec-cli-switch.md
/// AC-15; docs/features/roadmap-items.md this repository's plan prefix and
/// its shard; docs/features/flow-read.md the flow fixtures' prefixes and
/// kinds; docs/features/card-store.md the card fixture's;
/// docs/features/card-trace.md that fixture's ruling;
/// docs/features/trace-views.md the almanac's;
/// docs/features/flow-scenario.md the pottery's).
const FORBIDDEN: [&str; 57] = [
    "docs/",
    "ADR",
    "CLAUDE.md",
    "index.md",
    "cargo xtask",
    "cargo run -q -p specengine-cli",
    "RULE-",
    "MEC-",
    "REQ-",
    "mechanic",
    // docs/features/index-shards.md AC-11: the shard names of this
    // repository and of the fixtures' scratch configs.
    "index-archive",
    "index-canon-spec",
    "index-canon",
    "index-decisions",
    "index-crates",
    "index-records",
    "index-roadmap",
    // docs/features/roadmap-items.md SW-4: this repository's `[ids] RM`.
    "RM-",
    // docs/features/spec-cli-graph.md AC-03: no root rule by a prefix of
    // either fixture's scheme.
    "DOM-",
    "MOD-",
    "CMD-",
    // docs/features/flow-read.md AC-06: no `spec flow` line by a word of a
    // project it reads.
    "LEG-",
    "KIT-",
    "JRN-",
    "WING-",
    "LND-",
    "PLT-",
    "bookplate",
    "lending",
    // docs/features/card-store.md AC-10: no `spec card` line by a word of a
    // project it reads.
    "PRN-",
    "SHT-",
    "broadsheet",
    "print-run",
    // docs/features/card-trace.md: no `spec trace` line by a word of a
    // project it reads.
    "STY-",
    "style-ruling",
    // docs/features/trace-views.md: no `spec flow --trace`, `spec trace` or
    // `spec card list --marks` line by a word of a project it reads.
    "BOOK-",
    "STEP-",
    "yearbook",
    "committee",
    "errand",
    // docs/features/flow-scenario.md: no `spec flow --in-flow` line and no
    // `ancestors` entry by a word of the project it reads (`CONE-` aside).
    "ATL-",
    "KRUN-",
    "POT-",
    "WBAY-",
    "DESK-",
    "HUNCH-",
    "PUZZLE-",
    "EDICT-",
    "atelier",
    "kilnrun",
    "potstep",
    "workbay",
    "conerule",
    "deskpaper",
    "hunch",
    "puzzle",
    "edict",
];

fn cli_sources() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let mut stack = vec![dir];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("src").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// `file:line: word` for every forbidden word in `text` (comments and
/// strings alike: the whole text is scanned).
fn offences(name: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for word in FORBIDDEN {
            if line.contains(word) {
                found.push(format!("{name}:{}: {word}", index + 1));
            }
        }
    }
    found
}

#[test]
fn cli_sources_hold_no_project_words() {
    let files = cli_sources();
    assert!(files.len() >= 3, "the CLI sources: {files:?}");
    assert!(files.iter().any(|file| file.ends_with("main.rs")));
    assert!(files.iter().any(|file| file.ends_with("flow.rs")));
    assert!(files.iter().any(|file| file.ends_with("card.rs")));
    // docs/features/flow-authoring.md AC-04: the scaffold's paths come from
    // the default, never a literal. M: a `docs/` literal in `init.rs`.
    assert!(files.iter().any(|file| file.ends_with("init.rs")));
    let mut found = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("a UTF-8 source");
        found.extend(offences(&file.display().to_string(), &text));
    }
    assert!(
        found.is_empty(),
        "project words in the CLI sources:\n{}",
        found.join("\n")
    );
}

/// The scan sees a `"docs/"` default, in code or in a comment.
#[test]
fn the_scan_sees_a_docs_default() {
    let code = "fn roots() -> Vec<&'static str> {\n    vec![\"docs/\"] // the default\n}\n";
    assert_eq!(offences("x.rs", code), ["x.rs:2: docs/"]);
    assert_eq!(
        offences("y.rs", "// see ADR-0001\nlet k = \"mechanic\";\n").len(),
        2
    );
    assert!(offences("z.rs", "let slug = \"lantern\";\n").is_empty());
    assert_eq!(
        offences("w.rs", "let gate = \"cargo xtask docs check\";\n"),
        ["w.rs:1: cargo xtask"]
    );
    assert_eq!(
        offences(
            "v.rs",
            "const GATE: &str = \"cargo run -q -p specengine-cli -- check\";\n"
        ),
        ["v.rs:1: cargo run -q -p specengine-cli"]
    );
}

/// One pass of the read loop per fixture: init on a copy without its
/// config, index, a search hit per kind, a show per kind, the verdicts of
/// the not-found and refused forms; no project word on stdout of init or
/// index.
#[test]
fn the_read_loop_answers_alike_on_both_fixtures() {
    for (fixture, slug) in FIXTURES {
        let scratch = Scratch::new("genre");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let config = read_text(&root, "specengine.toml");

        // init writes only its file over a copy without a config.
        fs::remove_file(root.join("specengine.toml")).unwrap();
        let before = snapshot(&root);
        let init = spec(&home, &root, &["init"]);
        init.code(0);
        // docs/features/flow-authoring.md AC-04: the two lines, then comments
        // only (`init.rs` holds the bytes).
        assert!(
            read_text(&root, "specengine.toml").starts_with("[project]\nslug = \"copy\"\n"),
            "{fixture}"
        );
        let mut after = snapshot(&root);
        after.remove("specengine.toml");
        assert_eq!(after, before, "{fixture}: init wrote only its file");
        write(&root, "specengine.toml", &config);

        let files = md_files(&root.join("docs")).len();
        let run = spec(&home, &root, &["index"]);
        run.code(0);
        assert!(
            run.stdout
                .starts_with(&format!("indexed {slug}: walked {files}, parsed {files}, ")),
            "{fixture}: {}",
            run.show()
        );
        assert!(data_dir(&home).join(format!("{slug}.db")).is_file());

        // Every node with an ID, a kind and a title: a search for its
        // title's longest word within its kind finds it (Tier 3 with
        // `--archive`), and `show` of its ID answers.
        let scheme = specengine_core::ProjectConfig::from_toml(&config)
            .unwrap()
            .scheme;
        let mut checked = 0;
        for file in md_files(&root.join("docs")) {
            let path = format!("docs/{file}");
            let parsed =
                specengine_core::parse(&path, &fs::read(root.join(&path)).unwrap(), &scheme);
            for (ord, node) in parsed.nodes.iter().enumerate() {
                let (Some(id), Some(kind), Some(title)) = (&node.id, &node.kind, &node.title)
                else {
                    continue;
                };
                let Some(word) = title
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|word| word.chars().count() >= 3)
                    .max_by_key(|word| word.chars().count())
                else {
                    continue;
                };
                let json = spec(
                    &home,
                    &root,
                    &[
                        "--json",
                        "search",
                        word,
                        "--kind",
                        kind,
                        "--archive",
                        "--limit",
                        "200",
                    ],
                )
                .json();
                let hits = json["hits"].as_array().unwrap();
                assert!(
                    hits.iter().all(|hit| hit["kind"] == kind.as_str()),
                    "{fixture}: --kind {kind}: {json}"
                );
                assert!(
                    hits.iter()
                        .any(|hit| hit["path"] == path.as_str() && hit["ord"] == ord),
                    "{fixture}: {id} not found by {word:?} within {kind}: {json}"
                );
                spec(&home, &root, &["show", id]).code(0);
                checked += 1;
            }
        }
        assert!(checked >= 10, "{fixture}: {checked} nodes checked");

        let (dangling, lookalike, scoped) = if fixture == "spec-a" {
            ("R-99", "\u{0410}-101", "stamina-tuning/AC-07")
        } else {
            ("REQ-999", "R\u{0415}Q-001", "dry-run/CRIT-01")
        };
        spec(&home, &root, &["show", scoped]).code(0);
        spec(&home, &root, &["show", dangling]).code(1);
        spec(&home, &root, &["show", "NOPE-1"]).code(1);
        spec(&home, &root, &["show", lookalike]).code(2);
        spec(&home, &root, &["show", &format!("other:{dangling}")]).code(2);
        spec(&home, &root, &["search", "zz"]).code(2);
        spec(&home, &root, &["search", "zzqqxxwwvv"]).code(0);
        spec(&home, &root, &["search", "sync", "--limit", "0"]).code(2);
        // docs/features/flow-read.md AC-06: `spec flow` reads either genre
        // through its own config: a whole read, a fold, the refusals.
        spec(&home, &root, &["flow"]).code(0);
        spec(&home, &root, &["flow", "--depth", "0"]).code(0);
        spec(&home, &root, &["--json", "flow"]).code(0);
        spec(&home, &root, &["flow", dangling]).code(1);
        spec(&home, &root, &["flow", lookalike]).code(2);
        spec(&home, &root, &["flow", &format!("other:{dangling}")]).code(2);
        // docs/features/card-trace.md: `spec trace` reads either genre
        // through its own config: a whole read, the refusals.
        spec(&home, &root, &["trace"]).code(0);
        spec(&home, &root, &["--json", "trace"]).code(0);
        spec(&home, &root, &["trace", dangling]).code(1);
        spec(&home, &root, &["trace", lookalike]).code(2);
        spec(&home, &root, &["trace", &format!("other:{dangling}")]).code(2);
    }
}

/// AC-13 of docs/features/spec-cli-check.md: `spec check` and
/// `spec export index` answer alike on both fixtures (the library's report
/// and render); without `[paths] index` the index is refused on both, and
/// nothing is written anywhere.
#[test]
fn check_and_export_answer_alike_on_both_fixtures() {
    use common::check::{index_path, library, library_render, registered, text};

    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("genre-check");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let base = read_text(&root, "specengine.toml");

        let report = library(&root);
        let run = spec(&home, &root, &["check"]);
        assert_eq!(run.code, i32::from(report.exit_code()), "{fixture}");
        assert_eq!(run.stdout, text(&report, false), "{fixture}");

        // No `[paths] index`: refused, with or without a registered entry.
        let before = snapshot(scratch.path());
        let unregistered = format!(
            "{base}\n[[generators]]\ncommand = \"gen-index\"\nwrites  = [\"{}\"]\nindex   = true\n",
            index_path(fixture)
        );
        for (what, config, says) in [
            ("no [paths] index", unregistered, "[paths] index"),
            ("nothing registered", base.clone(), "[[generators]]"),
        ] {
            write(&root, "specengine.toml", &config);
            let before = snapshot(scratch.path());
            for args in [
                &["export", "index"][..],
                &["export", "index", "--stdout"][..],
            ] {
                let run = spec(&home, &root, args);
                assert_eq!(run.code, 2, "{fixture} {what}\n{}", run.show());
                assert_eq!(run.stdout, "", "{fixture} {what}");
                assert_eq!(
                    run.stderr_lines().len(),
                    1,
                    "{fixture} {what}: {}",
                    run.stderr
                );
                assert!(
                    run.stderr.contains(says),
                    "{fixture} {what}: {}",
                    run.stderr
                );
                assert_eq!(
                    snapshot(scratch.path()),
                    before,
                    "{fixture} {what}: written"
                );
            }
        }
        write(&root, "specengine.toml", &base);
        assert_eq!(snapshot(scratch.path()), before, "{fixture}");

        // Registered: the library's render.
        let index = index_path(fixture);
        write(
            &root,
            "specengine.toml",
            registered(&base, index, "gen-index", None),
        );
        let run = spec(&home, &root, &["export", "index"]);
        run.code(0);
        assert!(
            read_text(&root, index) == library_render(&root),
            "{fixture}"
        );
        assert!(!data_dir(&home).exists(), "{fixture}: a data directory");
    }
}
