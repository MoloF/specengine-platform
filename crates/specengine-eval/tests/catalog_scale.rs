//! docs/features/direction-catalog.md (RM-0106) "Acceptance criteria",
//! DC-11 [measure]: the catalog at scale. The tree of `specengine-eval
//! scale --corpus-only --nodes N --seed 1`, committed whole into git, then
//! a `git fast-import` stream written here of 10 000 commits, each editing
//! 1 to 5 part files drawn by a seeded generator (each edit appends one
//! paragraph `Edit <n>.` to the file's last section), every commit marked
//! so its OID is known; the last live part file is never edited, so the
//! walk reads to the root (the worst case); one question raised so the
//! queue exists.
//!
//! - `dc11_last_changes_hold_at_1000_nodes` (default run, 1 000 nodes, 600
//!   commits, the library's browser read): one row per part node, every
//!   `last_change` the commit worked out here (the last edit of its file,
//!   else the whole tree's), a section's its file's, `of_file` set.
//! - `dc11_measure_1k`, `dc11_measure_10k`, `dc11_measure_100k`
//!   (`#[ignore]`, release): the same check, then the median of five warm
//!   runs of the release `spec catalog --kind node --json`; the git half
//!   two ways: the store's `WorktreeGit::last_commits` over the rows' files
//!   alone (the walk and its two reads; `rev-parse` aside), and the
//!   library's whole read without a cache less its read with a warm
//!   `LastChangeCache` (the seam of DC-8 counts calls and is
//!   `#[cfg(test)]` inside the CLI crate, out of this crate's reach).
//!   Bars at 10 000 nodes: the git half at most 1 s, the whole at most
//!   2 s; 1 000 and 100 000 are reported.
//!
//! Run the measure: `cargo build --release -p specengine-cli --bin spec`,
//! then `cargo nextest run -p specengine-eval --release --test
//! catalog_scale --run-ignored only --no-capture`.
//!
//! The run writes only under its scratch directory in
//! `std::env::temp_dir()`, removed on drop, with a `HOME` of its own.
//! Nothing here names a pilot; every name is invented.

#![cfg(unix)]

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
mod import_support;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use git::Sandbox;
use import_support::{Scratch, eval, stderr};
use serde_json::Value;
use specengine_cli::{
    CatalogRequest, Env, Exit, GitEnv, Globals, LastChangeCache, QuestionRequest, View,
    catalog_with_view, propose_question,
};
use specengine_core::catalog::LAST_CHANGE_SCAN_MAX;
use specengine_store::WorktreeGit;

/// The kind of every part node of the generated tree.
const KIND: &str = "node";
/// Timed runs; the median is kept.
const RUNS: usize = 5;
/// DC-11's bars at 10 000 nodes.
const GIT_BAR: Duration = Duration::from_secs(1);
const WHOLE_BAR: Duration = Duration::from_secs(2);
/// The seeded history's length.
const COMMITS: usize = 10_000;

/// splitmix64, seeded: the files each commit edits.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).expect("an index")
    }
}

/// The committed tree, its history and what each file's last change is.
struct Seeded {
    _scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
    nodes: usize,
    /// The whole tree's commit.
    first: String,
    /// Each part file's newest commit, by path.
    last: BTreeMap<String, String>,
}

