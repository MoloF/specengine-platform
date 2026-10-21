//! The one record builder and seeder of core's comparer (task spec
//! `impact-since`, canon `spec-impact`): `spec impact --since REV` and
//! `spec verify --changed`'s `delta.symbols` both compare a commit's code
//! tree with the walk on disk through [`code_delta`], then core's
//! [`compare`].
//!
//! - **Compared files**: the `.rs` and `.ron` paths one side alone holds,
//!   or both with other bytes (equal bytes, an equal blob OID: nothing
//!   parsed or hashed); content that is not UTF-8 is skipped on its side,
//!   as the walk skips it.
//! - **Records**, made afresh from each side's bytes, never read from the
//!   code index: a Rust file parsed once per side, each named item a
//!   record in pre-order (its qpath and kind as that side's walk names
//!   them, its start line, its `sig` and `body` digests by `item_levels`,
//!   none when the item cannot be hashed); a RON file one record (`qpath`
//!   its path, `item_kind` `file`, `line` 1, `sig` and `body` the BLAKE3 of
//!   its bytes), so any edit of it counts. A comment, a marker or
//!   formatting alone changes no item's digest.
//! - **Seeds**: an item removed or changed reaches the nodes the old
//!   side's markers bind it to, one added or changed the new side's: a
//!   marker in its file bound to it, to an item enclosing it (an inline
//!   `mod`, an `impl` or a trait) or to the file's module; in a RON file,
//!   every marker anchored to a value. The ID judged against the current
//!   corpus naming nodes itself (resolved, an alias or ambiguous); a
//!   dangling, look-alike, queue or other-project ID, and a marker bound to
//!   nothing or that cannot be verified, reach nothing.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_code::{Binding, LevelState, RustParser, item_levels, markers_in};
use specengine_core::symbol_delta::{SymbolRecord, compare};
use specengine_store::b3_hash;

use crate::code_index::item_parents;
use crate::code_walk::CodeTree;
use crate::lock::Target;
use crate::refs::{Corpus, NodeKey};

/// The item kind of a RON file's one record.
const RON_FILE_KIND: &str = "file";

/// One item of the delta: `line` the new side's, the old side's for a
/// removed one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DeltaItem {
    pub file: String,
    /// The qpath; a RON file's path.
    pub symbol: String,
    pub item_kind: String,
    pub line: usize,
}

/// What the two sides differ by, each list in core's order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CodeDelta {
    pub changed: Vec<DeltaItem>,
    pub added: Vec<DeltaItem>,
    pub removed: Vec<DeltaItem>,
    /// The code files compared (see the module documentation).
    pub files: usize,
}

impl CodeDelta {
    /// Its items in their printed order (changed, added, removed), each
    /// with its word.
    pub(crate) fn items(&self) -> impl Iterator<Item = (&'static str, &DeltaItem)> {
        let word = |word: &'static str| move |item| (word, item);
        self.changed
            .iter()
            .map(word("changed"))
            .chain(self.added.iter().map(word("added")))
            .chain(self.removed.iter().map(word("removed")))
    }

    /// How many items it holds.
    pub(crate) fn len(&self) -> usize {
        self.changed.len() + self.added.len() + self.removed.len()
    }
}

/// `{added, removed, changed, renamed}`: `renamed` is never filled.
#[derive(Serialize)]
pub(crate) struct DeltaJson<'a> {
    added: &'a [DeltaItem],
    removed: &'a [DeltaItem],
    changed: &'a [DeltaItem],
    renamed: Option<()>,
}

impl<'a> DeltaJson<'a> {
    /// The first `shown` items of `delta` in the printed order (changed,
    /// added, removed), each list in its own order; `None`: every item.
    pub(crate) fn printed(delta: &'a CodeDelta, shown: Option<usize>) -> Self {
        let mut left = shown.unwrap_or(usize::MAX);
        let mut take = |items: &'a [DeltaItem]| -> &'a [DeltaItem] {
            let taken = &items[..left.min(items.len())];
            left -= taken.len();
            taken
        };
        let changed = take(&delta.changed);
        let added = take(&delta.added);
        let removed = take(&delta.removed);
        Self {
            added,
            removed,
            changed,
            renamed: None,
        }
    }
}

