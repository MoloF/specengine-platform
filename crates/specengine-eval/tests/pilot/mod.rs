//! The pilot-run plumbing shared by `index_cli.rs`, `check_cli.rs`,
//! `parse_cli.rs` and `import_cli.rs`
//! (`crates/specengine-eval/README.md` "Pilot runs and tests"): the label's
//! variables, the `[paths]` requirement, the read-only proof, the child
//! environment, and the invented setup of docs/features/pilot-schemes.md
//! AC-01 (a scratch copy of `fixtures/spec-b`, a test-written scheme with
//! every recipe table, a test-written census config). A measurement that
//! reads no scheme (`import`, docs/features/import-records.md AC-10) proves
//! over the census config's corpus and code roots instead
//! (`run_census_read_only`).
//!
//! Non-Latin text is built from `\u{...}` escapes at test time; nothing here
//! names a pilot.

// Each test file uses its own subset (the proof units live in `index_cli.rs`).
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

/// The variables of one pilot label.
pub struct Variables {
    pub corpus: &'static str,
    pub scheme: &'static str,
    pub config: &'static str,
}

pub fn variables(label: &str) -> Variables {
    match label {
        "pilot-a" => Variables {
            corpus: "SPECENGINE_PILOT_A",
            scheme: "SPECENGINE_SCHEME_A",
            config: "SPECENGINE_CENSUS_CONFIG_A",
        },
        "pilot-b" => Variables {
            corpus: "SPECENGINE_PILOT_B",
            scheme: "SPECENGINE_SCHEME_B",
            config: "SPECENGINE_CENSUS_CONFIG_B",
        },
        other => panic!("{other:?} is not a pilot label (pilot-a, pilot-b)"),
    }
}

/// The path in `variable`; unset or empty fails naming it.
pub fn required(variable: &str, what: &str) -> PathBuf {
    match std::env::var_os(variable).filter(|value| !value.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => panic!("set {variable} to {what} (outside the repository) to run this test"),
    }
}

/// The path in `variable` when set and not empty.
pub fn optional(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The walked roots of `scheme`, as the engine reads them. A scheme without
/// a `[paths]` table fails naming `scheme_variable`; nothing is skipped.
pub fn scheme_roots(scheme: &Path, scheme_variable: &str) -> Vec<String> {
    let text = fs::read_to_string(scheme)
        .unwrap_or_else(|error| panic!("{scheme_variable}: the scheme is unreadable: {error}"));
    let table: toml::Table = text
        .parse()
        .unwrap_or_else(|error| panic!("{scheme_variable}: the scheme is not TOML: {error}"));
    if !table.contains_key("paths") {
        panic!(
            "{scheme_variable}: the scheme sets no [paths] table; a pilot run needs its roots \
             (docs/features/pilot-schemes.md, recipe step 2)"
        );
    }
    specengine_core::Paths::from_toml(&text)
        .unwrap_or_else(|error| panic!("{scheme_variable}: {}", error.at("the scheme")))
        .roots
}

/// One entry under a scheme root: corpus-relative path, kind, size, mtime
/// in nanoseconds.
pub type Entry = (PathBuf, &'static str, u64, i128);

/// The read-only proof: the corpus's `git status` and the listing of every
/// entry under each scheme root.
#[derive(Debug, PartialEq, Eq)]
pub struct Proof {
    pub status: String,
    pub entries: BTreeSet<Entry>,
}

/// `git --no-optional-locks status --porcelain --untracked-files=all` in
/// `corpus` (no git worktree fails) and the sorted (path, kind, size, mtime
/// ns) of every entry under each root, directories included, symlinks not
/// followed.
pub fn proof(corpus: &Path, roots: &[String]) -> Proof {
    let output = Command::new("git")
        .current_dir(corpus)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
        ])
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "the corpus must be a git worktree for the read-only proof: git status exit {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut entries = BTreeSet::new();
    for root in roots {
        list(corpus, Path::new(root), &mut entries);
    }
    Proof {
        status: String::from_utf8_lossy(&output.stdout).into_owned(),
        entries,
    }
}

fn list(corpus: &Path, relative: &Path, out: &mut BTreeSet<Entry>) {
    let path = corpus.join(relative);
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        out.insert((relative.to_path_buf(), "missing", 0, 0));
        return;
    };
    let file_type = metadata.file_type();
    let kind = if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_dir() {
        "dir"
    } else if file_type.is_file() {
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

/// What one pilot-shaped run reads: the corpus, the scheme and (for
/// `parse`, or to pass it on) the census config.
pub struct Inputs<'a> {
    pub corpus: &'a Path,
    pub scheme: &'a Path,
    pub config: Option<&'a Path>,
}

