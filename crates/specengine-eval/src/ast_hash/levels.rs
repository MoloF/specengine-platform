//! `result.levels` (Phase 3 slice 5a-2, `docs/features/hash-levels-eval.md`):
//! 5a-1's levels and fingerprints ([`item_levels`], [`module_levels`],
//! [`LevelHashes::is_trivial`]) under the perturbations of [`super`], a
//! neighbour's edit and a rename; trivial items, twins and the threshold
//! sweep; their cost against v2's. Every walk, digest and the triviality
//! threshold are the library's: this module edits text in memory, matches
//! items and counts.

use std::collections::{BTreeMap, HashMap};
use std::hint::black_box;
use std::ops::Range;
use std::time::{Duration, Instant};

use serde::Serialize;
use specengine_code::hash::{attached_attributes, name_node};
use specengine_code::{
    Digest, LevelHashes, LevelState, RustParser, TRIVIAL_BODY_BYTES, analyze_tree,
    hash_bound_items, item_levels, module_levels,
};

use super::{Analyzed, Perturbation, SourceFile, analyze, cancelled, keys_of, relative_string};
use crate::harness::{percent, stability_percent};

/// The neighbour's edit, put before an item's first attached attribute (else
/// the item): an always-true `cfg` and an LF. The item's `sig` changes; every
/// later byte and line shifts.
const NEIGHBOUR_EDIT: &str = "#[cfg(all())]\n";

/// A renamed item's name: this prefix and the item's index in its file
/// (distinct, fixed).
const RENAME_PREFIX: &str = "specengine_renamed_";

/// The `trivial_at` sweep (Q2); the library's threshold is one of its points.
const TRIVIAL_SWEEP: [u32; 5] = [64, 96, TRIVIAL_BODY_BYTES, 160, 256];

/// Timed passes per `cost` figure; the least counts.
const COST_PASSES: usize = 3;

/// The perturbation name of the neighbour pass in `levels-unstable.json`.
const NEIGHBOUR: &str = "neighbour";

/// The perturbation name of the rename pass in `levels-unstable.json`.
const RENAME: &str = "rename";

/// `result.levels`.
#[derive(Serialize)]
pub struct LevelsResult {
    /// Items with levels.
    pub hashed: usize,
    /// Items refused (`CannotVerify`).
    pub cannot_verify: usize,
    /// Files, by their module's levels.
    pub modules: Modules,
    pub stable_pct: StableRows,
    pub module_stable_pct: ModuleRows,
    pub neighbour: Neighbour,
    pub trivial: Trivial,
    /// Groups of 2+ non-trivial items of one kind with one `body`, corpus-wide.
    pub body_groups: Groups,
    /// Groups of 2+ items of one kind and file with one `skeleton`.
    pub skeleton_groups_in_file: Groups,
    /// Of the groups of `body_groups` and `skeleton_groups_in_file`, those
    /// whose members' quotes all differ.
    pub quote_separates: usize,
    /// Per threshold of the sweep.
    pub trivial_at: BTreeMap<u32, TrivialAt>,
    pub rename: Rename,
    pub cost: Cost,
}

#[derive(Serialize)]
pub struct Modules {
    pub hashed: usize,
    pub cannot_verify: usize,
}

/// Per perturbation: the share of items with levels before whose field is
/// equal after. The `fmt_*` rows are `null` without rustfmt.
#[derive(Serialize)]
pub struct StableRows {
    pub fmt_default: Option<LevelPct>,
    pub fmt_contrast: Option<LevelPct>,
    pub comments: LevelPct,
    pub neighbour: LevelPct,
}

#[derive(Serialize)]
pub struct LevelPct {
    pub sig: f64,
    pub body: f64,
    pub deps: f64,
    pub skeleton: f64,
    pub body_bytes: f64,
    /// Reported, never judged: textual by design.
    pub quote: f64,
}

/// Per perturbation: the share of files whose module has levels before and
/// the field equal after.
#[derive(Serialize)]
pub struct ModuleRows {
    pub fmt_default: Option<ModulePct>,
    pub fmt_contrast: Option<ModulePct>,
    pub comments: ModulePct,
}

#[derive(Serialize)]
pub struct ModulePct {
    pub sig: f64,
    pub body: f64,
    pub deps: f64,
}

