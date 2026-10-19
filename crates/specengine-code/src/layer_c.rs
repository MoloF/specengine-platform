//! Layer C's words as data (`docs/features/layer-c.md` "Kind map";
//! `docs/features/layer-c-resolve.md` "Freshness", "Join"): which layer A
//! node kinds ([`crate::items::ITEM_KINDS`], [`crate::items::MEMBER_KINDS`])
//! an item kind of `specengine-ra` (its `ItemKind::as_str`, protocol v1's
//! `kind`) stands for, and the pure functions `spec resolve` and the reads
//! of its snapshot call:
//!
//! - **Freshness** ([`tree_files`]): the names of the files besides the
//!   code whose bytes a snapshot's `tree_fp` covers, in every directory
//!   from a walked file's up to the root; the caller hashes the bytes.
//! - **Join** ([`join_file`]): a layer A row and a layer C row of one file
//!   join on equal `(start, end)` where [`kind_matches`] holds, never on a
//!   line or a name.
//! - **Agreement** ([`target_crate`], [`agrees`]): a moniker's crate and
//!   its leading module descriptors against layer A's target crate and
//!   modules; names and owners are not compared.
//! - **Collisions** ([`collision_values`]): a raw moniker several items of
//!   one file share, told apart for display by [`COLLISION_SEPARATOR`].
//!
//! The words are data of this crate and `specengine-ra` only: the Cargo
//! file names, the descriptor syntax, the kind map (the genre rule,
//! `docs/canon/architecture.md#universal`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::qpath::{FileRole, PackageTargets, TargetIndex, UnitKind};

/// Every layer C item kind with the layer A kinds it stands for, in
/// `specengine-ra`'s `ItemKind::ALL` order: `method` stands for a member
/// with a body or without one, `type_alias`, `mod` and both macro kinds are
/// named apart, every other kind `<kind>_item`.
pub const KIND_MAP: [(&str, &[&str]); 13] = [
    ("function", &["function_item"]),
    ("method", &["function_item", "function_signature_item"]),
    ("struct", &["struct_item"]),
    ("enum", &["enum_item"]),
    ("union", &["union_item"]),
    ("trait", &["trait_item"]),
    ("impl", &["impl_item"]),
    ("const", &["const_item"]),
    ("static", &["static_item"]),
    ("type_alias", &["type_item"]),
    ("mod", &["mod_item"]),
    ("macro_rules", &["macro_definition"]),
    ("macro_def", &["macro_definition"]),
];

/// The layer A kinds a layer C kind stands for: its [`KIND_MAP`] row, or
/// `None` for a word the map does not hold (see [`kind_matches`] for the
/// rule such a word follows).
pub fn layer_a_kinds(layer_c_kind: &str) -> Option<&'static [&'static str]> {
    KIND_MAP
        .iter()
        .find(|(kind, _)| *kind == layer_c_kind)
        .map(|(_, kinds)| *kinds)
}

/// Whether layer A's node kind `layer_a_kind` is one the layer C item kind
/// `layer_c_kind` stands for: a [`KIND_MAP`] row, else `<kind>_item`.
pub fn kind_matches(layer_c_kind: &str, layer_a_kind: &str) -> bool {
    match layer_a_kinds(layer_c_kind) {
        Some(kinds) => kinds.contains(&layer_a_kind),
        None => layer_a_kind.strip_suffix("_item") == Some(layer_c_kind),
    }
}

/// The files besides the code whose bytes a layer C answer depends on:
/// the lock (a renamed or re-resolved dependency), the manifests (a
/// workspace root's `members`, `[patch]` or `[workspace.dependencies]`,
/// virtual or not), the toolchain pins and cargo's configuration, by name
/// relative to a directory.
pub const TREE_FILES: [&str; 6] = [
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    ".cargo/config",
    ".cargo/config.toml",
];

/// What separates the components of a root-relative path as the code index
/// stores it.
const SEPARATOR: char = '/';

/// The root-relative paths (`/`-separated) whose bytes `tree_fp` hashes:
/// each [`TREE_FILES`] name in each directory from a walked file's
/// directory up to the root, the root included and never above it
/// (`walked`: the walk's root-relative file paths; one with a `..`, `.` or
/// empty component, or an absolute one, adds the root only); byte-sorted,
/// unique. Whether each exists is the caller's to read.
#[must_use]
pub fn tree_files<'a>(walked: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    dirs.insert(String::new());
    for path in walked {
        let parts: Vec<&str> = path.split(SEPARATOR).collect();
        if parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
        {
            continue;
        }
        // The last component is the file's own name.
        for depth in 1..parts.len() {
            dirs.insert(parts[..depth].join("/"));
        }
    }
    let mut files: BTreeSet<String> = BTreeSet::new();
    for dir in &dirs {
        for name in TREE_FILES {
            files.insert(if dir.is_empty() {
                name.to_owned()
            } else {
                format!("{dir}{SEPARATOR}{name}")
            });
        }
    }
    files.into_iter().collect()
}

/// One item of a file as the join reads it: its byte range on the raw
/// text and its kind (layer A's node kind, or layer C's item kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinItem<'a> {
    pub start: u64,
    pub end: u64,
    pub kind: &'a str,
}

/// For each layer A item of one file (`a`), the index in `c` of the layer
/// C item it joins: equal `start` and `end`, and [`kind_matches`] (C's
/// kind, A's kind); `None` when no such item exists. One C item per
/// `start` (protocol v1's order); a C item no A item joins is counted by
/// the caller (`c_only`).
#[must_use]
pub fn join_file(a: &[JoinItem<'_>], c: &[JoinItem<'_>]) -> Vec<Option<usize>> {
    let by_start: BTreeMap<u64, usize> = c
        .iter()
        .enumerate()
        .map(|(index, item)| (item.start, index))
        .collect();
    a.iter()
        .map(|item| {
            by_start.get(&item.start).copied().filter(|&index| {
                let candidate = &c[index];
                candidate.end == item.end && kind_matches(candidate.kind, item.kind)
            })
        })
        .collect()
}

