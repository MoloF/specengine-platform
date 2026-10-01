//! This repository's own documents under its committed root
//! `specengine.toml` (docs/features/spec-cli-switch.md, "Migrations": the
//! criteria that compared `spec check` with the retired second
//! implementation now hold against the committed files; the config and the
//! std walk live in `parity_config/mod.rs`, shared with the CLI's
//! `parity.rs`).
//!
//! AC-03: the walk equals an independent std walk (every `*.md` outside
//! `.`-named and the frozen skipped directories, not `_*`); its named
//! mutations are run in the test on the config text or a scratch copy:
//! `crates` out of `roots`, no `**/_*.md`, a new top-level directory with a
//! README. AC-02: `enforce` → `clean`, 0 errors, debt, expired and stale;
//! every finding pinned by (code, path, subject): none, the repository has
//! no finding; mutations in the test: `mode = "observe"`, an `[ids]` prefix
//! matching prose, the five-digit mention `ADR-00011` put back into
//! `docs/canon/spec-check.md` on a scratch copy (exactly that one
//! `mention-dangling`, the verdict still `clean`). Citing the
//! superseded ADR-0002 adds one `ref-superseded` (spec-check-scopes AC-12);
//! the file links resolve (spec-check-links AC-10). AC-04 (the library
//! half): the core render set is the committed `docs/index.md` and its
//! archive shard `docs/index-archive.md` byte for byte (index-shards AC-12),
//! each header naming X and G; every walked document but `class: generated`
//! is listed once across the set, the shard's Archive exactly the Tier 3
//! files, the root none (index-compaction AC-05, index-shards AC-03); the
//! Tier 3 seeds render as compact Archive lines in the shard (AC-04 there).
//! AC-05: each of the 32 seeds, on a scratch copy, blocks exactly its target
//! with its code (plus each output the seed moves the render of, with
//! `index-drift` or `index-missing`: a Tier 3 line drifts the shard, not the
//! root); the unseeded copy is clean; nothing is written. AC-08 (second
//! half): W by an in-test implementation over the files' bytes and a line
//! reading of their front-matter equals `counts.worst_w_bytes`.
//!
//! The repository is only read; seeds are applied to scratch copies.

mod common;
mod parity_config;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Scratch, repository_root};
use parity_config::{
    DANGLING, EXPORT, GATE, INDEX, INDEX_SHARD, add_dangling_mention, mutated, root_toml, std_walk,
};
use specengine_core::check::{
    CheckConfig, CheckInput, Mode, Report, Verdict, is_tier3_file, render_index, render_index_set,
    worst_w,
};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::{IdScheme, Severity};
use specengine_store::{WorkingTree, check_input, check_worktree};

const TODAY: &str = "2026-09-29";

/// The findings of this repository, by (code, path, subject): none. The
/// last one, the five-digit mention `ADR-00011` in the file-name example of
/// docs/canon/spec-check.md "Rules", left at shipping; the clean test puts
/// it back on a scratch copy to show the pin still bites.
const PIN: [(&str, &str, &str); 0] = [];

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

/// `git status` of `root` without `.claude/` (the owner's, edited while
/// Claude Code runs; never written by a check).
fn git_status(root: &Path) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            ".",
            ":(exclude).claude",
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

/// Every finding of `report` by (code, path, subject).
fn pin_of(report: &Report) -> Vec<(&str, &str, &str)> {
    report
        .findings
        .iter()
        .map(|f| (f.code.as_str(), f.path.as_str(), f.subject.as_str()))
        .collect()
}

/// The files of `files` under the directory `dir`.
fn under(files: &BTreeSet<String>, dir: &str) -> BTreeSet<String> {
    let prefix = format!("{dir}/");
    files
        .iter()
        .filter(|path| path.starts_with(&prefix))
        .cloned()
        .collect()
}

// ------------------------------------------------------------------ AC-03

#[test]
fn the_walk_is_the_std_walk_of_this_repository() {
    let repository = repository_root();
    let toml = root_toml();
    let expected = std_walk(&repository);
    assert!(expected.len() >= 45, "{} documents", expected.len());
    assert_eq!(
        walked(&repository, &toml),
        expected,
        "the root config's walk vs the std walk"
    );

    // Mutation: `crates` out of `roots` — exactly the crate READMEs leave.
    let crates = under(&expected, "crates");
    assert!(crates.len() >= 8, "{crates:?}");
    let without_crates = walked(&repository, &mutated(&toml, "\"crates\", ", ""));
    let missing: BTreeSet<String> = expected.difference(&without_crates).cloned().collect();
    assert_eq!(missing, crates, "`crates` out of `roots`");

    // Mutation: no `**/_*.md` — the templates join, nothing else.
    let without_templates = walked(&repository, &mutated(&toml, "\"**/_*.md\", ", ""));
    let extra: Vec<&String> = without_templates.difference(&expected).collect();
    assert!(
        !extra.is_empty()
            && extra
                .iter()
                .all(|path| path.rsplit('/').next().unwrap().starts_with('_')),
        "without `**/_*.md`: {extra:?}"
    );

    // Mutation (scratch): a new top-level directory with a README stays
    // unwalked until listed in `roots` (spec, Rules), so the equality above
    // turns red by exactly that file.
    let scratch = Scratch::new("walk-new-top");
    let copy = scratch_copy(&scratch, &expected, "copy");
    fs::create_dir_all(copy.join("newtop")).unwrap();
    write_new(
        &copy,
        "newtop/README.md",
        "---\nclass: canon\ntier: 1\nscope: [newtop]\nowner: owner\nreviewed: 2026-09-29\n---\n\n# A new top-level directory\n",
    );
    let std_copy = std_walk(&copy);
    let walked_copy = walked(&copy, &toml);
    assert_eq!(
        std_copy.difference(&walked_copy).collect::<Vec<_>>(),
        ["newtop/README.md"],
        "a new top-level README"
    );
    assert!(walked_copy.is_subset(&std_copy));
    let listed = mutated(&toml, "\"docs\"]", "\"docs\", \"newtop\"]");
    assert_eq!(walked(&copy, &listed), std_copy, "listed in `roots`");
}