#[derive(Serialize)]
pub struct Neighbour {
    /// Items edited, over both passes.
    pub edited: usize,
    /// Edited items whose `sig` changed: equal to `edited` (non-vacuity).
    pub edited_changed: usize,
    /// Items of the other parity, neither holding nor inside an edited item.
    pub checked: usize,
}

#[derive(Serialize)]
pub struct Trivial {
    pub items: usize,
    /// Of the items with levels.
    pub pct: f64,
}

#[derive(Serialize)]
pub struct Groups {
    pub groups: usize,
    pub items: usize,
}

#[derive(Serialize)]
pub struct TrivialAt {
    /// Items with `body_bytes` below the threshold.
    pub items: usize,
    /// Items at or above it sharing one `body` with another of their kind.
    pub body_twins: usize,
}

#[derive(Serialize)]
pub struct Rename {
    /// Items with levels and a name, holding no other named item.
    pub renamed: usize,
    /// Renamed items whose field is equal after the rename.
    pub kept: LevelCounts,
    /// Renamed, non-trivial, with a `body` unique among the corpus's items
    /// of their kind, kept.
    pub followable: usize,
}

#[derive(Serialize)]
pub struct LevelCounts {
    pub sig: usize,
    pub body: usize,
    pub deps: usize,
    pub skeleton: usize,
    pub body_bytes: usize,
    pub quote: usize,
}

/// The least of [`COST_PASSES`] passes over the parsed originals, parse
/// excluded, per thousand `.rs` newlines; `null` for a corpus without one.
#[derive(Serialize)]
pub struct Cost {
    /// [`item_levels`] over every item plus [`module_levels`].
    pub levels_ms_per_kloc: Option<f64>,
    /// [`hash_bound_items`] over every item.
    pub v2_ms_per_kloc: Option<f64>,
}

/// One row of `levels-unstable.json`: an item (or a file's module, kind
/// `source_file`) whose judged level or fingerprint changed.
#[derive(Serialize)]
pub(super) struct UnstableLevel {
    path: String,
    line: usize,
    kind: String,
    label: String,
    perturbation: &'static str,
    /// The changed fields (`quote` never listed), or `cannot_verify`,
    /// `unmatched`.
    changed: Vec<&'static str>,
}

/// Where the passes edit one original item.
pub(super) struct Spot {
    /// The neighbour's edit goes here: the first attached attribute's start,
    /// else the item's.
    insert_at: usize,
    /// The name node's bytes; `None` without one (`impl`).
    name: Option<Range<usize>>,
}

/// The originals, each parsed once.
pub(super) struct Originals {
    pub(super) analyzed: Vec<Analyzed>,
    /// Parse and v2 hash ([`analyze_tree`]) of every file, in microseconds.
    pub(super) hash_us: u128,
    pub(super) spots: Vec<Vec<Spot>>,
    pub(super) cost: Cost,
}