/// Where a record lies on its side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    /// A Rust file's index in the tree, and the item's in its analysis.
    Item { file: usize, item: usize },
    /// A RON file's index in the tree.
    Ron { file: usize },
}

/// One side's records and, index for index, their units.
#[derive(Default)]
struct Side {
    records: Vec<SymbolRecord>,
    units: Vec<Unit>,
}

/// The delta, and on each side the units its seeds start from.
pub(crate) struct Compared {
    pub delta: CodeDelta,
    /// The old side's units of the removed and changed items.
    old: Vec<Unit>,
    /// The new side's units of the added and changed items.
    new: Vec<Unit>,
}

/// `old` (a commit's code tree; [`CodeTree::empty`] for an unborn one)
/// against `new` (the walk on disk): the files compared, their records on
/// both sides and core's comparison of them.
pub(crate) fn code_delta(old: &CodeTree, new: &CodeTree) -> Compared {
    let before = texts(old);
    let after = texts(new);
    let compared: BTreeSet<&str> = before
        .iter()
        .filter(|(path, text)| after.get(*path) != Some(*text))
        .chain(
            after
                .iter()
                .filter(|(path, text)| before.get(*path) != Some(*text)),
        )
        .map(|(path, _)| *path)
        .collect();
    let mut parser = RustParser::new().ok();
    let base = records(old, &compared, parser.as_mut());
    let disk = records(new, &compared, parser.as_mut());
    let found = compare(&base.records, &disk.records);
    let item = |record: &SymbolRecord| DeltaItem {
        file: record.file.clone(),
        symbol: record.qpath.clone(),
        item_kind: record.item_kind.clone(),
        line: record.line,
    };
    let delta = CodeDelta {
        changed: found
            .changed
            .iter()
            .map(|&(_, at)| item(&disk.records[at]))
            .collect(),
        added: found
            .added
            .iter()
            .map(|&at| item(&disk.records[at]))
            .collect(),
        removed: found
            .removed
            .iter()
            .map(|&at| item(&base.records[at]))
            .collect(),
        files: compared.len(),
    };
    Compared {
        delta,
        old: found
            .removed
            .iter()
            .copied()
            .chain(found.changed.iter().map(|&(at, _)| at))
            .map(|at| base.units[at])
            .collect(),
        new: found
            .added
            .iter()
            .copied()
            .chain(found.changed.iter().map(|&(_, at)| at))
            .map(|at| disk.units[at])
            .collect(),
    }
}

/// Each UTF-8 code file of `tree`, by path, with its text.
fn texts(tree: &CodeTree) -> BTreeMap<&str, &str> {
    tree.rust
        .iter()
        .map(|file| (file.path.as_str(), file.text.as_str()))
        .chain(
            tree.ron
                .iter()
                .map(|file| (file.path.as_str(), file.text.as_str())),
        )
        .collect()
}

