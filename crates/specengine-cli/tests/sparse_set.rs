//! docs/features/hub-git.md (RM-0051) P21, the terminal's half: `spec
//! sparse-set`, read-only, prints the paths a hub's sparse checkout must hold,
//! from the repository's top, out of the root's `specengine.toml` at `HEAD`:
//! `specengine.toml`, `spec.lock`, `.spec-debt.toml`, each `[paths] roots`
//! entry, `[decision_records]`' `dir` and `template`, the `index = true`
//! entry's `writes`, in config order, a directory ending in `/`, a path inside
//! a listed one dropped. A path `HEAD` holds that the worktree lacks is a
//! `warning:` and `--json`'s `missing`; exit 0 whatever is missing (ADR-0012).
//!
//! - P21: the spec's synthetic config, a copy of `fixtures/apply-outputs/forge`
//!   (an invented smithing game) with `roots = ["docs", "design",
//!   "README.md"]`, its records in `records` and its template in `templates`,
//!   its index writing `docs/index.md`: the set in "Data"'s order, byte for byte
//!   on a repeat, nothing written; a sparse checkout lacking `design/`: the
//!   warning and `missing`. M: `docs/` hardcoded.
//!
//! Beyond the criteria, from "Data": the set is `HEAD`'s, not the worktree's
//! config; no repository, no commit, an invalid config at `HEAD`: exit 2,
//! nothing on stdout. The hub's half of P21 (a sync `stopped` on the gap) is
//! `specengine-http`'s `tests/hub_git.rs`.

#![cfg(unix)]

mod common;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use common::git::Sandbox;
use common::{Run, Scratch, read_text, replace, snapshot, spec_with, write};
use serde_json::json;

/// The fixture's lines the synthetic config replaces.
const FIXTURE_ROOTS: &str = "roots = [\"README.md\", \"docs\", \"design\"]";
const FIXTURE_DIR: &str = "dir      = \"docs/records/DEC\"";
const ROOTS: &str = "roots = [\"docs\", \"design\", \"README.md\"]";

/// "Data"'s set of the synthetic config, in order: the three files, the roots
/// as listed, the records' directory and template; the index's outputs lie in
/// `docs/` and are dropped.
const SET: [&str; 8] = [
    "specengine.toml",
    "spec.lock",
    ".spec-debt.toml",
    "docs/",
    "design/",
    "README.md",
    "records/",
    "templates/decision.md",
];

const DESIGN_WARNING: &str =
    "warning: design/ is in HEAD but not in this checkout: widen the sparse checkout\n";

/// A scratch repository of the synthetic project, committed once on `main`.
struct Repo {
    scratch: Scratch,
    git: Sandbox,
    main: PathBuf,
    home: PathBuf,
}

fn synthetic(label: &str) -> Repo {
    let scratch = Scratch::new(label);
    let git = Sandbox::new(scratch.path());
    let main = scratch.copy("apply-outputs/forge", "main");
    replace(&main, "specengine.toml", FIXTURE_ROOTS, ROOTS);
    replace(
        &main,
        "specengine.toml",
        FIXTURE_DIR,
        "dir      = \"records\"",
    );
    write(&main, "records/README.md", "The decision records.\n");
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "the synthetic project");
    let home = scratch.home("h");
    Repo {
        scratch,
        git,
        main,
        home,
    }
}

impl Repo {
    /// `spec sparse-set <args>` in `cwd`, git's sandbox and `HOME` its own.
    fn sparse_set(&self, cwd: &Path, args: &[&str]) -> Run {
        let mut vars: Vec<(String, OsString)> = self
            .git
            .vars()
            .into_iter()
            .filter_map(|(name, value)| name.into_string().ok().map(|name| (name, value)))
            .filter(|(name, _)| name != "HOME")
            .collect();
        vars.push(("HOME".to_owned(), self.home.clone().into_os_string()));
        let env: Vec<(&str, &OsStr)> = vars
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_os_str()))
            .collect();
        let mut all = vec!["sparse-set"];
        all.extend_from_slice(args);
        spec_with(cwd, &all, &env)
    }

    /// A clone of `main` at `dir` whose sparse checkout (cone) holds `cone`.
    fn sparse_clone(&self, dir: &str, cone: &[&str]) -> PathBuf {
        let clone = self.scratch.join(dir);
        self.git.git(
            self.scratch.path(),
            &[
                "clone",
                "-q",
                "--template=",
                self.main.to_str().expect("UTF-8"),
                clone.to_str().expect("UTF-8"),
            ],
        );
        self.git.quiet(&clone);
        let mut args = vec!["sparse-checkout", "set", "--cone"];
        args.extend_from_slice(cone);
        self.git.git(&clone, &args);
        clone
    }
}

