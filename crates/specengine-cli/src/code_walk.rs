//! The code tree `spec refs` and `spec lock accept` read (canon
//! `code-bindings`, "The walk"):
//! the `[code]` roots walked, every file read once, Rust files parsed and
//! their comments bound (`specengine-code`), RON comments anchored, the
//! Rust items named over their packages. Nothing is written; nothing about
//! the code is stored.
//!
//! - **Files**: regular files ending exactly in `.rs` or `.ron` under the
//!   roots, minus `[code] exclude`, as the spec walk lists `.md` files
//!   (`crates/specengine-store/README.md` "Walk"): below a root, symlinks
//!   and `.`-names skipped; no `.gitignore`; names that are not UTF-8
//!   skipped and counted; root-relative `/` paths as the OS lists them,
//!   byte-sorted, repeats dropped. A directory every path below which an
//!   exclude glob `G/**` matches is not listed.
//! - **Roots**: each looked up by exact name, component by component; a
//!   missing one (a symlink on the way included) gives
//!   `note: [code] root <path> does not exist`, a file of another kind
//!   `note: [code] root <path> is not a .rs or .ron file`; neither
//!   contributes.
//! - **Content** that is not UTF-8: `note: <path> is not UTF-8: skipped`,
//!   counted.
//! - **Gaps**: a directory that cannot be listed (also one on the way to a
//!   root), a file that cannot be read, a Rust file the parser gives no tree
//!   for: the first by path (a file before a directory on the same path)
//!   ends the command, exit 2, `spec: the code walk is incomplete: <gap>`;
//!   never a partial answer.
//! - **Packages**: a Rust file's package is the nearest directory, its own
//!   up to the root, holding a regular file named exactly `Cargo.toml`; its
//!   targets are Cargo's layout over the package's walked files
//!   (`layout_targets`), named by the manifest's `[package] name`. A
//!   manifest that reads as TOML without `[package]` is a virtual root: no
//!   targets, as `cargo metadata` answers (its files are unrooted). One
//!   that cannot be read or parsed, or names no package, takes the
//!   directory's name, `.` for the root (never the root's own name: the
//!   output does not depend on where the tree is checked out). No package:
//!   unrooted. No subprocess.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, FileType};
use std::path::{Path, PathBuf};

use specengine_code::{
    CommentAnchor, NamedFile, Names, PackageTargets, ROOT_PACKAGE, RustBinding, RustParser,
    TargetSource, bind_file, comment_anchors, layout_targets, name_files,
};
use specengine_core::CodeConfig;

use crate::{CliError, Message};

/// The extension of a Rust file.
pub(crate) const RUST_EXTENSION: &str = ".rs";

/// The extension of a RON file.
pub(crate) const RON_EXTENSION: &str = ".ron";

/// The manifest that makes a directory a package.
pub(crate) const MANIFEST: &str = "Cargo.toml";

/// The manifest's table and key naming the package.
const PACKAGE_TABLE: &str = "package";
const PACKAGE_NAME: &str = "name";

/// One Rust file, read and bound.
pub(crate) struct RustFile {
    pub path: String,
    pub text: String,
    pub binding: RustBinding,
}

/// One RON file, read and anchored.
pub(crate) struct RonFile {
    pub path: String,
    pub text: String,
    pub anchors: Vec<CommentAnchor>,
}

/// The code tree as `spec refs` reads it.
pub(crate) struct CodeTree {
    /// By path.
    pub rust: Vec<RustFile>,
    /// By path.
    pub ron: Vec<RonFile>,
    /// Per Rust file (same order): its module's and its items' names.
    pub names: Names,
    /// Files and names skipped for not being UTF-8.
    pub skipped: usize,
}

/// What the listing found.
#[derive(Default)]
struct Listing {
    /// Root-relative file paths, byte-sorted, unique.
    files: BTreeSet<String>,
    /// Directories that could not be listed (`""`: the root itself).
    unlisted: BTreeSet<String>,
    /// Directories listed, and those of them holding a regular `Cargo.toml`.
    listed: BTreeSet<String>,
    manifests: BTreeSet<String>,
    skipped_names: usize,
}

