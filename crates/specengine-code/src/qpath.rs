//! Heuristic module paths (`qpath`) from a file's place among its package's
//! Cargo targets (`docs/canon/code-identity.md`).
//!
//! Layer A of 05 §5.1: the caller hands over a [`PackageTargets`] table —
//! from `cargo metadata --no-deps` ([`TargetSource::Metadata`]) or, when that
//! is unavailable, from Cargo's auto-discovery ([`layout_targets`]) — and
//! [`file_role`] puts every file in one *unit*: the target whose crate root
//! it is or whose own directory holds it, a shared unit for a directory of
//! a target dir that several crates `mod`-include (`shared:tests/common`),
//! the primary target for the rest of `src/`, else nothing (`Unrooted`).
//! The `qpath` is `<package>::<unit>::<module…>::<owner>::<name>`; the unit is
//! omitted for the primary target (the lib, else the default bin), else
//! `<kind>:<name>` (`bin:tool`) or `shared:<dir>`. A unit always holds a `:`
//! and a module segment never does, so module paths stay suffixes.
//!
//! Whatever the heuristic cannot decide is reported as an [`Ambiguity`],
//! never guessed. Link identity comes from markers, not from this path, so a
//! reported ambiguity costs nothing but honesty (ADR-0020). `mod x;` is not
//! followed (layer C): a module declared by a non-primary root inside `src/`
//! is named a module of the primary target, a stray `src/` file a module.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::items::ItemRecord;

/// Directories whose files are crate roots of their own by Cargo's
/// auto-discovery, with the target kind they hold.
const TARGET_DIRS: [(&[&str], &str); 4] = [
    (&["src", "bin"], KIND_BIN),
    (&["examples"], KIND_EXAMPLE),
    (&["tests"], KIND_TEST),
    (&["benches"], KIND_BENCH),
];

const KIND_LIB: &str = "lib";
const KIND_BIN: &str = "bin";
const KIND_EXAMPLE: &str = "example";
const KIND_TEST: &str = "test";
const KIND_BENCH: &str = "bench";
const KIND_BUILD: &str = "custom-build";
/// Cargo's name of every build-script target.
const BUILD_SCRIPT_NAME: &str = "build-script-build";

/// Target kinds that are not a library: any other kind (`lib`, `rlib`,
/// `proc-macro`, `cdylib`, …) is the package's library.
const NON_LIBRARY_KINDS: [&str; 5] = [KIND_BIN, KIND_EXAMPLE, KIND_TEST, KIND_BENCH, KIND_BUILD];

/// Where a package's target table came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetSource {
    /// `cargo metadata --no-deps`, run by the caller.
    Metadata,
    /// [`layout_targets`]: Cargo's auto-discovery over the package's files.
    Layout,
}

impl TargetSource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Layout => "layout",
        }
    }
}

/// One Cargo target of a package.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Target {
    /// Cargo's first kind: a library kind (`lib`, `rlib`, `proc-macro`, …),
    /// `bin`, `example`, `test`, `bench` or `custom-build`.
    pub kind: String,
    /// Cargo's target name (`build-script-build` for a build script).
    pub name: String,
    /// The crate root, relative to the package directory.
    pub root: PathBuf,
}

/// The targets of one package, as the caller found them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTargets {
    pub source: TargetSource,
    pub targets: Vec<Target>,
}

/// What a non-primary unit is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnitKind {
    /// One Cargo target of this kind (`bin`, `example`, `test`, `bench`,
    /// `custom-build`).
    Target(String),
    /// Files several crates may include: a directory of a target dir holding
    /// no crate root (`tests/common`), one holding several, or a file that is
    /// the root of several targets.
    Shared,
}

/// The unit of a `qpath` other than the primary target; rendered
/// `<kind>:<name>` or `shared:<dir>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Unit {
    pub kind: UnitKind,
    /// The target name, or the shared directory (file) relative to the
    /// package directory, `/`-separated.
    pub name: String,
}

impl Unit {
    /// The Cargo kind, or `shared`.
    #[must_use]
    pub fn kind_str(&self) -> &str {
        match &self.kind {
            UnitKind::Target(kind) => kind,
            UnitKind::Shared => "shared",
        }
    }

    fn target(target: &Target) -> Self {
        Self {
            kind: UnitKind::Target(target.kind.clone()),
            name: target.name.clone(),
        }
    }

    fn shared(parts: &[&str]) -> Self {
        Self {
            kind: UnitKind::Shared,
            name: parts.join("/"),
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind_str(), self.name)
    }
}

/// What a file is inside its package. The unit is `None` for the primary
/// target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRole {
    /// The crate root of the unit's target (`src/lib.rs`, `build.rs`,
    /// `tests/smoke.rs`, a `[[bin]] path`, …); of several targets: a shared
    /// unit named by the file.
    CrateRoot(Option<Unit>),
    /// A module file of the unit: `src/a/b.rs` → `a::b`, `src/a/mod.rs` → `a`,
    /// `tests/common/mod.rs` → `[]` of `shared:tests/common`.
    Module(Option<Unit>, Vec<String>),
    /// In no target and no shared directory; its module path is unknown.
    Unrooted,
}

