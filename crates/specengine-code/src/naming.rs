//! A code tree's Rust items and modules named at once (task spec
//! `code-refs`, "Binding rules": a bound item gives its `qpath` and its
//! ambiguity, shown, not resolved). The units and paths of
//! `docs/canon/code-identity.md` "Units and `qpath`" over the caller's
//! package tables, then one ambiguity per item, the first that applies — `path_attribute` (its file is the
//! target of a `#[path]` attribute and in no shared unit), `unrooted` (its
//! file is in no unit), `duplicate` (another item of the tree renders the
//! same `qpath`).
//!
//! A file's module is `<package>[::<unit>][::<module>…]`: the crate root
//! alone, the unit omitted for the primary target, as the items' paths
//! begin; an unrooted file's module is its package alone. It is ambiguous
//! by the first two reasons only (a module has no twin by path).
//!
//! The package is the package directory relative to the root, `.` for the
//! root and for a file outside every package.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::items::FileAnalysis;
use crate::qpath::{self, Ambiguity, FileRole, PackageTargets, TargetIndex, UnitKind};

/// The package label of the root package and of a file outside every
/// package.
pub const ROOT_PACKAGE: &str = ".";

/// One Rust file of the tree.
#[derive(Debug, Clone, Copy)]
pub struct NamedFile<'a> {
    /// Relative to the root.
    pub path: &'a Path,
    /// Its package directory relative to the root (empty: the root), or
    /// `None` outside every package.
    pub package: Option<&'a Path>,
    pub analysis: &'a FileAnalysis,
}

/// A rendered path and why it is not trusted, if it is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub qpath: String,
    pub ambiguity: Option<Ambiguity>,
}

/// The names of a tree's files, in the order given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names {
    /// Per file: its module.
    pub modules: Vec<Name>,
    /// Per file, per item of its [`FileAnalysis::items`].
    pub items: Vec<Vec<Name>>,
}

/// Names every file's module and items; `tables` maps a package
/// directory (relative to the root) to its targets.
#[must_use]
pub fn name_files(files: &[NamedFile<'_>], tables: &BTreeMap<PathBuf, PackageTargets>) -> Names {
    let indexes: BTreeMap<&Path, TargetIndex<'_>> = tables
        .iter()
        .map(|(dir, table)| (dir.as_path(), TargetIndex::new(table)))
        .collect();
    let mut path_targets: BTreeSet<PathBuf> = BTreeSet::new();
    for file in files {
        for declaration in &file.analysis.mod_declarations {
            if let Some(value) = &declaration.path_attribute {
                path_targets.insert(qpath::resolve_path_attribute(file.path, value));
            }
        }
    }
    let mut names = Names::default();
    let mut occurrences: HashMap<String, usize> = HashMap::new();
    for file in files {
        let label = file
            .package
            .map(|package| package.to_string_lossy().replace('\\', "/"))
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| ROOT_PACKAGE.to_owned());
        let role = file
            .package
            .and_then(|package| {
                let index = indexes.get(package)?;
                let inner = file.path.strip_prefix(package).ok()?;
                Some(index.file_role(inner))
            })
            .unwrap_or(FileRole::Unrooted);
        // A shared unit is named by its location, however it is included.
        let shared = role
            .unit()
            .is_some_and(|unit| unit.kind == UnitKind::Shared);
        let reason = if path_targets.contains(file.path) && !shared {
            Some(Ambiguity::PathAttribute)
        } else if role == FileRole::Unrooted {
            Some(Ambiguity::Unrooted)
        } else {
            None
        };
        names.modules.push(Name {
            qpath: module_path(&label, &role),
            ambiguity: reason,
        });
        let items = file
            .analysis
            .items
            .iter()
            .map(|item| {
                let path = qpath::qpath(&label, &role, item).to_string();
                *occurrences.entry(path.clone()).or_insert(0) += 1;
                Name {
                    qpath: path,
                    ambiguity: reason,
                }
            })
            .collect();
        names.items.push(items);
    }
    for name in names.items.iter_mut().flatten() {
        if name.ambiguity.is_none() && occurrences.get(&name.qpath).copied().unwrap_or(0) > 1 {
            name.ambiguity = Some(Ambiguity::Duplicate);
        }
    }
    names
}

/// `<package>[::<unit>][::<module>…]` of a file's role.
fn module_path(label: &str, role: &FileRole) -> String {
    let (unit, module): (_, &[String]) = match role {
        FileRole::CrateRoot(unit) => (unit.as_ref(), &[]),
        FileRole::Module(unit, path) => (unit.as_ref(), path),
        FileRole::Unrooted => (None, &[]),
    };
    let mut path = label.to_owned();
    if let Some(unit) = unit {
        path.push_str("::");
        path.push_str(&unit.to_string());
    }
    for segment in module {
        path.push_str("::");
        path.push_str(segment);
    }
    path
}