/// Parses every file once: v2's items and hashes (timed, `detail.hash_ms`),
/// then over the same trees the levels, the edit points and `cost`.
pub(super) fn originals(
    parser: &mut RustParser,
    files: &[SourceFile],
) -> Result<Originals, String> {
    let started = Instant::now();
    let mut parsed = Vec::with_capacity(files.len());
    for file in files {
        let tree = parser.parse(&file.text).ok_or_else(cancelled)?;
        let analysis = analyze_tree(&tree, &file.text);
        parsed.push((tree, analysis));
    }
    let hash_us = started.elapsed().as_micros();

    let mut analyzed = Vec::with_capacity(files.len());
    let mut spots = Vec::with_capacity(files.len());
    let mut trees = Vec::with_capacity(files.len());
    for (file, (tree, analysis)) in files.iter().zip(parsed) {
        let every: Vec<usize> = (0..analysis.items.len()).collect();
        let levels = item_levels(&tree, &file.text, &every)
            .into_iter()
            .flatten()
            .collect();
        let module = module_levels(&tree, &file.text);
        let root = tree.root_node();
        let file_spots = analysis
            .items
            .iter()
            .map(|item| {
                let range = &item.byte_range;
                // The item's own node: the smallest one over its bytes, or
                // the nearest ancestor of that kind over the same bytes.
                let mut node = root.descendant_for_byte_range(range.start, range.end);
                while let Some(candidate) = node {
                    if candidate.kind() == item.kind && candidate.byte_range() == *range {
                        break;
                    }
                    node = candidate.parent();
                }
                match node {
                    Some(node) => Spot {
                        insert_at: attached_attributes(node)
                            .first()
                            .map_or(range.start, |attribute| attribute.start_byte()),
                        name: name_node(node).map(|name| name.byte_range()),
                    },
                    None => Spot {
                        insert_at: range.start,
                        name: None,
                    },
                }
            })
            .collect();
        spots.push(file_spots);
        analyzed.push(Analyzed {
            keys: keys_of(&analysis.items),
            analysis,
            levels,
            module,
        });
        trees.push((tree, every));
    }

    let mut best_levels = Duration::MAX;
    let mut best_v2 = Duration::MAX;
    for _ in 0..COST_PASSES {
        let started = Instant::now();
        for (file, (tree, every)) in files.iter().zip(&trees) {
            black_box(item_levels(tree, &file.text, every));
            black_box(module_levels(tree, &file.text));
        }
        best_levels = best_levels.min(started.elapsed());
        let started = Instant::now();
        for (file, (tree, every)) in files.iter().zip(&trees) {
            black_box(hash_bound_items(tree, &file.text, every));
        }
        best_v2 = best_v2.min(started.elapsed());
    }
    let newlines = files
        .iter()
        .map(|file| file.text.bytes().filter(|&byte| byte == b'\n').count())
        .sum();
    Ok(Originals {
        analyzed,
        hash_us,
        spots,
        cost: Cost {
            levels_ms_per_kloc: per_kloc(best_levels, newlines),
            v2_ms_per_kloc: per_kloc(best_v2, newlines),
        },
    })
}

/// Milliseconds per thousand newlines, two decimals; `None` without a newline.
fn per_kloc(elapsed: Duration, newlines: usize) -> Option<f64> {
    if newlines == 0 {
        return None;
    }
    let per_kloc = elapsed.as_secs_f64() * 1000.0 * 1000.0 / newlines as f64;
    Some((per_kloc * 100.0).round() / 100.0)
}

/// An item's levels, `None` when refused or past the levels given.
fn hashed(levels: &[LevelState], index: usize) -> Option<&LevelHashes> {
    match levels.get(index) {
        Some(LevelState::Hashed(hashes)) => Some(hashes),
        Some(LevelState::CannotVerify { .. }) | None => None,
    }
}

/// Per field, the items equal after a perturbation, of those compared.
#[derive(Default, Clone, Copy)]
pub(super) struct Tally {
    compared: usize,
    sig: usize,
    body: usize,
    deps: usize,
    skeleton: usize,
    body_bytes: usize,
    quote: usize,
}

impl Tally {
    /// Counts one item with levels before; returns the judged fields that
    /// changed (`quote` aside), `cannot_verify` when refused after.
    fn add(&mut self, before: &LevelHashes, after: Option<&LevelHashes>) -> Vec<&'static str> {
        self.compared += 1;
        let Some(after) = after else {
            return vec!["cannot_verify"];
        };
        let mut changed = Vec::new();
        for (equal, count, name) in [
            (before.sig == after.sig, &mut self.sig, "sig"),
            (before.body == after.body, &mut self.body, "body"),
            (before.deps == after.deps, &mut self.deps, "deps"),
            (
                before.skeleton == after.skeleton,
                &mut self.skeleton,
                "skeleton",
            ),
            (
                before.body_bytes == after.body_bytes,
                &mut self.body_bytes,
                "body_bytes",
            ),
        ] {
            if equal {
                *count += 1;
            } else {
                changed.push(name);
            }
        }
        if before.quote == after.quote {
            self.quote += 1;
        }
        changed
    }

    fn pct(&self) -> LevelPct {
        let pct = |equal| stability_percent(equal, self.compared);
        LevelPct {
            sig: pct(self.sig),
            body: pct(self.body),
            deps: pct(self.deps),
            skeleton: pct(self.skeleton),
            body_bytes: pct(self.body_bytes),
            quote: pct(self.quote),
        }
    }

    fn counts(&self) -> LevelCounts {
        LevelCounts {
            sig: self.sig,
            body: self.body,
            deps: self.deps,
            skeleton: self.skeleton,
            body_bytes: self.body_bytes,
            quote: self.quote,
        }
    }
}