/// The records of `tree`'s files at the `compared` paths: each Rust file
/// parsed once (no parser or no tree: every item without a digest), each
/// RON file one record.
fn records(
    tree: &CodeTree,
    compared: &BTreeSet<&str>,
    mut parser: Option<&mut RustParser>,
) -> Side {
    let mut side = Side::default();
    for (index, file) in tree.rust.iter().enumerate() {
        if !compared.contains(file.path.as_str()) {
            continue;
        }
        let names = tree.names.items.get(index).map_or(&[][..], Vec::as_slice);
        let items = &file.binding.analysis.items;
        let every: Vec<usize> = (0..items.len()).collect();
        let levels = match parser
            .as_deref_mut()
            .and_then(|parser| parser.parse(&file.text))
        {
            Some(parsed) => item_levels(&parsed, &file.text, &every),
            None => vec![None; items.len()],
        };
        for (at, item) in items.iter().enumerate() {
            let Some(name) = names.get(at) else {
                continue;
            };
            let hashes = match levels.get(at) {
                Some(Some(LevelState::Hashed(hashes))) => Some(*hashes),
                _ => None,
            };
            side.records.push(SymbolRecord {
                file: file.path.clone(),
                qpath: name.qpath.clone(),
                item_kind: item.kind.clone(),
                line: item.line,
                sig: hashes.map(|hashes| format!("b3:{}", hashes.sig.to_hex())),
                body: hashes.map(|hashes| format!("b3:{}", hashes.body.to_hex())),
            });
            side.units.push(Unit::Item {
                file: index,
                item: at,
            });
        }
    }
    for (index, file) in tree.ron.iter().enumerate() {
        if !compared.contains(file.path.as_str()) {
            continue;
        }
        let digest = b3_hash(file.text.as_bytes());
        side.records.push(SymbolRecord {
            file: file.path.clone(),
            qpath: file.path.clone(),
            item_kind: RON_FILE_KIND.to_owned(),
            line: 1,
            sig: Some(digest.clone()),
            body: Some(digest),
        });
        side.units.push(Unit::Ron { file: index });
    }
    side
}

impl Compared {
    /// The nodes the delta's items reach (see the module documentation):
    /// the removed and changed ones through `old`'s markers, the added and
    /// changed ones through `new`'s, each ID judged by `corpus`.
    pub(crate) fn seeds(
        &self,
        old: &CodeTree,
        new: &CodeTree,
        corpus: &mut Corpus<'_>,
    ) -> BTreeSet<NodeKey> {
        let mut seeds = BTreeSet::new();
        reach(old, &self.old, corpus, &mut seeds);
        reach(new, &self.new, corpus, &mut seeds);
        seeds
    }
}

/// Into `seeds`, the nodes `tree`'s markers bind the `units` to.
fn reach(tree: &CodeTree, units: &[Unit], corpus: &mut Corpus<'_>, seeds: &mut BTreeSet<NodeKey>) {
    let mut items: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut ron: BTreeSet<usize> = BTreeSet::new();
    for unit in units {
        match *unit {
            Unit::Item { file, item } => {
                items.entry(file).or_default().insert(item);
            }
            Unit::Ron { file } => {
                ron.insert(file);
            }
        }
    }
    for (index, wanted) in items {
        let Some(file) = tree.rust.get(index) else {
            continue;
        };
        // Each item's parent, as the code index's `parent_ord` derives it.
        let parents = item_parents(&file.binding.analysis.items);
        // Each wanted item with every item enclosing it.
        let mut held: BTreeSet<usize> = BTreeSet::new();
        for &item in &wanted {
            let mut at = Some(item).filter(|&item| item < parents.len());
            while let Some(item) = at {
                if !held.insert(item) {
                    break;
                }
                at = parents[item];
            }
        }
        for comment in &file.binding.comments {
            let reaches = match comment.binding {
                Binding::Item(item) | Binding::Module(Some(item)) => held.contains(&item),
                Binding::Module(None) => true,
                Binding::Unanchored(_) | Binding::CannotVerify(_) | Binding::File => false,
            };
            if !reaches {
                continue;
            }
            if let Some(text) = file.text.get(comment.range.clone()) {
                named(text, corpus, seeds);
            }
        }
    }
    for index in ron {
        let Some(file) = tree.ron.get(index) else {
            continue;
        };
        for anchor in &file.anchors {
            if matches!(Target::of_ron(&anchor.anchor), Target::Keyless { .. }) {
                continue;
            }
            if let Some(text) = file.text.get(anchor.comment.clone()) {
                named(text, corpus, seeds);
            }
        }
    }
}

/// Into `seeds`, the nodes each marker of the comment `text` names itself.
fn named(text: &str, corpus: &mut Corpus<'_>, seeds: &mut BTreeSet<NodeKey>) {
    for marker in markers_in(text) {
        let judged = corpus.judge(&marker.id);
        if judged.state.names_nodes() {
            seeds.extend(judged.nodes);
        }
    }
}
