//! docs/features/index-shards.md AC-12 and AC-13 on a scratch copy of this
//! repository's walked documents with the spec's shipping config (the root
//! `specengine.toml` with its `[[generators]]` entry replaced by the one in
//! the spec's "Data", which since shipping step 2 is the root config byte
//! for byte). `spec export index` never runs in this repository; every run
//! gets a scratch `HOME`; `git status` is the same before and after.
//!
//! AC-12: export creates the root and `docs/index-archive.md` as
//! `render_index_set` has them; the shard lists exactly the walked Tier 3
//! documents, the root none; both are this repository's committed files
//! byte for byte; both deleted → recreated byte-identical; a rerun → both
//! `unchanged`; `--stdout` deterministic; `spec check` on the copy clean, W
//! the library's = this repository's.
//!
//! AC-13: a live spec set to `shipped` → the root loses exactly its line
//! (byte for byte), the shard gains exactly one; a new shipped spec → the
//! root `unchanged`, the shard gains one line.

#![cfg(unix)]

mod common;

#[allow(dead_code)]
#[path = "../../specengine-store/tests/parity_config/mod.rs"]
mod parity_config;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use common::check::library;
use common::{Scratch, read, repository_root, snapshot, spec, write};
use parity_config::{EXPORT, GATE, INDEX, root_toml};
use specengine_core::ProjectConfig;
use specengine_core::check::{
    CheckConfig, CheckInput, IndexOutput, Verdict, is_tier3_file, render_index_set, worst_w,
};
use specengine_store::{WorkingTree, check_input};

const SHARD: &str = "docs/index-archive.md";
const ARCHIVE_HEADING: &str = "## Archive \u{2014} Tier 3, by id only";
const POINTERS: &str = "\n## Shards\n\n- [docs/index-archive.md](index-archive.md) Archive \u{2014} Tier 3, by id only\n";

