//! docs/features/symbol-reads.md TE-4: `spec unmapped` over pilots A and
//! B, manual, the owner's run, never CI (`#[ignore]`), under 6a-2's TE-8
//! conditions (docs/features/symbol-index.md TE-8): release, in the
//! foreground under a self-terminating timeout, one pilot at a time, a
//! scratch `HOME` the test makes. The label's scheme (outside the
//! repository) is copied into a scratch directory with a `[code]` table of
//! the roots given; the pilot tree is only read (the eval README's proof:
//! `git status` and the listing of the scheme's and the code roots equal
//! before and after). `spec unmapped` runs in process (the CLI library)
//! twice: the first call indexes, the second answers from the index; both
//! documents are equal.
//!
//! Printed, counts only, for the spec's "Implementation": per code root
//! (named `root <n>` in the variable's order, never by its path) units,
//! mapped, mentioned and the percent; the total; each call's wall time.
//! Nothing here names a pilot; its paths come only from the variables:
//!
//! ```text
//! SPECENGINE_PILOT_A=… SPECENGINE_SCHEME_A=… SPECENGINE_CODE_ROOTS_A=dir,dir \
//!   perl -e 'alarm 1800; exec @ARGV' \
//!   cargo nextest run --release -p specengine-eval --test symbol_reads_pilot \
//!   te4_pilot_a --run-ignored only --no-capture --test-threads 1
//! ```
//!
//! (B alike.) It fails, never skips, naming an unset variable. The
//! unignored test runs the same over a scratch git copy of
//! `fixtures/code-index`.
//!
//! The library entry: `specengine_cli::unmapped(&Env, &Globals,
//! &UnmappedRequest { path: None })`, its outcome serialized as the
//! command's JSON document (as `refs`' outcome is).

mod pilot;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;
use specengine_cli::{Env, Globals, UnmappedRequest, unmapped};
use specengine_core::CodeConfig;

