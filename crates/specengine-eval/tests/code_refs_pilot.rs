//! docs/features/code-refs.md TE-9: `spec refs` over the pilots, manual
//! (`#[ignore]`, release, in the foreground under a self-terminating
//! timeout). The label's scheme (outside the repository) is copied into a
//! scratch directory with a `[code]` table of the roots given; the pilot
//! tree is only read (the eval README's proof,
//! `crates/specengine-eval/README.md` "Pilot runs and tests": `git status`
//! and the listing of the scheme's and the code table's roots equal before
//! and after; the index goes to a scratch `HOME`). Printed: the code files, every marker of the tree by relation,
//! binding and written revision (the binding library), every distinct
//! marker ID through `spec refs` in process (its rows by `id_state` and
//! `rev_state`, the IDs naming nothing or refused), the mentions of the
//! REFs asked for by binding, and the wall time of each call (AC-10's
//! baseline). A second call of the first REF gives the same JSON.
//!
//! Run (nothing here names a pilot):
//!
//! ```text
//! SPECENGINE_PILOT_A=… SPECENGINE_SCHEME_A=… SPECENGINE_CODE_ROOTS_A=dir,dir \
//!   [SPECENGINE_REFS_A="ID ID"] perl -e 'alarm 1800; exec @ARGV' \
//!   cargo nextest run --release -p specengine-eval --test code_refs_pilot \
//!   --run-ignored only --no-capture
//! ```
//!
//! It fails, never skips, naming an unset variable. The unignored test runs
//! the same census over a scratch git copy of `fixtures/code-a`.

mod pilot;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use specengine_cli::{Env, Globals, RefsRequest, refs};
use specengine_code::{Anchor, Binding, RustParser, bind_file, comment_anchors, markers_in};
use specengine_core::CodeConfig;

/// A scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-code-refs-pilot-{name}-{}",
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

/// What the census counted.
#[derive(Debug, Default)]
struct Census {
    files: BTreeMap<&'static str, usize>,
    markers: usize,
    by_relation: BTreeMap<String, usize>,
    by_binding: BTreeMap<String, usize>,
    by_written_rev: BTreeMap<&'static str, usize>,
    ids: BTreeSet<String>,
    /// Rows of every distinct marker ID's `refs`, each marker once per node.
    by_id_state: BTreeMap<String, usize>,
    by_rev_state: BTreeMap<String, usize>,
    listed: usize,
    naming_nothing: Vec<String>,
    refused: Vec<String>,
    mentions_by_binding: BTreeMap<String, usize>,
    times: Vec<Duration>,
}

/// The `.rs` and `.ron` files under the code table's roots, as the walk
/// lists them: symlinks and `.`-names skipped, excludes applied.
fn code_files(corpus: &Path, code: &CodeConfig) -> BTreeSet<String> {
    fn walk(corpus: &Path, relative: &str, code: &CodeConfig, out: &mut BTreeSet<String>) {
        let Ok(entries) = fs::read_dir(corpus.join(relative)) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if name.starts_with('.') || kind.is_symlink() {
                continue;
            }
            let child = format!("{relative}/{name}");
            if kind.is_dir() {
                walk(corpus, &child, code, out);
            } else if (name.ends_with(".rs") || name.ends_with(".ron")) && !code.is_excluded(&child)
            {
                out.insert(child);
            }
        }
    }
    let mut out = BTreeSet::new();
    for root in &code.roots {
        let path = corpus.join(root);
        if path.is_dir() {
            walk(corpus, root, code, &mut out);
        } else if path.is_file() && !code.is_excluded(root) {
            out.insert(root.clone());
        }
    }
    out
}

fn binding_word(binding: Binding) -> String {
    match binding {
        Binding::Item(_) => "item".to_owned(),
        Binding::Module(_) => "module".to_owned(),
        Binding::Unanchored(category) => format!("unanchored {}", category.as_str()),
        Binding::CannotVerify(category) => format!("cannot_verify {}", category.as_str()),
        Binding::File => "file".to_owned(),
    }
}