// ------------------------------------------------------------------ AC-02

#[test]
fn this_repository_is_clean_under_the_root_config() {
    let repository = repository_root();
    let config = repository.join("specengine.toml");
    // The repository is only read.
    let status = git_status(&repository);
    let index_bytes = fs::read(repository.join(INDEX)).expect("the index");
    let shard_bytes = fs::read(repository.join(INDEX_SHARD)).expect("the archive shard");
    let report = check_worktree(&repository, &config, None, TODAY);
    assert_eq!(
        git_status(&repository),
        status,
        "the check wrote into the repository"
    );
    assert_eq!(fs::read(repository.join(INDEX)).unwrap(), index_bytes);
    assert_eq!(fs::read(repository.join(INDEX_SHARD)).unwrap(), shard_bytes);

    assert_eq!(report.verdict, Verdict::Clean, "{:#?}", report.lines(true));
    assert!(report.cannot_check.is_empty(), "{:?}", report.cannot_check);
    assert_eq!(report.counts.documents, std_walk(&repository).len());
    assert_eq!(
        (
            report.counts.errors,
            report.counts.debt,
            report.counts.expired,
            report.counts.stale
        ),
        (0, 0, 0, 0),
        "{:#?}",
        report.lines(true)
    );
    assert_eq!(pin_of(&report), PIN, "{:#?}", report.lines(true));
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.severity == Severity::Warning),
        "{:#?}",
        report.lines(true)
    );
    assert_eq!(report.counts.warnings, PIN.len());
    assert_eq!(report.mode, Mode::Enforce);
    let summary = report.lines(false);
    assert_eq!(summary.len(), 1, "{summary:#?}");
    assert!(
        summary[0].starts_with("spec check [enforce]: ") && summary[0].ends_with(" \u{2014} clean"),
        "{}",
        summary[0]
    );

    // Mutation: `mode = "observe"` — the mode pin above fails (a clean tree
    // is `clean` in either mode; a seeded error stops blocking: the seeds).
    let toml = root_toml();
    let scratch = Scratch::new("clean-mutations");
    let observe = write_config(
        &scratch.join("observe"),
        &mutated(&toml, "mode = \"enforce\"", "mode = \"observe\""),
    );
    let observed = check_worktree(&repository, &observe, None, TODAY);
    assert_eq!(observed.mode, Mode::Observe);
    assert!(
        !observed.lines(false)[0].starts_with("spec check [enforce]: "),
        "{:#?}",
        observed.lines(false)
    );

    // Mutation: an `[ids]` prefix matching prose — findings beyond the pin.
    let prose = write_config(
        &scratch.join("prose"),
        &mutated(
            &toml,
            "[ids]\n",
            "[ids]\nAC  = { kind = \"criterion\", width = 2 }\n",
        ),
    );
    let noisy = check_worktree(&repository, &prose, None, TODAY);
    assert!(noisy.cannot_check.is_empty(), "{:?}", noisy.cannot_check);
    let beyond: Vec<&str> = noisy
        .findings
        .iter()
        .filter(|f| f.subject.starts_with("AC-"))
        .map(|f| f.code.as_str())
        .collect();
    assert!(beyond.len() > 10, "{:#?}", noisy.lines(true));
    assert_ne!(pin_of(&noisy), PIN, "{:#?}", noisy.lines(true));

    // Mutation (a scratch copy): the five-digit mention back in
    // docs/canon/spec-check.md — exactly that one warning, at its line, so
    // the empty pin above fails while the verdict stays `clean`.
    let copy = scratch_copy(&scratch, &std_walk(&repository), "dangling");
    let line = add_dangling_mention(&copy);
    let config = write_config(&scratch.join("dangling-config"), &toml);
    let dangling = check_worktree(&copy, &config, None, TODAY);
    assert!(
        dangling.cannot_check.is_empty(),
        "{:?}",
        dangling.cannot_check
    );
    assert_eq!(pin_of(&dangling), [DANGLING], "{:#?}", dangling.lines(true));
    assert_ne!(pin_of(&dangling), PIN);
    assert_eq!(dangling.findings[0].severity, Severity::Warning);
    assert_eq!(
        dangling.findings[0].line,
        line,
        "{:#?}",
        dangling.lines(true)
    );
    assert_eq!(dangling.counts.warnings, 1);
    assert_eq!(dangling.verdict, Verdict::Clean);
    assert_eq!(git_status(&repository), status, "the mutations wrote");
}