/// The spec's shipping entry, verbatim.
const SHIPPING: &str = "\
[[generators]]
command = \"cargo run -q -p specengine-cli -- export index\"
writes  = [\"docs/index.md\", \"docs/index-archive.md\"]
index   = true
gate    = \"cargo run -q -p specengine-cli -- check\"
shards  = [{ path = \"docs/index-archive.md\", tier3 = true }]
";

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
    format!("{}\n\n{block}", out.trim_end())
}

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

/// The walk of `root` under `toml`, and the render set.
fn walk(root: &Path, toml: &str) -> (CheckInput, Vec<IndexOutput>, u64) {
    let project = ProjectConfig::from_toml(toml).expect("the config");
    let check =
        CheckConfig::from_toml(toml).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    let generator = check.index_generator().expect("the index entry");
    let tree = WorkingTree::new(root, &project.paths).expect("the working tree");
    let input = check_input(&tree, &project.scheme);
    assert!(input.problems.is_empty(), "{:?}", input.problems);
    let outputs = render_index_set(&input, INDEX, generator);
    let w = worst_w(&input, &project.paths, Some(generator));
    (input, outputs, w)
}

/// `link` from `docs/`, root-relative.
fn resolve(link: &str) -> String {
    let mut parts: Vec<&str> = vec!["docs"];
    for part in link.split('/') {
        match part {
            ".." => assert!(parts.pop().is_some(), "{link}"),
            "." | "" => {}
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// `(heading, target, line)` of every entry line, pointers aside (every
/// output of this repository sits in `docs/`).
fn entries(text: &str) -> Vec<(String, String, String)> {
    let mut heading = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            heading = line.to_owned();
        } else if let Some(rest) = line.strip_prefix("- [") {
            if heading == "## Shards" {
                continue;
            }
            let (_, rest) = rest.split_once("](").expect("`](`");
            let link = rest.split_once(')').expect("`)`").0;
            out.push((heading.clone(), resolve(link), line.to_owned()));
        }
    }
    out
}

/// The lines of `after` not in `before` (as multisets), and the reverse.
fn line_diff(before: &str, after: &str) -> (Vec<String>, Vec<String>) {
    let mut gone: Vec<String> = before.lines().map(str::to_owned).collect();
    let mut added = Vec::new();
    for line in after.lines() {
        if let Some(at) = gone.iter().position(|old| old == line) {
            gone.remove(at);
        } else {
            added.push(line.to_owned());
        }
    }
    (added, gone)
}

/// The repository's walked documents copied to `copy` (the outputs aside),
/// the shipping config written there.
fn shipping_copy(scratch: &Scratch) -> (std::path::PathBuf, String) {
    let repository = repository_root();
    let repo_toml = root_toml();
    let project = ProjectConfig::from_toml(&repo_toml).expect("the root config");
    let tree = WorkingTree::new(&repository, &project.paths).expect("the working tree");
    let input = check_input(&tree, &project.scheme);
    let copy = scratch.dir("copy");
    for file in &input.files {
        if file.path != INDEX && file.path != SHARD {
            write(
                &copy,
                &file.path,
                fs::read(repository.join(&file.path)).unwrap(),
            );
        }
    }
    let toml = with_generators(&repo_toml, SHIPPING);
    write(&copy, "specengine.toml", &toml);
    (copy, toml)
}

/// AC-12.
#[test]
fn the_shipping_config_writes_the_root_and_the_archive_shard() {
    let repository = repository_root();
    let before = git_status(&repository);
    let repo_toml = root_toml();
    let scratch = Scratch::new("shards-repo");
    let home = scratch.home("h");
    let (copy, toml) = shipping_copy(&scratch);
    // The shipping entry is the root config's (step 2 of "Roles and order").
    assert!(
        toml == repo_toml,
        "the root config is not the shipping config:\n{repo_toml}"
    );
    assert!(!copy.join(INDEX).exists() && !copy.join(SHARD).exists());

    let run = spec(&home, &copy, &["export", "index"]);
    run.code(0);
    assert_eq!(run.stderr, "");
    let (input, outputs, w) = walk(&copy, &toml);
    assert_eq!(
        outputs.iter().map(|o| o.path.as_str()).collect::<Vec<_>>(),
        [INDEX, SHARD]
    );
    assert_eq!(
        run.stdout,
        format!(
            "wrote {INDEX}: {} bytes\nwrote {SHARD}: {} bytes\n",
            outputs[0].bytes.len(),
            outputs[1].bytes.len()
        )
    );
    let root_bytes = read(&copy, INDEX);
    let shard_bytes = read(&copy, SHARD);
    assert!(
        root_bytes == outputs[0].bytes.as_bytes(),
        "the root is the render"
    );
    assert!(
        shard_bytes == outputs[1].bytes.as_bytes(),
        "the shard is the render"
    );
    let root = outputs[0].bytes.as_str();
    let shard = outputs[1].bytes.as_str();

    // The shard: its header, back link, one Archive section of exactly the
    // walked Tier 3 documents; the root: none of them, the pointer last.
    assert!(
        shard.starts_with(&format!(
            "---\nclass: generated\ngenerator: {EXPORT}\nsource: front-matter of the repository's documents\n---\n\n# Documentation index: Archive \u{2014} Tier 3, by id only\n\n<!-- Built by `{EXPORT}`. Manual edits are overwritten on rebuild, and `{GATE}` rejects them. -->\n\nA shard of [docs/index.md](index.md), the index's one entry point.\n\n{ARCHIVE_HEADING}\n\n- ["
        )),
        "{shard}"
    );
    assert_eq!(shard.matches("\n## ").count(), 1, "one section: {shard}");
    let tier3: BTreeSet<String> = input
        .files
        .iter()
        .filter(|f| f.parsed.as_ref().is_some_and(is_tier3_file))
        .map(|f| f.path.clone())
        .collect();
    assert!(tier3.len() > 10, "{tier3:?}");
    let in_shard: BTreeSet<String> = entries(shard).into_iter().map(|(_, t, _)| t).collect();
    assert_eq!(
        in_shard, tier3,
        "the shard lists exactly the Tier 3 documents"
    );
    assert_eq!(entries(shard).len(), tier3.len(), "each once");
    let in_root = entries(root);
    assert!(
        in_root
            .iter()
            .all(|(heading, target, _)| heading != ARCHIVE_HEADING && !tier3.contains(target)),
        "a Tier 3 line in the root"
    );
    assert!(root.ends_with(POINTERS), "{root}");
    let listed: BTreeSet<String> = in_root.iter().map(|(_, t, _)| t.clone()).collect();
    assert_eq!(listed.len(), in_root.len(), "each once in the root");
    assert!(listed.is_disjoint(&in_shard));

    // Against this repository: the committed files themselves.
    let committed = fs::read_to_string(repository.join(INDEX)).expect("the committed index");
    assert!(root == committed, "the committed root is the render");
    let committed_shard = fs::read_to_string(repository.join(SHARD)).expect("the committed shard");
    assert!(
        shard == committed_shard,
        "the committed shard is the render"
    );

    // W: the library's with the entry, the same as this repository's.
    let w_here = {
        let project = ProjectConfig::from_toml(&repo_toml).unwrap();
        let tree = WorkingTree::new(&repository, &project.paths).unwrap();
        let input = check_input(&tree, &project.scheme);
        let check = CheckConfig::from_toml(&repo_toml).unwrap();
        worst_w(&input, &project.paths, check.index_generator())
    };
    assert_eq!(w, w_here, "W of the copy is this repository's");
    let report = library(&copy);
    assert_eq!(report.verdict, Verdict::Clean, "{:#?}", report.lines(true));
    assert_eq!(report.counts.worst_w_bytes, w);
    let run = spec(&home, &copy, &["check"]);
    run.code(0);
    assert!(
        run.stdout
            .ends_with(&format!(", worst W {w} B \u{2014} clean\n")),
        "{}",
        run.stdout
    );

    // A rerun: both unchanged.
    let run = spec(&home, &copy, &["export", "index"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        format!(
            "unchanged {INDEX}: {} bytes\nunchanged {SHARD}: {} bytes\n",
            root.len(),
            shard.len()
        )
    );
    // `--stdout`, twice: the same bytes, both outputs.
    let first = spec(&home, &copy, &["export", "index", "--stdout"]);
    first.code(0);
    let second = spec(&home, &copy, &["export", "index", "--stdout"]);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(
        first.stdout,
        format!("==> {INDEX} <==\n{root}==> {SHARD} <==\n{shard}")
    );

    // Both deleted: recreated byte for byte.
    fs::remove_file(copy.join(INDEX)).unwrap();
    fs::remove_file(copy.join(SHARD)).unwrap();
    spec(&home, &copy, &["export", "index"]).code(0);
    assert!(read(&copy, INDEX) == root_bytes, "the root recreated");
    assert!(read(&copy, SHARD) == shard_bytes, "the shard recreated");

    assert!(snapshot(&home).is_empty(), "something under HOME");
    assert_eq!(
        git_status(&repository),
        before,
        "the repository was written"
    );
}

/// Replaces the front-matter's `status:` line of `path`.
fn set_status(root: &Path, path: &str, status: &str) {
    let text = fs::read_to_string(root.join(path)).unwrap();
    let mut out = String::new();
    let mut fences = 0;
    let mut found = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end() == "---" {
            fences += 1;
        }
        if fences == 1 && line.starts_with("status:") {
            out.push_str(&format!("status: {status}\n"));
            found += 1;
        } else {
            out.push_str(line);
        }
    }
    assert_eq!(found, 1, "{path}: one status line");
    fs::write(root.join(path), out).unwrap();
}

/// AC-13.
#[test]
fn shipping_a_spec_costs_the_root_nothing() {
    let repository = repository_root();
    let before = git_status(&repository);
    let scratch = Scratch::new("shards-ship");
    let home = scratch.home("h");
    let (copy, toml) = shipping_copy(&scratch);
    spec(&home, &copy, &["export", "index"]).code(0);
    let root_before = String::from_utf8(read(&copy, INDEX)).unwrap();
    let shard_before = String::from_utf8(read(&copy, SHARD)).unwrap();

    // The first live spec, in path order, set to `shipped`.
    let (input, _, _) = walk(&copy, &toml);
    let live_spec = input
        .files
        .iter()
        .filter(|f| {
            let class = f
                .parsed
                .as_ref()
                .and_then(|p| p.document())
                .and_then(|d| d.fields.as_ref())
                .and_then(|fields| fields.class.as_deref());
            class == Some("spec") && !f.parsed.as_ref().is_some_and(is_tier3_file)
        })
        .map(|f| f.path.clone())
        .min()
        .expect("a live spec");
    let line = entries(&root_before)
        .into_iter()
        .find(|(_, target, _)| *target == live_spec)
        .map(|(_, _, line)| line)
        .unwrap_or_else(|| panic!("{live_spec} in the root"));
    set_status(&copy, &live_spec, "shipped");
    let run = spec(&home, &copy, &["export", "index"]);
    run.code(0);
    let root_after = String::from_utf8(read(&copy, INDEX)).unwrap();
    let shard_after = String::from_utf8(read(&copy, SHARD)).unwrap();
    assert!(
        root_after == root_before.replacen(&format!("{line}\n"), "", 1),
        "the root loses exactly the line of {live_spec}:\n{}",
        {
            let (added, gone) = line_diff(&root_before, &root_after);
            format!("added {added:?}\ngone {gone:?}")
        }
    );
    assert_eq!(root_before.len() - root_after.len(), line.len() + 1);
    let (added, gone) = line_diff(&shard_before, &shard_after);
    assert!(gone.is_empty(), "{gone:?}");
    assert_eq!(added.len(), 1, "{added:?}");
    assert!(
        added[0].starts_with("- [")
            && added[0].ends_with(") shipped")
            && resolve(
                added[0]
                    .split_once("](")
                    .unwrap()
                    .1
                    .split_once(')')
                    .unwrap()
                    .0
            ) == live_spec,
        "{added:?}"
    );

    // A new shipped spec: the root unchanged, the shard one line more.
    write(
        &copy,
        "docs/features/zz-new-shipped.md",
        "---\nclass: spec\nstatus: shipped\nscope: [docs]\n---\n\n# A new shipped spec\n",
    );
    let run = spec(&home, &copy, &["export", "index"]);
    run.code(0);
    let shard_new = String::from_utf8(read(&copy, SHARD)).unwrap();
    assert_eq!(
        run.stdout,
        format!(
            "unchanged {INDEX}: {} bytes\nwrote {SHARD}: {} bytes\n",
            root_after.len(),
            shard_new.len()
        )
    );
    assert!(read(&copy, INDEX) == root_after.as_bytes());
    let (added, gone) = line_diff(&shard_after, &shard_new);
    assert!(gone.is_empty(), "{gone:?}");
    assert_eq!(
        added,
        ["- [docs/features/zz-new-shipped.md](features/zz-new-shipped.md) shipped"]
    );

    assert!(snapshot(&home).is_empty(), "something under HOME");
    assert_eq!(
        git_status(&repository),
        before,
        "the repository was written"
    );
}