impl FileRole {
    /// The unit; `None` for the primary target and for `Unrooted`.
    #[must_use]
    pub fn unit(&self) -> Option<&Unit> {
        match self {
            Self::CrateRoot(unit) | Self::Module(unit, _) => unit.as_ref(),
            Self::Unrooted => None,
        }
    }
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

/// `package::unit::module::owner::name`; `package` is the package directory
/// relative to the corpus root (`.` for the root package), never an absolute
/// path; `unit` is `None` for the primary target.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QPath {
    pub package: String,
    pub unit: Option<Unit>,
    pub module: Vec<String>,
    pub owner: Option<String>,
    pub name: String,
}

impl fmt::Display for QPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.package)?;
        if let Some(unit) = &self.unit {
            write!(f, "::{unit}")?;
        }
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
/// `<Type as Trait>::method`
/// (`docs/canon/code-identity.md` "Units and `qpath`"). Two adjacent inherent
/// impls of one type do collide, and that is reported as
/// [`Ambiguity::Duplicate`].
#[must_use]
pub fn qpath(package: &str, role: &FileRole, item: &ItemRecord) -> QPath {
    let (unit, mut module) = match role {
        FileRole::CrateRoot(unit) => (unit.clone(), Vec::new()),
        FileRole::Module(unit, path) => (unit.clone(), path.clone()),
        FileRole::Unrooted => (None, Vec::new()),
    };
    module.extend(item.mod_path.iter().cloned());
    let name = if item.kind == "impl_item" {
        format!("impl {}", item.label)
    } else {
        item.label.clone()
    };
    QPath {
        package: package.to_owned(),
        unit,
        module,
        owner: item.owner.clone(),
        name,
    }
}

/// Classifies a file by its path relative to the package directory against
/// the package's targets. Target dirs `<d>`: `src/bin`, `examples`, `tests`,
/// `benches`; the first rule that matches decides:
///
/// 1. the root of one target → that target's unit, module `[]`; of several →
///    `shared:<file>`;
/// 2. under `<d>/<n>/` holding a target root → that target (two or more →
///    `shared:<d>/<n>`), modules below `<n>/`;
/// 3. under `<d>/<x>/` holding none → `shared:<d>/<x>`, modules below `<x>/`;
/// 4. any other file of `<d>` → `Unrooted`; a package with no target at all
///    (a virtual workspace root, a target-less layout) makes every file of
///    `<d>` `Unrooted` (no rule 2 or 3);
/// 5. under `src/` → the primary target, modules below `src/`; without a
///    primary target → `Unrooted`;
/// 6. else `Unrooted`.
///
/// Primary: the library, else the bin rooted at `src/main.rs`, else the sole
/// bin. Modules: `a/b.rs` → `a::b`, `a/mod.rs` → `a`. Builds a
/// [`TargetIndex`] per call; over many files of one package build it once.
#[must_use]
pub fn file_role(relative: &Path, package: &PackageTargets) -> FileRole {
    TargetIndex::new(package).file_role(relative)
}

/// A [`PackageTargets`] prepared for [`file_role`] over many files: every
/// target root split into components and the primary target chosen once.
#[derive(Debug, Clone)]
pub struct TargetIndex<'a> {
    targets: &'a [Target],
    roots: Vec<Option<Vec<&'a str>>>,
    primary: Option<usize>,
}

impl<'a> TargetIndex<'a> {
    #[must_use]
    pub fn new(package: &'a PackageTargets) -> Self {
        let targets = package.targets.as_slice();
        let roots: Vec<Option<Vec<&'a str>>> = targets.iter().map(|t| parts_of(&t.root)).collect();
        let primary = primary_target(targets, &roots);
        Self {
            targets,
            roots,
            primary,
        }
    }

    /// [`file_role`] of one file of this package.
    #[must_use]
    pub fn file_role(&self, relative: &Path) -> FileRole {
        let Some(parts) = parts_of(relative) else {
            return FileRole::Unrooted;
        };
        let roots = &self.roots;

        // 1. A crate root.
        let mut rooted =
            (0..self.targets.len()).filter(|i| roots[*i].as_deref() == Some(&parts[..]));
        match (rooted.next(), rooted.next()) {
            (Some(one), None) => return FileRole::CrateRoot(self.unit_of(one)),
            (Some(_), Some(_)) => return FileRole::CrateRoot(Some(Unit::shared(&parts))),
            (None, _) => {}
        }

        // 2–4. A target dir.
        for (dir, _) in TARGET_DIRS {
            let Some(rest) = parts.strip_prefix(dir) else {
                continue;
            };
            if self.targets.is_empty() {
                return FileRole::Unrooted;
            }
            let [_, below @ ..] = rest else {
                return FileRole::Unrooted;
            };
            if below.is_empty() {
                return FileRole::Unrooted;
            }
            let Some(module) = module_of(below) else {
                return FileRole::Unrooted;
            };
            let holder = &parts[..dir.len() + 1];
            let mut holding = (0..self.targets.len()).filter(|i| {
                roots[*i]
                    .as_deref()
                    .is_some_and(|root| root.len() > holder.len() && root.starts_with(holder))
            });
            let unit = match (holding.next(), holding.next()) {
                (Some(one), None) => self.unit_of(one),
                _ => Some(Unit::shared(holder)),
            };
            return FileRole::Module(unit, module);
        }

        // 5. The rest of `src/`.
        if let ["src", below @ ..] = parts.as_slice()
            && !below.is_empty()
            && let Some(index) = self.primary
            && let Some(module) = module_of(below)
        {
            return FileRole::Module(self.unit_of(index), module);
        }
        FileRole::Unrooted
    }

