//! docs/features/impact.md TE-4, manual (`#[ignore]`), release, warm
//! indexes, the median of five runs; the figures printed (`--no-capture`)
//! for the spec's "Implementation":
//!
//! - a 10 000-node radius: `HUB-CORE` with 10 000 records depending on
//!   it, `spec impact HUB-CORE` cut with its tail, beside `spec graph
//!   HUB-CORE --impact`;
//! - a 1 000-match name tier: `spec impact --symbol nowhere::tick` over
//!   1 000 unbound `tick`s (the suffix tier empty, the name tier all),
//!   cut in its matches;
//! - `fixtures/code-a` (the golden lock) plus 10 000 `.rs` files (every
//!   other bound to PREM-001): `spec impact GOAL-01` and `spec impact
//!   --symbol step` beside `spec show GOAL-01 --bindings` and `spec graph
//!   GOAL-01 --impact`.
//!
//! `cargo nextest run -p specengine-cli --release --test impact_cost
//! --run-ignored ignored-only --no-capture`

#![cfg(unix)]

mod common;
mod lock_common;
mod verify_common;

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use common::git::Sandbox;
use common::{Run, Scratch};

/// `OUTPUT_CAP_CHARS`.
const CAP: usize = 40_000;

/// The median of five timings of `call`.
fn median(mut call: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..5).map(|_| call()).collect();
    times.sort();
    times[2]
}