/// Lists, reads, binds and names the code tree under `root` (canonical).
pub(crate) fn read_tree(
    root: &Path,
    config: &CodeConfig,
    messages: &mut Vec<Message>,
) -> Result<CodeTree, CliError> {
    let listing = list(root, config, messages);
    let first_unlisted = listing.unlisted.first().cloned();
    let mut parser = RustParser::new()
        .map_err(|error| CliError::spec(format!("the Rust grammar cannot be loaded: {error}")))?;
    let mut tree = CodeTree {
        rust: Vec::new(),
        ron: Vec::new(),
        names: Names::default(),
        skipped: listing.skipped_names,
    };
    for path in &listing.files {
        if let Some(dir) = &first_unlisted
            && dir.as_str() < path.as_str()
        {
            break;
        }
        let bytes = fs::read(root.join(path))
            .map_err(|error| incomplete(&format!("`{path}` cannot be read ({error})")))?;
        let Ok(text) = String::from_utf8(bytes) else {
            messages.push(Message::Note(format!("{path} is not UTF-8: skipped")));
            tree.skipped += 1;
            continue;
        };
        if path.ends_with(RUST_EXTENSION) {
            let binding = bind_file(&mut parser, &text).ok_or_else(|| {
                incomplete(&format!(
                    "`{path}` cannot be parsed (the parser gave no tree)"
                ))
            })?;
            tree.rust.push(RustFile {
                path: path.clone(),
                text,
                binding,
            });
        } else {
            let anchors = comment_anchors(&text);
            tree.ron.push(RonFile {
                path: path.clone(),
                text,
                anchors,
            });
        }
    }
    if let Some(dir) = first_unlisted {
        let shown = if dir.is_empty() { "." } else { dir.as_str() };
        return Err(incomplete(&format!(
            "the directory `{shown}` cannot be listed"
        )));
    }
    tree.names = name_tree(root, &listing, &tree.rust);
    Ok(tree)
}

/// Exit 2: the walk could not see everything.
fn incomplete(gap: &str) -> CliError {
    CliError::spec(format!("the code walk is incomplete: {gap}"))
}

/// The files under the roots, the directories that could not be listed.
fn list(root: &Path, config: &CodeConfig, messages: &mut Vec<Message>) -> Listing {
    let mut listing = Listing::default();
    for written in &config.roots {
        match resolve(root, written) {
            Ok(Some(kind)) if kind.is_dir() => walk(root, written, config, &mut listing),
            Ok(Some(kind)) if kind.is_file() && is_code_name(written) => {
                if !config.is_excluded(written) {
                    listing.files.insert(written.clone());
                }
            }
            Ok(Some(kind)) if !kind.is_symlink() => messages.push(Message::Note(format!(
                "[code] root {written} is not a .rs or .ron file"
            ))),
            Ok(_) => messages.push(Message::Note(format!(
                "[code] root {written} does not exist"
            ))),
            Err(dir) => {
                listing.unlisted.insert(dir);
            }
        }
    }
    listing
}

/// The kind of the root-relative `path`, looked up by exact name component
/// by component: `Ok(None)` when a component is missing, or a symlink or
/// no directory on the way (the last component's own kind is returned,
/// a symlink included); `Err` names the directory on the way (`""`: the
/// root) that could not be listed.
fn resolve(root: &Path, path: &str) -> Result<Option<FileType>, String> {
    let mut dir = root.to_path_buf();
    let mut read: Vec<&str> = Vec::new();
    let mut components = path.split('/').peekable();
    while let Some(component) = components.next() {
        let Some(kind) = entry_kind(&dir, OsStr::new(component)).map_err(|_| read.join("/"))?
        else {
            return Ok(None);
        };
        if components.peek().is_none() {
            return Ok(Some(kind));
        }
        if kind.is_symlink() || !kind.is_dir() {
            return Ok(None);
        }
        dir.push(component);
        read.push(component);
    }
    Ok(None)
}

