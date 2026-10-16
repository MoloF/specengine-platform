//! docs/features/code-tools.md TE-3: the three code tools over pilots A
//! and B, manual, the owner's run, never CI (`#[ignore]`): release, in the
//! foreground under a self-terminating timeout, one pilot at a time, read
//! only. A scratch `HOME`, a scratch working directory and a config copy
//! outside the corpus (the scheme with `[code] roots` appended, as
//! `specengine-eval`'s `code_index_pilot.rs` makes it); the server spawned
//! with `--root <corpus> --config <copy>`, its environment cleared but
//! `HOME`. Per tool and form: the cold call and the warm median of five
//! against `docs/canon/mcp-read.md` "Latency"'s 1 s (printed, never
//! failed: a breach plans 6c, "Risks"); `unmapped`'s units, mapped and
//! mentioned per root row and in total. The read-only proof (`git
//! --no-optional-locks status --porcelain --untracked-files=all --ignored`
//! and a (path, kind, size, mtime) listing of every code root) is equal
//! before and after. Only counts and times are printed: no path, name or
//! ID of a pilot.
//!
//! ```text
//! cargo nextest run --release -p specengine-mcp --test mcp_code_pilot --no-run
//! SPECENGINE_PILOT_A=… SPECENGINE_SCHEME_A=… SPECENGINE_CODE_ROOTS_A=dir,dir \
//!   perl -e 'alarm 1800; exec @ARGV' cargo nextest run --release -p specengine-mcp \
//!   --test mcp_code_pilot te3_pilot_a --run-ignored only --no-capture --test-threads 1
//! ```
//!
//! (B alike.) Optional: `SPECENGINE_CODE_QUERY_<A|B>`, a name to look up
//! (else `main`); `SPECENGINE_CODE_ID_<A|B>`, a REF (else the tree's first
//! ID). It fails, never skips, naming an unset variable. A cold call past
//! the harness's 60 s read timeout fails the run: record it as a breach.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::read::{Era, Session};
use common::*;
use serde_json::{Value, json};

/// The warm-call bound of `docs/canon/mcp-read.md` "Latency".
const WARM_BOUND: Duration = Duration::from_secs(1);

/// The non-empty value of `variable`, or a failure naming it.
fn required(variable: &str, what: &str) -> String {
    std::env::var(variable)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            panic!("set {variable} to {what} (outside the repository) to run this test")
        })
}

fn optional(variable: &str) -> Option<String> {
    std::env::var(variable)
        .ok()
        .filter(|value| !value.is_empty())
}

/// One entry under a code root: corpus-relative path, kind, size, mtime in
/// nanoseconds.
type Entry = (PathBuf, &'static str, u64, i128);

fn list(corpus: &Path, relative: &Path, out: &mut BTreeSet<Entry>) {
    let path = corpus.join(relative);
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        out.insert((relative.to_path_buf(), "missing", 0, 0));
        return;
    };
    let kind = if metadata.file_type().is_symlink() {
        "symlink"
    } else if metadata.is_dir() {
        "dir"
    } else if metadata.is_file() {
        "file"
    } else {
        "other"
    };
    let mtime = i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec());
    out.insert((relative.to_path_buf(), kind, metadata.len(), mtime));
    if kind != "dir" {
        return;
    }
    match fs::read_dir(&path) {
        Ok(children) => {
            for child in children {
                let child = child.expect("a directory entry");
                list(corpus, &relative.join(child.file_name()), out);
            }
        }
        Err(_) => {
            out.insert((relative.join(""), "unlistable", 0, 0));
        }
    }
}

/// The read-only proof: the corpus's `git status` (ignored files too) and
/// the listing of every code root.
fn proof(corpus: &Path, roots: &[String]) -> (String, BTreeSet<Entry>) {
    let output = Command::new("git")
        .current_dir(corpus)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--ignored",
        ])
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "the corpus must be a git worktree for the read-only proof: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut entries = BTreeSet::new();
    for root in roots {
        list(corpus, Path::new(root), &mut entries);
    }
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        entries,
    )
}

/// One call, its wall time and its result.
fn timed(session: &mut Session, tool: &str, args: &Value) -> (Duration, Value) {
    let start = Instant::now();
    let reply = session.call(tool, args.clone());
    let elapsed = start.elapsed();
    (elapsed, result(&reply).clone())
}