fn anchor_word(anchor: &Anchor) -> String {
    match anchor {
        Anchor::Path { .. } => "ron".to_owned(),
        Anchor::Ambiguous { .. } => "ron ambiguous".to_owned(),
        Anchor::Unanchored => "unanchored no_value".to_owned(),
        Anchor::CannotVerify => "cannot_verify nesting_too_deep".to_owned(),
    }
}

/// Every marker of the tree, counted through the binding library.
fn count_markers(corpus: &Path, code: &CodeConfig, census: &mut Census) {
    let mut parser = RustParser::new().expect("the grammar");
    for path in code_files(corpus, code) {
        let Ok(text) = fs::read_to_string(corpus.join(&path)) else {
            *census.files.entry("skipped").or_default() += 1;
            continue;
        };
        let comments: Vec<(String, String)> = if path.ends_with(".rs") {
            *census.files.entry("rs").or_default() += 1;
            let bound = bind_file(&mut parser, &text).expect("a tree");
            bound
                .comments
                .iter()
                .map(|c| (text[c.range.clone()].to_owned(), binding_word(c.binding)))
                .collect()
        } else {
            *census.files.entry("ron").or_default() += 1;
            comment_anchors(&text)
                .iter()
                .map(|a| (text[a.comment.clone()].to_owned(), anchor_word(&a.anchor)))
                .collect()
        };
        for (comment, binding) in comments {
            for marker in markers_in(&comment) {
                census.markers += 1;
                *census
                    .by_relation
                    .entry(marker.relation.as_str().to_owned())
                    .or_default() += 1;
                *census.by_binding.entry(binding.clone()).or_default() += 1;
                let token = marker.token(&comment);
                let written = if token.len() == marker.id.len() {
                    "unversioned"
                } else if marker.rev.is_some() {
                    "rev"
                } else {
                    "bad_rev"
                };
                *census.by_written_rev.entry(written).or_default() += 1;
                if !marker.id.is_empty() {
                    census.ids.insert(marker.id.clone());
                }
            }
        }
    }
}

