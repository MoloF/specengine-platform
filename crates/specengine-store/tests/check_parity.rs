//! AC-19 and AC-20 of docs/features/spec-check.md, extended by AC-02, AC-06
//! and AC-07 of docs/features/spec-check-graph.md and AC-12 of
//! docs/features/spec-check-scopes.md: parity with `cargo xtask
//! docs check` and `cargo xtask docs index` on this repository's own
//! documents.
//!
//! The parity config is built in `parity_config/mod.rs` (Q-7; shared with
//! the CLI's `parity.rs` through a `#[path]` module): the Data example of
//! the spec with `roots` = the top-level `.md` files and the directories
//! not `.`-named nor in `xtask`'s `SKIP_DIRS` (read from
//! `xtask/src/docs/mod.rs`), `exclude` =
//! `**/_*.md` plus `**/<name>/**` per `SKIP_DIRS`, `[classes]` all four
//! closed as `docs/README.md` "Front-matter contract" (= `xtask`'s schema),
//! and the generator registry of spec-check-graph's Data (the index entry).
//! No baseline: since the owner's Q-2 edit every front-matter parses.
//!
//! AC-19/AC-07/AC-12: the walk equals `cargo xtask docs budget`'s list;
//! `enforce` → `clean`, no debt, the seven codes of increment 2 part 1 plus
//! `id-scope` and pass B's `link-dangling` / `link-anchor`
//! (docs/features/spec-check-links.md AC-10: the parity config has no
//! `link_base`) give exactly the one `mention-dangling` of the spec's
//! Findings; citing the superseded ADR-0002 adds one `ref-superseded`. AC-02: the core render equals
//! `xtask docs index --root` stdout and the committed `docs/index.md`, byte
//! for byte. AC-20/AC-06: per seed, on a scratch copy of the documents, the
//! files with a blocking finding from `xtask docs check --root` equal the
//! new check's, symmetrically, with nothing set aside (§11.5–6 and failed
//! front-matter included), and the seeded file blocks with its code.
//!
//! The repository is only read; seeds are applied to scratch copies. The
//! `xtask` binary is built once and run directly (no racing `cargo run`s).

mod common;
mod parity_config;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use common::{Scratch, blake3_hex, repository_root};
use parity_config::{INDEX, parity_toml};
use specengine_core::check::{CheckConfig, CheckInput, Report, Verdict, render_index};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::{IdScheme, Severity};
use specengine_store::{WorkingTree, check_input, check_worktree};

const TODAY: &str = "2026-09-29";

/// The codes increment 2 adds (spec-check-graph, Findings; part 2's
/// `id-scope`, spec-check-scopes AC-12; pass B's file link warnings,
/// spec-check-links AC-10).
const NEW_CODES: [&str; 10] = [
    "index-missing",
    "index-drift",
    "generator-unknown",
    "generator-path",
    "mention-dangling",
    "depends-cycle",
    "ref-superseded",
    "id-scope",
    "link-dangling",
    "link-anchor",
];

// ------------------------------------------------------------------ xtask

/// The `xtask` binary, built once per test process, run from a private
/// copy: every `cargo build` (fresh or not) and `cargo xtask` of another
/// test process re-links `target/debug/xtask`, so spawning that path races
/// with them (`NotFound`, seen under a full `nextest` run).
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
            .map(|built| private_copy(&built))
            .expect("the xtask executable in cargo's messages")
    })
}

/// `built` copied to `CARGO_TARGET_TMPDIR` under its content hash (written
/// to a per-process name, then renamed: every process sees a whole file);
/// read again while a concurrent re-link has it missing.
fn private_copy(built: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    for _ in 0..100 {
        let bytes = match fs::read(built) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::thread::sleep(std::time::Duration::from_millis(20));
                continue;
            }
            Err(error) => panic!("{}: {error}", built.display()),
        };
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
        fs::create_dir_all(dir).expect("CARGO_TARGET_TMPDIR");
        let copy = dir.join(format!("xtask-parity-{}", &blake3_hex(&bytes)[..16]));
        if !copy.exists() {
            let partial = dir.join(format!("xtask-parity.{}.partial", std::process::id()));
            fs::write(&partial, &bytes).expect("the copy is written");
            fs::set_permissions(&partial, fs::Permissions::from_mode(0o755)).expect("chmod");
            fs::rename(&partial, &copy).expect("the copy is renamed into place");
        }
        return copy;
    }
    panic!("{} stayed missing for 2 s", built.display());
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