/// Every part file of `tree` (`areas/**/*.md` but the READMEs), relative,
/// sorted.
fn part_files(tree: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("a readable directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if path.extension().is_some_and(|extension| extension == "md")
                && path.file_name().is_some_and(|name| name != "README.md")
            {
                let relative = path.strip_prefix(root).expect("under the tree");
                out.push(relative.to_str().expect("UTF-8").to_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(tree, &tree.join("areas"), &mut out);
    out.sort();
    out
}

impl Seeded {
    /// The tree of `nodes` part nodes committed, then `commits` seeded
    /// commits on top, the queue made by one question.
    fn new(label: &str, nodes: usize, commits: usize) -> Self {
        let scratch = Scratch::new(&format!("catalog-scale-{label}"));
        let home = scratch.join("home");
        fs::create_dir_all(&home).expect("a home");
        let out = scratch.join("out");
        let output = eval()
            .env("HOME", &home)
            .env_remove("XDG_DATA_HOME")
            .arg("scale")
            .arg("--out")
            .arg(&out)
            .args([
                "--corpus-only",
                "--nodes",
                &nodes.to_string(),
                "--seed",
                "1",
            ])
            .output()
            .expect("specengine-eval runs");
        assert!(
            output.status.success(),
            "`scale --corpus-only --nodes {nodes} --seed 1`: {}",
            stderr(&output)
        );
        let root =
            fs::canonicalize(out.join(format!("scale/synthetic/n{nodes}/tree"))).expect("the tree");
        // The generated config walks `areas` but names no `[paths] spec`, so
        // the tree's default `docs/spec` holds no node and a directionless
        // tree (the catalog's rows) lists none: the areas are named the spec
        // here, the one change to the generated tree.
        let config = root.join("specengine.toml");
        let text = fs::read_to_string(&config).expect("the config");
        let anchor = "tier1_name = \"README.md\"\n";
        assert_eq!(text.matches(anchor).count(), 1, "{text}");
        fs::write(
            &config,
            text.replacen(anchor, &format!("{anchor}spec = \"areas\"\n"), 1),
        )
        .expect("the config");
        let git = Sandbox::new(&scratch.0);
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the whole tree");
        let first = git.git_text(&root, &["rev-parse", "HEAD"]);
        let files = part_files(&root);
        assert!(files.len() > 10, "premise: {} part files", files.len());

        // The last live (`status: draft`) part file stays as first committed.
        let untouched = files
            .iter()
            .rposition(|path| {
                fs::read_to_string(root.join(path))
                    .expect("a part file")
                    .contains("\nstatus: draft\n")
            })
            .expect("a live part file");
        let mut rng = Rng(1);
        let mut texts: BTreeMap<String, String> = BTreeMap::new();
        let mut edited_by: BTreeMap<String, usize> = BTreeMap::new();
        let mut stream: Vec<u8> = Vec::new();
        for n in 1..=commits {
            let count = 1 + rng.below(5);
            let mut chosen = BTreeSet::new();
            while chosen.len() < count.min(files.len() - 1) {
                let at = rng.below(files.len());
                if at != untouched {
                    chosen.insert(at);
                }
            }
            let message = format!("Edit {n}\n");
            stream.extend_from_slice(
                format!(
                    "commit refs/heads/main\nmark :{n}\ncommitter Seed Writer \
                     <seed@example.org> {} +0000\ndata {}\n{message}",
                    1_780_000_000 + n,
                    message.len()
                )
                .as_bytes(),
            );
            if n == 1 {
                stream.extend_from_slice(format!("from {first}\n").as_bytes());
            }
            for at in chosen {
                let path = &files[at];
                let text = texts
                    .entry(path.clone())
                    .or_insert_with(|| fs::read_to_string(root.join(path)).expect("a part file"));
                text.push_str(&format!("\nEdit {n}.\n"));
                stream.extend_from_slice(
                    format!("M 100644 inline {path}\ndata {}\n", text.len()).as_bytes(),
                );
                stream.extend_from_slice(text.as_bytes());
                stream.push(b'\n');
                edited_by.insert(path.clone(), n);
            }
        }
        let marks_file = scratch.join("marks");
        let marks_arg = format!("--export-marks={}", marks_file.display());
        let mut child = git
            .command(git.git_program(), &root, &[])
            .args(["fast-import", "--quiet", &marks_arg])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("fast-import runs");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(&stream)
            .expect("the stream");
        let done = child.wait_with_output().expect("fast-import ends");
        assert!(
            done.status.success(),
            "fast-import: {}",
            String::from_utf8_lossy(&done.stderr)
        );
        git.git(&root, &["reset", "-q", "--hard", "refs/heads/main"]);
        let marks: BTreeMap<usize, String> = fs::read_to_string(&marks_file)
            .expect("the marks")
            .lines()
            .map(|line| {
                let (mark, oid) = line.split_once(' ').expect(":<n> <oid>");
                (
                    mark.trim_start_matches(':').parse().expect("a mark"),
                    oid.to_owned(),
                )
            })
            .collect();
        assert_eq!(marks.len(), commits);
        let last = files
            .iter()
            .map(|path| {
                let commit = edited_by
                    .get(path)
                    .map_or_else(|| first.clone(), |n| marks[n].clone());
                (path.clone(), commit)
            })
            .collect();
        let seeded = Self {
            _scratch: scratch,
            git,
            root,
            home,
            nodes,
            first,
            last,
        };
        seeded.question();
        seeded
    }

    fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    /// One open question on the first part file's document: the queue.
    fn question(&self) {
        let first = self.last.keys().next().expect("a part file");
        let id = Path::new(first)
            .file_stem()
            .and_then(OsStr::to_str)
            .expect("the file is named by its document's ID")
            .to_owned();
        let outcome = propose_question(
            &self.env(),
            &Globals::default(),
            &QuestionRequest {
                node_ids: vec![id],
                text: "Does the first part hold?".to_owned(),
                working_answer: "Yes.".to_owned(),
                price_of_other: "A part rewritten.".to_owned(),
                severity: None,
                distinct_from: Vec::new(),
                card: None,
                author_role: Some("developer".to_owned()),
                author_model: Some("m".to_owned()),
                run: None,
                login: None,
                now: "2026-11-12T10:00:00Z".to_owned(),
                git: GitEnv::new(self.root.clone(), self.git.vars()),
            },
        )
        .unwrap_or_else(|error| panic!("question: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }

    fn request(archive: bool) -> CatalogRequest {
        CatalogRequest {
            kinds: vec![KIND.to_owned()],
            archive,
        }
    }

    /// The library's browser read of the live nodes (`spec catalog --kind
    /// node`), `cache` as given.
    fn library(&self, cache: Option<&LastChangeCache>) -> specengine_cli::CatalogOutcome {
        self.library_of(false, cache)
    }

    fn library_of(
        &self,
        archive: bool,
        cache: Option<&LastChangeCache>,
    ) -> specengine_cli::CatalogOutcome {
        catalog_with_view(
            &self.env(),
            &Globals::default(),
            &Self::request(archive),
            View::Browser,
            cache,
        )
        .unwrap_or_else(|error| panic!("catalog: {error}"))
    }

    /// With `--archive` (the generator archives some documents), the
    /// library's rows against the history worked out here: one per part
    /// node, each with its file's newest commit, `of_file` on a section.
    fn check(&self) {
        let outcome = self.library_of(true, None);
        assert_eq!(outcome.rows.len(), self.nodes, "one row per part node");
        let mut documents: BTreeSet<&str> = BTreeSet::new();
        for row in &outcome.rows {
            let expected = self
                .last
                .get(&row.path)
                .unwrap_or_else(|| panic!("{}: not a part file", row.path));
            let change = row
                .last_change
                .as_ref()
                .unwrap_or_else(|| panic!("{:?}: no last change", row.id));
            assert_eq!(&change.commit, expected, "{:?} in {}", row.id, row.path);
            if !change.of_file {
                assert!(documents.insert(&row.path), "one document per file");
            }
        }
        assert_eq!(
            documents.len(),
            self.last.len(),
            "each part file has its document row"
        );
        assert!(outcome.notes.is_empty(), "no note: {:?}", outcome.notes);
    }
}

/// DC-11 (correctness at a size the default run affords): 1 000 nodes,
/// 600 seeded commits; every row's last change is the one worked out here.
#[test]
fn dc11_last_changes_hold_at_1000_nodes() {
    let seeded = Seeded::new("check-1k", 1_000, 600);
    seeded.check();
    let outcome = seeded.library(None);
    assert!(
        !outcome.rows.is_empty() && outcome.rows.len() < seeded.nodes,
        "the live nodes: {} of {}",
        outcome.rows.len(),
        seeded.nodes
    );
    let edited = seeded
        .last
        .values()
        .filter(|commit| **commit != seeded.first)
        .count();
    assert!(edited > 0, "some files edited");
    // A warm cache answers the same.
    let cache = LastChangeCache::default();
    let first = seeded.library(Some(&cache));
    let second = seeded.library(Some(&cache));
    assert_eq!(first, outcome);
    assert_eq!(second, outcome);
}

/// The release `spec` beside this test binary (`target.noindex/release`).
fn release_spec() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary");
    exe.parent()
        .and_then(Path::parent)
        .expect("the profile directory")
        .join("spec")
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

/// [`RUNS`] timed runs of `run` after one warm-up: the median and all.
fn timed(mut run: impl FnMut()) -> (Duration, Vec<Duration>) {
    run();
    let times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            run();
            started.elapsed()
        })
        .collect();
    (median(times.clone()), times)
}

fn millis(times: &[Duration]) -> Vec<u128> {
    times.iter().map(Duration::as_millis).collect()
}

/// What one size measured.
struct Measured {
    whole: Duration,
    git_store: Duration,
    git_delta: Duration,
}

/// The measure at `nodes`: seeding, the check, then the timings, printed.
fn measure(nodes: usize) -> Measured {
    let started = Instant::now();
    let seeded = Seeded::new(&format!("measure-{nodes}"), nodes, COMMITS);
    let seeding = started.elapsed();
    let spec = release_spec();
    assert!(
        spec.exists(),
        "{}: build the release `spec` first",
        spec.display()
    );
    let vars: Vec<(String, std::ffi::OsString)> = seeded
        .git
        .vars()
        .into_iter()
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
        .filter(|(name, _)| name != "HOME")
        .chain([("HOME".to_owned(), seeded.home.clone().into_os_string())])
        .collect();
    let mut stdout = Vec::new();
    let (whole, whole_runs) = timed(|| {
        let output = Command::new(&spec)
            .env_clear()
            .envs(vars.iter().map(|(name, value)| (name, value)))
            .current_dir(&seeded.root)
            .args(["catalog", "--kind", KIND, "--json"])
            .stdin(Stdio::null())
            .output()
            .expect("spec runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout = output.stdout;
    });
    let document: Value = serde_json::from_slice(&stdout).expect("JSON");
    let rows = document["rows"].as_array().expect("rows");
    let cut = document["truncated"] == true;
    // The library's browser read: the check, uncut.
    seeded.check();
    let handle = WorktreeGit::new(
        &seeded.root,
        &GitEnv::new(seeded.root.clone(), seeded.git.vars()),
    )
    .expect("a handle");
    // The files the live rows read (the root is the worktree's top).
    let live = seeded.library(None);
    let live_rows = live.rows.len();
    let paths: BTreeSet<String> = live.rows.iter().map(|row| row.path.clone()).collect();
    assert!(
        live.rows.iter().all(|row| row.last_change.is_some()),
        "every live row's change read"
    );
    let mut walked = 0;
    let (git_store, git_runs) = timed(|| {
        let answer = handle
            .last_commits(&paths, LAST_CHANGE_SCAN_MAX)
            .expect("the walk");
        assert_eq!(answer.found.len(), paths.len());
        walked = answer.walked;
    });
    assert_eq!(walked, COMMITS + 1, "the untouched file walks to the root");
    let (library_uncached, _) = timed(|| {
        let _ = seeded.library(None);
    });
    let cache = LastChangeCache::default();
    let _ = seeded.library(Some(&cache));
    let (library_cached, _) = timed(|| {
        let _ = seeded.library(Some(&cache));
    });
    let git_delta = library_uncached.saturating_sub(library_cached);
    let peak = Path::new("/usr/bin/time")
        .exists()
        .then(|| {
            let output = Command::new("/usr/bin/time")
                .arg("-l")
                .arg(&spec)
                .env_clear()
                .envs(vars.iter().map(|(name, value)| (name, value)))
                .current_dir(&seeded.root)
                .args(["catalog", "--kind", KIND, "--json"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .output()
                .expect("time runs");
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .find(|line| line.contains("maximum resident set size"))
                .and_then(|line| line.split_whitespace().next()?.parse::<u64>().ok())
        })
        .flatten();
    println!(
        "DC-11 at {nodes} nodes: {} live rows in {} part files, {COMMITS} commits on top \
         ({walked} walked); seeding {:.1} s; spec catalog --kind node --json (release, warm, {} \
         rows printed, cut {cut}) median {} ms, runs {:?} ms; git half: last_commits median {} ms, runs {:?} ms; \
         library read uncached {} ms, cached {} ms, delta {} ms; peak resident {} MB",
        live_rows,
        paths.len(),
        seeding.as_secs_f64(),
        rows.len(),
        whole.as_millis(),
        millis(&whole_runs),
        git_store.as_millis(),
        millis(&git_runs),
        library_uncached.as_millis(),
        library_cached.as_millis(),
        git_delta.as_millis(),
        peak.map_or_else(
            || "-".to_owned(),
            |bytes| format!("{:.1}", bytes as f64 / 1_048_576.0)
        )
    );
    Measured {
        whole,
        git_store,
        git_delta,
    }
}

#[test]
#[ignore = "DC-11 [measure], release: cargo build --release -p specengine-cli --bin spec, then \
            cargo nextest run -p specengine-eval --release --test catalog_scale --run-ignored \
            only --no-capture"]
fn dc11_measure_1k() {
    measure(1_000);
}

#[test]
#[ignore = "DC-11 [measure], release: cargo build --release -p specengine-cli --bin spec, then \
            cargo nextest run -p specengine-eval --release --test catalog_scale --run-ignored \
            only --no-capture"]
fn dc11_measure_10k() {
    let measured = measure(10_000);
    assert!(
        measured.git_store <= GIT_BAR && measured.git_delta <= GIT_BAR,
        "DC-11's git bar at 10k: last_commits {} ms, delta {} ms > 1 s",
        measured.git_store.as_millis(),
        measured.git_delta.as_millis()
    );
    assert!(
        measured.whole <= WHOLE_BAR,
        "DC-11's whole bar at 10k: {} ms > 2 s",
        measured.whole.as_millis()
    );
}

#[test]
#[ignore = "DC-11 [measure], release, reported only: cargo build --release -p specengine-cli \
            --bin spec, then cargo nextest run -p specengine-eval --release --test catalog_scale \
            --run-ignored only --no-capture"]
fn dc11_measure_100k() {
    measure(100_000);
}