/// Runs `specengine-eval <measurement> --label <label> --out <scratch>/out
/// --timeout 600` read-only: the scheme must set `[paths]` (else fails
/// naming the label's scheme variable, before anything runs); the child
/// gets only `PATH`, `HOME` = the empty `<scratch>/home` (also its working
/// directory) and the label's corpus, scheme and census-config variables;
/// fails unless the exit is 0, the proof is equal before and after, and
/// `<scratch>/home` is still empty.
pub fn run_read_only(measurement: &str, label: &str, scratch: &Path, inputs: &Inputs) -> Output {
    let variables = variables(label);
    let roots = scheme_roots(inputs.scheme, variables.scheme);
    let mut set = vec![
        (variables.corpus, inputs.corpus),
        (variables.scheme, inputs.scheme),
    ];
    if let Some(config) = inputs.config {
        set.push((variables.config, config));
    }
    run_read_only_over(measurement, label, scratch, inputs.corpus, &roots, &set)
}

/// The walked roots of the census config at `config`, as the engine reads
/// them: `[corpus] roots` (default `.`), then `[code] roots`. A config the
/// engine refuses fails naming `config_variable`.
pub fn census_roots(config: &Path, config_variable: &str) -> Vec<String> {
    let config = specengine_import::CensusConfig::load(config)
        .unwrap_or_else(|error| panic!("{config_variable}: {error}"));
    config
        .roots
        .iter()
        .chain(&config.import.code.roots)
        .map(|root| {
            root.to_str()
                .unwrap_or_else(|| panic!("{config_variable}: a root is not UTF-8: {root:?}"))
                .to_owned()
        })
        .collect()
}

/// `run_read_only` for a measurement that reads no scheme: the proof covers
/// the census config's corpus and code roots (`census_roots`); the child
/// gets only `PATH`, `HOME` = the empty `<scratch>/home` and the label's
/// corpus and census-config variables.
pub fn run_census_read_only(
    measurement: &str,
    label: &str,
    scratch: &Path,
    corpus: &Path,
    config: &Path,
) -> Output {
    let variables = variables(label);
    let roots = census_roots(config, variables.config);
    run_read_only_over(
        measurement,
        label,
        scratch,
        corpus,
        &roots,
        &[(variables.corpus, corpus), (variables.config, config)],
    )
}

/// The run both entry points share: the proof over `roots` of `corpus`
/// before and after, the child with `PATH`, `HOME` = the empty
/// `<scratch>/home` (also its working directory) and `set` only; exit 0,
/// the proof equal and `HOME` still empty, else fails.
pub fn run_read_only_over(
    measurement: &str,
    label: &str,
    scratch: &Path,
    corpus: &Path,
    roots: &[String],
    set: &[(&str, &Path)],
) -> Output {
    let home = scratch.join("home");
    let out = scratch.join("out");
    fs::create_dir_all(&home).expect("the scratch HOME");
    assert!(empty(&home), "the scratch HOME starts empty");
    assert!(!out.exists(), "--out must not exist before the run");
    let before = proof(corpus, roots);
    let mut command = Command::new(BIN);
    command.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    command.current_dir(&home).env("HOME", &home);
    for (variable, value) in set {
        command.env(variable, value);
    }
    let output = command
        .args([
            measurement,
            "--label",
            label,
            "--out",
            out.to_str().expect("a UTF-8 scratch path"),
            "--timeout",
            "600",
        ])
        .output()
        .expect("specengine-eval runs");
    let after = proof(corpus, roots);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{measurement} --label {label}: exit {:?}, stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        before == after,
        "{measurement} --label {label}: the read-only proof changed (git status or an entry \
         under a scheme root)\nbefore: {before:?}\nafter: {after:?}"
    );
    assert!(
        empty(&home),
        "{measurement} --label {label}: the child wrote under HOME: {:?}",
        fs::read_dir(&home)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>()
    );
    output
}

fn empty(dir: &Path) -> bool {
    fs::read_dir(dir).expect("readable").next().is_none()
}

// ------------------------------------------------------- the AC-01 setup

/// The AC-01 setup under `scratch`: the corpus, the scheme and the census
/// config.
pub struct Setup {
    pub corpus: PathBuf,
    pub scheme: PathBuf,
    pub config: PathBuf,
}

impl Setup {
    pub fn inputs(&self) -> Inputs<'_> {
        Inputs {
            corpus: &self.corpus,
            scheme: &self.scheme,
            config: Some(&self.config),
        }
    }
}