/// Files with a finding of `xtask docs check --root`, every check included
/// (§11.5–6 too: nothing is set aside since increment 2).
fn xtask_blocking(root: &Path) -> BTreeSet<String> {
    let (_, stdout) = run_xtask(&["docs", "check", "--root", root.to_str().unwrap()]);
    assert!(stdout.contains("docs check: "), "no summary:\n{stdout}");
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("error  "))
        .map(|finding| finding.split(": ").next().unwrap().to_owned())
        .collect()
}

// ------------------------------------------------------------------ the new check

/// Writes the config to `dir` (outside every walked root).
fn write_config(dir: &Path, toml: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let config = dir.join("specengine.toml");
    fs::write(&config, toml).unwrap();
    config
}

fn tables(toml: &str) -> (IdScheme, Paths, CheckConfig) {
    (
        IdScheme::from_toml(toml).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml"))),
        Paths::from_toml(toml).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml"))),
        CheckConfig::from_toml(toml).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml"))),
    )
}

fn input_of(root: &Path, toml: &str) -> CheckInput {
    let (scheme, paths, _) = tables(toml);
    let tree = WorkingTree::new(root, &paths).expect("working tree");
    let input = check_input(&tree, &scheme);
    assert!(input.problems.is_empty(), "{:?}", input.problems);
    input
}

fn walked(root: &Path, toml: &str) -> BTreeSet<String> {
    input_of(root, toml)
        .files
        .into_iter()
        .map(|file| file.path)
        .collect()
}

/// Files blocking under `enforce`: every finding counts, nothing aside.
fn new_blocking(report: &Report) -> BTreeSet<String> {
    assert!(
        report.cannot_check.is_empty(),
        "cannot check: {:?}",
        report.cannot_check
    );
    report
        .findings
        .iter()
        .filter(|f| report.blocks(f))
        .map(|f| f.path.clone())
        .collect()
}

/// Every file under `root` with its bytes (AC-15: the check writes nothing).
fn snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.insert(path.clone(), fs::read(&path).unwrap());
            }
        }
    }
    files
}

fn git_status(root: &Path) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
        ])
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git status");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The first line where `a` and `b` differ, for assertion messages.
fn first_difference(a: &str, b: &str) -> String {
    let line = a
        .lines()
        .zip(b.lines())
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.lines().count().min(b.lines().count()));
    format!(
        "line {}:\n  left:  {:?}\n  right: {:?}\n(len {} vs {})",
        line + 1,
        a.lines().nth(line),
        b.lines().nth(line),
        a.len(),
        b.len()
    )
}

// ------------------------------------------------------------------ AC-19, AC-07

