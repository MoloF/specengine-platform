//! A code tree's Rust items and modules named at once
//! (`docs/canon/code-bindings.md` "Rust binding": a bound item gives its
//! `qpath` and its ambiguity, shown, not resolved). The units and paths of
//! `docs/canon/code-identity.md` "Units and `qpath`" over the caller's
//! package tables, then one ambiguity per item, the first that applies —
//! `path_attribute` (its file is the target of a `#[path]` attribute and in
//! no shared unit), `unrooted` (its file is in no unit), `duplicate` (an
//! item of another file renders the same undiscriminated `qpath`).
//!
//! In-file twins
//! (`docs/canon/symbol-names.md` "Discriminator"): two or more
//! items of one file rendering one `qpath` form a group, each member named
//! `<qpath> #<value>` by the first rule
//! giving every member a non-empty value, all distinct — `cfg(<p>)` over
//! its `cfg` chain ([`ItemRecord::cfg`] joined by `,`), else its 1-based
//! ordinal within the group in source order. An inline `mod`'s module
//! takes its item's name; a file module and a RON path are never
//! discriminated.
//!
//! Two halves, [`name_files`] their composition: the per-file half
//! ([`name_file`]: a file's module and items over its package table, twins
//! discriminated, `unrooted` read from its role) needs no other file; the
//! tree-wide passes ([`tree_passes`]: `path_attribute` over
//! [`path_targets`], `duplicate` across files) read every file.
//!
//! A file's module is `<package>[::<unit>][::<module>…]`: the crate root
//! alone, the unit omitted for the primary target, as the items' paths
//! begin; an unrooted file's module is its package alone. It is ambiguous
//! by the first two reasons only (a module has no twin by path).
//!
//! The package is the package directory relative to the root, `.` for the
//! root and for a file outside every package.
//!
//! A symbol lookup's query is read here too ([`query_name`]): the one
//! reader of `::` in a query, so the store and the CLI compare strings
//! only (`docs/canon/symbol-reads.md` "Lookup"; Q27).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::items::{FileAnalysis, ItemRecord};
use crate::qpath::{self, Ambiguity, FileRole, PackageTargets, QPath, TargetIndex, UnitKind};

/// The package label of the root package and of a file outside every
/// package.
pub const ROOT_PACKAGE: &str = ".";

/// What separates a twin's `qpath` from its discriminator.
pub const DISCRIMINATOR_SEPARATOR: &str = " #";

/// Rule 1's wrapping of a twin's `cfg` chain: `cfg(<p>)`.
const CFG_OPEN: &str = "cfg(";
const CFG_CLOSE: &str = ")";

/// What joins the predicates of a `cfg` chain.
const CFG_JOIN: &str = ",";

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
    /// Rendered; an in-file twin's ends with ` #<discriminator>`.
    pub qpath: String,
    pub ambiguity: Option<Ambiguity>,
    /// An in-file twin's discriminator, the value without ` #`; `None` for
    /// every other item and for every module.
    pub discriminator: Option<String>,
}

impl Name {
    /// The `qpath` without its discriminator: what twins across files are
    /// compared by.
    #[must_use]
    pub fn undiscriminated(&self) -> &str {
        self.discriminator
            .as_deref()
            .and_then(|value| {
                self.qpath
                    .strip_suffix(value)?
                    .strip_suffix(DISCRIMINATOR_SEPARATOR)
            })
            .unwrap_or(&self.qpath)
    }
}

/// The names of a tree's files, in the order given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names {
    /// Per file: its module.
    pub modules: Vec<Name>,
    /// Per file, per item of its [`FileAnalysis::items`].
    pub items: Vec<Vec<Name>>,
}

/// One file named alone ([`name_file`]): no tree-wide ambiguity yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileNames {
    /// Its module; ambiguous by `unrooted` only.
    pub module: Name,
    /// Per item of its [`FileAnalysis::items`], in-file twins
    /// discriminated; ambiguous by `unrooted` only.
    pub items: Vec<Name>,
    /// Its unit is shared: named by location however it is included, so
    /// never `path_attribute`.
    pub shared: bool,
}

