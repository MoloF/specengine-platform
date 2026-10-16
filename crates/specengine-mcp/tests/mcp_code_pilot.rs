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
//!
//! docs/features/bevy-roles-reads.md TE-8 (S20), the same rules
//! (`te8_pilot_a_find_symbols_roles_latency`, B alike): `find_symbols`
//! with and without `[code] frameworks` (`SPECENGINE_CODE_FRAMEWORKS_<A|B>`,
//! required, comma-separated as the eval's `harness::PILOT_CODE_FRAMEWORKS`
//! reads it), each config a copy outside the corpus and a `HOME` of its
//! own: a QUERY (`SPECENGINE_CODE_QUERY_<A|B>`, else `main`) and `{file}`
//! of the file holding most role rows (read from the opt-in index with the
//! system's `sqlite3`; `SPECENGINE_CODE_ROLE_FILE_<A|B>` overrides it),
//! cold and the warm median of five against the same 1 s (printed, never
//! failed: a breach plans 6c); counts only (targets, roles, the file's role
//! rows, answer characters), never a path, name or ID.
//!
//! ```text
//! SPECENGINE_PILOT_A=… SPECENGINE_SCHEME_A=… SPECENGINE_CODE_ROOTS_A=dir,dir \
//!   SPECENGINE_CODE_FRAMEWORKS_A=bevy \
//!   perl -e 'alarm 1800; exec @ARGV' cargo nextest run --release -p specengine-mcp \
//!   --test mcp_code_pilot te8_pilot_a --run-ignored only --no-capture --test-threads 1
//! ```

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

// ------------------------------------------- bevy-roles-reads TE-8 (S20)

/// A comma-separated value's names, trimmed, empty ones dropped (as the
/// eval's `harness::PILOT_CODE_FRAMEWORKS` reads its variable).
fn names(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The system's `sqlite3` client.
fn sqlite3() -> &'static str {
    ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|path| Path::new(path).exists())
        .expect("a sqlite3 client: TE-8 reads the role rows per file with it")
}

/// The code file holding most role rows in the one index under `home`
/// (ties by path bytes) and its row count; `None` without a role row.
/// A `SELECT` on the scratch `HOME`'s index (never the corpus); not
/// `-readonly`, which cannot open the index's WAL without its `-shm`. The
/// file's path is never printed.
fn most_role_rows(home: &Path) -> Option<(String, u64)> {
    let databases: Vec<PathBuf> = fs::read_dir(data_dir(home))
        .expect("the data directory")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "db"))
        .collect();
    assert_eq!(databases.len(), 1, "one index under the scratch HOME");
    let output = Command::new(sqlite3())
        .env_clear()
        .args(["-json", "-cmd", ".timeout 5000"])
        .arg(&databases[0])
        .arg(
            "SELECT f.path AS path, COUNT(*) AS n FROM code_roles r \
             JOIN code_files f ON f.file_id = r.file \
             GROUP BY f.path ORDER BY n DESC, f.path LIMIT 1",
        )
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("sqlite3 prints UTF-8");
    if text.trim().is_empty() {
        return None;
    }
    let rows: Value = serde_json::from_str(&text).expect("sqlite3 -json");
    Some((rows[0]["path"].as_str()?.to_owned(), rows[0]["n"].as_u64()?))
}