fn lines(paths: &[&str]) -> String {
    paths.iter().map(|path| format!("{path}\n")).collect()
}

/// P21: the synthetic config's set in "Data"'s order, no warning on a whole
/// checkout, `--json` `{paths, missing}` with nothing missing; a repeat byte for
/// byte; the worktree and `.git` as they were. M: `docs/` hardcoded.
#[test]
fn p21_the_set_is_the_configs_paths_in_datas_order() {
    let repo = synthetic("sparse-set-order");
    let before = snapshot(&repo.main);
    let run = repo.sparse_set(&repo.main, &[]);
    run.code(0);
    assert_eq!(run.stdout, lines(&SET), "{}", run.show());
    assert_eq!(run.stderr, "", "a whole checkout lacks nothing");
    let again = repo.sparse_set(&repo.main, &[]);
    assert_eq!(
        (again.code, &again.stdout, &again.stderr),
        (0, &run.stdout, &run.stderr),
        "byte for byte"
    );
    let json = repo.sparse_set(&repo.main, &["--json"]);
    json.code(0);
    assert_eq!(json.json(), json!({"paths": SET, "missing": []}));
    assert_eq!(snapshot(&repo.main), before, "read-only");
}

/// P21: with `design/` outside a sparse checkout (cone `docs`, `records`,
/// `templates`; the top's files always in), the set is the same, one warning
/// names it, `--json`'s `missing` is `["design/"]`, exit 0. M: `docs/`
/// hardcoded.
#[test]
fn p21_a_path_outside_the_sparse_checkout_is_a_warning_and_missing() {
    let repo = synthetic("sparse-set-gap");
    let clone = repo.sparse_clone("sparse", &["docs", "records", "templates"]);
    assert!(!clone.join("design").exists(), "design/ is out");
    assert!(clone.join("README.md").is_file(), "the top's files are in");
    let run = repo.sparse_set(&clone, &[]);
    run.code(0);
    assert_eq!(run.stdout, lines(&SET), "{}", run.show());
    assert_eq!(run.stderr, DESIGN_WARNING);
    let json = repo.sparse_set(&clone, &["--json"]);
    json.code(0);
    assert_eq!(json.json(), json!({"paths": SET, "missing": ["design/"]}));
    assert_eq!(json.stderr, DESIGN_WARNING, "the warning with --json too");
}

/// "Data": the set comes from `HEAD`'s config, not the worktree's: a roots line
/// edited and not committed changes nothing printed.
#[test]
fn the_set_is_heads_config_not_the_worktrees() {
    let repo = synthetic("sparse-set-head");
    replace(&repo.main, "specengine.toml", ROOTS, "roots = [\"docs\"]");
    let run = repo.sparse_set(&repo.main, &[]);
    run.code(0);
    assert_eq!(run.stdout, lines(&SET), "{}", run.show());
}

/// "Data": exit 2 with no repository, no commit, or a config at `HEAD` that does
/// not read (the worktree's own valid), nothing on stdout, the reason on
/// stderr, the last naming `specengine.toml`.
#[test]
fn no_repository_no_commit_or_an_invalid_config_at_head_exits_2() {
    let repo = synthetic("sparse-set-refused");
    let plain = repo.scratch.copy("apply-outputs/forge", "no-repository");
    let unborn = repo.scratch.copy("apply-outputs/forge", "no-commit");
    repo.git.init(&unborn);
    let broken = repo.scratch.copy("apply-outputs/forge", "broken");
    repo.git.init(&broken);
    let good = read_text(&broken, "specengine.toml");
    write(&broken, "specengine.toml", "[paths]\nroots = 3\n");
    repo.git.add_all(&broken);
    repo.git.commit(&broken, "a config that does not read");
    write(&broken, "specengine.toml", &good);
    for (label, dir) in [
        ("no repository", &plain),
        ("no commit", &unborn),
        ("an invalid config at HEAD", &broken),
    ] {
        let run = repo.sparse_set(dir, &[]);
        assert_eq!(run.code, 2, "{label}: {}", run.show());
        assert_eq!(run.stdout, "", "{label}");
        assert!(!run.stderr.is_empty(), "{label}: a reason");
        let json = repo.sparse_set(dir, &["--json"]);
        assert_eq!(json.code, 2, "{label} --json: {}", json.show());
        assert_eq!(json.stdout, "", "{label} --json: no document on exit 2");
    }
    let run = repo.sparse_set(&broken, &[]);
    assert!(
        run.stderr.contains("specengine.toml"),
        "names the config: {}",
        run.show()
    );
}