/// Names every file's module and items; `tables` maps a package
/// directory (relative to the root) to its targets. The composition of
/// the per-file half ([`name_file`], each package's table indexed once)
/// and the tree-wide passes ([`tree_passes`]).
#[must_use]
pub fn name_files(files: &[NamedFile<'_>], tables: &BTreeMap<PathBuf, PackageTargets>) -> Names {
    let indexes: BTreeMap<&Path, TargetIndex<'_>> = tables
        .iter()
        .map(|(dir, table)| (dir.as_path(), TargetIndex::new(table)))
        .collect();
    let named: Vec<FileNames> = files
        .iter()
        .map(|file| {
            let index = file.package.and_then(|package| indexes.get(package));
            name_one(file, index)
        })
        .collect();
    tree_passes(files, named)
}

/// The per-file half of [`name_files`]: `file`'s module and items over its
/// package's table in `tables` (none: the file is unrooted), in-file twins
/// discriminated, of the ambiguities only `unrooted` (read from its role).
/// Reads no other file: a pure function of the file's analysis, its path
/// and its package's table.
#[must_use]
pub fn name_file(file: &NamedFile<'_>, tables: &BTreeMap<PathBuf, PackageTargets>) -> FileNames {
    let table = file.package.and_then(|package| tables.get(package));
    let index = table.map(TargetIndex::new);
    name_one(file, index.as_ref())
}

/// The tree-wide passes of [`name_files`] over the per-file halves
/// `named` of `files` (in the same order, one half per file): `path_attribute`
/// on every file a `#[path]` attribute of the tree names ([`path_targets`])
/// outside a shared unit, outranking `unrooted`; then `duplicate` on every
/// item with no other ambiguity whose undiscriminated `qpath` an item of
/// another file renders too. The caller gives exactly one half per file;
/// a debug build asserts it (a release build pairs the two lists only as
/// far as both reach for `path_attribute`).
#[must_use]
pub fn tree_passes(files: &[NamedFile<'_>], mut named: Vec<FileNames>) -> Names {
    debug_assert_eq!(files.len(), named.len(), "one per-file half per file");
    let targets = path_targets(files);
    mark_path_attributes(files, &mut named, &targets);
    mark_duplicates(&mut named);
    let mut names = Names::default();
    for file in named {
        names.modules.push(file.module);
        names.items.push(file.items);
    }
    names
}

/// [`name_file`] over the package's [`TargetIndex`], prepared once for all
/// its files (`None`: the file is unrooted): the per-file half without a
/// table indexed per call.
#[must_use]
pub fn indexed_name_file(file: &NamedFile<'_>, index: Option<&TargetIndex<'_>>) -> FileNames {
    name_one(file, index)
}

/// Where [`name_file`] places `file`: its package label (the package
/// directory, [`ROOT_PACKAGE`] for the root and outside every package) and
/// its role over its package's prepared [`TargetIndex`] (none: unrooted).
/// [`qpath::qpath`] over these two gives an item's structured path, the one
/// its name renders.
#[must_use]
pub fn file_place(file: &NamedFile<'_>, index: Option<&TargetIndex<'_>>) -> (String, FileRole) {
    let label = file
        .package
        .map(package_label)
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| ROOT_PACKAGE.to_owned());
    let role = file
        .package
        .zip(index)
        .and_then(|(package, index)| {
            let inner = file.path.strip_prefix(package).ok()?;
            Some(index.file_role(inner))
        })
        .unwrap_or(FileRole::Unrooted);
    (label, role)
}

/// A package directory as a qpath's label: `/`-separated. Only where `\`
/// separates components is it rewritten (as the CLI's `dir_text`): on Unix
/// it is a character of a name, so a directory holding one keeps one
/// spelling.
fn package_label(package: &Path) -> String {
    let text = package.to_string_lossy();
    if std::path::MAIN_SEPARATOR == '\\' {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    }
}

/// One file named alone ([`indexed_name_file`]) with what its names were
/// read from: its place ([`file_place`]) and each item's structured path
/// ([`qpath::qpath`]), each read once — for a caller that stores an item's
/// suffixes ([`QPath::suffixes`]) beside its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedNames {
    pub names: FileNames,
    /// [`file_place`]'s package label.
    pub label: String,
    /// [`file_place`]'s role.
    pub role: FileRole,
    /// Per item of its [`FileAnalysis::items`], the structured path its
    /// name renders (without a twin's discriminator).
    pub paths: Vec<QPath>,
}