/// A scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-symbol-reads-pilot-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch");
        Self(fs::canonicalize(&path).expect("canonical scratch"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// One root's counts.
#[derive(Debug, PartialEq, Eq)]
struct Counts {
    units: u64,
    mapped: u64,
    mentioned: u64,
}

fn counts(row: &Value) -> Counts {
    let number = |key: &str| row[key].as_u64().unwrap_or_else(|| panic!("{key}: {row}"));
    Counts {
        units: number("units"),
        mapped: number("mapped"),
        mentioned: number("mentioned"),
    }
}

/// `spec unmapped` in process over `corpus` with `config` and `home`,
/// twice; per root (in `roots`' order) and the total, and the two wall
/// times.
fn census(
    corpus: &Path,
    config: &Path,
    home: &Path,
    roots: &[String],
) -> (Vec<Counts>, Counts, [Duration; 2]) {
    let env = Env {
        cwd: corpus.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(corpus.to_path_buf()),
        config: Some(config.to_path_buf()),
    };
    let call = || {
        let started = Instant::now();
        let outcome = unmapped(&env, &globals, &UnmappedRequest { path: None })
            .unwrap_or_else(|error| panic!("spec unmapped: {error:?}"));
        let value = serde_json::to_value(&outcome).expect("the outcome serializes");
        (value, started.elapsed())
    };
    let (first, first_time) = call();
    let (second, second_time) = call();
    assert_eq!(first, second, "two calls give the same document");
    let rows = first["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), roots.len(), "a row per root: {first}");
    let per_root = roots
        .iter()
        .map(|root| {
            let row = rows
                .iter()
                .find(|row| row["path"].as_str() == Some(root.trim_end_matches('/')))
                .unwrap_or_else(|| panic!("no row for a root: {first}"));
            counts(row)
        })
        .collect();
    (per_root, counts(&first["total"]), [first_time, second_time])
}

fn percent(counts: &Counts) -> String {
    if counts.units == 0 {
        return "-".to_owned();
    }
    let permille = (2 * counts.mapped * 1000 + counts.units) / (2 * counts.units);
    format!("{}.{} %", permille / 10, permille % 10)
}

fn print(label: &str, per_root: &[Counts], total: &Counts, times: [Duration; 2]) {
    let profile = if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    };
    println!("[{label}] profile {profile}");
    for (at, counts) in per_root.iter().enumerate() {
        println!(
            "[{label}] root {}: units {}, mapped {}, {}, mentioned {}",
            at + 1,
            counts.units,
            counts.mapped,
            percent(counts),
            counts.mentioned
        );
    }
    println!(
        "[{label}] total: units {}, mapped {}, {}, mentioned {}",
        total.units,
        total.mapped,
        percent(total),
        total.mentioned
    );
    println!(
        "[{label}] unmapped: first call {:.2} s (indexes), second {:.2} s",
        times[0].as_secs_f64(),
        times[1].as_secs_f64()
    );
}

fn run(label: &str, suffix: &str) {
    let variables = pilot::variables(label);
    let corpus = pilot::required(variables.corpus, "the pilot's worktree");
    let scheme = pilot::required(variables.scheme, "the pilot's scheme with [paths]");
    let roots_variable = format!("SPECENGINE_CODE_ROOTS_{suffix}");
    let roots: Vec<String> = std::env::var(&roots_variable)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("set {roots_variable} to the code roots, comma-separated"))
        .split(',')
        .map(|root| root.trim().to_owned())
        .collect();
    let scratch = Scratch::new(label);
    assert!(
        !scratch.0.starts_with(&corpus) && !corpus.starts_with(&scratch.0),
        "{label}: the scratch directory and the corpus must not nest"
    );
    let config = scratch.0.join("specengine.toml");
    let text = fs::read_to_string(&scheme).expect("the scheme");
    assert!(
        CodeConfig::from_toml(&text)
            .expect("the scheme reads")
            .is_none(),
        "{}: the scheme already has [code]",
        variables.scheme
    );
    let list = roots
        .iter()
        .map(|root| format!("{root:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(&config, format!("{text}\n[code]\nroots = [{list}]\n")).expect("config copy");
    let home = scratch.0.join("home");
    fs::create_dir_all(&home).expect("home");
    let mut proof_roots = pilot::scheme_roots(&scheme, variables.scheme);
    proof_roots.extend(roots.iter().cloned());
    let before = pilot::proof(&corpus, &proof_roots);
    let (per_root, total, times) = census(&corpus, &config, &home, &roots);
    let after = pilot::proof(&corpus, &proof_roots);
    assert!(before == after, "{label}: the pilot tree changed");
    print(label, &per_root, &total, times);
}

#[test]
#[ignore = "manual pilot run: SPECENGINE_PILOT_A, SPECENGINE_SCHEME_A, SPECENGINE_CODE_ROOTS_A"]
fn te4_pilot_a_unmapped() {
    run("pilot-a", "A");
}

#[test]
#[ignore = "manual pilot run: SPECENGINE_PILOT_B, SPECENGINE_SCHEME_B, SPECENGINE_CODE_ROOTS_B"]
fn te4_pilot_b_unmapped() {
    run("pilot-b", "B");
}

/// The census itself over a scratch git copy of `fixtures/code-index`
/// (config copy outside it): the per-root counts of `expected.json`
/// `reads.unmapped` (`roots`), the copy unchanged.
#[test]
fn the_unmapped_census_runs_read_only_on_a_fixture_copy() {
    let scratch = Scratch::new("fixture");
    let corpus = scratch.0.join("corpus");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-index");
    copy(&fixture, &corpus);
    pilot::git_init(&corpus);
    let config = scratch.0.join("specengine.toml");
    fs::write(
        &config,
        fs::read(corpus.join("specengine.toml")).expect("config"),
    )
    .expect("config copy");
    let home = scratch.0.join("home");
    fs::create_dir_all(&home).expect("home");
    let roots = vec!["crates".to_owned(), "data".to_owned()];
    let before = pilot::proof(&corpus, &roots);
    let (per_root, total, times) = census(&corpus, &config, &home, &roots);
    assert!(before == pilot::proof(&corpus, &roots), "the copy changed");
    assert_eq!(
        per_root,
        [
            Counts {
                units: 31,
                mapped: 20,
                mentioned: 28
            },
            Counts {
                units: 1,
                mapped: 1,
                mentioned: 1
            },
        ]
    );
    assert_eq!(
        total,
        Counts {
            units: 32,
            mapped: 21,
            mentioned: 29
        }
    );
    print("fixture", &per_root, &total, times);
}

/// Copies `from` to `to`, `expected.json` left out.
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    for entry in fs::read_dir(from).expect("fixture").flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy(&entry.path(), &target);
        } else if entry.file_name() != "expected.json" {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
        }
    }
}
