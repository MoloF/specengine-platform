//! AC-19 and AC-20 of docs/features/spec-check.md: parity with
//! `cargo xtask docs check` on this repository's own documents.
//!
//! The parity config is built here (Q-7): the Data example of the spec with
//! `roots` = the top-level `.md` files and the directories not `.`-named nor
//! in `xtask`'s `SKIP_DIRS` (read from `xtask/src/docs/mod.rs`), `exclude` =
//! `**/_*.md` plus `**/<name>/**` per `SKIP_DIRS`, and `[classes]` all four
//! closed as `docs/README.md` "Front-matter contract" (= `xtask`'s schema).
//! The A4 baseline: the four invalid-YAML front-matters and the three
//! `ref-dangling` from `docs/specs/specengine-platform/README.md`.
//!
//! AC-19: the walk equals `cargo xtask docs budget`'s list; `enforce` + A4
//! → `clean`, debt = the seven. AC-20: per seed, on a scratch copy of the
//! documents, the files with a blocking finding from `xtask docs check
//! --root` (§11.5–6 aside) equal the new check's, among files whose
//! front-matter parses.
//!
//! The repository is only read; seeds are applied to scratch copies. The
//! `xtask` binary is built once and run directly (no racing `cargo run`s).

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use common::{Scratch, repository_root};
use specengine_core::check::{Report, Verdict};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;
use specengine_store::{WorkingTree, check_input, check_worktree};

/// Fixed so that the A4 baseline (expires 2026-12-31) stays live.
const TODAY: &str = "2026-09-29";

/// The A4 baseline (spec, Q-2).
const A4: &str = "\
[[debt]]
code    = \"frontmatter-yaml\"
path    = \"docs/decisions/ADR-0015.md\"
reason  = \"core Q6\"
expires = \"2026-12-31\"

[[debt]]
code    = \"frontmatter-yaml\"
path    = \"docs/decisions/ADR-0018.md\"
reason  = \"core Q6\"
expires = \"2026-12-31\"

[[debt]]
code    = \"frontmatter-yaml\"
path    = \"docs/decisions/ADR-0020.md\"
reason  = \"core Q6\"
expires = \"2026-12-31\"

[[debt]]
code    = \"frontmatter-yaml\"
path    = \"docs/features/phase-0-spikes.md\"
reason  = \"core Q6\"
expires = \"2026-12-31\"

[[debt]]
code    = \"ref-dangling\"
path    = \"docs/specs/specengine-platform/README.md\"
subject = \"ADR-0015\"
reason  = \"core Q6\"
expires = \"2026-12-31\"

[[debt]]
code    = \"ref-dangling\"
path    = \"docs/specs/specengine-platform/README.md\"
subject = \"ADR-0018\"
reason  = \"core Q6\"
expires = \"2026-12-31\"

[[debt]]
code    = \"ref-dangling\"
path    = \"docs/specs/specengine-platform/README.md\"
subject = \"ADR-0020\"
reason  = \"core Q6\"
expires = \"2026-12-31\"
";

/// Parser codes after which a front-matter is not read.
const FRONT_MATTER_FAILED: [&str; 4] = [
    "not-utf8",
    "frontmatter-unclosed",
    "frontmatter-yaml",
    "frontmatter-not-mapping",
];

// ------------------------------------------------------------------ xtask

/// The `xtask` binary, built once per test process.
fn xtask() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(repository_root())
            .args([
                "build",
                "--quiet",
                "--package",
                "xtask",
                "--message-format=json",
            ])
            .output()
            .expect("cargo build -p xtask runs");
        assert!(
            output.status.success(),
            "cargo build -p xtask: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|message| message["target"]["name"] == "xtask")
            .find_map(|message| message["executable"].as_str().map(PathBuf::from))
            .expect("the xtask executable in cargo's messages")
    })
}

