//! Shared helpers of the `specengine-store` integration tests
//! (docs/features/spec-index.md): scratch copies of `fixtures/spec-a` and
//! `fixtures/spec-b` in temp directories (the fixtures are only read), the
//! corpus's scheme and walk read through its own `specengine.toml`, fresh
//! indexes to compare against, the AC-07 edit script, and a source that
//! walks in reverse.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;
use specengine_store::{
    IndexWriter, Listing, Source, SpecIndex, SqliteIndex, UpdateReport, WorkingTree,
};

/// The project every test handle is bound to, unless a test needs another.
pub const PROJECT: &str = "demo";

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

pub fn fixture(name: &str) -> PathBuf {
    repository_root().join("fixtures").join(name)
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique() -> usize {
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// A scratch directory under the system temp dir, canonical, removed on drop
/// (permissions restored first, so a mode-000 entry cannot keep it alive).
pub struct Scratch {
    root: PathBuf,
}

impl Scratch {
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-store-{name}-{}-{}",
            std::process::id(),
            unique()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self {
            root: fs::canonicalize(&path).expect("canonical scratch directory"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn join(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// A DB path in `<scratch>/db/`, outside every worktree of the scratch.
    pub fn db(&self, name: &str) -> PathBuf {
        let dir = self.root.join("db");
        fs::create_dir_all(&dir).expect("db directory");
        dir.join(format!("{name}.db"))
    }

    /// A new, never used DB path.
    pub fn fresh_db(&self) -> PathBuf {
        self.db(&format!("fresh-{}", unique()))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        restore_permissions(&self.root);
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn restore_permissions(dir: &Path) {
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            restore_permissions(&path);
        } else {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
        }
    }
}

/// Copies every directory and regular file under `from` to `to`.
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("copy target");
    for entry in fs::read_dir(from).expect("readable fixture directory") {
        let entry = entry.expect("entry");
        let kind = entry.file_type().expect("file type");
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target);
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// Writes `bytes` at `relative` under `root`, creating parents.
pub fn write(root: &Path, relative: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("parent directory");
    fs::write(&path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

pub fn remove(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::remove_file(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

pub fn read_text(root: &Path, relative: &str) -> String {
    let path = root.join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Sets the modification time of a file without touching its bytes.
pub fn touch(root: &Path, relative: &str, seconds_after_epoch: u64) {
    let path = root.join(relative);
    let file = fs::File::options()
        .write(true)
        .open(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds_after_epoch))
        .expect("set mtime");
}

pub fn chmod(root: &Path, relative: &str, mode: u32) {
    let path = root.join(relative);
    fs::set_permissions(&path, fs::Permissions::from_mode(mode))
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

/// A worktree: its root, and the `[ids]` scheme and `[paths]` walk read
/// from its own `specengine.toml` (the caller re-reads it after an edit).
pub struct Corpus {
    pub root: PathBuf,
    pub scheme: IdScheme,
    pub paths: Paths,
}

impl Corpus {
    /// A scratch copy of `fixtures/<name>` at `<scratch>/<dir>`.
    pub fn copy_of(name: &str, scratch: &Scratch, dir: &str) -> Self {
        let root = scratch.join(dir);
        copy_dir(&fixture(name), &root);
        Self::load(&root)
    }

    pub fn load(root: &Path) -> Self {
        let root = fs::canonicalize(root).expect("corpus root exists");
        let (scheme, paths) = read_config(&root);
        Self {
            root,
            scheme,
            paths,
        }
    }

    /// Re-reads `specengine.toml`, as a caller does after an edit.
    pub fn reload(&mut self) {
        let (scheme, paths) = read_config(&self.root);
        self.scheme = scheme;
        self.paths = paths;
    }

    pub fn tree(&self) -> WorkingTree {
        WorkingTree::new(&self.root, &self.paths).expect("working tree")
    }

    pub fn listing(&self) -> Listing {
        self.tree().list().expect("listing")
    }

    pub fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        write(&self.root, relative, bytes);
    }

    pub fn remove(&self, relative: &str) {
        remove(&self.root, relative);
    }

    pub fn read_text(&self, relative: &str) -> String {
        read_text(&self.root, relative)
    }

    pub fn bytes(&self, relative: &str) -> Vec<u8> {
        let path = self.root.join(relative);
        fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// Replaces the one occurrence of `from` in the file by `to`.
    pub fn replace(&self, relative: &str, from: &str, to: &str) {
        let text = self.read_text(relative);
        assert_eq!(
            text.matches(from).count(),
            1,
            "{relative}: {from:?} must occur exactly once"
        );
        self.write(relative, text.replacen(from, to, 1));
    }

    /// Opens (creating) the index DB `db` bound to this worktree.
    pub fn open(&self, db: &Path) -> SqliteIndex {
        open(db, &self.root)
    }

    /// `update` through this corpus's walk and scheme.
    pub fn update(&self, index: &mut SqliteIndex) -> UpdateReport {
        index
            .update(&self.tree(), &self.scheme)
            .unwrap_or_else(|error| panic!("update of {}: {error}", self.root.display()))
    }

    /// A new DB with one `update` of the corpus as it is now.
    pub fn fresh(&self, scratch: &Scratch) -> SqliteIndex {
        let mut index = self.open(&scratch.fresh_db());
        self.update(&mut index);
        index
    }

    /// The whole-DB dump of a fresh index of the corpus as it is now.
    pub fn fresh_dump(&self, scratch: &Scratch) -> String {
        self.fresh(scratch).dump().expect("dump of a fresh index")
    }
}

fn read_config(root: &Path) -> (IdScheme, Paths) {
    let path = root.join("specengine.toml");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let scheme = IdScheme::from_toml(&text)
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    let paths =
        Paths::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    (scheme, paths)
}

pub fn open(db: &Path, root: &Path) -> SqliteIndex {
    SqliteIndex::open(db, PROJECT, root)
        .unwrap_or_else(|error| panic!("open {} for {}: {error}", db.display(), root.display()))
}

/// BLAKE3 of `bytes`, lower-case hex (as the index stores it).
pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// A readable account of where two dumps differ (the first few lines only).
pub fn dump_diff(left: &str, right: &str) -> String {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();
    let only_left: Vec<&&str> = left_lines
        .iter()
        .filter(|line| !right_lines.contains(line))
        .take(6)
        .collect();
    let only_right: Vec<&&str> = right_lines
        .iter()
        .filter(|line| !left_lines.contains(line))
        .take(6)
        .collect();
    format!(
        "{} vs {} lines\nonly in the incremental index:\n{}\nonly in the fresh index:\n{}",
        left_lines.len(),
        right_lines.len(),
        only_left
            .iter()
            .map(|line| format!("  {}", clip(line)))
            .collect::<Vec<_>>()
            .join("\n"),
        only_right
            .iter()
            .map(|line| format!("  {}", clip(line)))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn clip(line: &str) -> String {
    let clipped: String = line.chars().take(240).collect();
    if clipped.len() < line.len() {
        format!("{clipped}...")
    } else {
        clipped
    }
}

/// Asserts the dump of `index` equals a fresh index of `corpus`.
pub fn assert_equals_fresh(index: &SqliteIndex, corpus: &Corpus, scratch: &Scratch, step: &str) {
    let incremental = index.dump().expect("dump");
    let fresh = corpus.fresh_dump(scratch);
    assert!(
        incremental == fresh,
        "after {step}: the incremental index differs from a fresh one\n{}",
        dump_diff(&incremental, &fresh)
    );
}

/// The file of spec-a whose `[ids]`-prefixed mention is parsed only once the
/// AC-07 script adds the prefix.
pub const ADDED_FILE: &str = "docs/records/R/R-13.md";

/// The spec-a edit script of AC-07: `(name, edit)`; the caller updates the
/// index after every step. The `[ids]` steps re-read `specengine.toml`, the
/// root step re-reads `[paths]`.
/// One named step of the edit script.
pub type Step = (&'static str, fn(&mut Corpus));

pub fn ac07_script() -> Vec<Step> {
    vec![
        ("edit a section", |corpus: &mut Corpus| {
            corpus.replace(
                "docs/spec/movement/stamina.md",
                "- Base rate 10 units/s",
                "- Base rate 12 units/s, fed by emberwick draughts",
            );
        }),
        ("add a file", |corpus: &mut Corpus| {
            corpus.write(
                ADDED_FILE,
                "---\nid: R-13\nkind: requirement\nstatus: draft\n---\n\n\
                 # Oil lasts one night\n\n\
                 The quillfeather lantern holds oil for one night; see EXTRA-01 and R-12.\n",
            );
        }),
        ("delete a file", |corpus: &mut Corpus| {
            corpus.remove("docs/records/Q/Q-032.md");
        }),
        ("rename keeping the bytes", |corpus: &mut Corpus| {
            fs::rename(
                corpus.root.join("docs/records/A/A-102.md"),
                corpus.root.join("docs/records/A/A-109.md"),
            )
            .expect("rename");
        }),
        ("touch an mtime", |corpus: &mut Corpus| {
            touch(&corpus.root, "docs/spec/game.md", 1_000_000_000);
        }),
        ("add an [ids] prefix", |corpus: &mut Corpus| {
            let text = corpus.read_text("specengine.toml");
            corpus.write(
                "specengine.toml",
                format!("{text}EXTRA = {{ kind = \"extra\", width = 2 }}\n"),
            );
            corpus.reload();
        }),
        ("remove the [ids] prefix", |corpus: &mut Corpus| {
            let text = corpus.read_text("specengine.toml");
            let line = "EXTRA = { kind = \"extra\", width = 2 }\n";
            assert!(text.ends_with(line), "the added prefix is the last line");
            corpus.write("specengine.toml", &text[..text.len() - line.len()]);
            corpus.reload();
        }),
        ("drop a root", |corpus: &mut Corpus| {
            let text = corpus.read_text("specengine.toml");
            corpus.write(
                "specengine.toml",
                format!("{text}\n[paths]\nroots = [\"docs/spec\", \"docs/records\"]\n"),
            );
            corpus.reload();
            assert!(
                !corpus
                    .paths
                    .roots
                    .iter()
                    .any(|root| root == "docs/features"),
                "the features root is dropped: {:?}",
                corpus.paths.roots
            );
        }),
    ]
}

/// A source that lists the inner source's paths in reverse order (rows
/// must not depend on it).
pub struct Reversed<'a>(pub &'a dyn Source);

impl Source for Reversed<'_> {
    fn root(&self) -> &Path {
        self.0.root()
    }

    fn list(&self) -> io::Result<Listing> {
        let mut listing = self.0.list()?;
        listing.paths.reverse();
        Ok(listing)
    }

    fn probe(&self, path: &str) -> bool {
        self.0.probe(path)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.0.read(path)
    }
}

/// Every `.md` file under `dir` (recursively, following nothing special),
/// `root`-relative with `/`, sorted.
pub fn md_files_under(root: &Path, dir: &str) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let kind = entry.file_type().expect("file type");
            if kind.is_dir() {
                walk(root, &path, out);
            } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                out.push(
                    path.strip_prefix(root)
                        .expect("under root")
                        .to_str()
                        .expect("UTF-8 name")
                        .to_owned(),
                );
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &root.join(dir), &mut out);
    out.sort();
    out
}

/// `git status --porcelain --untracked-files=all -- fixtures/` of the repository.
pub fn fixtures_git_status() -> String {
    let output = Command::new("git")
        .current_dir(repository_root())
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "fixtures/",
        ])
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Every file under `fixtures/<name>`, relative path to bytes.
pub fn fixture_bytes(name: &str) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("readable fixture").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).expect("under root");
                out.insert(
                    relative.to_string_lossy().into_owned(),
                    fs::read(&path).expect("readable fixture file"),
                );
            }
        }
    }
    let root = fixture(name);
    let mut out = std::collections::BTreeMap::new();
    walk(&root, &root, &mut out);
    assert!(!out.is_empty(), "fixtures/{name} is empty");
    out
}

/// Every stored `(path, ord, id)` of the index.
pub fn stored_nodes(index: &SqliteIndex) -> Vec<(String, usize, Option<String>)> {
    let mut out = Vec::new();
    for path in index.files().expect("files") {
        let file = index.file(&path).expect("file").expect("listed file");
        if let Some(parsed) = file.parsed {
            for (ord, node) in parsed.nodes.iter().enumerate() {
                out.push((path.clone(), ord, node.id.clone()));
            }
        }
    }
    out
}
