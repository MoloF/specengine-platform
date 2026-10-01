//! AC-07 of docs/features/spec-cli-switch.md: the summary of `spec check`
//! ends `, worst W <n> B — <verdict>` and `--json` carries
//! `counts.worst_w_bytes` as the last count; n is the library's
//! (`specengine_core::check::worst_w` over the same walk, and
//! `counts.worst_w_bytes` of `check_worktree` / `check_staged`) for a
//! plain check, a fully staged `--staged`, `observe`, on both fixtures
//! (this repository: `parity.rs`); under `--staged` the sizes are the
//! staged blobs'. `cannot-check` → 0: `check.rs` and core's
//! `check_worst_w.rs`.

#![cfg(unix)]

mod common;

use std::path::Path;

use common::check::{library, text};
use common::staged::{Repo, base_aside};
use common::{FIXTURES, Run, Scratch, read_text, spec, write};
use specengine_core::ProjectConfig;
use specengine_core::check::worst_w;
use specengine_store::{WorkingTree, check_input};

/// W of `root`'s working tree by the library, from its own config.
fn library_w(root: &Path) -> u64 {
    let project =
        ProjectConfig::from_toml(&read_text(root, "specengine.toml")).expect("the fixture config");
    let tree = WorkingTree::new(root, &project.paths).expect("the working tree");
    worst_w(&check_input(&tree, &project.scheme), &project.paths, None)
}

/// The `worst W <n> B` value of the summary line of `run`, which must end
/// ` — <verdict>`.
fn summary_w(run: &Run, verdict: &str) -> u64 {
    let summary = run.stdout.lines().last().expect("a summary line");
    let (_, rest) = summary
        .split_once(", worst W ")
        .unwrap_or_else(|| panic!("W in the summary: {summary}"));
    let (w, end) = rest
        .split_once(" B \u{2014} ")
        .unwrap_or_else(|| panic!("` B — ` after W: {summary}"));
    assert_eq!(end, verdict, "{summary}");
    w.parse().expect("W is a number")
}

/// `counts` of a `--json` run: the value and the raw tail `"stale":…,"worst_w_bytes":W}`.
fn json_w(run: &Run) -> u64 {
    let document = run.json();
    let w = document["counts"]["worst_w_bytes"]
        .as_u64()
        .unwrap_or_else(|| panic!("counts.worst_w_bytes: {document}"));
    let stale = &document["counts"]["stale"];
    assert!(
        run.stdout
            .contains(&format!("\"stale\":{stale},\"worst_w_bytes\":{w}}}")),
        "W is the last count: {}",
        run.stdout
    );
    w
}

#[test]
fn plain_and_observe_report_the_library_s_w_on_both_fixtures() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("worst-w");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let w = library_w(&root);
        assert!(w > 0, "{fixture}: W {w}");
        let report = library(&root);
        assert_eq!(report.counts.worst_w_bytes, w, "{fixture}");
        let verdict = report
            .lines(false)
            .last()
            .unwrap()
            .rsplit(' ')
            .next()
            .unwrap()
            .to_owned();

        let run = spec(&home, &root, &["check"]);
        assert_eq!(run.stdout, text(&report, false), "{fixture}");
        assert_eq!(summary_w(&run, &verdict), w, "{fixture} plain");
        let json = spec(&home, &root, &["--json", "check"]);
        assert_eq!(json_w(&json), w, "{fixture} --json");

        // `observe`: the same W.
        let config = read_text(&root, "specengine.toml");
        assert!(!config.contains("[check]"), "{fixture}");
        write(
            &root,
            "specengine.toml",
            format!("{config}\n[check]\nmode = \"observe\"\n"),
        );
        let observed = library(&root);
        let verdict = observed
            .lines(false)
            .last()
            .unwrap()
            .rsplit(' ')
            .next()
            .unwrap()
            .to_owned();
        let run = spec(&home, &root, &["check"]);
        run.code(0);
        assert!(
            run.stdout.contains("spec check [observe]: "),
            "{}",
            run.stdout
        );
        assert_eq!(summary_w(&run, &verdict), w, "{fixture} observe");
        assert_eq!(observed.counts.worst_w_bytes, w, "{fixture} observe");
        let json = spec(&home, &root, &["--json", "check"]);
        assert_eq!(json_w(&json), w, "{fixture} observe --json");
    }
}