/// Per field, the modules equal after a perturbation, of those compared.
#[derive(Default, Clone, Copy)]
pub(super) struct ModuleTally {
    compared: usize,
    sig: usize,
    body: usize,
    deps: usize,
}

impl ModuleTally {
    fn add(&mut self, before: &LevelHashes, after: Option<&LevelHashes>) -> Vec<&'static str> {
        self.compared += 1;
        let Some(after) = after else {
            return vec!["cannot_verify"];
        };
        let mut changed = Vec::new();
        for (equal, count, name) in [
            (before.sig == after.sig, &mut self.sig, "sig"),
            (before.body == after.body, &mut self.body, "body"),
            (before.deps == after.deps, &mut self.deps, "deps"),
        ] {
            if equal {
                *count += 1;
            } else {
                changed.push(name);
            }
        }
        changed
    }

    fn pct(&self) -> ModulePct {
        let pct = |equal| stability_percent(equal, self.compared);
        ModulePct {
            sig: pct(self.sig),
            body: pct(self.body),
            deps: pct(self.deps),
        }
    }
}

/// The counts of every pass, filled file by file.
#[derive(Default)]
pub(super) struct Passes {
    /// Items and modules per rustfmt or comment perturbation.
    keyed: BTreeMap<Perturbation, (Tally, ModuleTally)>,
    neighbour: Tally,
    edited: usize,
    edited_changed: usize,
    renamed: Tally,
    followable: usize,
    pub(super) unstable: Vec<UnstableLevel>,
}

impl Passes {
    /// One file under a rustfmt or comment perturbation: each item with
    /// levels before against the item of the same key after (rustfmt may
    /// reorder `mod` and `use`), the module against the module.
    pub(super) fn keyed(
        &mut self,
        perturbation: Perturbation,
        file: &SourceFile,
        before: &Analyzed,
        after: &Analyzed,
    ) {
        let name = perturbation.as_str();
        let after_index: HashMap<_, usize> = after
            .keys
            .iter()
            .enumerate()
            .map(|(index, key)| (key, index))
            .collect();
        let (items, module) = self.keyed.entry(perturbation).or_default();
        for (index, key) in before.keys.iter().enumerate() {
            let Some(levels) = hashed(&before.levels, index) else {
                continue;
            };
            let changed = match after_index.get(key) {
                Some(&at) => items.add(levels, hashed(&after.levels, at)),
                None => {
                    items.compared += 1;
                    vec!["unmatched"]
                }
            };
            unstable_item(&mut self.unstable, file, before, index, name, changed);
        }
        if let LevelState::Hashed(levels) = &before.module {
            let after_module = match &after.module {
                LevelState::Hashed(after) => Some(after),
                LevelState::CannotVerify { .. } => None,
            };
            let changed = module.add(levels, after_module);
            unstable_module(&mut self.unstable, file, name, changed);
        }
    }
}

fn unstable_item(
    rows: &mut Vec<UnstableLevel>,
    file: &SourceFile,
    original: &Analyzed,
    index: usize,
    perturbation: &'static str,
    changed: Vec<&'static str>,
) {
    if changed.is_empty() {
        return;
    }
    let Some(item) = original.analysis.items.get(index) else {
        return;
    };
    rows.push(UnstableLevel {
        path: relative_string(&file.relative),
        line: item.line,
        kind: item.kind.clone(),
        label: item.label.clone(),
        perturbation,
        changed,
    });
}

fn unstable_module(
    rows: &mut Vec<UnstableLevel>,
    file: &SourceFile,
    perturbation: &'static str,
    changed: Vec<&'static str>,
) {
    if changed.is_empty() {
        return;
    }
    rows.push(UnstableLevel {
        path: relative_string(&file.relative),
        line: 1,
        kind: "source_file".to_owned(),
        label: String::new(),
        perturbation,
        changed,
    });
}

/// Whether `inner` lies within `outer`.
fn within(inner: &Range<usize>, outer: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// `text` with `insert` put at each of `offsets` (ascending, distinct).
fn with_inserts(text: &str, offsets: &[usize], insert: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len() + offsets.len() * insert.len());
    let mut position = 0;
    for &offset in offsets {
        out.push_str(text.get(position..offset)?);
        out.push_str(insert);
        position = offset;
    }
    out.push_str(text.get(position..)?);
    Some(out)
}