/// The `[paths]` table of the recipe; the corpus gets the files its
/// single-file roots, `index` and the generator name, and one template the
/// `exclude` glob drops.
const PATHS: &str = r#"[paths]
roots      = ["AGENTS.md", "docs", "crates/engine/README.md"]
tier0      = "AGENTS.md"
tier1_name = "README.md"
index      = "docs/index.md"
exclude    = ["**/_*.md"]
link_base  = "docs"
"#;

/// The scheme of the "before" recipe over spec-b: every table and key of
/// the example; the one `aliases_from` is spec-b's legacy requirement
/// prefix, written into the file from `\u{...}` escapes; one
/// `[[generators]]`, without `index`.
pub fn scheme_text(with_paths: bool) -> String {
    let alias = "\u{0422}\u{0420}\u{0411}";
    format!(
        r#"[project]
slug = "pilot-a"
{paths}[ids]
REQ  = {{ kind = "requirement", width = 3, aliases_from = ["{alias}"] }}
ASM  = {{ kind = "assumption",  width = 2 }}
QN   = {{ kind = "question",    width = 2 }}
ADR  = {{ kind = "decision",    width = 4 }}
GLS  = {{ kind = "term",        shape = "name" }}
MOD  = {{ kind = "module",      shape = "name" }}
CMD  = {{ kind = "command",     shape = "name" }}
FLAG = {{ kind = "flag",        shape = "name" }}
[budgets]
tier0_bytes = 16384
tier1_bytes = 10240
index_bytes = 10240
decision_bytes = 1536
[classes]
spec = {{ required = ["class", "status", "scope"], optional = ["ref", "shipped", "adrs"] }}
[[generators]]
command = "make docs-map"
writes  = ["docs/generated/map.md"]
[check]
mode = "observe"
"#,
        paths = if with_paths { PATHS } else { "" },
    )
}

/// A census config over the copy's `docs/`.
pub const CENSUS: &str = r#"[corpus]
roots = ["docs"]
[front_matter]
class_key = "class"
[ids]
regex = '^[A-Z]{2,4}-[0-9]{2,4}$'
[tables]
id_column = 0
[sections]
id_attr = true
"#;

/// Files the scheme names, invented; `docs/_template.md` is dropped by the
/// `exclude` glob.
const ADDED: [(&str, &str); 5] = [
    (
        "AGENTS.md",
        "# Agent guide\n\nStart at docs/index.md. The command line is MOD-CLI; the first requirement is REQ-001.\n",
    ),
    (
        "crates/engine/README.md",
        "# Engine\n\nThe synchroniser engine; it serves REQ-001.\n",
    ),
    (
        "docs/index.md",
        "# Index\n\n- [Command line](spec/cli.md)\n- [Dry run](features/dry-run.md)\n",
    ),
    (
        "docs/generated/map.md",
        "---\nclass: generated\ngenerator: make docs-map\n---\n\n# Map\n\nGenerated; not edited by hand.\n",
    ),
    (
        "docs/_template.md",
        "---\nclass: spec\nstatus: draft\nscope: []\n---\n\n# Template\n",
    ),
];

/// `.md` files the scheme's walk finds in the setup: spec-b's documents,
/// the added ones, the template excluded.
pub fn walked_files(setup: &Setup) -> usize {
    fn count(dir: &Path) -> usize {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .map(|path| {
                if path.is_dir() {
                    count(&path)
                } else {
                    usize::from(path.extension().is_some_and(|ext| ext == "md"))
                }
            })
            .sum()
    }
    count(&setup.corpus.join("docs")) - 1 + 2
}

/// The AC-01 setup under `scratch` (which must exist): a copy of
/// `fixtures/spec-b` without its `specengine.toml`, the files the scheme
/// names, `git init -q` with nothing committed; the scheme and the census
/// config beside the corpus, outside it.
pub fn invented_setup(scratch: &Path, with_paths: bool) -> Setup {
    let corpus = scratch.join("corpus");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/spec-b")
        .canonicalize()
        .expect("fixtures/spec-b exists");
    copy_files(&fixture, &corpus);
    fs::remove_file(corpus.join("specengine.toml")).expect("spec-b has a specengine.toml");
    for (relative, text) in ADDED {
        let path = corpus.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let init = Command::new("git")
        .current_dir(&corpus)
        .args(["init", "-q"])
        .output()
        .expect("git runs");
    assert!(
        init.status.success(),
        "git init: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let scheme = scratch.join("scheme-a.toml");
    fs::write(&scheme, scheme_text(with_paths)).unwrap();
    let config = scratch.join("census-a.toml");
    fs::write(&config, CENSUS).unwrap();
    Setup {
        corpus,
        scheme,
        config,
    }
}

fn copy_files(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_files(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
