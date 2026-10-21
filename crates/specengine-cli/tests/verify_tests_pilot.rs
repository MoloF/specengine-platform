//! docs/features/verify-tests.md (8a-1) TE-9: manual, over a real project,
//! never written, never built. `SPECENGINE_PILOT_DIR` names the project's
//! root (its `specengine.toml`, inside a git worktree); the test clones the
//! worktree into a scratch directory (`git clone --no-hardlinks`), gives
//! the clone a `[verify] tests` naming the fake runner of the 8a-1 tests
//! (never the project's own command) and commits it there, then runs
//! `spec verify --tests` in the clone through the library, the owner's
//! answer `n`. Nothing starts before `y` (the probe is step 8, after the
//! question), so no test of the project runs and nothing builds. It prints
//! the verdict, the counts, the bound tests listed by reason and the
//! distinct argv the question names (a project without a bound test gives
//! `empty`, one with bound tests `declined`), and checks that the runner never started and that the project's HEAD,
//! status and index are unchanged. No project name or path is printed or
//! written anywhere.
//!
//! Run in the foreground, one at a time:
//! `SPECENGINE_PILOT_DIR=<root> cargo nextest run -p specengine-cli --test
//! verify_tests_pilot --run-ignored ignored-only --no-capture`.

mod common;
mod lock_common;
mod verify_tests_common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use common::Scratch;
use common::git::Sandbox;
use lock_common::LockRepo;
use verify_tests_common::{Kiln, RUNNER, toml_list};

/// HEAD, status (without the optional index refresh) and index.
fn state(git: &Sandbox, top: &Path) -> (Option<String>, Vec<u8>, String) {
    (
        git.head(top),
        git.git(
            top,
            &[
                "--no-optional-locks",
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
            ],
        ),
        git.git_text(top, &["ls-files", "-s"]),
    )
}

/// `config` with `tests = <template>` in its `[verify]` table (made when
/// absent), any `tests` or `timeout` it had dropped.
fn with_tests(config: &str, template: &str) -> String {
    let mut out = String::new();
    let mut in_verify = false;
    let mut placed = false;
    for line in config.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_verify = trimmed == "[verify]";
        }
        if in_verify && (trimmed.starts_with("tests") || trimmed.starts_with("timeout")) {
            continue;
        }
        out.push_str(line);
        if in_verify && trimmed == "[verify]" {
            if !line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("tests = {template}\n"));
            placed = true;
        }
    }
    if !placed {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("\n[verify]\ntests = {template}\n"));
    }
    out
}

#[test]
#[ignore = "manual (TE-9): set SPECENGINE_PILOT_DIR to a project root; read-only"]
fn pilot_bound_tests() {
    let root = PathBuf::from(
        std::env::var_os("SPECENGINE_PILOT_DIR")
            .expect("SPECENGINE_PILOT_DIR names the project root (manual test)"),
    );
    let root = root.canonicalize().expect("the project root exists");
    let scratch = Scratch::new("verify-tests-pilot");
    let git = Sandbox::new(scratch.path());
    let pilot_git = git.clone();
    let top = PathBuf::from(pilot_git.git_text(&root, &["rev-parse", "--show-toplevel"]));
    let inner = root
        .strip_prefix(&top)
        .expect("the root lies in its worktree")
        .to_path_buf();
    let before = state(&pilot_git, &top);

    let clone = scratch.join("clone");
    git.git(
        scratch.path(),
        &[
            "clone",
            "-q",
            "--no-hardlinks",
            top.to_str().expect("a UTF-8 path"),
            clone.to_str().expect("a UTF-8 path"),
        ],
    );
    git.quiet(&clone);
    let repo = LockRepo {
        root: clone.join(&inner),
        home: scratch.home("h"),
        git,
        scratch,
    };
    let config = repo.read("specengine.toml");
    let slug = config
        .parse::<toml::Table>()
        .ok()
        .and_then(|table| {
            table
                .get("project")
                .and_then(|project| project.get("slug"))
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        })
        .expect("the project's slug");
    let template = toml_list(&[RUNNER, "{test}"]);
    repo.write("specengine.toml", with_tests(&config, &template));
    let kiln = Kiln::bare(repo, Box::leak(slug.into_boxed_str()));
    let _reaper = kiln.reaper();
    kiln.write_runner(RUNNER, 0o755);
    kiln.repo
        .commit_all("a scratch verify table and a fake runner");
    let clone_before = state(&kiln.repo.git, &clone);

    let started = Instant::now();
    let run = kiln.run_with(kiln.root(), None, false, &[]);
    let elapsed = started.elapsed();
    kiln.assert_nothing_started("the pilot, answered n");
    assert!([1, 2].contains(&run.code), "{}", run.show());
    if run.outcome.is_some() {
        let doc = run.json();
        let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
        for row in doc["tests"].as_array().into_iter().flatten() {
            let reason = row["reason"].as_str().unwrap_or("runnable").to_owned();
            *reasons.entry(reason).or_default() += 1;
        }
        eprintln!(
            "TE-9 verify --tests: exit {}, verdict {}, counts {}, {} bound tests, by reason {reasons:?}, question {:?}, wall {elapsed:?}",
            run.code,
            doc["verdict"],
            doc["counts"],
            doc["tests"].as_array().map_or(0, Vec::len),
            run.prompts
                .first()
                .and_then(|text| text.lines().last())
                .map(|q| q.split(" as ").next().unwrap_or_default().to_owned()),
        );
        assert!(
            ["empty", "declined"].contains(&doc["verdict"].as_str().unwrap_or_default()),
            "{doc}"
        );
        let root_text = kiln.root().display().to_string();
        assert!(!run.text().contains(&root_text), "an absolute path printed");
    } else {
        eprintln!(
            "TE-9 verify --tests: exit {}: {:?}",
            run.code,
            run.stderr_lines()
        );
    }
    assert_eq!(
        state(&kiln.repo.git, &clone),
        clone_before,
        "the run changed the clone"
    );
    assert_eq!(state(&pilot_git, &top), before, "the project changed");
}