/// `text` with each range replaced by its text (ascending, disjoint).
fn with_replacements(text: &str, replacements: &[(Range<usize>, String)]) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut position = 0;
    for (range, replacement) in replacements {
        out.push_str(text.get(position..range.start)?);
        out.push_str(replacement);
        position = range.end;
    }
    out.push_str(text.get(position..)?);
    Some(out)
}

impl Passes {
    /// The neighbour passes of one file, one re-parse each: pass 1 edits
    /// every odd item (0-based index over every item in source order, a
    /// broken one too) and checks every even one, pass 2 swaps. Counted:
    /// items with levels. Checked: those of the other parity, neither
    /// holding nor inside an edited item; matched by index.
    pub(super) fn neighbour(
        &mut self,
        parser: &mut RustParser,
        file: &SourceFile,
        original: &Analyzed,
        spots: &[Spot],
    ) -> Result<(), String> {
        let items = &original.analysis.items;
        for parity in [1, 0] {
            let edited: Vec<usize> = (0..items.len())
                .filter(|&index| index % 2 == parity)
                .collect();
            let mut offsets: Vec<usize> = edited
                .iter()
                .filter_map(|&index| spots.get(index).map(|spot| spot.insert_at))
                .collect();
            offsets.sort_unstable();
            offsets.dedup();
            let Some(text) = with_inserts(&file.text, &offsets, NEIGHBOUR_EDIT) else {
                eprintln!(
                    "ast-hash: neighbour pass skipped on {}: an edit point off a character boundary",
                    file.relative.display()
                );
                continue;
            };
            let after = analyze(parser, &text)?;
            for &index in &edited {
                let Some(before) = hashed(&original.levels, index) else {
                    continue;
                };
                self.edited += 1;
                if hashed(&after.levels, index).is_some_and(|after| before.sig != after.sig) {
                    self.edited_changed += 1;
                }
            }
            for (index, item) in items.iter().enumerate() {
                if index % 2 == parity {
                    continue;
                }
                let Some(before) = hashed(&original.levels, index) else {
                    continue;
                };
                let near = edited.iter().any(|&edit| {
                    let edit = &items[edit].byte_range;
                    within(edit, &item.byte_range) || within(&item.byte_range, edit)
                });
                if near {
                    continue;
                }
                let changed = self.neighbour.add(before, hashed(&after.levels, index));
                unstable_item(
                    &mut self.unstable,
                    file,
                    original,
                    index,
                    NEIGHBOUR,
                    changed,
                );
            }
        }
        Ok(())
    }

    /// The rename of one file, one re-parse: every item with a name node
    /// holding no other named item, a broken one too, gets a distinct fixed
    /// name; counted: those with levels, matched by index (a rename keeps
    /// the order). `unique_bodies` counts the corpus's `(kind, body)` pairs.
    pub(super) fn rename(
        &mut self,
        parser: &mut RustParser,
        file: &SourceFile,
        original: &Analyzed,
        spots: &[Spot],
        unique_bodies: &HashMap<(&str, Digest), usize>,
    ) -> Result<(), String> {
        let items = &original.analysis.items;
        let named: Vec<usize> = (0..items.len())
            .filter(|&index| spots.get(index).is_some_and(|spot| spot.name.is_some()))
            .collect();
        let renamed: Vec<usize> = named
            .iter()
            .copied()
            .filter(|&index| {
                !named.iter().any(|&other| {
                    other != index && within(&items[other].byte_range, &items[index].byte_range)
                })
            })
            .collect();
        if renamed.is_empty() {
            return Ok(());
        }
        let mut replacements: Vec<(Range<usize>, String)> = renamed
            .iter()
            .filter_map(|&index| {
                let name = spots.get(index)?.name.clone()?;
                Some((name, format!("{RENAME_PREFIX}{index}")))
            })
            .collect();
        replacements.sort_by_key(|(range, _)| range.start);
        let Some(text) = with_replacements(&file.text, &replacements) else {
            eprintln!(
                "ast-hash: rename skipped on {}: a name off a character boundary",
                file.relative.display()
            );
            return Ok(());
        };
        let after = analyze(parser, &text)?;
        for &index in &renamed {
            let Some(before) = hashed(&original.levels, index) else {
                continue;
            };
            let after_levels = hashed(&after.levels, index);
            let changed = self.renamed.add(before, after_levels);
            let body_kept = after_levels.is_some_and(|after| after.body == before.body);
            let unique = unique_bodies
                .get(&(items[index].kind.as_str(), before.body))
                .is_some_and(|&count| count == 1);
            if body_kept && unique && !before.is_trivial() {
                self.followable += 1;
            }
            unstable_item(&mut self.unstable, file, original, index, RENAME, changed);
        }
        Ok(())
    }
}