/// AC-10 of docs/features/spec-check-links.md: this repository's file
/// links under the root config (no `link_base`). The root `README.md`
/// records 9 (7 `.md` targets that resolve; `docs/decisions/` and `LICENSE`
/// recorded, never checked); the platform spec's `README.md` 6 sibling
/// links that resolve; no link finding anywhere (the clean pin above).
#[test]
fn this_repository_s_file_links_are_recorded_and_resolve() {
    use specengine_model::{LinkOrigin, LinkTarget};
    let repository = repository_root();
    let toml = root_toml();
    assert!(!toml.contains("link_base"), "the root config has no base");
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
    let documents = std_walk(&repository_root());
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
    let config = write_config(&scratch.join("config"), &root_toml());
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

// ------------------------------------------------------------------ AC-04

#[test]
fn the_core_render_is_the_committed_index() {
    let repository = repository_root();
    let toml = root_toml();
    let (_, paths, check) = tables(&toml);
    assert_eq!(paths.index.as_deref(), Some(INDEX));
    let generator = check.index_generator().expect("the index entry");
    assert_eq!(generator.command, EXPORT);
    assert_eq!(generator.gate(), GATE);
    assert_eq!(generator.writes, [INDEX, INDEX_SHARD]);
    // One shard, the archive (index-shards, ADR-0030).
    assert_eq!(
        generator
            .shards
            .iter()
            .map(|shard| (shard.path.as_str(), shard.is_archive()))
            .collect::<Vec<_>>(),
        [(INDEX_SHARD, true)]
    );
    let mut input = input_of(&repository, &toml);
    let set = render_index_set(&input, INDEX, generator);
    assert_eq!(
        set.iter()
            .map(|output| output.path.as_str())
            .collect::<Vec<_>>(),
        [INDEX, INDEX_SHARD],
        "the root, then the shard"
    );
    assert_eq!(
        render_index(&input, INDEX, generator),
        set[0].bytes,
        "render_index is the root"
    );
    for output in &set {
        let path = output.path.as_str();
        let committed = fs::read_to_string(repository.join(path))
            .unwrap_or_else(|e| panic!("the committed {path}: {e}"));
        assert!(
            output.bytes == committed,
            "core render vs the committed {path}, {}",
            first_difference(&output.bytes, &committed)
        );
        // The header names X (as `generator:` and in the note) and G.
        let header = committed.split("\n## ").next().unwrap();
        assert!(
            header.starts_with(&format!("---\nclass: generated\ngenerator: {EXPORT}\n")),
            "{path}: {header}"
        );
        for command in [EXPORT, GATE] {
            assert!(
                header.contains(&format!("`{command}`")),
                "{path}: {command}: {header}"
            );
        }
    }
    // Every section is exercised here but `No class — fix`: the live ones
    // and the pointer in the root, the Archive alone in the shard.
    let (root, shard) = (set[0].bytes.as_str(), set[1].bytes.as_str());
    for section in [
        "\n## Canon\n\n",
        "\n## Decisions\n\n",
        "\n## Specs\n\n",
        "\n## Shards\n\n",
    ] {
        assert!(root.contains(section), "{INDEX}: {section:?}");
    }
    assert!(!root.contains(ARCHIVE), "{INDEX}: an Archive section");
    assert!(
        shard.contains(&format!("\n{ARCHIVE}\n\n")),
        "{INDEX_SHARD}: no Archive section"
    );
    assert_eq!(
        shard.matches("\n## ").count(),
        1,
        "{INDEX_SHARD}: one section"
    );
    // Independent of the walk's order.
    input.files.reverse();
    assert_eq!(render_index_set(&input, INDEX, generator), set);
}

// ------------------------------------------------ index-compaction AC-04, AC-05

const ARCHIVE: &str = "## Archive — Tier 3, by id only";

/// `(section heading, label, link)` of every entry line of an index.
fn entries(index: &str) -> Vec<(String, String, String)> {
    let mut heading = String::new();
    let mut out = Vec::new();
    for line in index.lines() {
        if line.starts_with("## ") {
            heading = line.to_owned();
        } else if let Some(rest) = line.strip_prefix("- [") {
            let (label, rest) = rest.split_once("](").expect("`](` in an entry");
            let link = rest.split_once(')').expect("`)` in an entry").0;
            out.push((heading.clone(), label.to_owned(), link.to_owned()));
        }
    }
    out
}

/// The lines under `heading` (up to the next heading).
fn section_lines<'i>(index: &'i str, heading: &str) -> Vec<&'i str> {
    index
        .lines()
        .skip_while(|line| *line != heading)
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
        .filter(|line| line.starts_with("- ["))
        .collect()
}

/// The root's pointer section, present only with shards (ADR-0030).
const SHARDS: &str = "## Shards";