/// [`indexed_name_file`] with the file's place and each item's
/// structured path ([`PlacedNames`]): the same names, nothing read twice.
#[must_use]
pub fn placed_name_file(file: &NamedFile<'_>, index: Option<&TargetIndex<'_>>) -> PlacedNames {
    let (label, role) = file_place(file, index);
    // A shared unit is named by its location, however it is included.
    let shared = role
        .unit()
        .is_some_and(|unit| unit.kind == UnitKind::Shared);
    let reason = (role == FileRole::Unrooted).then_some(Ambiguity::Unrooted);
    let items = &file.analysis.items;
    let structured: Vec<QPath> = items
        .iter()
        .map(|item| qpath::qpath(&label, &role, item))
        .collect();
    let paths: Vec<String> = structured.iter().map(ToString::to_string).collect();
    let discriminators = discriminate(items, &paths);
    let items = paths
        .into_iter()
        .zip(discriminators)
        .map(|(path, discriminator)| Name {
            qpath: match &discriminator {
                Some(value) => format!("{path}{DISCRIMINATOR_SEPARATOR}{value}"),
                None => path,
            },
            ambiguity: reason,
            discriminator,
        })
        .collect();
    let names = FileNames {
        module: Name {
            qpath: module_path(&label, &role),
            ambiguity: reason,
            discriminator: None,
        },
        items,
        shared,
    };
    PlacedNames {
        names,
        label,
        role,
        paths: structured,
    }
}

/// [`name_file`] over the package's prepared target index.
fn name_one(file: &NamedFile<'_>, index: Option<&TargetIndex<'_>>) -> FileNames {
    placed_name_file(file, index).names
}

/// What separates two segments of a rendered `qpath`.
const SEGMENT_SEPARATOR: &[u8] = b"::";

/// How an `impl` block's segment starts: it runs to the end of a query
/// (its type may hold `::`).
const IMPL_SEGMENT: &[u8] = b"impl ";

/// The `name` tier's key of a symbol lookup query
/// (`docs/canon/symbol-reads.md` "Lookup"; Q27): the text after the last
/// `::` outside `<>`, `()` and `[]`, a segment starting `impl ` running to
/// the end, a trailing ` #<discriminator>` dropped — what a stored item's
/// `name` holds. The one reader of `::` in a query: the store and the CLI
/// compare strings only. `<T as a::Tr>` and a `cfg(…)` value are read
/// whole; the `>` of a `->` closes nothing; an unbalanced closer counts as
/// none. `crates/x::twins::impl Sheet #2` gives `impl Sheet`,
/// `run #cfg(feature = "a::b")` gives `run`.
#[must_use]
pub fn query_name(query: &str) -> &str {
    let bytes = query.as_bytes();
    let mut depth = 0usize;
    let mut start = 0;
    let mut at = 0;
    while at < bytes.len() {
        if at == start && bytes[at..].starts_with(IMPL_SEGMENT) {
            break;
        }
        if depth == 0 && bytes[at..].starts_with(SEGMENT_SEPARATOR) {
            at += SEGMENT_SEPARATOR.len();
            start = at;
            continue;
        }
        depth = bracket_depth(bytes, at, depth);
        at += 1;
    }
    without_discriminator(&query[start..])
}

/// The bracket depth after the byte at `at` of `bytes` (the depth before
/// it given): `<`, `(`, `[` open; `>`, `)`, `]` close, but the `>` of `->`.
fn bracket_depth(bytes: &[u8], at: usize, depth: usize) -> usize {
    match bytes[at] {
        b'<' | b'(' | b'[' => depth + 1,
        b'>' if at > 0 && bytes[at - 1] == b'-' => depth,
        b'>' | b')' | b']' => depth.saturating_sub(1),
        _ => depth,
    }
}