fn run_xtask(args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(xtask())
        .args(args)
        .output()
        .expect("xtask runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// `xtask`'s `SKIP_DIRS`, read from its source.
fn skip_dirs() -> Vec<String> {
    let source = fs::read_to_string(repository_root().join("xtask/src/docs/mod.rs"))
        .expect("xtask/src/docs/mod.rs");
    let start = source
        .find("const SKIP_DIRS: &[&str] = &[")
        .expect("SKIP_DIRS in xtask");
    let block = &source[start..];
    let block = &block[..block.find("];").expect("end of SKIP_DIRS")];
    let dirs: Vec<String> = block
        .lines()
        .skip(1)
        .filter_map(|line| line.split('"').nth(1))
        .map(str::to_owned)
        .collect();
    assert!(
        dirs.len() >= 5 && dirs.iter().any(|d| d == "fixtures"),
        "{dirs:?}"
    );
    dirs
}

/// The documents `xtask docs budget` lists for `root`.
fn budget_documents(root: &Path) -> BTreeSet<String> {
    let (code, stdout) = run_xtask(&["docs", "budget", "--root", root.to_str().unwrap()]);
    let mut lines = stdout.lines();
    let header = lines.next().unwrap_or_default();
    assert!(
        header.starts_with("document") && header.contains("class"),
        "unexpected budget output (exit {code:?}):\n{stdout}"
    );
    lines
        .take_while(|line| !line.trim().is_empty())
        .map(|line| line.split_whitespace().next().unwrap().to_owned())
        .collect()
}

/// Files with a finding of `xtask docs check --root`, §11.5–6 aside (the
/// index and generated documents, the generator registry: increment 2).
fn xtask_blocking(root: &Path) -> BTreeSet<String> {
    let (_, stdout) = run_xtask(&["docs", "check", "--root", root.to_str().unwrap()]);
    assert!(stdout.contains("docs check: "), "no summary:\n{stdout}");
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("error  "))
        .filter(|finding| {
            !(finding.contains("drifted from front-matter")
                || finding.contains("index is missing")
                || finding.contains(": generator `"))
        })
        .map(|finding| finding.split(": ").next().unwrap().to_owned())
        .collect()
}

// ------------------------------------------------------------------ the new check

/// The parity config for the documents under `root`.
fn parity_toml(root: &Path, with_template_exclude: bool) -> String {
    let skip = skip_dirs();
    let mut roots = Vec::new();
    for entry in fs::read_dir(root).expect("root listing") {
        let entry = entry.unwrap();
        let name = entry.file_name().to_str().expect("UTF-8 name").to_owned();
        let kind = entry.file_type().unwrap();
        let is_md = kind.is_file() && name.ends_with(".md") && !name.starts_with('_');
        let is_dir = kind.is_dir() && !name.starts_with('.') && !skip.contains(&name);
        if is_md || is_dir {
            roots.push(format!("{name:?}"));
        }
    }
    roots.sort();
    let mut exclude = Vec::new();
    if with_template_exclude {
        exclude.push("\"**/_*.md\"".to_owned());
    }
    exclude.extend(skip.iter().map(|dir| format!("\"**/{dir}/**\"")));
    format!(
        "\
[paths]
roots      = [{roots}]
records    = \"docs/decisions\"
tier0      = \"CLAUDE.md\"
tier1_name = \"README.md\"
index      = \"docs/index.md\"
exclude    = [{exclude}]

[ids]
ADR = {{ kind = \"decision\", width = 4 }}

[budgets]
tier0_bytes    = 16384
tier1_bytes    = 10240
index_bytes    = 10240
decision_bytes = 1536
canon_bytes    = 12288

[classes]
canon     = {{ required = [\"class\", \"tier\", \"scope\", \"owner\", \"reviewed\"], closed = true }}
decision  = {{ required = [\"class\", \"id\", \"title\", \"status\", \"date\", \"scope\"], optional = [\"canon\", \"supersedes\", \"ref\"], closed = true }}
spec      = {{ required = [\"class\", \"status\", \"scope\"], optional = [\"ref\", \"shipped\", \"adrs\"], closed = true }}
generated = {{ required = [\"class\", \"generator\", \"source\"], closed = true }}

[check]
mode = \"enforce\"
",
        roots = roots.join(", "),
        exclude = exclude.join(", ")
    )
}

/// Writes the config and the A4 baseline to `dir` (outside every walked root).
fn write_config(dir: &Path, toml: &str) -> (PathBuf, PathBuf) {
    fs::create_dir_all(dir).unwrap();
    let config = dir.join("specengine.toml");
    let baseline = dir.join("a4.toml");
    fs::write(&config, toml).unwrap();
    fs::write(&baseline, A4).unwrap();
    (config, baseline)
}

fn walked(root: &Path, toml: &str) -> BTreeSet<String> {
    let scheme = IdScheme::from_toml(toml).expect("[ids]");
    let paths = Paths::from_toml(toml).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    let tree = WorkingTree::new(root, &paths).expect("working tree");
    let input = check_input(&tree, &scheme);
    assert!(input.problems.is_empty(), "{:?}", input.problems);
    input.files.into_iter().map(|file| file.path).collect()
}

/// The files whose front-matter the new check could not read (the parser's
/// front-matter failures): outside the comparison on both sides (Rules'
/// divergence: `xtask` reads such YAML leniently).
fn front_matter_failed(report: &Report) -> BTreeSet<String> {
    report
        .findings
        .iter()
        .filter(|f| FRONT_MATTER_FAILED.contains(&f.code.as_str()))
        .map(|f| f.path.clone())
        .collect()
}

/// Files blocking under `enforce` among those whose front-matter parses.
fn new_blocking(report: &Report) -> BTreeSet<String> {
    assert!(
        report.cannot_check.is_empty(),
        "cannot check: {:?}",
        report.cannot_check
    );
    let failed = front_matter_failed(report);
    report
        .findings
        .iter()
        .filter(|f| report.blocks(f) && !failed.contains(&f.path))
        .map(|f| f.path.clone())
        .collect()
}

fn a4_paths() -> BTreeSet<String> {
    [
        "docs/decisions/ADR-0015.md",
        "docs/decisions/ADR-0018.md",
        "docs/decisions/ADR-0020.md",
        "docs/features/phase-0-spikes.md",
    ]
    .iter()
    .map(|p| (*p).to_owned())
    .collect()
}

// ------------------------------------------------------------------ AC-19

#[test]
fn the_parity_config_walks_the_budget_documents_and_is_clean_with_the_a4_baseline() {
    let repository = repository_root();
    let budget = budget_documents(&repository);
    assert!(budget.len() >= 45, "{} documents", budget.len());

    let toml = parity_toml(&repository, true);
    assert_eq!(
        walked(&repository, &toml),
        budget,
        "walked set vs xtask docs budget"
    );

    // The `_*.md` exclude is load-bearing: without it the templates join.
    let without = walked(&repository, &parity_toml(&repository, false));
    let extra: Vec<&String> = without.difference(&budget).collect();
    assert!(
        !extra.is_empty()
            && extra
                .iter()
                .all(|path| path.rsplit('/').next().unwrap().starts_with('_')),
        "without `**/_*.md`: {extra:?}"
    );

    let scratch = Scratch::new("parity-clean");
    let (config, baseline) = write_config(&scratch.join("config"), &toml);
    let report = check_worktree(&repository, &config, Some(&baseline), TODAY);
    assert_eq!(report.verdict, Verdict::Clean, "{:#?}", report.lines(true));
    assert_eq!(report.counts.documents, budget.len());
    assert_eq!(
        (
            report.counts.debt,
            report.counts.errors,
            report.counts.stale,
            report.counts.expired
        ),
        (7, 0, 0, 0),
        "{:#?}",
        report.lines(true)
    );
    let in_debt: BTreeSet<(String, String, String)> = report
        .findings
        .iter()
        .filter(|f| f.is_live_debt())
        .map(|f| (f.code.clone(), f.path.clone(), f.subject.clone()))
        .collect();
    let mut want = BTreeSet::new();
    for path in a4_paths() {
        want.insert(("frontmatter-yaml".to_owned(), path, String::new()));
    }
    for id in ["ADR-0015", "ADR-0018", "ADR-0020"] {
        want.insert((
            "ref-dangling".to_owned(),
            "docs/specs/specengine-platform/README.md".to_owned(),
            id.to_owned(),
        ));
    }
    assert_eq!(in_debt, want);
    // xtask agrees: nothing but the index/generated checks may speak.
    assert!(xtask_blocking(&repository).is_empty());
}

// ------------------------------------------------------------------ AC-20

/// A scratch copy of the repository's documents (the budget list).
fn scratch_copy(scratch: &Scratch, documents: &BTreeSet<String>, dir: &str) -> PathBuf {
    let root = scratch.join(dir);
    for path in documents {
        let to = root.join(path);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::copy(repository_root().join(path), &to).unwrap();
    }
    root
}

/// Replaces the one line starting with `key:` of the front-matter.
fn set_key(root: &Path, path: &str, key: &str, line: Option<&str>) {
    let file = root.join(path);
    let text = fs::read_to_string(&file).unwrap();
    let mut out = String::new();
    let mut found = 0;
    let mut fences = 0;
    for current in text.split_inclusive('\n') {
        if current.trim_end() == "---" {
            fences += 1;
        }
        if fences == 1 && current.starts_with(&format!("{key}:")) {
            found += 1;
            if let Some(line) = line {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        out.push_str(current);
    }
    assert_eq!(found, 1, "{path}: `{key}:` once in the front-matter");
    fs::write(&file, out).unwrap();
}

/// Adds a front-matter line after `after:`.
fn add_key(root: &Path, path: &str, after: &str, line: &str) {
    let file = root.join(path);
    let text = fs::read_to_string(&file).unwrap();
    let at = text
        .find(&format!("\n{after}:"))
        .unwrap_or_else(|| panic!("{path}: no `{after}:`"));
    let end = at + 1 + text[at + 1..].find('\n').unwrap() + 1;
    fs::write(&file, format!("{}{line}\n{}", &text[..end], &text[end..])).unwrap();
}

/// Pads the body so the file is exactly `size` bytes.
fn pad(root: &Path, path: &str, size: usize) {
    let file = root.join(path);
    let mut text = fs::read_to_string(&file).unwrap();
    assert!(
        text.len() + 2 < size,
        "{path} is already {} bytes",
        text.len()
    );
    let fill = size - text.len() - 2;
    text.push('\n');
    text.push_str(&"x".repeat(fill));
    text.push('\n');
    assert_eq!(text.len(), size);
    fs::write(&file, text).unwrap();
}

fn strip_front_matter(root: &Path, path: &str) {
    let file = root.join(path);
    let text = fs::read_to_string(&file).unwrap();
    let rest = text.strip_prefix("---\n").expect("front-matter");
    let end = rest.find("\n---\n").expect("closing fence");
    fs::write(&file, &rest[end + 5..]).unwrap();
}

type Seed = (&'static str, &'static str, &'static str, fn(&Path));

/// `(name, file expected among the blocking, the code the new check must
/// give it, apply)`. The code makes "any check off → its seed red" hold
/// where another check flags the same file (`id: ADR-001` is also a
/// `file-name`, a taken ID also a `file-name` under `records`).
const SEEDS: &[Seed] = &[
    ("CLAUDE.md 16 385 B", "CLAUDE.md", "budget", |r| {
        pad(r, "CLAUDE.md", 16_385)
    }),
    (
        "a README 10 241 B",
        "crates/specengine-model/README.md",
        "budget",
        |r| pad(r, "crates/specengine-model/README.md", 10_241),
    ),
    (
        "an ADR 1 537 B",
        "docs/decisions/ADR-0001.md",
        "budget",
        |r| pad(r, "docs/decisions/ADR-0001.md", 1_537),
    ),
    (
        "front-matter removed",
        "docs/features/spec-parser.md",
        "class-missing",
        |r| strip_front_matter(r, "docs/features/spec-parser.md"),
    ),
    (
        "class: memo",
        "docs/features/spec-index.md",
        "class-unknown",
        |r| {
            set_key(
                r,
                "docs/features/spec-index.md",
                "class",
                Some("class: memo"),
            )
        },
    ),
    ("owner removed", "docs/README.md", "key-missing", |r| {
        set_key(r, "docs/README.md", "owner", None)
    }),
    (
        "foo: bar",
        "crates/specengine-model/README.md",
        "key-extra",
        |r| {
            add_key(
                r,
                "crates/specengine-model/README.md",
                "reviewed",
                "foo: bar",
            )
        },
    ),
    (
        "reviewed: 2026-9-1",
        "xtask/README.md",
        "date-invalid",
        |r| set_key(r, "xtask/README.md", "reviewed", Some("reviewed: 2026-9-1")),
    ),
    (
        "tier: 1 on architecture.md",
        "docs/canon/architecture.md",
        "tier-invalid",
        |r| set_key(r, "docs/canon/architecture.md", "tier", Some("tier: 1")),
    ),
    (
        "shipped: removed from spec-index.md",
        "docs/features/spec-index.md",
        "shipped-missing",
        |r| set_key(r, "docs/features/spec-index.md", "shipped", None),
    ),
    (
        "accepted ADR without canon:",
        "docs/decisions/ADR-0002.md",
        "canon-missing",
        |r| set_key(r, "docs/decisions/ADR-0002.md", "canon", None),
    ),
    (
        "canon: without #",
        "docs/decisions/ADR-0003.md",
        "canon-form",
        |r| {
            set_key(
                r,
                "docs/decisions/ADR-0003.md",
                "canon",
                Some("canon: docs/canon/architecture.md"),
            )
        },
    ),
    (
        "canon: to a missing file",
        "docs/decisions/ADR-0004.md",
        "canon-file",
        |r| {
            set_key(
                r,
                "docs/decisions/ADR-0004.md",
                "canon",
                Some("canon: docs/canon/missing.md#apply"),
            )
        },
    ),
    (
        "canon: to a spec",
        "docs/decisions/ADR-0005.md",
        "canon-file",
        |r| {
            set_key(
                r,
                "docs/decisions/ADR-0005.md",
                "canon",
                Some("canon: docs/features/spec-index.md#why"),
            )
        },
    ),
    (
        "canon: to a missing anchor",
        "docs/decisions/ADR-0006.md",
        "canon-anchor",
        |r| {
            set_key(
                r,
                "docs/decisions/ADR-0006.md",
                "canon",
                Some("canon: docs/canon/architecture.md#nope"),
            )
        },
    ),
    (
        "ADR-0099 via superseded-by",
        "docs/decisions/ADR-0007.md",
        "ref-dangling",
        |r| {
            set_key(
                r,
                "docs/decisions/ADR-0007.md",
                "status",
                Some("status: superseded-by ADR-0099"),
            )
        },
    ),
    (
        "ADR-0099 via supersedes",
        "docs/decisions/ADR-0008.md",
        "ref-dangling",
        |r| {
            add_key(
                r,
                "docs/decisions/ADR-0008.md",
                "scope",
                "supersedes: [ADR-0099]",
            )
        },
    ),
    (
        "ADR-0099 via adrs",
        "docs/features/spec-parser.md",
        "ref-dangling",
        |r| {
            set_key(
                r,
                "docs/features/spec-parser.md",
                "adrs",
                Some("adrs: [ADR-0099]"),
            )
        },
    ),
    (
        "ADR-0002 as id: ADR-0001",
        "docs/decisions/ADR-0002.md",
        "id-taken",
        |r| set_key(r, "docs/decisions/ADR-0002.md", "id", Some("id: ADR-0001")),
    ),
    (
        "ADR-0003.md renamed 3.md",
        "docs/decisions/3.md",
        "file-name",
        |r| {
            fs::rename(
                r.join("docs/decisions/ADR-0003.md"),
                r.join("docs/decisions/3.md"),
            )
            .unwrap()
        },
    ),
    (
        "id: ADR-001",
        "docs/decisions/ADR-0004.md",
        "id-width",
        |r| set_key(r, "docs/decisions/ADR-0004.md", "id", Some("id: ADR-001")),
    ),
    // Beyond the spec's list: the other `xtask` checks.
    (
        "tier: 0 off CLAUDE.md",
        "docs/README.md",
        "tier-invalid",
        |r| set_key(r, "docs/README.md", "tier", Some("tier: 0")),
    ),
    (
        "status: done on a spec",
        "docs/features/spec-parser.md",
        "status-invalid",
        |r| {
            set_key(
                r,
                "docs/features/spec-parser.md",
                "status",
                Some("status: done"),
            )
        },
    ),
    (
        "scope: [] on canon",
        "xtask/README.md",
        "scope-empty",
        |r| set_key(r, "xtask/README.md", "scope", Some("scope: []")),
    ),
];

#[test]
fn per_seed_xtask_and_the_new_check_block_the_same_files() {
    let documents = budget_documents(&repository_root());
    let scratch = Scratch::new("parity-seeds");

    // The unseeded copy: both clean (A4 in debt, its four files aside).
    let clean = scratch_copy(&scratch, &documents, "clean");
    let toml = parity_toml(&clean, true);
    let (config, baseline) = write_config(&scratch.join("config-clean"), &toml);
    assert_eq!(walked(&clean, &toml), documents, "the copy walks the same");
    let report = check_worktree(&clean, &config, Some(&baseline), TODAY);
    assert_eq!(report.verdict, Verdict::Clean, "{:#?}", report.lines(true));
    assert!(xtask_blocking(&clean).is_empty());

    let a4 = a4_paths();
    let mut failures = Vec::new();
    for (index, (name, target, code, apply)) in SEEDS.iter().enumerate() {
        let root = scratch_copy(&scratch, &documents, &format!("seed-{index:02}"));
        apply(&root);
        let (config, baseline) = write_config(
            &scratch.join(&format!("config-{index:02}")),
            &parity_toml(&root, true),
        );
        let report = check_worktree(&root, &config, Some(&baseline), TODAY);
        let ours = new_blocking(&report);
        let failed = front_matter_failed(&report);
        assert!(
            failed.is_superset(&a4),
            "seed {name}: the A4 four still fail"
        );
        let theirs: BTreeSet<String> = xtask_blocking(&root)
            .into_iter()
            .filter(|path| !failed.contains(path))
            .collect();
        eprintln!("seed {name}: xtask {theirs:?}, spec check {ours:?}");
        let coded = report
            .findings
            .iter()
            .any(|f| f.path == *target && f.code == *code && report.blocks(f));
        if ours != theirs || !ours.contains(*target) || !coded {
            failures.push(format!(
                "seed {name}: xtask {theirs:?}, spec check {ours:?} (expected {target} with {code})\n  {}",
                report.lines(false).join("\n  ")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
