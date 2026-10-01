//! This repository's documentation config and walk, shared by the store's
//! `check_parity.rs` and the CLI's `parity.rs` (the latter through a
//! `#[path]` module; docs/features/spec-cli-switch.md, "Migrations"): the
//! committed root `specengine.toml` as read (never rebuilt here), the
//! registered export and gate commands of its `[[generators]]` entry, and
//! AC-03's independent std walk: every `*.md` under the root outside
//! `.`-named directories and the frozen [`SKIP_DIRS`], its name not
//! starting with `_`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The index path of the root config (`[paths] index`): the root of the
/// index set.
pub const INDEX: &str = "docs/index.md";

/// The one shard of the root config's index entry, the archive (`shards`,
/// `tier3 = true`; ADR-0030, docs/features/index-shards.md).
pub const INDEX_SHARD: &str = "docs/index-archive.md";

/// X: the registered export command of the index (`[[generators]] command`).
pub const EXPORT: &str = "cargo run -q -p specengine-cli -- export index";

/// G: the registered gate of the index entry (`[[generators]] gate`).
pub const GATE: &str = "cargo run -q -p specengine-cli -- check";

/// The directory names the std walk never enters (AC-03, frozen: the old
/// walk's list without its `.`-named entries, which the dot rule covers).
pub const SKIP_DIRS: [&str; 5] = [
    "fixtures",
    "target",
    "target.noindex",
    "node_modules",
    "dist",
];

/// This repository (both including crates live at `crates/<name>`).
pub fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// The committed root `specengine.toml`, as read.
pub fn root_toml() -> String {
    let toml =
        fs::read_to_string(repository().join("specengine.toml")).expect("the root specengine.toml");
    for registered in [EXPORT, GATE, INDEX, INDEX_SHARD] {
        let quoted = format!("\"{registered}\"");
        assert!(toml.contains(&quoted), "{quoted} in:\n{toml}");
    }
    toml
}

/// `toml` with the one occurrence of `from` replaced by `to` (the in-test
/// mutations of the config; the committed file is never written).
pub fn mutated(toml: &str, from: &str, to: &str) -> String {
    assert_eq!(toml.matches(from).count(), 1, "{from:?} once in the config");
    toml.replacen(from, to, 1)
}

/// The finding the pin mutation must give, by (code, path, subject): a
/// five-digit mention that resolves nowhere.
pub const DANGLING: (&str, &str, &str) =
    ("mention-dangling", "docs/canon/spec-check.md", "ADR-00011");

/// The pin mutation (AC-02): the five-digit mention the spec-writer removed
/// from docs/canon/spec-check.md "Rules" at shipping, put back into the
/// copy under `root` (never this repository) in place of a longer
/// parenthesis, so the file stays under its canon cap. Returns the 1-based
/// line of the mention.
pub fn add_dangling_mention(root: &Path) -> usize {
    assert_ne!(
        root.canonicalize().expect("the copy exists"),
        repository(),
        "the repository is only read"
    );
    let path = root.join(DANGLING.1);
    let text = fs::read_to_string(&path).expect("the copied canon");
    let from = "else `file-name` (a bare prefix is not enough).";
    let to = "else `file-name` (`ADR-00011.md`, ADR-0001).";
    assert!(to.len() <= from.len(), "the copy does not grow");
    assert_eq!(text.matches(from).count(), 1, "{from:?} once in {path:?}");
    let at = text.find(from).unwrap();
    fs::write(&path, text.replacen(from, to, 1)).expect("the copy is written");
    text[..at].matches('\n').count() + 1
}

/// AC-03's std walk of `root`: root-relative `/` paths, sorted. Symlinks
/// are neither followed nor listed.
pub fn std_walk(root: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let entry = entry.expect("a directory entry");
            let name = entry.file_name().to_str().expect("a UTF-8 name").to_owned();
            let kind = entry.file_type().expect("a file type");
            if kind.is_dir() {
                if !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push(entry.path());
                }
            } else if kind.is_file() && name.ends_with(".md") && !name.starts_with('_') {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("under the root")
                    .to_str()
                    .expect("a UTF-8 path")
                    .replace('\\', "/");
                found.insert(relative);
            }
        }
    }
    found
}