/// A `find_symbols` result's counts: targets, role objects over them
/// (`null` when every target's `roles` is `null`), and the characters of
/// its text plus its serialized `structuredContent`.
fn counts(result: &Value) -> (usize, String, usize) {
    let targets: &[Value] = result["structuredContent"]["symbols"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let lists: Vec<&Vec<Value>> = targets
        .iter()
        .filter_map(|target| target["roles"].as_array())
        .collect();
    let roles = if lists.is_empty() && !targets.is_empty() {
        "null".to_owned()
    } else {
        lists
            .iter()
            .map(|list| list.len())
            .sum::<usize>()
            .to_string()
    };
    let text = result["content"][0]["text"]
        .as_str()
        .map_or(0, |text| text.chars().count());
    let structured = result
        .get("structuredContent")
        .map_or(0, |value| value.to_string().chars().count());
    (targets.len(), roles, text + structured)
}

/// TE-8 over one pilot: the config copy with and without the opt-in, each
/// its own `HOME` and session; the first call builds the index, then each
/// form's first call and the warm median of five.
fn roles_run(label: &str, suffix: &str) {
    let corpus = PathBuf::from(required(
        &format!("SPECENGINE_PILOT_{suffix}"),
        "the pilot's project root",
    ));
    let scheme = PathBuf::from(required(
        &format!("SPECENGINE_SCHEME_{suffix}"),
        "the pilot's scheme (its specengine.toml without [code])",
    ));
    let roots = names(&required(
        &format!("SPECENGINE_CODE_ROOTS_{suffix}"),
        "the code roots, comma-separated, corpus-relative",
    ));
    let frameworks = names(&required(
        &format!("SPECENGINE_CODE_FRAMEWORKS_{suffix}"),
        "the frameworks to list, comma-separated (bevy)",
    ));
    assert!(
        !roots.is_empty() && !frameworks.is_empty(),
        "{label}: name the code roots and the frameworks"
    );
    let scratch = Scratch::new("roles-pilot");
    assert!(
        !scratch.path().starts_with(&corpus) && !corpus.starts_with(scratch.path()),
        "{label}: the scratch directory and the corpus must not nest"
    );
    let quoted = |names: &[String]| {
        names
            .iter()
            .map(|name| format!("{name:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let scheme_text = fs::read_to_string(&scheme).expect("the scheme");
    let table = format!("{scheme_text}\n[code]\nroots = [{}]\n", quoted(&roots));
    let configs = [
        (
            "with",
            format!("{table}frameworks = [{}]\n", quoted(&frameworks)),
        ),
        ("without", table),
    ];
    let query =
        optional(&format!("SPECENGINE_CODE_QUERY_{suffix}")).unwrap_or_else(|| "main".to_owned());
    let mut file = optional(&format!("SPECENGINE_CODE_ROLE_FILE_{suffix}"));
    let before = proof(&corpus, &roots);
    let corpus_text = corpus.to_str().expect("a UTF-8 corpus path");
    let profile = if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    };
    println!(
        "[{label}] TE-8 profile {profile}, {} code roots, {} frameworks",
        roots.len(),
        frameworks.len()
    );
    for (side, config_text) in &configs {
        let home = scratch.home(side);
        let cwd = scratch.dir(&format!("cwd-{side}"));
        let config = scratch.join(&format!("{side}.toml"));
        fs::write(&config, config_text).expect("a config copy");
        let config_arg = config.to_str().expect("a UTF-8 scratch path");
        let mut session = Session::open(
            Era::Legacy,
            &["--root", corpus_text, "--config", config_arg],
            Some(&cwd),
            Home::At(&home),
        );
        let query_args = json!({"query": query});
        let (built, first) = timed(&mut session, "find_symbols", &query_args);
        println!(
            "[{label}] {side}: first call (the index built) {} ms, isError {}",
            built.as_millis(),
            first["isError"]
        );
        if *side == "with" && file.is_none() {
            match most_role_rows(&home) {
                Some((path, rows)) => {
                    println!("[{label}] the file holding most role rows holds {rows}");
                    file = Some(path);
                }
                None => println!("[{label}] no role row stored"),
            }
        }
        let mut forms = vec![("find_symbols {query}", query_args)];
        if let Some(path) = &file {
            forms.push(("find_symbols {file}", json!({"file": path})));
        }
        for (form, args) in &forms {
            let (cold, result) = timed(&mut session, "find_symbols", args);
            let mut warm: Vec<Duration> = (0..5)
                .map(|_| timed(&mut session, "find_symbols", args).0)
                .collect();
            warm.sort();
            let median = warm[2];
            let (targets, roles, characters) = counts(&result);
            println!(
                "[{label}] {side} {form}: first {} ms, warm median {} ms (<= {} ms: {}), \
                 max {} ms, targets {targets}, roles {roles}, characters {characters}, \
                 isError {}",
                cold.as_millis(),
                median.as_millis(),
                WARM_BOUND.as_millis(),
                median <= WARM_BOUND,
                warm[4].as_millis(),
                result["isError"]
            );
        }
        let done = session.finish();
        assert!(done.status.success(), "{label} {side}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{label} {side}: stderr");
    }
    let after = proof(&corpus, &roots);
    assert!(before == after, "{label}: the corpus changed");
}

#[test]
#[ignore = "manual (TE-8): set SPECENGINE_PILOT_A, SPECENGINE_SCHEME_A, SPECENGINE_CODE_ROOTS_A, SPECENGINE_CODE_FRAMEWORKS_A; read-only"]
fn te8_pilot_a_find_symbols_roles_latency() {
    roles_run("pilot-a", "A");
}

#[test]
#[ignore = "manual (TE-8): set SPECENGINE_PILOT_B, SPECENGINE_SCHEME_B, SPECENGINE_CODE_ROOTS_B, SPECENGINE_CODE_FRAMEWORKS_B; read-only"]
fn te8_pilot_b_find_symbols_roles_latency() {
    roles_run("pilot-b", "B");
}