    /// The unit of target `index`; `None` for the primary target.
    fn unit_of(&self, index: usize) -> Option<Unit> {
        (Some(index) != self.primary).then(|| Unit::target(&self.targets[index]))
    }
}

/// The targets Cargo's auto-discovery finds among a package's files
/// (relative to the package directory): `src/lib.rs` → `lib` named
/// `<package>` with `-` → `_`; `src/main.rs` → `bin` `<package>`;
/// `src/bin/<n>.rs`, `src/bin/<n>/main.rs` → `bin` `<n>`; the same under
/// `examples`, `tests`, `benches` → `example`, `test`, `bench`; `build.rs` →
/// `custom-build` `build-script-build`. Sorted: library, bins, examples,
/// tests, benches, build script; by root within a kind.
#[must_use]
pub fn layout_targets<'a>(
    package: &str,
    files: impl IntoIterator<Item = &'a Path>,
) -> PackageTargets {
    let mut library = None;
    let mut build = None;
    let mut default_bin = None;
    let mut by_dir: [Vec<Target>; TARGET_DIRS.len()] = Default::default();
    for file in files {
        let Some(parts) = parts_of(file) else {
            continue;
        };
        let target = |kind: &str, name: &str| Target {
            kind: kind.to_owned(),
            name: name.to_owned(),
            root: file.to_path_buf(),
        };
        match parts.as_slice() {
            ["src", "lib.rs"] => library = Some(target(KIND_LIB, &package.replace('-', "_"))),
            ["src", "main.rs"] => default_bin = Some(target(KIND_BIN, package)),
            ["build.rs"] => build = Some(target(KIND_BUILD, BUILD_SCRIPT_NAME)),
            _ => {
                for (slot, (dir, kind)) in by_dir.iter_mut().zip(TARGET_DIRS) {
                    let name = match parts.strip_prefix(dir) {
                        Some([file]) => file.strip_suffix(".rs"),
                        Some([name, "main.rs"]) => Some(*name),
                        _ => None,
                    };
                    if let Some(name) = name.filter(|name| !name.is_empty()) {
                        slot.push(target(kind, name));
                    }
                }
            }
        }
    }
    let mut targets: Vec<Target> = library.into_iter().chain(default_bin).collect();
    for mut group in by_dir {
        group.sort_by(|a, b| a.root.cmp(&b.root));
        targets.extend(group);
    }
    targets.extend(build);
    PackageTargets {
        source: TargetSource::Layout,
        targets,
    }
}

/// The library, else the bin rooted at `src/main.rs`, else the sole bin.
fn primary_target(targets: &[Target], roots: &[Option<Vec<&str>>]) -> Option<usize> {
    if let Some(library) = targets
        .iter()
        .position(|t| !NON_LIBRARY_KINDS.contains(&t.kind.as_str()))
    {
        return Some(library);
    }
    let mut bins = (0..targets.len()).filter(|i| targets[*i].kind == KIND_BIN);
    if let Some(default) = bins
        .clone()
        .find(|i| roots[*i].as_deref() == Some(&["src", "main.rs"][..]))
    {
        return Some(default);
    }
    match (bins.next(), bins.next()) {
        (Some(sole), None) => Some(sole),
        _ => None,
    }
}

/// The normal components of a relative path; `None` when one is not UTF-8
/// or not a plain name (`..`, a root).
fn parts_of(path: &Path) -> Option<Vec<&str>> {
    path.components()
        .filter(|c| !matches!(c, Component::CurDir))
        .map(|c| match c {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect()
}

/// `a/b.rs` → `a::b`, `a/mod.rs` → `a`, `mod.rs` → `[]`; `None` when the
/// file is not a `.rs` file.
fn module_of(parts: &[&str]) -> Option<Vec<String>> {
    let (last, dirs) = parts.split_last()?;
    let mut module: Vec<String> = dirs.iter().map(|d| (*d).to_owned()).collect();
    if *last != "mod.rs" {
        module.push(last.strip_suffix(".rs")?.to_owned());
    }
    Some(module)
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