/// `segment` up to its first ` #` outside brackets: a twin's
/// discriminator dropped (a `cfg` value may hold ` #` itself).
fn without_discriminator(segment: &str) -> &str {
    let bytes = segment.as_bytes();
    let separator = DISCRIMINATOR_SEPARATOR.as_bytes();
    let mut depth = 0usize;
    for at in 0..bytes.len() {
        if depth == 0 && bytes[at..].starts_with(separator) {
            return &segment[..at];
        }
        depth = bracket_depth(bytes, at, depth);
    }
    segment
}

/// The files a `#[path]` attribute of a `mod` declaration of `files`
/// names, resolved against the declaring file (the first tree-wide pass;
/// 6a-2's store keeps it as `declared_by`).
#[must_use]
pub fn path_targets(files: &[NamedFile<'_>]) -> BTreeSet<PathBuf> {
    let mut targets = BTreeSet::new();
    for file in files {
        for declaration in &file.analysis.mod_declarations {
            if let Some(value) = &declaration.path_attribute {
                targets.insert(qpath::resolve_path_attribute(file.path, value));
            }
        }
    }
    targets
}

/// `path_attribute` on the module and items of every file of `files`
/// (`named` its [`name_file`] halves, in the same order) that `targets`
/// holds and whose unit is not shared; it outranks `unrooted`.
fn mark_path_attributes(
    files: &[NamedFile<'_>],
    named: &mut [FileNames],
    targets: &BTreeSet<PathBuf>,
) {
    for (file, names) in files.iter().zip(named.iter_mut()) {
        if names.shared || !targets.contains(file.path) {
            continue;
        }
        names.module.ambiguity = Some(Ambiguity::PathAttribute);
        for name in &mut names.items {
            name.ambiguity = Some(Ambiguity::PathAttribute);
        }
    }
}

/// `duplicate` on every item with no other ambiguity whose undiscriminated
/// `qpath` an item of another file renders too (the last tree-wide pass):
/// in-file twins alone are named apart, never `duplicate` (6a-2's store
/// keeps it as `twin_files`).
fn mark_duplicates(named: &mut [FileNames]) {
    // Undiscriminated qpath → the first file rendering it, and whether
    // another one does.
    let mut files_of: HashMap<String, (usize, bool)> = HashMap::new();
    for (index, file) in named.iter().enumerate() {
        for name in &file.items {
            files_of
                .entry(name.undiscriminated().to_owned())
                .and_modify(|(first, more)| *more |= *first != index)
                .or_insert((index, false));
        }
    }
    for name in named.iter_mut().flat_map(|file| file.items.iter_mut()) {
        if name.ambiguity.is_none()
            && files_of
                .get(name.undiscriminated())
                .is_some_and(|(_, more)| *more)
        {
            name.ambiguity = Some(Ambiguity::Duplicate);
        }
    }
}

/// Each item's discriminator: `None` outside a group of in-file twins
/// (`paths`, the items' undiscriminated `qpath`s, equal); in a group the
/// first rule giving every member a non-empty value, all distinct — rule
/// 1 `cfg(<p>)` over its `cfg` chain, rule 2 its 1-based ordinal within
/// the group in source order (the order of `items`).
fn discriminate(items: &[ItemRecord], paths: &[String]) -> Vec<Option<String>> {
    let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, path) in paths.iter().enumerate() {
        groups.entry(path.as_str()).or_default().push(index);
    }
    let mut discriminators = vec![None; paths.len()];
    for members in groups.values().filter(|members| members.len() > 1) {
        let by_cfg: Vec<Option<String>> = members
            .iter()
            .map(|&index| {
                let chain = &items.get(index)?.cfg;
                (!chain.is_empty())
                    .then(|| format!("{CFG_OPEN}{}{CFG_CLOSE}", chain.join(CFG_JOIN)))
            })
            .collect();
        let distinct: BTreeSet<&str> = by_cfg.iter().flatten().map(String::as_str).collect();
        let values: Vec<String> = if distinct.len() == members.len() {
            by_cfg.into_iter().flatten().collect()
        } else {
            (1..=members.len())
                .map(|ordinal| ordinal.to_string())
                .collect()
        };
        for (&index, value) in members.iter().zip(values) {
            discriminators[index] = Some(value);
        }
    }
    discriminators
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
