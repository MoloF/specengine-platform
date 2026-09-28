//! Heuristic module paths (`qpath`) from a file's place in a Cargo package.
//!
//! Layer A of 05 §5.1: the path is derived from the file path and the inline
//! `mod` chain; whatever the heuristic cannot decide is reported as an
//! [`Ambiguity`], never guessed. Link identity comes from markers, not from
//! this path, so a reported ambiguity costs nothing but honesty.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::items::ItemRecord;

/// What a file is inside its package, judged by its relative path alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRole {
    /// `src/lib.rs`, `src/main.rs`, `build.rs`, `src/bin/x.rs`, `examples/x.rs`, `tests/x.rs`, …
    CrateRoot,
    /// A module file: `src/a/b.rs` → `a::b`, `src/a/mod.rs` → `a`.
    Module(Vec<String>),
    /// Not under any layout Cargo recognises; its module path is unknown.
    Unrooted,
}

/// Why an item's `qpath` is not trustworthy. One reason per item, in this priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ambiguity {
    /// The file is the target of a `#[path]` attribute: its path says nothing about its module.
    PathAttribute,
    /// The file is outside every recognised target layout.
    Unrooted,
    /// Another item resolves to the same `qpath` (adjacent impls, `cfg`-gated twins).
    Duplicate,
}

impl Ambiguity {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PathAttribute => "path_attribute",
            Self::Unrooted => "unrooted",
            Self::Duplicate => "duplicate",
        }
    }
}

/// `package::module::owner::name`; `package` is the package directory relative
/// to the corpus root (`.` for the root package), never an absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QPath {
    pub package: String,
    pub module: Vec<String>,
    pub owner: Option<String>,
    pub name: String,
}

impl fmt::Display for QPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.package)?;
        for segment in &self.module {
            write!(f, "::{segment}")?;
        }
        if let Some(owner) = &self.owner {
            write!(f, "::{owner}")?;
        }
        write!(f, "::{}", self.name)
    }
}

/// The `qpath` of an item from its file's role and inline `mod` chain.
///
/// An `impl` block is named `impl Type` / `impl <Type as Trait>` so it never
/// collides with the type it implements; its methods are `Type::method` and
/// `<Type as Trait>::method` (05 §5.1). Two adjacent inherent impls of one
/// type do collide, and that is reported as [`Ambiguity::Duplicate`].
#[must_use]
pub fn qpath(package: &str, role: &FileRole, item: &ItemRecord) -> QPath {
    let mut module = match role {
        FileRole::Module(path) => path.clone(),
        FileRole::CrateRoot | FileRole::Unrooted => Vec::new(),
    };
    module.extend(item.mod_path.iter().cloned());
    let name = if item.kind == "impl_item" {
        format!("impl {}", item.label)
    } else {
        item.label.clone()
    };
    QPath {
        package: package.to_owned(),
        module,
        owner: item.owner.clone(),
        name,
    }
}

/// Classifies a file by its path relative to the package directory.
#[must_use]
pub fn file_role(relative: &Path) -> FileRole {
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    match parts.as_slice() {
        ["build.rs"] => FileRole::CrateRoot,
        ["src", "lib.rs" | "main.rs"] => FileRole::CrateRoot,
        ["src", "bin", _name] => FileRole::CrateRoot,
        ["src", "bin", _name, rest @ ..] => module_of(rest),
        ["examples" | "tests" | "benches", _name] => FileRole::CrateRoot,
        ["examples" | "tests" | "benches", _name, rest @ ..] => module_of(rest),
        ["src", rest @ ..] if !rest.is_empty() => module_of(rest),
        _ => FileRole::Unrooted,
    }
}

/// `a/b.rs` → `a::b`, `a/mod.rs` → `a`, `main.rs` → crate root of that target.
fn module_of(parts: &[&str]) -> FileRole {
    let Some((last, dirs)) = parts.split_last() else {
        return FileRole::Unrooted;
    };
    let Some(stem) = last.strip_suffix(".rs") else {
        return FileRole::Unrooted;
    };
    let mut module: Vec<String> = dirs.iter().map(|d| (*d).to_owned()).collect();
    match stem {
        "main" if dirs.is_empty() => return FileRole::CrateRoot,
        "mod" | "main" => {}
        other => module.push(other.to_owned()),
    }
    if module.is_empty() {
        FileRole::CrateRoot
    } else {
        FileRole::Module(module)
    }
}

/// The nearest ancestor directory of `file` (relative to the corpus root) for
/// which `is_package_dir` holds; `Some("")` means the corpus root itself.
pub fn package_dir(file: &Path, is_package_dir: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut dir = file.parent();
    while let Some(candidate) = dir {
        if is_package_dir(candidate) {
            return Some(candidate.to_path_buf());
        }
        dir = candidate.parent();
    }
    None
}

/// Target of `#[path = "value"]` on a `mod` declaration in `declaring_file`:
/// relative to the declaring file's directory, normalised lexically.
#[must_use]
pub fn resolve_path_attribute(declaring_file: &Path, value: &str) -> PathBuf {
    let base = declaring_file.parent().unwrap_or(Path::new(""));
    let mut out = PathBuf::new();
    for component in base.join(value).components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