/// `spec args` from `root` with the sandbox's variables and `home`.
fn spec_in(git: &Sandbox, home: &Path, root: &Path, args: &[&str]) -> Run {
    let mut vars: Vec<(String, std::ffi::OsString)> = git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
        .collect();
    vars.push(("HOME".to_owned(), home.as_os_str().to_owned()));
    let env: Vec<(&str, &std::ffi::OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    common::spec_with(root, args, &env)
}

/// The warm median of `spec args` (one run first), each run answering
/// with `code`.
fn timed(git: &Sandbox, home: &Path, root: &Path, args: &[&str], code: i32) -> Duration {
    spec_in(git, home, root, args).code(code);
    median(|| {
        let started = Instant::now();
        let run = spec_in(git, home, root, args);
        let took = started.elapsed();
        run.code(code);
        took
    })
}

/// The text before the tail, in characters.
fn before_tail(stdout: &str) -> usize {
    stdout
        .split_inclusive('\n')
        .filter(|line| !line.starts_with("[truncated: "))
        .map(|line| line.chars().count())
        .sum()
}

/// A cut answer: within the cap before its tail, the tail last.
#[track_caller]
fn assert_cut(run: &Run, context: &str) {
    run.code(0);
    assert!(
        before_tail(&run.stdout) <= CAP,
        "{context}: {} characters before the tail",
        before_tail(&run.stdout)
    );
    assert!(
        run.stdout
            .lines()
            .last()
            .is_some_and(|line| line.starts_with("[truncated: ")),
        "{context}: the tail last"
    );
}

/// TE-4: the three corpora, figures printed.
#[test]
#[ignore = "manual (TE-4): release, figures recorded in the spec"]
fn impact_cost() {
    let scratch = Scratch::new("im-cost");
    let git = Sandbox::new(scratch.path());
    let home = scratch.home("h");
    let root = scratch.dir("radius");
    let write = |relative: &str, body: String| {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("dirs");
        fs::write(path, body).expect("write");
    };
    write(
        "specengine.toml",
        "[project]\nslug = \"cost-radius\"\n\n[ids]\nHUB = { kind = \"hub\", shape = \"name\" }\n\
         SPOKE = { kind = \"spoke\", width = 5 }\n\n[code]\nroots = [\"pkg\"]\n"
            .to_owned(),
    );
    write(
        "docs/spec/hub.md",
        "---\nclass: canon\nid: HUB-CORE\nowner: owner\nreviewed: 2026-10-21\n---\n\n# Core\n\nThe hub.\n"
            .to_owned(),
    );
    for n in 0..10_000 {
        write(
            &format!("docs/records/SPOKE-{n:05}.md"),
            format!(
                "---\nclass: canon\nid: SPOKE-{n:05}\nowner: owner\nreviewed: 2026-10-21\nlinks:\n  depends_on: [HUB-CORE]\n---\n\n# Spoke {n:05}\n\nIt leans on the hub.\n"
            ),
        );
    }
    write(
        "pkg/w/Cargo.toml",
        "[package]\nname = \"w\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n"
            .to_owned(),
    );
    write("pkg/w/src/lib.rs", "pub mod ticks;\n".to_owned());
    let mut ticks = String::new();
    for n in 0..1_000 {
        ticks.push_str(&format!(
            "pub mod m{n:04} {{\n    pub fn tick() -> u32 {{\n        {n}\n    }}\n}}\n"
        ));
    }
    write("pkg/w/src/ticks.rs", ticks);
    git.init(&root);
    git.add_all(&root);
    git.commit(&root, "the cost corpus");

    let radius = spec_in(&git, &home, &root, &["impact", "HUB-CORE"]);
    assert_cut(&radius, "a 10 000-node radius");
    assert!(
        radius.stdout.starts_with("impact: nodes 10001, "),
        "the totals first"
    );
    let ticks = spec_in(&git, &home, &root, &["impact", "--symbol", "nowhere::tick"]);
    assert_cut(&ticks, "a 1 000-match name tier");
    assert!(
        ticks
            .stdout
            .starts_with("seed symbol nowhere::tick: 1000 at name\n"),
        "the name tier: {}",
        ticks.stdout.lines().next().unwrap_or_default()
    );
    let hub = timed(&git, &home, &root, &["impact", "HUB-CORE"], 0);
    let hub_json = timed(&git, &home, &root, &["impact", "HUB-CORE", "--json"], 0);
    let hub_graph = timed(&git, &home, &root, &["graph", "HUB-CORE", "--impact"], 0);
    let tier = timed(
        &git,
        &home,
        &root,
        &["impact", "--symbol", "nowhere::tick"],
        0,
    );
    eprintln!(
        "TE-4: a 10 000-node radius, warm, median of 5: impact HUB-CORE {} ms (--json {} ms), graph HUB-CORE --impact {} ms; a 1 000-match name tier: impact --symbol nowhere::tick {} ms",
        hub.as_millis(),
        hub_json.as_millis(),
        hub_graph.as_millis(),
        tier.as_millis()
    );

    let repo = verify_common::golden("im-cost-code-a");
    repo.write(
        "crates/filler/Cargo.toml",
        "[package]\nname = \"filler\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    let mut lib = String::new();
    for at in 0..10_000 {
        lib.push_str(&format!("pub mod part_{at:05};\n"));
        let marker = if at % 2 == 0 {
            "// @implements PREM-001\n"
        } else {
            ""
        };
        repo.write(
            &format!("crates/filler/src/part_{at:05}.rs"),
            format!(
                "/// Part {at}.\n{marker}pub fn value_{at:05}(input: u32) -> u32 {{\n    input.wrapping_mul({at}).wrapping_add(1)\n}}\n"
            ),
        );
    }
    repo.write("crates/filler/src/lib.rs", lib);
    repo.commit_all("10 000 files");
    repo.spec_piped(&repo.root, &["index"]).code(0);
    let time = |args: &[&str]| -> Duration { timed(&repo.git, &repo.home, &repo.root, args, 0) };
    let goal = time(&["impact", "GOAL-01"]);
    let step = time(&["impact", "--symbol", "step"]);
    let show = time(&["show", "GOAL-01", "--bindings"]);
    let graph = time(&["graph", "GOAL-01", "--impact"]);
    let premise = time(&["impact", "PREM-001"]);
    eprintln!(
        "TE-4: code-a + 10 000 .rs files, warm, median of 5: impact GOAL-01 {} ms, impact --symbol step {} ms, impact PREM-001 (5 000 bound items) {} ms; show GOAL-01 --bindings {} ms, graph GOAL-01 --impact {} ms",
        goal.as_millis(),
        step.as_millis(),
        premise.as_millis(),
        show.as_millis(),
        graph.as_millis()
    );
}