/// Every hashed item's `(kind, body)`, counted over the corpus.
pub(super) fn body_counts(originals: &[Analyzed]) -> HashMap<(&str, Digest), usize> {
    let mut counts = HashMap::new();
    for original in originals {
        for (index, item) in original.analysis.items.iter().enumerate() {
            if let Some(levels) = hashed(&original.levels, index) {
                *counts.entry((item.kind.as_str(), levels.body)).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// The keys met twice or more (the groups): their number, their items, and
/// how many groups hold members whose quotes all differ.
fn groups<K: std::hash::Hash + Eq>(members: impl Iterator<Item = (K, Digest)>) -> (Groups, usize) {
    let mut quotes: HashMap<K, Vec<Digest>> = HashMap::new();
    for (key, quote) in members {
        quotes.entry(key).or_default().push(quote);
    }
    let mut groups = Groups {
        groups: 0,
        items: 0,
    };
    let mut separated = 0;
    for mut group in quotes.into_values() {
        if group.len() < 2 {
            continue;
        }
        groups.groups += 1;
        groups.items += group.len();
        group.sort_unstable();
        if group.windows(2).all(|pair| pair[0] != pair[1]) {
            separated += 1;
        }
    }
    (groups, separated)
}

impl Passes {
    /// `result.levels` from the originals and the passes; `rustfmt` says
    /// whether the `fmt_*` rows were measured.
    pub(super) fn result(&self, originals: &[Analyzed], cost: Cost, rustfmt: bool) -> LevelsResult {
        // (file, kind, levels) of every item with levels.
        let every: Vec<(usize, &str, &LevelHashes)> = originals
            .iter()
            .enumerate()
            .flat_map(|(file, original)| {
                original
                    .analysis
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, item)| {
                        hashed(&original.levels, index)
                            .map(|levels| (file, item.kind.as_str(), levels))
                    })
            })
            .collect();
        let items: usize = originals.iter().map(|o| o.analysis.items.len()).sum();
        let hashed_items = every.len();
        let modules_hashed = originals
            .iter()
            .filter(|o| matches!(o.module, LevelState::Hashed(_)))
            .count();

        let trivial = every
            .iter()
            .filter(|(_, _, levels)| levels.is_trivial())
            .count();
        let (body_groups, body_separated) = groups(
            every
                .iter()
                .filter(|(_, _, levels)| !levels.is_trivial())
                .map(|(_, kind, levels)| ((*kind, levels.body), levels.quote)),
        );
        let (skeleton_groups_in_file, skeleton_separated) = groups(
            every
                .iter()
                .map(|(file, kind, levels)| ((*file, *kind, levels.skeleton), levels.quote)),
        );
        let trivial_at = TRIVIAL_SWEEP
            .iter()
            .map(|&threshold| {
                let below = every
                    .iter()
                    .filter(|(_, _, levels)| levels.body_bytes < threshold)
                    .count();
                let (twins, _) = groups(
                    every
                        .iter()
                        .filter(|(_, _, levels)| levels.body_bytes >= threshold)
                        .map(|(_, kind, levels)| ((*kind, levels.body), levels.quote)),
                );
                (
                    threshold,
                    TrivialAt {
                        items: below,
                        body_twins: twins.items,
                    },
                )
            })
            .collect();

        let keyed = |perturbation| self.keyed.get(&perturbation).copied().unwrap_or_default();
        let fmt_row = |perturbation| rustfmt.then(|| keyed(perturbation).0.pct());
        let fmt_module = |perturbation| rustfmt.then(|| keyed(perturbation).1.pct());
        LevelsResult {
            hashed: hashed_items,
            cannot_verify: items - hashed_items,
            modules: Modules {
                hashed: modules_hashed,
                cannot_verify: originals.len() - modules_hashed,
            },
            stable_pct: StableRows {
                fmt_default: fmt_row(Perturbation::FmtDefault),
                fmt_contrast: fmt_row(Perturbation::FmtContrast),
                comments: keyed(Perturbation::Comments).0.pct(),
                neighbour: self.neighbour.pct(),
            },
            module_stable_pct: ModuleRows {
                fmt_default: fmt_module(Perturbation::FmtDefault),
                fmt_contrast: fmt_module(Perturbation::FmtContrast),
                comments: keyed(Perturbation::Comments).1.pct(),
            },
            neighbour: Neighbour {
                edited: self.edited,
                edited_changed: self.edited_changed,
                checked: self.neighbour.compared,
            },
            trivial: Trivial {
                items: trivial,
                pct: percent(trivial, hashed_items),
            },
            body_groups,
            skeleton_groups_in_file,
            quote_separates: body_separated + skeleton_separated,
            trivial_at,
            rename: Rename {
                renamed: self.renamed.compared,
                kept: self.renamed.counts(),
                followable: self.followable,
            },
            cost,
        }
    }
}

/// The levels lines of the human summary (stderr).
pub(super) fn summarize(levels: &LevelsResult) {
    let row = |pct: Option<&LevelPct>| {
        pct.map_or("n/a".to_owned(), |p| {
            format!(
                "sig {:.1} body {:.1} deps {:.1} skeleton {:.1} body_bytes {:.1} quote {:.1}",
                p.sig, p.body, p.deps, p.skeleton, p.body_bytes, p.quote
            )
        })
    };
    let module = |pct: Option<&ModulePct>| {
        pct.map_or("n/a".to_owned(), |p| {
            format!("sig {:.1} body {:.1} deps {:.1}", p.sig, p.body, p.deps)
        })
    };
    let ms = |value: Option<f64>| value.map_or("n/a".to_owned(), |v| format!("{v:.2}"));
    eprintln!(
        "  levels: {} items hashed, {} cannot_verify; modules {} hashed, {} cannot_verify",
        levels.hashed, levels.cannot_verify, levels.modules.hashed, levels.modules.cannot_verify
    );
    let stable = &levels.stable_pct;
    eprintln!("    fmt_default  {}", row(stable.fmt_default.as_ref()));
    eprintln!("    fmt_contrast {}", row(stable.fmt_contrast.as_ref()));
    eprintln!("    comments     {}", row(Some(&stable.comments)));
    eprintln!("    neighbour    {}", row(Some(&stable.neighbour)));
    let modules = &levels.module_stable_pct;
    eprintln!(
        "    modules: fmt_default {}; fmt_contrast {}; comments {}",
        module(modules.fmt_default.as_ref()),
        module(modules.fmt_contrast.as_ref()),
        module(Some(&modules.comments))
    );
    eprintln!(
        "    neighbour: {} edited, {} with sig changed, {} checked",
        levels.neighbour.edited, levels.neighbour.edited_changed, levels.neighbour.checked
    );
    eprintln!(
        "    trivial {} ({:.1} %); body groups {} ({} items); in-file skeleton groups {} ({} items); {} of these groups separated by quote",
        levels.trivial.items,
        levels.trivial.pct,
        levels.body_groups.groups,
        levels.body_groups.items,
        levels.skeleton_groups_in_file.groups,
        levels.skeleton_groups_in_file.items,
        levels.quote_separates
    );
    let sweep: Vec<String> = levels
        .trivial_at
        .iter()
        .map(|(threshold, at)| format!("<{threshold}: {} ({} twins)", at.items, at.body_twins))
        .collect();
    eprintln!("    trivial_at {}", sweep.join(", "));
    let kept = &levels.rename.kept;
    eprintln!(
        "    rename: {} renamed, kept sig {} body {} deps {} skeleton {} body_bytes {} quote {}, {} followable",
        levels.rename.renamed,
        kept.sig,
        kept.body,
        kept.deps,
        kept.skeleton,
        kept.body_bytes,
        kept.quote,
        levels.rename.followable
    );
    eprintln!(
        "    cost: levels {} ms/kLOC, v2 {} ms/kLOC (least of {COST_PASSES} passes, parse excluded)",
        ms(levels.cost.levels_ms_per_kloc),
        ms(levels.cost.v2_ms_per_kloc)
    );
}