#[test]
fn the_parity_config_walks_the_budget_documents_and_is_clean() {
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
    let config = write_config(&scratch.join("config"), &toml);
    // AC-15: the repository is only read.
    let status = git_status(&repository);
    let index_bytes = fs::read(repository.join(INDEX)).expect("the index");
    let report = check_worktree(&repository, &config, None, TODAY);
    assert_eq!(
        git_status(&repository),
        status,
        "the check wrote into the repository"
    );
    assert_eq!(fs::read(repository.join(INDEX)).unwrap(), index_bytes);
    assert_eq!(report.verdict, Verdict::Clean, "{:#?}", report.lines(true));
    assert_eq!(report.counts.documents, budget.len());
    assert_eq!(
        (
            report.counts.debt,
            report.counts.errors,
            report.counts.stale,
            report.counts.expired
        ),
        (0, 0, 0, 0),
        "{:#?}",
        report.lines(true)
    );
    // The new codes, `id-scope` included: exactly the one finding of the
    // spec's Findings (the file-name example of docs/canon/spec-check.md).
    let new: Vec<(String, Severity, String, usize, String)> = report
        .findings
        .iter()
        .filter(|f| NEW_CODES.contains(&f.code.as_str()))
        .map(|f| {
            (
                f.code.clone(),
                f.severity,
                f.path.clone(),
                f.line,
                f.subject.clone(),
            )
        })
        .collect();
    assert_eq!(
        new,
        [(
            "mention-dangling".to_owned(),
            Severity::Warning,
            "docs/canon/spec-check.md".to_owned(),
            50,
            "ADR-00011".to_owned()
        )],
        "{:#?}",
        report.lines(true)
    );
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.severity != Severity::Error),
        "{:#?}",
        report.lines(true)
    );
    // spec-check-scopes AC-12: that warning is the only finding of any code,
    // and the canon document the pass shipped is walked and clean.
    let all: Vec<(&str, &str, usize, &str)> = report
        .findings
        .iter()
        .map(|f| (f.code.as_str(), f.path.as_str(), f.line, f.subject.as_str()))
        .collect();
    assert_eq!(
        all,
        [(
            "mention-dangling",
            "docs/canon/spec-check.md",
            50,
            "ADR-00011"
        )],
        "{:#?}",
        report.lines(true)
    );
    assert_eq!(report.counts.warnings, 1, "{:#?}", report.lines(true));
    assert!(
        budget.contains("docs/canon/spec-check-links.md"),
        "the scope canon is walked"
    );
    // xtask agrees: it blocks nothing, §11.5–6 included.
    let (code, stdout) = run_xtask(&["docs", "check", "--root", repository.to_str().unwrap()]);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(xtask_blocking(&repository).is_empty(), "{stdout}");
}

/// AC-10 of docs/features/spec-check-links.md: this repository's file
/// links under the parity config (no `link_base`). The root `README.md`
/// records 9 (7 `.md` targets that resolve; `docs/decisions/` and `LICENSE`
/// recorded, never checked); the platform spec's `README.md` 6 sibling
/// links that resolve; no link finding anywhere (the clean pin above).
#[test]
fn this_repository_s_file_links_are_recorded_and_resolve() {
    use specengine_model::{LinkOrigin, LinkTarget};
    let repository = repository_root();
    let toml = parity_toml(&repository, true);
    assert!(!toml.contains("link_base"), "the parity config has no base");
    let input = input_of(&repository, &toml);
    let file_links = |path: &str| -> Vec<String> {
        let file = input
            .files
            .iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("{path} is walked"));
        file.parsed
            .as_ref()
            .expect("parsed")
            .links
            .iter()
            .filter(|link| link.origin == LinkOrigin::Inline)
            .filter_map(|link| match &link.dst {
                LinkTarget::Path(target) => Some(target.path.clone()),
                LinkTarget::Reference(_) => None,
            })
            .collect()
    };
    let walked: BTreeSet<&str> = input.files.iter().map(|file| file.path.as_str()).collect();
    let readme = file_links("README.md");
    assert_eq!(readme.len(), 9, "{readme:#?}");
    let (checked, unchecked): (Vec<&String>, Vec<&String>) =
        readme.iter().partition(|path| path.ends_with(".md"));
    assert_eq!(unchecked, ["docs/decisions/", "LICENSE"], "{readme:#?}");
    for path in &checked {
        assert!(
            walked.contains(path.as_str()),
            "README.md -> {path} is walked"
        );
    }
    let platform = "docs/specs/specengine-platform/README.md";
    let siblings = file_links(platform);
    assert_eq!(siblings.len(), 6, "{siblings:#?}");
    for path in &siblings {
        assert!(!path.contains('/'), "a sibling: {path}");
        let target = format!("docs/specs/specengine-platform/{path}");
        assert!(walked.contains(target.as_str()), "{platform} -> {target}");
    }
    let (scheme, paths, config) = tables(&toml);
    let report = specengine_core::check::run(
        &input,
        &scheme,
        &paths,
        &config,
        &specengine_core::check::Baseline::empty(),
        TODAY,
    );
    let links: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("link-"))
        .map(|f| format!("{}:{}: {} {}", f.path, f.line, f.code, f.subject))
        .collect();
    assert!(links.is_empty(), "{links:#?}");
}