/// The kind of the entry named exactly `name` in `dir` (never followed);
/// `Ok(None)` when there is none, `Err` when `dir` cannot be listed.
fn entry_kind(dir: &Path, name: &OsStr) -> std::io::Result<Option<FileType>> {
    let mut unread = None;
    for entry in fs::read_dir(dir)? {
        match entry {
            Ok(entry) if entry.file_name() == name => return entry.file_type().map(Some),
            Ok(_) => {}
            Err(error) => unread = Some(error),
        }
    }
    match unread {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

/// `name` ends exactly in `.rs` or `.ron`.
fn is_code_name(name: &str) -> bool {
    name.ends_with(RUST_EXTENSION) || name.ends_with(RON_EXTENSION)
}

/// Lists the directory `start` (root-relative) and everything below it, on
/// an explicit stack.
fn walk(root: &Path, start: &str, config: &CodeConfig, listing: &mut Listing) {
    let mut pending = vec![start.to_owned()];
    while let Some(relative) = pending.pop() {
        if config.excludes_all_below(&relative) {
            continue;
        }
        let Ok(entries) = fs::read_dir(root.join(&relative)) else {
            listing.unlisted.insert(relative);
            continue;
        };
        let mut unreadable = false;
        for entry in entries {
            let Ok(entry) = entry else {
                unreadable = true;
                continue;
            };
            let name = entry.file_name();
            let bytes = name.as_encoded_bytes();
            if bytes.starts_with(b".") {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                unreadable = true;
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let Some(text) = name.to_str() else {
                let code = bytes.ends_with(RUST_EXTENSION.as_bytes())
                    || bytes.ends_with(RON_EXTENSION.as_bytes());
                if kind.is_dir() || (kind.is_file() && code) {
                    listing.skipped_names += 1;
                }
                continue;
            };
            let child = format!("{relative}/{text}");
            if kind.is_dir() {
                pending.push(child);
            } else if kind.is_file() {
                if text == MANIFEST {
                    listing.manifests.insert(relative.clone());
                }
                if is_code_name(text) && !config.is_excluded(&child) {
                    listing.files.insert(child);
                }
            }
        }
        if unreadable {
            listing.unlisted.insert(relative.clone());
        }
        listing.listed.insert(relative);
    }
}

/// The names of the Rust files: packages found, their targets laid out,
/// every item and module named.
fn name_tree(root: &Path, listing: &Listing, rust: &[RustFile]) -> Names {
    let mut probed: BTreeMap<String, bool> = BTreeMap::new();
    let mut packages: Vec<Option<PathBuf>> = Vec::with_capacity(rust.len());
    let mut package_files: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for file in rust {
        let package = package_of(root, &file.path, listing, &mut probed);
        if let Some(dir) = &package {
            let inner = Path::new(&file.path)
                .strip_prefix(dir)
                .map(Path::to_path_buf)
                .unwrap_or_default();
            package_files.entry(dir.clone()).or_default().push(inner);
        }
        packages.push(package);
    }
    let tables: BTreeMap<PathBuf, PackageTargets> = package_files
        .iter()
        .map(|(dir, files)| (dir.clone(), package_targets(root, dir, files)))
        .collect();
    let paths: Vec<PathBuf> = rust.iter().map(|file| PathBuf::from(&file.path)).collect();
    let named: Vec<NamedFile<'_>> = rust
        .iter()
        .zip(&paths)
        .zip(&packages)
        .map(|((file, path), package)| NamedFile {
            path,
            package: package.as_deref(),
            analysis: &file.binding.analysis,
        })
        .collect();
    name_files(&named, &tables)
}

/// The nearest directory of `path`, its own up to the root (`""`),
/// holding a regular `Cargo.toml`.
fn package_of(
    root: &Path,
    path: &str,
    listing: &Listing,
    probed: &mut BTreeMap<String, bool>,
) -> Option<PathBuf> {
    let mut dir = path;
    loop {
        dir = match dir.rfind('/') {
            Some(slash) => &dir[..slash],
            None if dir.is_empty() => return None,
            None => "",
        };
        let holds = if listing.listed.contains(dir) {
            listing.manifests.contains(dir)
        } else {
            *probed
                .entry(dir.to_owned())
                .or_insert_with(|| holds_manifest(&root.join(dir)))
        };
        if holds {
            return Some(PathBuf::from(dir));
        }
        if dir.is_empty() {
            return None;
        }
    }
}

/// `dir` holds a regular file named exactly `Cargo.toml` (a listing error:
/// no).
fn holds_manifest(dir: &Path) -> bool {
    entry_kind(dir, OsStr::new(MANIFEST))
        .ok()
        .flatten()
        .is_some_and(|kind| kind.is_file())
}

/// What a package's manifest says about its name.
enum Manifest {
    /// `[package] name`, a string.
    Named(String),
    /// TOML without `[package]`: a virtual root.
    Virtual,
    /// Unreadable, not TOML, or a `[package]` without a string `name`.
    Unread,
}

/// The targets of the package in `dir` (relative to the root) over its
/// walked `files` (relative to `dir`).
fn package_targets(root: &Path, dir: &Path, files: &[PathBuf]) -> PackageTargets {
    let name = match read_manifest(&root.join(dir).join(MANIFEST)) {
        Manifest::Virtual => {
            return PackageTargets {
                source: TargetSource::Layout,
                targets: Vec::new(),
            };
        }
        Manifest::Named(name) => name,
        Manifest::Unread => dir.file_name().map_or_else(
            || ROOT_PACKAGE.to_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
    };
    layout_targets(&name, files.iter().map(PathBuf::as_path))
}

/// The manifest at `path`, read for its `[package] name` only.
fn read_manifest(path: &Path) -> Manifest {
    let Some(manifest) = fs::read_to_string(path)
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
    else {
        return Manifest::Unread;
    };
    match manifest.get(PACKAGE_TABLE) {
        None => Manifest::Virtual,
        Some(package) => package
            .get(PACKAGE_NAME)
            .and_then(toml::Value::as_str)
            .map_or(Manifest::Unread, |name| Manifest::Named(name.to_owned())),
    }
}