fn run(label: &str, suffix: &str) {
    let corpus = PathBuf::from(required(
        &format!("SPECENGINE_PILOT_{suffix}"),
        "the pilot's project root",
    ));
    let scheme = PathBuf::from(required(
        &format!("SPECENGINE_SCHEME_{suffix}"),
        "the pilot's scheme (its specengine.toml without [code])",
    ));
    let roots: Vec<String> = required(
        &format!("SPECENGINE_CODE_ROOTS_{suffix}"),
        "the code roots, comma-separated, corpus-relative",
    )
    .split(',')
    .map(str::trim)
    .filter(|root| !root.is_empty())
    .map(str::to_owned)
    .collect();
    let scratch = Scratch::new("code-pilot");
    assert!(
        !scratch.path().starts_with(&corpus) && !corpus.starts_with(scratch.path()),
        "{label}: the scratch directory and the corpus must not nest"
    );
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let config = scratch.join("config.toml");
    let scheme_text = fs::read_to_string(&scheme).expect("the scheme");
    let listed: Vec<String> = roots.iter().map(|root| format!("{root:?}")).collect();
    fs::write(
        &config,
        format!("{scheme_text}\n[code]\nroots = [{}]\n", listed.join(", ")),
    )
    .expect("a config copy");
    let before = proof(&corpus, &roots);

    let corpus_text = corpus.to_str().expect("a UTF-8 corpus path");
    let config_text = config.to_str().expect("a UTF-8 scratch path");
    let mut session = Session::open(
        Era::Legacy,
        &["--root", corpus_text, "--config", config_text],
        Some(&cwd),
        Home::At(&home),
    );
    let (_, tree) = timed(&mut session, "get_tree", &json!({}));
    let id = optional(&format!("SPECENGINE_CODE_ID_{suffix}")).unwrap_or_else(|| {
        tree["structuredContent"]["nodes"]
            .as_array()
            .and_then(|nodes| nodes.iter().find_map(|node| node["id"].as_str()))
            .expect("a tree with an ID")
            .to_owned()
    });
    let query =
        optional(&format!("SPECENGINE_CODE_QUERY_{suffix}")).unwrap_or_else(|| "main".to_owned());
    let (_, found) = timed(&mut session, "find_symbols", &json!({"query": query}));
    let first = found["structuredContent"]["symbols"][0].clone();
    let mut forms = vec![
        (
            "find_symbols {query}",
            "find_symbols",
            json!({"query": query}),
        ),
        ("find_symbols {node}", "find_symbols", json!({"node": id})),
        ("refs {id}", "refs", json!({"id": id})),
        ("unmapped {}", "unmapped", json!({})),
    ];
    if let (Some(path), Some(line)) = (first["path"].as_str(), first["start_line"].as_u64()) {
        forms.push(("find_symbols {file}", "find_symbols", json!({"file": path})));
        forms.push((
            "find_symbols {at}",
            "find_symbols",
            json!({"at": format!("{path}:{line}")}),
        ));
        forms.push(("unmapped {file}", "unmapped", json!({"path": path})));
    }
    let profile = if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    };
    println!("[{label}] profile {profile}, {} code roots", roots.len());
    for (form, tool, args) in &forms {
        let (cold, result) = timed(&mut session, tool, args);
        let mut warm: Vec<Duration> = (0..5).map(|_| timed(&mut session, tool, args).0).collect();
        warm.sort();
        let median = warm[2];
        println!(
            "[{label}] {form}: cold {} ms, warm median {} ms (<= {} ms: {}), max {} ms, isError {}",
            cold.as_millis(),
            median.as_millis(),
            WARM_BOUND.as_millis(),
            median <= WARM_BOUND,
            warm[4].as_millis(),
            result["isError"]
        );
    }
    let (_, unmapped) = timed(&mut session, "unmapped", &json!({}));
    let document = &unmapped["structuredContent"];
    let figures = |row: &Value| {
        format!(
            "units {}, mapped {}, mentioned {}",
            row["units"], row["mapped"], row["mentioned"]
        )
    };
    for (index, row) in document["rows"]
        .as_array()
        .expect("unmapped rows")
        .iter()
        .enumerate()
    {
        println!("[{label}] unmapped root {}: {}", index + 1, figures(row));
    }
    println!("[{label}] unmapped total: {}", figures(&document["total"]));
    let done = session.finish();
    assert!(done.status.success(), "{label}: {}", done.stderr);
    assert_eq!(done.stderr, "", "{label}: stderr");
    let after = proof(&corpus, &roots);
    assert!(before == after, "{label}: the corpus changed");
}

#[test]
#[ignore = "manual (TE-3): set SPECENGINE_PILOT_A, SPECENGINE_SCHEME_A, SPECENGINE_CODE_ROOTS_A; read-only"]
fn te3_pilot_a_code_tools_latency() {
    run("pilot-a", "A");
}

#[test]
#[ignore = "manual (TE-3): set SPECENGINE_PILOT_B, SPECENGINE_SCHEME_B, SPECENGINE_CODE_ROOTS_B; read-only"]
fn te3_pilot_b_code_tools_latency() {
    run("pilot-b", "B");
}