/// The census of one corpus: `config` (a scratch copy with `[code]`),
/// `refs_asked` REFs whose mentions are counted too.
fn census(corpus: &Path, config: &Path, home: &Path, refs_asked: &[String]) -> Census {
    let text = fs::read_to_string(config).expect("the config copy");
    let code = CodeConfig::from_toml(&text)
        .expect("a valid [code]")
        .expect("a [code] table");
    let mut census = Census::default();
    count_markers(corpus, &code, &mut census);
    let env = Env {
        cwd: corpus.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(corpus.to_path_buf()),
        config: Some(config.to_path_buf()),
    };
    let call = |reference: &str, census: &mut Census| {
        let started = Instant::now();
        let outcome = refs(
            &env,
            &globals,
            &RefsRequest {
                reference: reference.to_owned(),
            },
        );
        census.times.push(started.elapsed());
        outcome
    };
    let mut seen: BTreeSet<(String, usize, usize, String)> = BTreeSet::new();
    let ids: Vec<String> = census.ids.iter().cloned().collect();
    for id in &ids {
        match call(id, &mut census) {
            Err(_) => census.refused.push(id.clone()),
            Ok(outcome) if outcome.reason.is_some() => census.naming_nothing.push(id.clone()),
            Ok(outcome) => {
                for node in &outcome.nodes {
                    for row in &node.markers {
                        let key = (row.path.clone(), row.line, row.column, node.path.clone());
                        if seen.insert(key) {
                            census.listed += 1;
                            *census
                                .by_id_state
                                .entry(row.id_state.to_owned())
                                .or_default() += 1;
                            *census
                                .by_rev_state
                                .entry(row.rev_state.to_owned())
                                .or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    for reference in refs_asked {
        let outcome = call(reference, &mut census).expect("an asked REF runs");
        for node in &outcome.nodes {
            for row in &node.mentions {
                *census
                    .mentions_by_binding
                    .entry(row.binding.kind.to_owned())
                    .or_default() += 1;
            }
        }
    }
    if let Some(first) = ids.first().or(refs_asked.first()) {
        let a = serde_json::to_string(&call(first, &mut census).expect("runs")).expect("JSON");
        let b = serde_json::to_string(&call(first, &mut census).expect("runs")).expect("JSON");
        assert_eq!(a, b, "two calls of `{first}` give the same JSON");
    }
    census
}

fn print(label: &str, census: &Census) {
    let total: Duration = census.times.iter().sum();
    let max = census.times.iter().max().copied().unwrap_or_default();
    println!("[{label}] code files {:?}", census.files);
    println!(
        "[{label}] markers {} by relation {:?}",
        census.markers, census.by_relation
    );
    println!("[{label}] markers by binding {:?}", census.by_binding);
    println!(
        "[{label}] markers by written rev {:?}",
        census.by_written_rev
    );
    println!(
        "[{label}] distinct ids {}: listed rows {}, by id_state {:?}, by rev_state {:?}",
        census.ids.len(),
        census.listed,
        census.by_id_state,
        census.by_rev_state
    );
    println!(
        "[{label}] ids naming nothing {}, refused {}",
        census.naming_nothing.len(),
        census.refused.len()
    );
    println!(
        "[{label}] mentions by binding {:?}",
        census.mentions_by_binding
    );
    println!(
        "[{label}] refs calls {}: total {:.1} s, max {:.2} s",
        census.times.len(),
        total.as_secs_f64(),
        max.as_secs_f64()
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
    let refs_asked: Vec<String> = std::env::var(format!("SPECENGINE_REFS_{suffix}"))
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let scratch = Scratch::new(label);
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
    proof_roots.extend(roots);
    let before = pilot::proof(&corpus, &proof_roots);
    let counted = census(&corpus, &config, &home, &refs_asked);
    let after = pilot::proof(&corpus, &proof_roots);
    assert!(before == after, "{label}: the pilot tree changed");
    print(label, &counted);
}

#[test]
#[ignore = "manual pilot run: SPECENGINE_PILOT_A, SPECENGINE_SCHEME_A, SPECENGINE_CODE_ROOTS_A"]
fn pilot_a_code_refs_census() {
    run("pilot-a", "A");
}

#[test]
#[ignore = "manual pilot run: SPECENGINE_PILOT_B, SPECENGINE_SCHEME_B, SPECENGINE_CODE_ROOTS_B"]
fn pilot_b_code_refs_census() {
    run("pilot-b", "B");
}

/// The census itself over a scratch git copy of `fixtures/code-a`
/// (config copy outside it): every marker counted, the listed rows and
/// their states as `expected.json` gives them, the copy unchanged.
#[test]
fn the_census_runs_read_only_on_a_fixture_copy() {
    let scratch = Scratch::new("fixture");
    let corpus = scratch.0.join("corpus");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-a");
    copy(&fixture, &corpus);
    pilot::git_init(&corpus);
    let config = scratch.0.join("specengine.toml");
    fs::copy(corpus.join("specengine.toml"), &config).expect("config copy");
    let home = scratch.0.join("home");
    fs::create_dir_all(&home).expect("home");
    let roots = vec![
        "crates".to_owned(),
        "data".to_owned(),
        "tools/loose.rs".to_owned(),
    ];
    let before = pilot::proof(&corpus, &roots);
    let counted = census(&corpus, &config, &home, &["GOAL-01".to_owned()]);
    assert!(before == pilot::proof(&corpus, &roots), "the copy changed");
    assert_eq!(counted.files, BTreeMap::from([("ron", 1), ("rs", 7)]));
    assert_eq!(counted.markers, 42);
    assert_eq!(counted.by_relation["configures"], 5);
    assert_eq!(counted.by_written_rev["bad_rev"], 3);
    // Markers naming a node: 42 less other_project, queue and 3 dangling.
    assert_eq!(counted.listed, 37);
    assert_eq!(counted.by_id_state["alias"], 1);
    assert_eq!(counted.by_rev_state["predated"], 2);
    assert_eq!(counted.naming_nothing, ["GOAL-99", "PR-7", "ZZZ-1"]);
    assert_eq!(counted.refused, ["other:GOAL-01"]);
    assert_eq!(counted.mentions_by_binding.values().sum::<usize>(), 4);
    print("fixture", &counted);
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    for entry in fs::read_dir(from).expect("fixture").flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
        }
    }
}