#[test]
fn staged_reports_the_library_s_w_of_the_staged_blobs() {
    for (fixture, _) in FIXTURES {
        // Fully staged: the working tree's W.
        let repo = Repo::staged("worst-w-staged", fixture);
        let w = library_w(&repo.top);
        let staged_report = repo.library();
        assert_eq!(staged_report.counts.worst_w_bytes, w, "{fixture}");
        let verdict = staged_report
            .lines(false)
            .last()
            .unwrap()
            .rsplit(' ')
            .next()
            .unwrap()
            .to_owned();
        let run = repo.staged_check(&[]);
        assert_eq!(summary_w(&run, &verdict), w, "{fixture} --staged");
        // With a base, `counts.introduced` and `counts.new_debt` precede W
        // (docs/features/spec-cli-introduced.md): set aside, W is last.
        let json = base_aside(&repo.staged_check(&["--json"]));
        assert_eq!(json_w(&json), w, "{fixture} --staged --json");

        // A large live file staged, then shrunk on disk: `--staged` counts
        // the staged blob, the plain check the file on disk.
        let big = format!("# Big\n\n{}\n", "x".repeat(300_000));
        write(&repo.top, "docs/spec/zz-big.md", &big);
        repo.git(&["add", "docs/spec/zz-big.md"]);
        write(&repo.top, "docs/spec/zz-big.md", "# Small\n");
        let staged = repo.library().counts.worst_w_bytes;
        assert!(staged >= big.len() as u64, "{fixture}: staged W {staged}");
        let run = repo.staged_check(&[]);
        let verdict = repo
            .library()
            .lines(false)
            .last()
            .unwrap()
            .rsplit(' ')
            .next()
            .unwrap()
            .to_owned();
        assert_eq!(summary_w(&run, &verdict), staged, "{fixture} --staged, big");
        let plain = repo.plain_check(&[]);
        let plain_w = library_w(&repo.top);
        assert!(
            plain_w < staged,
            "{fixture}: plain {plain_w} vs staged {staged}"
        );
        let plain_verdict = library(&repo.top)
            .lines(false)
            .last()
            .unwrap()
            .rsplit(' ')
            .next()
            .unwrap()
            .to_owned();
        assert_eq!(
            summary_w(&plain, &plain_verdict),
            plain_w,
            "{fixture} plain, small"
        );
    }
}

// ------------------------------------------------- index shards (ADR-0030)
// docs/features/index-shards.md AC-08 through the binary: on sharded copies
// of both fixtures (live shards of 3 000 and 5 000 B, an archive shard of
// 20 000 B, all `class: generated`), the summary's and `--json`'s W is the
// library's with the index entry, which is the unsharded W (the three
// generated shards count 0 there) + 5 000.

fn generated_of(size: usize) -> String {
    let mut text =
        "---\nclass: generated\ngenerator: gen-index\nsource: front-matter\n---\n\n# Shard\n"
            .to_owned();
    text.push_str(&"x".repeat(size - text.len() - 1));
    text.push('\n');
    assert_eq!(text.len(), size);
    text
}

#[test]
fn w_counts_the_largest_live_shard_on_both_fixtures() {
    use common::check::{index_path, quoted, set_paths_key};
    use specengine_core::check::CheckConfig;
    let shards = [
        (
            "docs/spec/index-records.md",
            3_000,
            "{ path = \"docs/spec/index-records.md\", claims = [\"docs/records/**\"] }",
        ),
        (
            "docs/features/index-archive.md",
            20_000,
            "{ path = \"docs/features/index-archive.md\", tier3 = true }",
        ),
        (
            "docs/features/index-records.md",
            5_000,
            "{ path = \"docs/features/index-records.md\", claims = [\"docs/features/**\"] }",
        ),
    ];
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("worst-w-shards");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let index = index_path(fixture);
        let base = read_text(&root, "specengine.toml");
        let mut config = set_paths_key(&base, "index", &quoted(index));
        let mut writes = vec![quoted(index)];
        writes.extend(shards.iter().map(|(path, _, _)| quoted(path)));
        config.push_str(&format!(
            "\n[[generators]]\ncommand = \"gen-index\"\nwrites  = [{}]\nindex   = true\nshards  = [\n",
            writes.join(", ")
        ));
        for (_, _, item) in &shards {
            config.push_str(&format!("  {item},\n"));
        }
        config.push_str("]\n");
        write(&root, "specengine.toml", &config);
        for (path, size, _) in &shards {
            write(&root, path, generated_of(*size));
        }

        let project = ProjectConfig::from_toml(&config).expect("the config");
        let check = CheckConfig::from_toml(&config)
            .unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
        let tree = WorkingTree::new(&root, &project.paths).expect("the working tree");
        let input = check_input(&tree, &project.scheme);
        let w = worst_w(&input, &project.paths, check.index_generator());
        assert_eq!(
            w,
            worst_w(&input, &project.paths, None) + 5_000,
            "{fixture}"
        );
        assert_eq!(library(&root).counts.worst_w_bytes, w, "{fixture}");

        let report = library(&root);
        let verdict = report
            .lines(false)
            .last()
            .unwrap()
            .rsplit(' ')
            .next()
            .unwrap()
            .to_owned();
        let run = spec(&home, &root, &["check"]);
        assert_eq!(summary_w(&run, &verdict), w, "{fixture} plain");
        let json = spec(&home, &root, &["--json", "check"]);
        assert_eq!(json_w(&json), w, "{fixture} --json");
    }
}