/// What a file's one target is to `agrees`: its crate's name (Cargo's
/// target name with `-` -> `_`) and the file's module path in that crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetCrate {
    pub name: String,
    pub modules: Vec<String>,
}

/// The crate of the one target a file (`relative` to its package
/// directory) belongs to by layer A's units ([`TargetIndex::file_role`]
/// over the package's `targets`), and its module path there; `None` when
/// the file has no one target: a shared unit, unrooted, or no primary
/// target for a file that would be the primary's.
#[must_use]
pub fn target_crate(relative: &Path, targets: &PackageTargets) -> Option<TargetCrate> {
    let index = TargetIndex::new(targets);
    let role = index.file_role(relative);
    let name = match role.unit() {
        Some(unit) if unit.kind == UnitKind::Shared => return None,
        Some(unit) => unit.name.clone(),
        None if role == FileRole::Unrooted => return None,
        None => index.primary()?.name.clone(),
    };
    let modules = match role {
        FileRole::Module(_, modules) => modules,
        FileRole::CrateRoot(_) | FileRole::Unrooted => Vec::new(),
    };
    Some(TargetCrate {
        name: name.replace('-', "_"),
        modules,
    })
}

/// What separates a moniker's crate from its descriptors.
const CRATE_END: char = ' ';

/// A namespace descriptor's suffix (`a/`).
const NAMESPACE: char = '/';

/// Every descriptor suffix or opening other than a namespace's: type `#`,
/// term `.`, method `().` and parameter `(p)`, type parameter `[T]`, macro
/// `!`, meta `:`.
const OTHER_DESCRIPTOR: [char; 6] = ['#', '.', '(', '[', '!', ':'];

/// A moniker's crate: the text before its first space (all of it without
/// one).
#[must_use]
pub fn moniker_crate(moniker: &str) -> &str {
    moniker
        .split_once(CRATE_END)
        .map_or(moniker, |(name, _)| name)
}

/// A moniker's leading namespace descriptors (`a/b/`), before the item's
/// own last descriptor: a module item's own `name/` is not one of its
/// modules (`arena internal/` has none, `arena internal/seed().` has
/// `internal`).
#[must_use]
pub fn moniker_modules(moniker: &str) -> Vec<&str> {
    let mut rest = moniker
        .split_once(CRATE_END)
        .map_or("", |(_, descriptors)| descriptors);
    let mut modules = Vec::new();
    loop {
        let next = rest.find(|c: char| c == NAMESPACE || OTHER_DESCRIPTOR.contains(&c));
        match next {
            Some(at) if rest[at..].starts_with(NAMESPACE) => {
                modules.push(&rest[..at]);
                rest = &rest[at + NAMESPACE.len_utf8()..];
            }
            _ => break,
        }
    }
    // Nothing after the namespaces: the last one is the item's own.
    if rest.is_empty() {
        modules.pop();
    }
    modules
}

/// Whether a `moniker` row agrees with layer A: its crate equals the
/// file's target crate and its leading module descriptors ([`moniker_modules`])
/// equal `modules`, the file's module path and the item's enclosing inline
/// modules; names and owners are not compared.
#[must_use]
pub fn agrees(moniker: &str, crate_name: &str, modules: &[String]) -> bool {
    moniker_crate(moniker) == crate_name
        && moniker_modules(moniker)
            .iter()
            .copied()
            .eq(modules.iter().map(String::as_str))
}

/// What separates a shared moniker from its collision value when shown:
/// a raw moniker never holds it.
pub const COLLISION_SEPARATOR: &str = crate::naming::DISCRIMINATOR_SEPARATOR;

/// For each item of one file, in `start` order, its collision value when
/// its raw moniker is shared by two or more items of the file: the joined
/// layer A rows' discriminators (`discriminators`, same order) when every
/// member has one and all are distinct, else its 1-based ordinal by
/// `start` within the group; `None` for every other item.
#[must_use]
pub fn collision_values(
    monikers: &[Option<&str>],
    discriminators: &[Option<&str>],
) -> Vec<Option<String>> {
    let mut values = vec![None; monikers.len()];
    for members in collision_groups(monikers) {
        let given: Vec<&str> = members
            .iter()
            .filter_map(|&at| discriminators.get(at).copied().flatten())
            .collect();
        let distinct: BTreeSet<&str> = given.iter().copied().collect();
        let by_discriminator = given.len() == members.len() && distinct.len() == members.len();
        for (ordinal, &at) in members.iter().enumerate() {
            values[at] = Some(if by_discriminator {
                given[ordinal].to_owned()
            } else {
                (ordinal + 1).to_string()
            });
        }
    }
    values
}

/// The groups of two or more items of one file sharing a raw moniker, each
/// its members' indices in `start` order, by moniker.
#[must_use]
pub fn collision_groups(monikers: &[Option<&str>]) -> Vec<Vec<usize>> {
    let mut by_moniker: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (at, moniker) in monikers.iter().enumerate() {
        if let Some(moniker) = moniker {
            by_moniker.entry(moniker).or_default().push(at);
        }
    }
    by_moniker
        .into_values()
        .filter(|members| members.len() > 1)
        .collect()
}

/// A moniker as shown: the raw text, then ` #<value>` when it collides.
#[must_use]
pub fn shown_moniker(raw: &str, collision: Option<&str>) -> String {
    match collision {
        Some(value) => format!("{raw}{COLLISION_SEPARATOR}{value}"),
        None => raw.to_owned(),
    }
}