/// `link` resolved against the index's directory `docs/`, root-relative.
fn resolve_link(link: &str) -> String {
    let mut parts: Vec<&str> = vec!["docs"];
    for part in link.split('/') {
        match part {
            ".." => {
                assert!(parts.pop().is_some(), "{link} leaves the root");
            }
            "." | "" => {}
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// The AC-05 property of an index set over the walk of `input`
/// (index-shards AC-03): `outputs` are `(path, text)`, the root first,
/// every one in `docs/` (so links resolve from there). Their link targets,
/// the root's `## Shards` pointers aside, are the walked documents minus
/// `class: generated`, each exactly once across the set; the `## Archive`
/// ones exactly the Tier 3 files, none with ` · `, and with shards all in
/// one shard, none in the root; the pointers are the shards, in order.
fn every_document_listed_once(
    what: &str,
    outputs: &[(&str, &str)],
    input: &CheckInput,
) -> Vec<String> {
    let mut failures = Vec::new();
    let generated = |file: &specengine_core::check::CheckFile| {
        file.parsed
            .as_ref()
            .and_then(|parsed| parsed.document())
            .and_then(|document| document.fields.as_ref())
            .and_then(|fields| fields.class.as_deref())
            == Some("generated")
    };
    let expected: BTreeSet<String> = input
        .files
        .iter()
        .filter(|file| !generated(file))
        .map(|file| file.path.clone())
        .collect();
    let tier3: BTreeSet<String> = input
        .files
        .iter()
        .filter(|file| file.parsed.as_ref().is_some_and(is_tier3_file))
        .map(|file| file.path.clone())
        .collect();
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    let mut archived = BTreeSet::new();
    let mut archive_in = BTreeSet::new();
    let mut pointers = Vec::new();
    for (at, (path, text)) in outputs.iter().enumerate() {
        assert!(
            path.strip_prefix("docs/")
                .is_some_and(|name| !name.contains('/')),
            "{what}: {path} is not in docs/"
        );
        for (heading, _, link) in entries(text) {
            let target = resolve_link(&link);
            if heading == SHARDS {
                if at == 0 {
                    pointers.push(target);
                } else {
                    failures.push(format!("{what}: pointers in the shard {path}"));
                }
                continue;
            }
            if heading == ARCHIVE {
                archived.insert(target.clone());
                archive_in.insert(*path);
            }
            *seen.entry(target).or_default() += 1;
        }
        for line in section_lines(text, ARCHIVE) {
            if line.contains(" · ") {
                failures.push(format!(
                    "{what}: an Archive line with ` · ` in {path}: {line:?}"
                ));
            }
        }
    }
    let twice: Vec<_> = seen.iter().filter(|(_, n)| **n > 1).collect();
    if !twice.is_empty() {
        failures.push(format!("{what}: listed more than once: {twice:?}"));
    }
    let listed: BTreeSet<String> = seen.keys().cloned().collect();
    if listed != expected {
        failures.push(format!(
            "{what}: unlisted {:?}, listed but not walked {:?}",
            expected.difference(&listed).collect::<Vec<_>>(),
            listed.difference(&expected).collect::<Vec<_>>()
        ));
    }
    if archived != tier3 {
        failures.push(format!(
            "{what}: `{ARCHIVE}` lists {archived:?}, the Tier 3 files are {tier3:?}"
        ));
    }
    let shards: Vec<String> = outputs[1..]
        .iter()
        .map(|(path, _)| (*path).to_owned())
        .collect();
    if pointers != shards {
        failures.push(format!(
            "{what}: the root points at {pointers:?}, the shards are {shards:?}"
        ));
    }
    if !shards.is_empty() && (archive_in.len() > 1 || archive_in.contains(outputs[0].0)) {
        failures.push(format!(
            "{what}: `{ARCHIVE}` in {archive_in:?}, not in one shard"
        ));
    }
    failures
}

#[test]
fn every_document_of_this_repository_is_listed_once() {
    let repository = repository_root();
    let toml = root_toml();
    let (_, _, check) = tables(&toml);
    let generator = check.index_generator().expect("the index entry");
    let input = input_of(&repository, &toml);
    assert!(
        input.files.len() > 50,
        "the walk found {} files",
        input.files.len()
    );
    let render = render_index_set(&input, INDEX, generator);
    let paths: Vec<&str> = render.iter().map(|output| output.path.as_str()).collect();
    assert_eq!(paths, [INDEX, INDEX_SHARD]);
    let committed: Vec<String> = paths
        .iter()
        .map(|path| {
            fs::read_to_string(repository.join(path))
                .unwrap_or_else(|e| panic!("the committed {path}: {e}"))
        })
        .collect();
    let committed: Vec<(&str, &str)> = paths
        .iter()
        .copied()
        .zip(committed.iter().map(String::as_str))
        .collect();
    let rendered: Vec<(&str, &str)> = render
        .iter()
        .map(|output| (output.path.as_str(), output.bytes.as_str()))
        .collect();
    let mut failures = Vec::new();
    for (what, set) in [
        ("the committed index set", &committed),
        ("the core render", &rendered),
    ] {
        assert!(
            !set[0].1.contains(ARCHIVE),
            "{what}: an Archive section in {INDEX}"
        );
        assert!(
            set[1].1.contains(&format!("\n{ARCHIVE}\n\n")),
            "{what}: no Archive section in {INDEX_SHARD}"
        );
        failures.extend(every_document_listed_once(what, set, &input));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The seeds of index-compaction AC-04: `(path, text, the expected line, its section)`.
/// The seeds of AC-04: `(path, text, the expected line, its section)`.
const COMPACTION_SEEDS: &[(&str, &str, &str, &str)] = &[
    // The two of the criterion.
    (
        "docs/decisions/ADR-0999.md",
        "---\nid: ADR-0999\nclass: decision\ntitle: A seeded rejection\nstatus: rejected\nscope: [core]\n---\n\n# ADR-0999: a seeded rejection\n",
        "- [ADR-0999](decisions/ADR-0999.md) rejected",
        ARCHIVE,
    ),
    (
        "docs/features/seed-abandoned.md",
        "---\nclass: spec\nstatus: abandoned\n---\n\n# A seeded abandoned spec\n",
        "- [docs/features/seed-abandoned.md](features/seed-abandoned.md) abandoned",
        ARCHIVE,
    ),
    // The same spec with `scope:`: the same line but for the path.
    (
        "docs/features/seed-abandoned-scoped.md",
        "---\nclass: spec\nstatus: abandoned\nscope: [a, b]\n---\n\n# A seeded abandoned spec\n",
        "- [docs/features/seed-abandoned-scoped.md](features/seed-abandoned-scoped.md) abandoned",
        ARCHIVE,
    ),
    // A Tier 3 decision without `id:`: labelled with its path.
    (
        "docs/decisions/seed-no-id.md",
        "---\nclass: decision\ntitle: No id\nstatus: superseded-by ADR-0001\nscope: [core]\n---\n",
        "- [docs/decisions/seed-no-id.md](decisions/seed-no-id.md) superseded-by ADR-0001",
        ARCHIVE,
    ),
    // Accepted divergences of the live line that must not reach Tier 3: a
    // decision without `title:` whose H1 has inline markup; `title:` on a
    // spec with a setext H1, outside `docs/` (under a walked root).
    (
        "docs/decisions/ADR-0998.md",
        "---\nid: ADR-0998\nclass: decision\nstatus: rejected\n---\n\n# A *marked* `heading`\n",
        "- [ADR-0998](decisions/ADR-0998.md) rejected",
        ARCHIVE,
    ),
    (
        "crates/notes/y.md",
        "---\nclass: spec\ntitle: A front-matter title\nstatus: shipped\nshipped: 2026-09-01\nscope: [core]\n---\n\nA setext heading\n================\n",
        "- [crates/notes/y.md](../crates/notes/y.md) shipped",
        ARCHIVE,
    ),
    // By status, not folder: a draft under an `archive` directory is live.
    (
        "docs/archive/seed-draft.md",
        "---\nclass: spec\nstatus: draft\nscope: [core]\n---\n\n# A seeded draft\n",
        "- [docs/archive/seed-draft.md](archive/seed-draft.md) A seeded draft · core · draft",
        "## Specs",
    ),
];

#[test]
fn seeded_tier3_cases_render_as_compact_archive_lines() {
    let documents = std_walk(&repository_root());
    let scratch = Scratch::new("index-compaction");
    let root = scratch_copy(&scratch, &documents, "seeded");
    for (path, text, _, _) in COMPACTION_SEEDS {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        write_new(&root, path, text);
    }
    let toml = root_toml();
    let (_, _, check) = tables(&toml);
    let generator = check.index_generator().expect("the index entry");
    let input = input_of(&root, &toml);
    let walked: BTreeSet<&str> = input.files.iter().map(|file| file.path.as_str()).collect();
    for (path, _, _, _) in COMPACTION_SEEDS {
        assert!(walked.contains(path), "{path} is walked");
    }
    let set = render_index_set(&input, INDEX, generator);
    let paths: Vec<&str> = set.iter().map(|output| output.path.as_str()).collect();
    assert_eq!(paths, [INDEX, INDEX_SHARD]);
    let (root, shard) = (set[0].bytes.as_str(), set[1].bytes.as_str());
    for (path, _, line, heading) in COMPACTION_SEEDS {
        // An Archive line goes to the archive shard, a live one to the root.
        let (output, text) = if *heading == ARCHIVE {
            (INDEX_SHARD, shard)
        } else {
            (INDEX, root)
        };
        assert!(
            section_lines(text, heading).contains(line),
            "{path}: {line:?} not under {heading:?} of {output}:\n{text}"
        );
        let link = format!("]({})", resolve_relative(path));
        assert_eq!(
            root.matches(&link).count() + shard.matches(&link).count(),
            1,
            "{path}: listed once"
        );
    }
    let outputs: Vec<(&str, &str)> = vec![(INDEX, root), (INDEX_SHARD, shard)];
    let failures = every_document_listed_once("the seeded render", &outputs, &input);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // The seeded copy's root is the committed root plus the draft; its
    // shard the committed shard plus the six Tier 3 seeds (lines, sorted).
    let sorted_lines = |text: &str, extra: Vec<&str>| -> Vec<String> {
        let mut lines: Vec<String> = text.lines().chain(extra).map(str::to_owned).collect();
        lines.sort();
        lines
    };
    let committed_root = fs::read_to_string(repository_root().join(INDEX)).unwrap();
    let committed_shard = fs::read_to_string(repository_root().join(INDEX_SHARD)).unwrap();
    type CompactionSeed = (&'static str, &'static str, &'static str, &'static str);
    let (archive_seeds, live_seeds): (Vec<&CompactionSeed>, Vec<&CompactionSeed>) =
        COMPACTION_SEEDS
            .iter()
            .partition(|(_, _, _, heading)| *heading == ARCHIVE);
    assert_eq!((archive_seeds.len(), live_seeds.len()), (6, 1));
    assert_eq!(
        sorted_lines(root, Vec::new()),
        sorted_lines(
            &committed_root,
            live_seeds.iter().map(|seed| seed.2).collect()
        ),
        "root lines, sorted"
    );
    assert_eq!(
        sorted_lines(shard, Vec::new()),
        sorted_lines(
            &committed_shard,
            archive_seeds.iter().map(|seed| seed.2).collect()
        ),
        "shard lines, sorted"
    );
}

/// The link of a root-relative `path` from `docs/index.md`.
/// The link of a root-relative `path` from `docs/index.md`.
fn resolve_relative(path: &str) -> String {
    path.strip_prefix("docs/")
        .map_or_else(|| format!("../{path}"), str::to_owned)
}

// ------------------------------------------------------------------ AC-05

/// A scratch copy of the repository's documents (the std walk's list).
fn scratch_copy(scratch: &Scratch, documents: &BTreeSet<String>, dir: &str) -> PathBuf {
    let root = scratch.join(dir);
    for path in documents {
        let to = root.join(path);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::copy(repository_root().join(path), &to).unwrap();
    }
    root
}

/// `(seed, file, code)`: what a seed blocks besides its target and the
/// index. An ID taken off its record leaves the front-matter references to
/// it dangling (ADR-0026 `supersedes: [ADR-0002]`; the platform spec's
/// `adrs:` lists ADR-0004); every ADR of this repository is so referenced.
const COLLATERAL: &[(&str, &str, &str)] = &[
    (
        "ADR-0002 as id: ADR-0001",
        "docs/decisions/ADR-0026.md",
        "ref-dangling",
    ),
    (
        "id: ADR-001",
        "docs/specs/specengine-platform/README.md",
        "ref-dangling",
    ),
];

#[test]
fn each_seed_blocks_exactly_its_target() {
    assert_eq!(
        SEEDS.len(),
        24 + 6 + 2,
        "the 24 front-matter seeds, the six of §11.5–6 and the two of the archive shard"
    );
    let documents = std_walk(&repository_root());
    let toml = root_toml();
    let (_, _, check) = tables(&toml);
    let generator = check.index_generator().expect("the index entry");
    let scratch = Scratch::new("parity-seeds");
    let config = write_config(&scratch.join("config"), &toml);

    // The unseeded copy: clean, nothing set aside, nothing written.
    let clean = scratch_copy(&scratch, &documents, "clean");
    assert_eq!(walked(&clean, &toml), documents, "the copy walks the same");
    let before = snapshot(&clean);
    let report = check_worktree(&clean, &config, None, TODAY);
    assert!(snapshot(&clean) == before, "the check wrote a file");
    assert_eq!(report.verdict, Verdict::Clean, "{:#?}", report.lines(true));
    assert!(new_blocking(&report).is_empty());
    assert_eq!(pin_of(&report), PIN, "{:#?}", report.lines(true));

    let mut failures = Vec::new();
    let mut targets = BTreeSet::new();
    let mut drifts = BTreeMap::<&str, Vec<String>>::new();
    for (index, (name, target, code, apply)) in SEEDS.iter().enumerate() {
        let root = scratch_copy(&scratch, &documents, &format!("seed-{index:02}"));
        apply(&root);
        // Nothing is written, a drifted index included.
        let before = snapshot(&root);
        let report = check_worktree(&root, &config, None, TODAY);
        assert!(
            snapshot(&root) == before,
            "seed {name}: the check wrote a file"
        );
        let ours = new_blocking(&report);
        // A seed that moves the render of an output (the root or the
        // archive shard) drifts that copied output, each with its finding.
        let rendered = render_index_set(&input_of(&root, &toml), INDEX, generator);
        let mut expected = BTreeSet::from([(*target).to_owned()]);
        let mut drifted = Vec::new();
        let mut index_coded = true;
        for output in &rendered {
            let on_disk = fs::read_to_string(root.join(&output.path)).ok();
            if on_disk.as_deref() == Some(output.bytes.as_str()) {
                continue;
            }
            expected.insert(output.path.clone());
            drifted.push(output.path.clone());
            index_coded &= report.findings.iter().any(|f| {
                f.path == output.path
                    && report.blocks(f)
                    && matches!(f.code.as_str(), "index-drift" | "index-missing")
            });
        }
        let coded = report
            .findings
            .iter()
            .any(|f| f.path == *target && f.code == *code && report.blocks(f));
        let mut collateral_coded = true;
        for (_, path, code) in COLLATERAL.iter().filter(|(seed, _, _)| seed == name) {
            expected.insert((*path).to_owned());
            collateral_coded &= report
                .findings
                .iter()
                .any(|f| f.path == *path && f.code == *code && report.blocks(f));
        }
        eprintln!("seed {name}: blocking {ours:?}, drifted {drifted:?}");
        drifts.insert(name, drifted);
        if ours != expected || !coded || !index_coded || !collateral_coded {
            failures.push(format!(
                "seed {name}: blocking {ours:?} (expected {expected:?}, {target} with {code})\n  {}",
                report.lines(false).join("\n  ")
            ));
        }
        targets.insert(*target);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(targets.len() >= 15, "{targets:?}");
    // Which output drifts (ADR-0030): a live line the root only, a Tier 3
    // line the shard only, a document turning Tier 3 both, an output edited
    // or deleted itself only.
    for (name, outputs) in [
        ("docs/index.md hand-edited", &[INDEX][..]),
        ("a new canon document, not rendered", &[INDEX]),
        ("docs/index.md deleted", &[INDEX]),
        ("ADR-0002 as id: ADR-0001", &[INDEX_SHARD]),
        (
            "ADR-0010 superseded-by ADR-0001, not rendered",
            &[INDEX, INDEX_SHARD],
        ),
        ("docs/index-archive.md hand-edited", &[INDEX_SHARD]),
        ("docs/index-archive.md deleted", &[INDEX_SHARD]),
    ] {
        assert_eq!(drifts[name], outputs, "seed {name}: the drifting outputs");
    }

    // Mutation: `mode = "observe"` — the first seed no longer blocks.
    let (name, target, _, apply) = SEEDS[0];
    let root = scratch_copy(&scratch, &documents, "seed-observe");
    apply(&root);
    let observe = write_config(
        &scratch.join("config-observe"),
        &mutated(&toml, "mode = \"enforce\"", "mode = \"observe\""),
    );
    let report = check_worktree(&root, &observe, None, TODAY);
    assert_eq!(report.verdict, Verdict::Observed, "seed {name} observed");
    assert!(new_blocking(&report).is_empty(), "seed {name}: {target}");
}

// ------------------------------------------------------------------ AC-08

/// The front-matter keys of `bytes`, read line by line: `key: value` at
/// column 0 between an opening `---` line and the next `---` line, quotes
/// trimmed. Empty without a closed front-matter (a failed one is pooled).
fn front_matter(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut keys = BTreeMap::new();
    let Ok(text) = std::str::from_utf8(bytes) else {
        return keys;
    };
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return keys;
    }
    for line in lines {
        if line.trim_end() == "---" {
            return keys;
        }
        if let Some((key, value)) = line.split_once(':') {
            let plain = key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            if !key.is_empty() && plain {
                keys.insert(
                    key.to_owned(),
                    value.trim().trim_matches(['"', '\'']).to_owned(),
                );
            }
        }
    }
    BTreeMap::new()
}

/// W over `documents` under `root` (§3; spec-cli-switch "Worst W"),
/// computed here: every canon `tier: 0` summed, the largest canon `tier:
/// 1`, the index root, the three largest of the rest that are neither Tier 3
/// (a spec `shipped`/`abandoned`, a decision with a status not `accepted`)
/// nor `class: generated`. The root config's archive shard is outside W by
/// its path, whatever its front-matter (ADR-0030).
fn oracle_w(root: &Path, documents: &BTreeSet<String>) -> u64 {
    let (mut tier0, mut tier1, mut index) = (0_u64, 0_u64, 0_u64);
    let mut pool = Vec::new();
    for path in documents {
        let bytes = fs::read(root.join(path)).unwrap();
        let size = bytes.len() as u64;
        if path == INDEX {
            index = size;
            continue;
        }
        if path == INDEX_SHARD {
            continue;
        }
        let keys = front_matter(&bytes);
        let key = |name: &str| keys.get(name).map(String::as_str);
        match (key("class"), key("status")) {
            (Some("canon"), _) if key("tier") == Some("0") => tier0 += size,
            (Some("canon"), _) if key("tier") == Some("1") => tier1 = tier1.max(size),
            (Some("generated"), _) => {}
            (Some("spec"), Some("shipped" | "abandoned")) => {}
            (Some("decision"), Some(status)) if status != "accepted" => {}
            _ => pool.push(size),
        }
    }
    pool.sort_unstable_by(|a, b| b.cmp(a));
    tier0 + tier1 + index + pool.iter().take(3).sum::<u64>()
}

#[test]
fn worst_w_is_the_in_test_oracle_s_on_this_repository() {
    let repository = repository_root();
    let toml = root_toml();
    let (_, paths, check) = tables(&toml);
    let documents = std_walk(&repository);
    assert!(documents.contains(INDEX_SHARD), "{INDEX_SHARD} walked");
    let oracle = oracle_w(&repository, &documents);
    assert!(oracle > 50_000, "W {oracle}");
    let report = check_worktree(
        &repository,
        &repository.join("specengine.toml"),
        None,
        TODAY,
    );
    assert_eq!(report.counts.worst_w_bytes, oracle, "counts.worst_w_bytes");
    assert_eq!(worst_w(&input_of(&repository, &toml), &paths, None), oracle);
    assert_eq!(
        worst_w(
            &input_of(&repository, &toml),
            &paths,
            check.index_generator()
        ),
        oracle,
        "with the index entry's archive shard"
    );
    let summary = report.lines(false);
    assert!(
        summary[0].ends_with(&format!(", worst W {oracle} B \u{2014} clean")),
        "{}",
        summary[0]
    );

    // A copy where the Tier 3 and generated files are the largest and the
    // compaction seeds are present: the oracle and the library still agree.
    let scratch = Scratch::new("worst-w");
    let root = scratch_copy(&scratch, &documents, "copy");
    for (path, text, _, _) in COMPACTION_SEEDS {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        write_new(&root, path, text);
    }
    let big = "x".repeat(200_000);
    for (path, front) in [
        (
            "docs/features/zz-shipped.md",
            "class: spec\nstatus: shipped\nshipped: 2026-09-01\nscope: [core]",
        ),
        (
            "docs/decisions/ADR-0997.md",
            "id: ADR-0997\nclass: decision\ntitle: Big\nstatus: superseded-by ADR-0001\ndate: 2026-09-01\nscope: [core]",
        ),
        (
            "docs/zz-generated.md",
            "class: generated\ngenerator: seed\nsource: a seed",
        ),
        (
            "docs/features/zz-live.md",
            "class: spec\nstatus: draft\nscope: [core]",
        ),
    ] {
        write_new(
            &root,
            path,
            &format!("---\n{front}\n---\n\n# Big\n\n{big}\n"),
        );
    }
    let seeded = std_walk(&root);
    let oracle = oracle_w(&root, &seeded);
    let input = input_of(&root, &toml);
    assert_eq!(
        input
            .files
            .iter()
            .map(|f| f.path.clone())
            .collect::<BTreeSet<_>>(),
        seeded
    );
    assert_eq!(worst_w(&input, &paths, None), oracle, "the seeded copy");
    assert!(
        oracle > 200_000 && oracle < 400_000,
        "one big live file: {oracle}"
    );
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
        "crates/specengine-core/README.md",
        "date-invalid",
        |r| {
            set_key(
                r,
                "crates/specengine-core/README.md",
                "reviewed",
                Some("reviewed: 2026-9-1"),
            )
        },
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
    // Beyond the spec's list: the other front-matter checks.
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
        "crates/specengine-store/README.md",
        "scope-empty",
        |r| {
            set_key(
                r,
                "crates/specengine-store/README.md",
                "scope",
                Some("scope: []"),
            )
        },
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
    // index-shards AC-05 on this repository: the archive shard is compared
    // on its own (ADR-0030).
    (
        "docs/index-archive.md hand-edited",
        INDEX_SHARD,
        "index-drift",
        |r| {
            replace_once(
                r,
                INDEX_SHARD,
                "\n## Archive — Tier 3, by id only\n",
                "\n## Archive — Tier 3, by ID only\n",
            )
        },
    ),
    (
        "docs/index-archive.md deleted",
        INDEX_SHARD,
        "index-missing",
        |r| fs::remove_file(r.join(INDEX_SHARD)).unwrap(),
    ),
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
        "a generated document naming the registered export command",
        "docs/zz-generated-index.md",
        "generator-path",
        |r| {
            write_new(
                r,
                "docs/zz-generated-index.md",
                &format!(
                    "---\nclass: generated\ngenerator: {EXPORT}\nsource: a seed\n---\n\n# Not the index\n"
                ),
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

// ------------------------------------------------- index shards (ADR-0030)
// docs/features/index-shards.md AC-08 against the in-test oracle, on a
// scratch copy of this repository's documents whose config (written outside
// the copy) gives the index entry live shards of 3 000 and 5 000 B and an
// archive shard of 20 000 B: W = the oracle of the documents without the
// shards + 5 000. The 5 000 B shard is `class: canon tier: 0` (summed as
// Tier 0 were it not known by path), the archive shard a live spec.

/// `toml` with its `[[generators]]` tables replaced by `block`.
fn with_generators(toml: &str, block: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in toml.lines() {
        if line.starts_with('[') {
            inside = line.trim() == "[[generators]]" || line.trim() == "[[generators.shards]]";
        }
        if !inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    format!("{}\n{block}", out.trim_end())
}

const LIVE_A: (&str, usize) = ("docs/live-a.md", 3_000);
const LIVE_B: (&str, usize) = ("docs/live-b.md", 5_000);
const ARCHIVE_SHARD: (&str, usize) = ("docs/index-archive.md", 20_000);

fn sized_doc(front: &str, size: usize) -> String {
    let mut text = format!("---\n{front}\n---\n\n# Sized\n");
    text.push_str(&"x".repeat(size - text.len() - 1));
    text.push('\n');
    assert_eq!(text.len(), size);
    text
}

#[test]
fn worst_w_with_shards_is_the_oracle_plus_the_largest_live_shard() {
    let documents = std_walk(&repository_root());
    let scratch = Scratch::new("worst-w-shards");
    let root = scratch_copy(&scratch, &documents, "copy");
    let block = format!(
        "[[generators]]\ncommand = \"{EXPORT}\"\nwrites  = [\"{INDEX}\", \"{}\", \"{}\", \"{}\"]\nindex   = true\ngate    = \"{GATE}\"\nshards  = [\n  {{ path = \"{}\", claims = [\"docs/canon/**\"] }},\n  {{ path = \"{}\", tier3 = true }},\n  {{ path = \"{}\", claims = [\"docs/features/**\"] }},\n]\n",
        LIVE_A.0, LIVE_B.0, ARCHIVE_SHARD.0, LIVE_A.0, ARCHIVE_SHARD.0, LIVE_B.0
    );
    let toml = with_generators(&root_toml(), &block);
    let config = write_config(&scratch.join("config"), &toml);
    fs::write(
        root.join(LIVE_A.0),
        sized_doc("class: generated\ngenerator: x\nsource: y", LIVE_A.1),
    )
    .unwrap();
    fs::write(
        root.join(LIVE_B.0),
        sized_doc(
            "class: canon\ntier: 0\nscope: [x]\nowner: o\nreviewed: 2026-09-29",
            LIVE_B.1,
        ),
    )
    .unwrap();
    fs::write(
        root.join(ARCHIVE_SHARD.0),
        sized_doc("class: spec\nstatus: draft\nscope: [x]", ARCHIVE_SHARD.1),
    )
    .unwrap();

    let walked = std_walk(&root);
    let shards = [LIVE_A.0, LIVE_B.0, ARCHIVE_SHARD.0];
    for shard in shards {
        assert!(walked.contains(shard), "{shard} walked");
    }
    let rest: BTreeSet<String> = walked
        .iter()
        .filter(|path| !shards.contains(&path.as_str()))
        .cloned()
        .collect();
    let oracle = oracle_w(&root, &rest) + LIVE_B.1 as u64;

    let (_, paths, check) = tables(&toml);
    let generator = check.index_generator().expect("the index entry");
    assert_eq!(generator.shards.len(), 3);
    let input = input_of(&root, &toml);
    assert_eq!(worst_w(&input, &paths, Some(generator)), oracle);
    let report = check_worktree(&root, &config, None, TODAY);
    assert_eq!(report.counts.worst_w_bytes, oracle, "counts.worst_w_bytes");
}
