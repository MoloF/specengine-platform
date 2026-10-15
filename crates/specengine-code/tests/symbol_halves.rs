//! docs/features/symbol-discriminator.md (6a-1) RD-1, Q21 ("For 6a-2"):
//! `naming.rs` ships split, a per-file half (one file's module and items
//! named and discriminated over its package table) and the tree-wide
//! passes (`path_targets`; `duplicate` by undiscriminated qpath across
//! files), `name_files` their composition. Over trees holding every
//! ambiguity and both rules, the halves composed give `name_files`' names
//! exactly, and the per-file half of a file is the same whatever tree it
//! sits in.
//!
//! The spec names the halves' roles, not their items (reading L9); as
//! built: `naming::name_file(&NamedFile, &tables) -> FileNames { module,
//! items, shared }` and `naming::tree_passes(&[NamedFile], Vec<FileNames>)
//! -> Names`, called only by [`per_file`] and [`by_halves`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specengine_code::naming::{FileNames, name_file, tree_passes};
use specengine_code::{
    Ambiguity, FileAnalysis, NamedFile, Names, PackageTargets, RustParser, bind_file,
    layout_targets, name_files,
};

const PACKAGE: &str = "crates/demo";
const PACKAGE_NAME: &str = "demo";
const LIB: &str = "crates/demo/src/lib.rs";

/// Every tree: in-file `cfg` and ordinal twins, a cross-file pair (one side
/// discriminated), a `#[path]` target, an unrooted file with twins.
const TREES: &[&[(&str, &str)]] = &[
    &[
        (LIB, "#[path = \"extra/moved.rs\"]\nmod moved;\n"),
        (
            "crates/demo/src/x.rs",
            "#[cfg(unix)]\npub fn f() {}\n\n#[cfg(windows)]\npub fn f() {}\n\nimpl G {}\n\nimpl G {}\n",
        ),
        (
            "crates/demo/src/x/mod.rs",
            "pub fn f() {}\n\npub fn g() {}\n",
        ),
        (
            "crates/demo/src/extra/moved.rs",
            "pub fn shift() {}\n\npub fn shift() {}\n",
        ),
        (
            "tools/loose.rs",
            "#[cfg(test)]\npub fn seed() {}\n\n#[cfg(not(test))]\npub fn seed() {}\n",
        ),
    ],
    &[
        (LIB, ""),
        (
            "crates/demo/src/os.rs",
            "#[cfg(unix)]\nmod imp {\n    //! Unix.\n    pub fn open() {}\n}\n\n#[cfg(windows)]\nmod imp {\n    pub fn open() {}\n}\n",
        ),
    ],
];

struct Tree {
    paths: Vec<PathBuf>,
    analyses: Vec<FileAnalysis>,
    tables: BTreeMap<PathBuf, PackageTargets>,
}

impl Tree {
    fn of(files: &[(&str, &str)]) -> Self {
        let mut parser = RustParser::new().expect("the pinned grammar loads");
        let package = Path::new(PACKAGE);
        let paths: Vec<PathBuf> = files.iter().map(|(path, _)| PathBuf::from(path)).collect();
        let inner: Vec<PathBuf> = paths
            .iter()
            .filter_map(|path| path.strip_prefix(package).ok().map(Path::to_path_buf))
            .collect();
        let mut tables = BTreeMap::new();
        tables.insert(
            package.to_path_buf(),
            layout_targets(PACKAGE_NAME, inner.iter().map(PathBuf::as_path)),
        );
        let analyses = files
            .iter()
            .map(|(_, source)| {
                bind_file(&mut parser, source)
                    .expect("the parser gives a tree")
                    .analysis
            })
            .collect();
        Self {
            paths,
            analyses,
            tables,
        }
    }

    fn named(&self) -> Vec<NamedFile<'_>> {
        let package = Path::new(PACKAGE);
        self.paths
            .iter()
            .zip(&self.analyses)
            .map(|(path, analysis)| NamedFile {
                path,
                package: path.starts_with(package).then_some(package),
                analysis,
            })
            .collect()
    }
}

/// The per-file half of every file of `tree`.
fn per_file(tree: &Tree) -> Vec<FileNames> {
    tree.named()
        .iter()
        .map(|file| name_file(file, &tree.tables))
        .collect()
}

/// The per-file halves composed by the tree-wide passes.
fn by_halves(tree: &Tree) -> Names {
    tree_passes(&tree.named(), per_file(tree))
}

/// RD-1 (Q21): the halves composed equal `name_files` on every tree. M:
/// the halves diverging from `name_files`.
#[test]
fn name_files_equals_its_halves() {
    for files in TREES {
        let tree = Tree::of(files);
        let whole = name_files(&tree.named(), &tree.tables);
        assert_eq!(by_halves(&tree), whole, "{files:?}");
        assert!(
            whole
                .items
                .iter()
                .flatten()
                .any(|name| name.discriminator.is_some()),
            "the tree holds twins: {files:?}"
        );
    }
}

/// Q21 ("a per-file half … over its package table"): the per-file half
/// gives each file the discriminators `name_files` gives it, and of the
/// ambiguities only `unrooted` (a file's own); `path_attribute` and
/// `duplicate` are the tree-wide passes'. A file's half is the same named
/// in its tree or alone with its package's library root.
#[test]
fn the_per_file_half_reads_one_file() {
    for files in TREES {
        let tree = Tree::of(files);
        let whole = name_files(&tree.named(), &tree.tables);
        let halves = per_file(&tree);
        for (index, half) in halves.iter().enumerate() {
            let wanted: Vec<Option<String>> = whole.items[index]
                .iter()
                .map(|name| name.discriminator.clone())
                .collect();
            let got: Vec<Option<String>> = half
                .items
                .iter()
                .map(|name| name.discriminator.clone())
                .collect();
            assert_eq!(got, wanted, "{}", files[index].0);
            for name in half.items.iter().chain([&half.module]) {
                assert!(
                    matches!(name.ambiguity, None | Some(Ambiguity::Unrooted)),
                    "{}: {name:?}",
                    files[index].0
                );
            }
            let alone = if index == 0 {
                Tree::of(&files[..1])
            } else {
                Tree::of(&[files[0], files[index]])
            };
            let alone_half = per_file(&alone).pop().expect("the file's half");
            assert_eq!(
                alone_half.items, half.items,
                "{}: alone with the library root",
                files[index].0
            );
        }
    }
}
