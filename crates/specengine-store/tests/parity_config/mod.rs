//! The parity config of this repository's documents (Q-7), shared by the
//! store's `check_parity.rs` and the CLI's `parity.rs` (the latter through
//! a `#[path]` module, docs/features/spec-cli-check.md AC-10, AC-14): the
//! Data example of docs/features/spec-check.md with `roots` = the
//! top-level `.md` files and the directories not `.`-named nor in
//! `xtask`'s `SKIP_DIRS` (read from `xtask/src/docs/mod.rs`), `exclude` =
//! `**/_*.md` plus `**/<name>/**` per `SKIP_DIRS`, `[classes]` all four
//! closed as `docs/README.md` "Front-matter contract", and the generator
//! registry of spec-check-graph's Data: this repository registers
//! nothing until 2b, so the entry keeps `xtask`'s command and gate.

use std::fs;
use std::path::{Path, PathBuf};

/// The index path of the parity config.
pub const INDEX: &str = "docs/index.md";

/// The generator registry of the parity config (spec-check-graph, Data).
pub const REGISTRY: &str = "\
[[generators]]
command = \"cargo xtask docs index --write\"
writes  = [\"docs/index.md\"]
index   = true
gate    = \"cargo xtask docs check\"
";

/// This repository (both including crates live at `crates/<name>`).
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// `xtask`'s `SKIP_DIRS`, read from its source.
pub fn skip_dirs() -> Vec<String> {
    let source = fs::read_to_string(repository().join("xtask/src/docs/mod.rs"))
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

/// The parity config for the documents under `root`.
pub fn parity_toml(root: &Path, with_template_exclude: bool) -> String {
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

{REGISTRY}",
        roots = roots.join(", "),
        exclude = exclude.join(", ")
    )
}