/// AC-12 of docs/features/spec-check-scopes.md, its named red: the
/// superseded ADR-0002 put back into the `adrs:` of
/// `docs/specs/specengine-platform/README.md` (on a scratch copy) gives
/// exactly one `ref-superseded` there, so the clean pin above would fail.
#[test]
fn citing_the_superseded_layout_decision_is_one_ref_superseded() {
    let documents = budget_documents(&repository_root());
    let scratch = Scratch::new("parity-superseded");
    let root = scratch_copy(&scratch, &documents, "copy");
    let readme = "docs/specs/specengine-platform/README.md";
    let text = fs::read_to_string(root.join(readme)).unwrap();
    let adrs = text
        .lines()
        .find(|line| line.starts_with("adrs: [ADR-0001, "))
        .expect("the README cites its ADRs")
        .to_owned();
    assert!(!adrs.contains("ADR-0002"), "{adrs}");
    set_key(
        &root,
        readme,
        "adrs",
        Some(&adrs.replacen("ADR-0001, ", "ADR-0001, ADR-0002, ", 1)),
    );
    let config = write_config(&scratch.join("config"), &parity_toml(&root, true));
    let report = check_worktree(&root, &config, None, TODAY);
    let superseded: Vec<(&str, &str, &str)> = report
        .findings
        .iter()
        .filter(|f| f.code == "ref-superseded")
        .map(|f| (f.path.as_str(), f.subject.as_str(), f.message.as_str()))
        .collect();
    assert_eq!(
        superseded,
        [(readme, "ADR-0002", "`ADR-0002` is superseded by ADR-0026")],
        "{:#?}",
        report.lines(true)
    );
    // A warning: the verdict stays clean.
    assert_eq!(report.verdict, Verdict::Clean, "{:#?}", report.lines(true));
}

// ------------------------------------------------------------------ AC-02

