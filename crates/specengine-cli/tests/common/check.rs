//! Helpers of the pass 2a.1 tests (docs/features/spec-cli-check.md): "the
//! library" (`check_worktree` on the same copy, `today_utc()`), the stdout
//! a report prints, baselines built from a report, the index registry
//! written into a copy's config (never into `fixtures/`), and the library's
//! own `render_index` of a copy.

use std::path::Path;

use specengine_core::ProjectConfig;
use specengine_core::check::{CheckConfig, Finding, Report, render_index};
use specengine_store::{WorkingTree, check_input, check_worktree, today_utc};

use super::read_text;

/// A far expiry: the entry holds.
pub const FAR: &str = "2999-12-31";
/// A past expiry: the entry has expired.
pub const PAST: &str = "2000-01-01";

/// `check_worktree` on `root` with its `specengine.toml`, the default
/// baseline and today's UTC date.
pub fn library(root: &Path) -> Report {
    check_worktree(root, &root.join("specengine.toml"), None, &today_utc())
}

/// `check_worktree` with the config and baseline given.
pub fn library_with(root: &Path, config: &Path, baseline: Option<&Path>) -> Report {
    check_worktree(root, config, baseline, &today_utc())
}

/// Each of `Report::lines(detail)` on one line (CR and LF → space) and
/// `\n`-ended: what `spec check` prints.
pub fn text(report: &Report, detail: bool) -> String {
    report
        .lines(detail)
        .iter()
        .map(|line| format!("{}\n", line.replace("\r\n", " ").replace(['\r', '\n'], " ")))
        .collect()
}

/// `Report::to_json()` and a line end: what `spec --json check` prints.
pub fn json(report: &Report) -> String {
    format!("{}\n", report.to_json())
}

/// `text` as a TOML basic string.
pub fn quoted(text: &str) -> String {
    serde_json::to_string(text).expect("a string encodes")
}

/// The findings that block under `enforce`.
pub fn blocking(report: &Report) -> Vec<&Finding> {
    report
        .findings
        .iter()
        .filter(|finding| finding.blocks_when_enforced())
        .collect()
}

/// One `[[debt]]` entry per distinct `(code, path, subject)` of the
/// blocking findings, in order; the first expires `first`, the others
/// [`FAR`].
pub fn baseline_covering(report: &Report, first: &str) -> String {
    let mut seen = Vec::new();
    let mut out = String::new();
    for finding in blocking(report) {
        let triple = (&finding.code, &finding.path, &finding.subject);
        if seen.contains(&triple) {
            continue;
        }
        let expires = if seen.is_empty() { first } else { FAR };
        seen.push(triple);
        out.push_str(&format!(
            "[[debt]]\ncode    = {}\npath    = {}\nsubject = {}\nreason  = \"test debt\"\nexpires = \"{expires}\"\n\n",
            quoted(&finding.code),
            quoted(&finding.path),
            quoted(&finding.subject),
        ));
    }
    assert!(!seen.is_empty(), "no blocking finding to cover");
    out
}

/// An ID of the fixture's scheme that nothing defines.
pub fn undefined_id(fixture: &str) -> &'static str {
    if fixture == "spec-a" {
        "R-77"
    } else {
        "REQ-777"
    }
}

/// A spec document whose one error is a front-matter reference to an
/// undefined ID (`ref-dangling`).
pub fn dangling_document(fixture: &str) -> String {
    format!(
        "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\nrefs: [{}]\n---\n\n# Dangling\n\nText.\n",
        undefined_id(fixture)
    )
}

/// The `[paths] index` of the fixture's registry: inside its walk.
pub fn index_path(fixture: &str) -> &'static str {
    if fixture == "spec-a" {
        "docs/spec/index.md"
    } else {
        "docs/index.md"
    }
}

/// `config` with `key = value` in its `[paths]` table: the key's line
/// replaced when present, else added after the header; a `[paths]` table
/// appended when there is none.
pub fn set_paths_key(config: &str, key: &str, value: &str) -> String {
    let lines: Vec<&str> = config.lines().collect();
    let Some(header) = lines.iter().position(|line| line.trim() == "[paths]") else {
        return format!("{config}\n[paths]\n{key} = {value}\n");
    };
    let end = lines[header + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with('['))
        .map_or(lines.len(), |at| header + 1 + at);
    let mut out: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
    let existing = (header + 1..end).find(|&at| {
        let line = lines[at].trim_start();
        line.strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    });
    match existing {
        Some(at) => out[at] = format!("{key} = {value}"),
        None => out.insert(header + 1, format!("{key} = {value}")),
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// `config` without its `[project]` table.
pub fn without_project(config: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in config.lines() {
        if line.trim_start().starts_with('[') {
            inside = line.trim() == "[project]";
        }
        if !inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// `config` with `key = value` added as the first key of `[project]`.
pub fn with_project_key(config: &str, key_line: &str) -> String {
    let mut out = String::new();
    for line in config.lines() {
        out.push_str(line);
        out.push('\n');
        if line.trim() == "[project]" {
            out.push_str(key_line);
            out.push('\n');
        }
    }
    assert!(out.contains(key_line), "no [project] table");
    out
}

/// `config` with `[paths] index` and one `[[generators]]` entry
/// (`index = true`, `writes` = the index) for `command` and `gate`.
pub fn registered(config: &str, index: &str, command: &str, gate: Option<&str>) -> String {
    let mut text = set_paths_key(config, "index", &quoted(index));
    text.push_str(&format!(
        "\n[[generators]]\ncommand = {}\nwrites  = [{}]\nindex   = true\n",
        quoted(command),
        quoted(index)
    ));
    if let Some(gate) = gate {
        text.push_str(&format!("gate    = {}\n", quoted(gate)));
    }
    text
}

/// The 1-based line of the first line of `text` that starts with `start`.
pub fn line_of(text: &str, start: &str) -> usize {
    text.lines()
        .position(|line| line.starts_with(start))
        .unwrap_or_else(|| panic!("no line starts with {start:?}"))
        + 1
}

/// The library's render of `root`'s index with its own config: core's
/// `render_index` over the store's walk, the registered entry, `[paths]
/// index`.
pub fn library_render(root: &Path) -> String {
    let text = read_text(root, "specengine.toml");
    let project = ProjectConfig::from_toml(&text).expect("a valid ProjectConfig");
    let check = CheckConfig::from_toml(&text).expect("a valid CheckConfig");
    let generator = check.index_generator().expect("an index = true entry");
    let index = project.paths.index.as_deref().expect("[paths] index");
    let tree = WorkingTree::new(root, &project.paths).expect("a working tree");
    let input = check_input(&tree, &project.scheme);
    render_index(&input, index, generator)
}

/// The finding codes of a `--json` report on stdout.
pub fn codes(stdout: &str) -> Vec<String> {
    let json: serde_json::Value = serde_json::from_str(stdout).expect("a JSON report");
    json["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|finding| finding["code"].as_str().expect("code").to_owned())
        .collect()
}