#[test]
fn the_core_render_is_xtask_s_index_and_the_committed_one() {
    let repository = repository_root();
    let toml = parity_toml(&repository, true);
    let (_, paths, check) = tables(&toml);
    assert_eq!(paths.index.as_deref(), Some(INDEX));
    let generator = check.index_generator().expect("the index entry");
    let mut input = input_of(&repository, &toml);
    let render = render_index(&input, INDEX, generator);

    let (code, stdout) = run_xtask(&["docs", "index", "--root", repository.to_str().unwrap()]);
    assert_eq!(code, Some(0), "xtask docs index");
    assert!(
        render == stdout,
        "core render vs xtask docs index stdout, {}",
        first_difference(&render, &stdout)
    );
    let committed = fs::read_to_string(repository.join(INDEX)).expect("the committed index");
    assert!(
        render == committed,
        "core render vs the committed {INDEX}, {}",
        first_difference(&render, &committed)
    );
    // Every section is exercised here but `No class — fix`.
    for section in [
        "\n## Canon\n\n",
        "\n## Decisions\n\n",
        "\n## Specs\n\n",
        "\n## Archive — Tier 3, by id only\n\n",
    ] {
        assert!(render.contains(section), "{section:?}");
    }
    // Independent of the walk's order.
    input.files.reverse();
    assert_eq!(render_index(&input, INDEX, generator), render);
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
    // ADR-0002 is superseded by ADR-0026 (spec-check-scopes AC-12): the
    // accepted decision to strip is ADR-0026.
    (
        "accepted ADR without canon:",
        "docs/decisions/ADR-0026.md",
        "canon-missing",
        |r| set_key(r, "docs/decisions/ADR-0026.md", "canon", None),
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
    // Increment 2 (spec-check-graph AC-06): §11.5–6.
    ("docs/index.md hand-edited", INDEX, "index-drift", |r| {
        replace_once(
            r,
            INDEX,
            "# Documentation index\n",
            "# Documentation Index\n",
        )
    }),
    (
        "a new canon document, not rendered",
        INDEX,
        "index-drift",
        |r| {
            write_new(
                r,
                "docs/canon/zz-seed.md",
                "---\nclass: canon\ntier: 2\nscope: [seed]\nowner: owner\nreviewed: 2026-09-29\n---\n\n# A seeded canon document\n\nText.\n",
            )
        },
    ),
    (
        "ADR-0010 superseded-by ADR-0001, not rendered",
        INDEX,
        "index-drift",
        |r| {
            set_key(
                r,
                "docs/decisions/ADR-0010.md",
                "status",
                Some("status: superseded-by ADR-0001"),
            )
        },
    ),
    ("docs/index.md deleted", INDEX, "index-missing", |r| {
        fs::remove_file(r.join(INDEX)).unwrap()
    }),
    (
        "a generated document by `foo`",
        "docs/zz-generated-foo.md",
        "generator-unknown",
        |r| {
            write_new(
                r,
                "docs/zz-generated-foo.md",
                "---\nclass: generated\ngenerator: foo\nsource: a seed\n---\n\n# Generated by foo\n",
            )
        },
    ),
    (
        "a generated document naming the index command",
        "docs/zz-generated-index.md",
        "generator-path",
        |r| {
            write_new(
                r,
                "docs/zz-generated-index.md",
                "---\nclass: generated\ngenerator: cargo xtask docs index --write\nsource: a seed\n---\n\n# Not the index\n",
            )
        },
    ),
];

/// Replaces the one occurrence of `from` in `path`.
fn replace_once(root: &Path, path: &str, from: &str, to: &str) {
    let file = root.join(path);
    let text = fs::read_to_string(&file).unwrap();
    assert_eq!(text.matches(from).count(), 1, "{path}: {from:?} once");
    fs::write(&file, text.replacen(from, to, 1)).unwrap();
}

/// Writes a file that must not exist yet.
fn write_new(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    assert!(!file.exists(), "{path} exists");
    fs::write(&file, text).unwrap();
}

#[test]
fn per_seed_xtask_and_the_new_check_block_the_same_files() {
    assert_eq!(SEEDS.len(), 24 + 6, "the 24 old seeds and the six new");
    let documents = budget_documents(&repository_root());
    let scratch = Scratch::new("parity-seeds");

    // The unseeded copy: both clean, nothing set aside.
    let clean = scratch_copy(&scratch, &documents, "clean");
    let toml = parity_toml(&clean, true);
    let config = write_config(&scratch.join("config-clean"), &toml);
    assert_eq!(walked(&clean, &toml), documents, "the copy walks the same");
    let report = check_worktree(&clean, &config, None, TODAY);
    assert_eq!(report.verdict, Verdict::Clean, "{:#?}", report.lines(true));
    assert!(new_blocking(&report).is_empty());
    assert!(xtask_blocking(&clean).is_empty());

    let mut failures = Vec::new();
    for (index, (name, target, code, apply)) in SEEDS.iter().enumerate() {
        let root = scratch_copy(&scratch, &documents, &format!("seed-{index:02}"));
        apply(&root);
        let config = write_config(
            &scratch.join(&format!("config-{index:02}")),
            &parity_toml(&root, true),
        );
        // AC-15: nothing is written, a drifted index included.
        let before = snapshot(&root);
        let report = check_worktree(&root, &config, None, TODAY);
        assert!(
            snapshot(&root) == before,
            "seed {name}: the check wrote a file"
        );
        let ours = new_blocking(&report);
        let theirs = xtask_blocking(&root);
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
