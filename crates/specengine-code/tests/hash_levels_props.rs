//! docs/features/hash-levels.md (5a-1) over every `.rs` file of
//! `fixtures/hash-levels/` (the only fixture committing rustfmt variants):
//!
//! - TE-2: levels, `skeleton` and `body_bytes` hold under the committed
//!   rustfmt variants (default, `max_width = 60`), comment stripping and a
//!   neighbour's edit (`#[cfg(all())]` and an LF before every other item, both
//!   parities, as 5a-2 measures it); each level changes under its own
//!   targeted edit. M: a constant hash; a hash that reads a neighbour.
//! - TE-6 over the same edits: `hash_item` (v2) changes exactly when `sig` or
//!   `body` changes; the only exception met is N5 (rustfmt reorders `mod x;`
//!   and `extern crate` runs, v2 does not sort them).
//! - The recipe re-stated by hand on v2's public walk, so the bytes of
//!   `sig`, `body`, `deps`, `quote` and the count `body_bytes` are checked
//!   against the spec rather than against the implementation (L1-L3 in
//!   `levels_common`); `skeleton` has no public walk to restate and is held
//!   by relations and RD-3's golden pin.
//!
//! PHASE-B: `item_levels`, `module_levels`, `body_nodes`, `call_targets`,
//! `level_header`, `LEVEL_RECIPE`, `TRIVIAL_BODY_BYTES`, `LevelState`,
//! `LevelHashes` at the crate root, as "Entry points" names them.

mod levels_common;

use std::collections::BTreeSet;
use std::ops::Range;

use levels_common::{
    Key, Levels, STABLE, Targeted, VARIANTS, base_files, body_by_hand, body_stream_by_hand,
    changed, deps_after_header, digest_by_hand, header_by_hand, item_node,
    leading_inner_attributes, leaf_text_bytes, new_parser, on_big_stack, parse, put,
    quote_stream_by_hand, read, read_variant, sig_stream_by_hand, snapshot, targeted, v2_walk,
    variant_files, with_cfg_before,
};
use specengine_code::comments::strip_comments;
use specengine_code::hash::{RECIPE, recipe_header};
use specengine_code::items::{ITEM_KINDS, MEMBER_KINDS};
use specengine_code::{
    LEVEL_RECIPE, TRIVIAL_BODY_BYTES, analyze_tree, body_nodes, call_targets, level_header,
};
use tree_sitter::Node;

fn report(what: &str, failures: &[String]) {
    assert!(
        failures.is_empty(),
        "{what}: {} failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

/// TE-6 on one item and one perturbation: v2 changes exactly when `sig` or
/// `body` does; a v2 change with both held is an N5 exception only where the
/// item holds an N5 run. Returns whether it was such an exception.
fn v2_follows_the_levels(
    what: &str,
    v2: (
        Option<specengine_code::Digest>,
        Option<specengine_code::Digest>,
    ),
    a: &Levels,
    b: &Levels,
    n5_run: bool,
    failures: &mut Vec<String>,
) -> bool {
    let v2_moved = v2.0 != v2.1;
    let levels_moved = a.sig != b.sig || a.body != b.body;
    if v2_moved && !levels_moved {
        if n5_run {
            return true;
        }
        failures.push(format!(
            "{what}: v2 changed while sig and body held, and no N5 run: a leaf in v2 only"
        ));
    } else if v2_moved != levels_moved {
        failures.push(format!(
            "{what}: v2 changed {v2_moved}, sig or body changed {levels_moved}: a leaf in neither stream"
        ));
    }
    false
}

// ------------------------------------------------------------------- TE-2

#[test]
fn te2_levels_hold_under_the_committed_rustfmt_variants() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        let mut n5_exceptions = Vec::new();
        let mut compared = 0;
        for file in variant_files() {
            let base_text = read(&file);
            let base = snapshot(&base_text);
            for variant in VARIANTS {
                let text = read_variant(variant, &file);
                assert_ne!(
                    text, base_text,
                    "{variant}/{file}: a committed variant differs from its source"
                );
                let other = snapshot(&text);
                let mut base_keys = base.keys.clone();
                base_keys.sort();
                let mut other_keys = other.keys.clone();
                other_keys.sort();
                assert_eq!(base_keys, other_keys, "{variant}/{file}: the same items");
                for (index, key) in base.keys.iter().enumerate() {
                    let what = format!("{variant}/{file}: {key}");
                    let at = other.index_of(key).expect("matched by key");
                    let (Some(a), Some(b)) = (base.units[index].levels(), other.units[at].levels())
                    else {
                        failures.push(format!(
                            "{what}: refused: {:?} / {:?}",
                            base.units[index], other.units[at]
                        ));
                        continue;
                    };
                    compared += 1;
                    let moved = changed(a, b, &STABLE);
                    if !moved.is_empty() {
                        failures.push(format!("{what}: {moved:?} changed under rustfmt"));
                    }
                    if v2_follows_the_levels(
                        &what,
                        (base.v2_digest(index), other.v2_digest(at)),
                        a,
                        b,
                        base.n5[index],
                        &mut failures,
                    ) {
                        n5_exceptions.push(what);
                    }
                }
                match (base.module.levels(), other.module.levels()) {
                    (Some(a), Some(b)) => {
                        let moved = changed(a, b, &STABLE);
                        if !moved.is_empty() {
                            failures.push(format!(
                                "{variant}/{file}: module {moved:?} changed under rustfmt"
                            ));
                        }
                    }
                    pair => failures.push(format!("{variant}/{file}: module refused: {pair:?}")),
                }
            }
        }
        report("rustfmt variants", &failures);
        assert!(
            compared > 100,
            "every item of every variant compared: {compared}"
        );
        assert!(
            !n5_exceptions.is_empty(),
            "rustfmt reorders `mod hub`'s runs: the N5 exception must occur (non-vacuity)"
        );
    });
}

#[test]
fn te2_levels_hold_under_comment_stripping() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        for file in base_files() {
            let text = read(&file);
            let tree = parse(&mut new_parser(), &text);
            let stripped = strip_comments(&text, &tree);
            assert_ne!(stripped, text, "{file}: the fixture file holds comments");
            let base = snapshot(&text);
            let other = snapshot(&stripped);
            assert_eq!(
                base.keys, other.keys,
                "{file}: the same items without comments"
            );
            for (index, key) in base.keys.iter().enumerate() {
                let what = format!("{file} stripped: {key}");
                match (base.units[index].levels(), other.units[index].levels()) {
                    (Some(a), Some(b)) => {
                        let moved = changed(a, b, &STABLE);
                        if !moved.is_empty() {
                            failures.push(format!("{what}: {moved:?} changed"));
                        }
                        // Stripping reorders nothing: no exception applies.
                        v2_follows_the_levels(
                            &what,
                            (base.v2_digest(index), other.v2_digest(index)),
                            a,
                            b,
                            false,
                            &mut failures,
                        );
                    }
                    (None, None) => {}
                    pair => failures.push(format!("{what}: refusal changed: {pair:?}")),
                }
            }
            match (base.module.levels(), other.module.levels()) {
                (Some(a), Some(b)) => {
                    let moved = changed(a, b, &STABLE);
                    if !moved.is_empty() {
                        failures.push(format!("{file} stripped: module {moved:?} changed"));
                    }
                }
                (None, None) => {}
                pair => failures.push(format!("{file} stripped: module refusal changed: {pair:?}")),
            }
        }
        report("comment stripping", &failures);
    });
}

/// 5a-2's neighbour pass: pass 1 edits every odd item and checks every even
/// one, pass 2 swaps; checked are the items of the other parity neither
/// holding nor inside an edited item. Every edited item's `sig` changes
/// (non-vacuity: a constant hash fails here).
#[test]
fn te2_levels_ignore_a_neighbour_edit_in_both_parities() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        let mut checked = 0;
        let mut edited_total = 0;
        for file in base_files() {
            let text = read(&file);
            let base = snapshot(&text);
            for parity in [1usize, 0] {
                let edited: Vec<usize> = (0..base.keys.len()).filter(|i| i % 2 == parity).collect();
                if edited.is_empty() {
                    continue;
                }
                let other = snapshot(&with_cfg_before(&text, &base.spans, &edited));
                assert_eq!(base.keys, other.keys, "{file}: the attribute adds no item");
                for &index in &edited {
                    let what = format!("{file} pass {parity}: edited {}", base.keys[index]);
                    if let (Some(a), Some(b)) =
                        (base.units[index].levels(), other.units[index].levels())
                    {
                        edited_total += 1;
                        if a.sig == b.sig {
                            failures.push(format!("{what}: sig held under its own attribute edit"));
                        }
                        if base.v2_digest(index) == other.v2_digest(index) {
                            failures.push(format!("{what}: v2 held under an attribute edit"));
                        }
                    }
                }
                for index in (0..base.keys.len()).filter(|i| i % 2 != parity) {
                    if edited
                        .iter()
                        .any(|&e| overlaps(&base.spans[e], &base.spans[index]))
                    {
                        continue;
                    }
                    let what = format!("{file} pass {parity}: neighbour {}", base.keys[index]);
                    match (base.units[index].levels(), other.units[index].levels()) {
                        (Some(a), Some(b)) => {
                            checked += 1;
                            let moved = changed(a, b, &STABLE);
                            if !moved.is_empty() {
                                failures.push(format!("{what}: {moved:?} changed"));
                            }
                            v2_follows_the_levels(
                                &what,
                                (base.v2_digest(index), other.v2_digest(index)),
                                a,
                                b,
                                false,
                                &mut failures,
                            );
                        }
                        (None, None) => {}
                        pair => failures.push(format!("{what}: refusal changed: {pair:?}")),
                    }
                }
            }
        }
        report("neighbour edit", &failures);
        assert!(checked > 30, "neighbours checked: {checked}");
        assert!(edited_total > 30, "items edited: {edited_total}");
    });
}

/// An attribute before the item moves `sig` (and `skeleton`) only; an edit
/// inside the body moves `body` (`skeleton`, `body_bytes`) only; a call
/// inside the body moves `deps` and `body` only (a macro's never moves
/// `deps`: its token trees are invisible). v2 moves under each (TE-6).
#[test]
fn te2_each_level_changes_under_its_own_targeted_edit() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        let mut exercised: BTreeSet<(String, &str)> = BTreeSet::new();
        for file in base_files() {
            let text = read(&file);
            let base = snapshot(&text);
            if base.root_has_error {
                continue; // TE-3's subject
            }
            let tree = parse(&mut new_parser(), &text);
            let analysis = analyze_tree(&tree, &text);
            for (index, item) in analysis.items.iter().enumerate() {
                let Some(a) = base.units[index].levels() else {
                    continue; // nesting_too_deep: TE-3's subject
                };
                let key = &base.keys[index];
                let node = item_node(tree.root_node(), &item.byte_range, &item.kind);
                let edits = [
                    (
                        "attribute",
                        Some(with_cfg_before(&text, &base.spans, &[index])),
                    ),
                    ("body", targeted(&text, node, Targeted::Body)),
                    ("deps", targeted(&text, node, Targeted::Deps)),
                ];
                for (what, edited) in edits {
                    let Some(edited) = edited else {
                        continue;
                    };
                    let label = format!("{file}: {key}: {what} edit");
                    let other = snapshot(&edited);
                    assert!(
                        !other.root_has_error,
                        "{label} must parse cleanly:\n{edited}"
                    );
                    let at = other
                        .index_of(key)
                        .unwrap_or_else(|| panic!("{label}: the item is still there"));
                    let Some(b) = other.units[at].levels() else {
                        failures.push(format!("{label}: refused {:?}", other.units[at]));
                        continue;
                    };
                    exercised.insert((item.kind.clone(), what));
                    let want: &[&str] = match (what, item.kind.as_str()) {
                        ("attribute", _) => &["sig", "skeleton"],
                        ("deps", "macro_definition") | ("body", _) => {
                            &["body", "skeleton", "body_bytes"]
                        }
                        _ => &["body", "deps", "skeleton", "body_bytes"],
                    };
                    let moved = changed(a, b, &STABLE);
                    if moved != want {
                        failures.push(format!("{label}: changed {moved:?}, expected {want:?}"));
                    }
                    if what == "body" && b.body_bytes <= a.body_bytes {
                        failures.push(format!(
                            "{label}: body_bytes {} -> {}: added leaves must count",
                            a.body_bytes, b.body_bytes
                        ));
                    }
                    if base.v2_digest(index) == other.v2_digest(at) {
                        failures.push(format!("{label}: v2 held under a sig or body edit (TE-6)"));
                    }
                }
            }
        }
        report("targeted edits", &failures);
        for kind in ITEM_KINDS.iter().chain(MEMBER_KINDS) {
            for what in ["attribute", "body", "deps"] {
                if *kind == "function_signature_item" && what != "attribute" {
                    continue; // no body field
                }
                assert!(
                    exercised.contains(&((*kind).to_owned(), what)),
                    "no {what} edit exercised on a {kind}"
                );
            }
        }
    });
}

// ---------------------------------------------------- the recipe, by hand

/// Calls `check` with every hashed item of every fixture file.
fn each_hashed_item(check: &mut dyn FnMut(&str, &Key, Node, &str, &Levels)) {
    for file in base_files() {
        let text = read(&file);
        let tree = parse(&mut new_parser(), &text);
        let analysis = analyze_tree(&tree, &text);
        let snap = snapshot(&text);
        for (index, item) in analysis.items.iter().enumerate() {
            let Some(levels) = snap.units[index].levels() else {
                continue;
            };
            let node = item_node(tree.root_node(), &item.byte_range, &item.kind);
            check(&file, &snap.keys[index], node, &text, levels);
        }
    }
}

#[test]
fn recipe_level_header_is_v3_beside_v2() {
    assert_eq!(LEVEL_RECIPE, "specengine-hash/v3");
    assert_eq!(RECIPE, "specengine-hash/v2", "v2's recipe stays");
    assert_eq!(level_header(), header_by_hand().as_slice(), "L1");
    assert_ne!(
        level_header(),
        recipe_header(),
        "one header shared by v2 and v3"
    );
    assert_eq!(TRIVIAL_BODY_BYTES, 124);
}

/// TE-6's ground, byte for byte (L2): `sig` is the v2 walk of the
/// attributes and of the item without its name and body nodes, `body` the v2
/// walk of the body nodes; together they read exactly the leaves `hash_item`
/// reads. Items holding an N5 run are skipped (walk3 sorts what v2 does not).
#[test]
fn recipe_sig_and_body_split_the_v2_walk() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        let (mut checked, mut skipped) = (0, 0);
        each_hashed_item(&mut |file, key, node, text, levels| {
            if levels_common::has_n5_run(node) {
                skipped += 1;
                return;
            }
            checked += 1;
            let sig = sig_stream_by_hand(node, text);
            let body = body_stream_by_hand(node, text);
            if *levels.sig.as_bytes() != digest_by_hand("sig", &sig) {
                failures.push(format!("{file}: {key}: sig is not the recipe's"));
            }
            if *levels.body.as_bytes() != digest_by_hand("body", &body) {
                failures.push(format!("{file}: {key}: body is not the recipe's"));
            }
            // The oracle's own ground: sig and body partition v2's leaves.
            let mut v2 = Vec::new();
            for attribute in specengine_code::hash::attached_attributes(node) {
                v2.extend(v2_walk(attribute, text, None));
            }
            v2.extend(v2_walk(node, text, specengine_code::hash::name_node(node)));
            assert_eq!(
                leaf_text_bytes(&v2),
                leaf_text_bytes(&sig) + leaf_text_bytes(&body),
                "{file}: {key}: the oracle's sig and body cover v2's leaves"
            );
        });
        report("sig and body by hand", &failures);
        assert!(checked > 60, "items checked: {checked}");
        assert!(
            skipped > 0,
            "the N5 items are skipped, not missing: {skipped}"
        );
    });
}

/// `deps` = BLAKE3(header || put("deps") || put(target)...) over
/// `call_targets`, byte-sorted and deduplicated.
#[test]
fn recipe_deps_hash_the_byte_sorted_call_targets() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        each_hashed_item(&mut |file, key, node, text, levels| {
            let targets = call_targets(node, text);
            let mut sorted = targets.clone();
            sorted.sort();
            sorted.dedup();
            if targets != sorted {
                failures.push(format!(
                    "{file}: {key}: targets not byte-sorted, deduplicated: {targets:?}"
                ));
            }
            if targets.iter().any(String::is_empty) {
                failures.push(format!("{file}: {key}: an empty target"));
            }
            let mut stream = Vec::new();
            for target in &targets {
                put(&mut stream, target.as_bytes());
            }
            if *levels.deps.as_bytes() != digest_by_hand("deps", &stream) {
                failures.push(format!(
                    "{file}: {key}: deps is not the hash of {targets:?}"
                ));
            }
        });
        report("deps by hand", &failures);
    });
}

/// Review 1: the call set `deps` hashes and the public `call_targets` are
/// computed apart; they must agree. For every item `deps` is the digest of
/// `call_targets(item)`, for every file module of `call_targets(root)`.
#[test]
fn deps_are_the_digest_of_call_targets_for_items_and_modules() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        let mut items = 0;
        each_hashed_item(&mut |file, key, node, text, levels| {
            items += 1;
            let targets = call_targets(node, text);
            let targets: Vec<&str> = targets.iter().map(String::as_str).collect();
            if *levels.deps.as_bytes() != deps_after_header(&targets) {
                failures.push(format!(
                    "{file}: {key}: deps is not call_targets' {targets:?}"
                ));
            }
        });
        let mut modules = 0;
        for file in base_files() {
            let text = read(&file);
            let tree = parse(&mut new_parser(), &text);
            let snap = snapshot(&text);
            let Some(module) = snap.module.levels() else {
                continue;
            };
            modules += 1;
            let targets = call_targets(tree.root_node(), &text);
            let targets: Vec<&str> = targets.iter().map(String::as_str).collect();
            if *module.deps.as_bytes() != deps_after_header(&targets) {
                failures.push(format!(
                    "{file}: module deps is not call_targets(root)'s {targets:?}"
                ));
            }
        }
        report("deps against call_targets", &failures);
        assert!(
            items > 60 && modules >= 9,
            "{items} items, {modules} modules"
        );
    });
}

/// L3: `quote` restated from the source text (`levels_common::quote_stream_by_hand`).
#[test]
fn recipe_quote_is_the_text_around_the_name() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        each_hashed_item(&mut |file, key, node, text, levels| {
            let stream = quote_stream_by_hand(node, text);
            if *levels.quote.as_bytes() != digest_by_hand("quote", &stream) {
                failures.push(format!("{file}: {key}: quote is not L3's"));
            }
        });
        report("quote by hand", &failures);
    });
}

/// `body_bytes` = the text lengths of the leaves the body walk emits
/// (comments, `,` and N1-N3 forms excluded), whatever the layout; trivial iff
/// below 124. Valid for N5 items too: sorting moves no leaf.
#[test]
fn recipe_body_bytes_count_the_leaves_of_the_body_walk() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        let (mut trivial, mut not_trivial) = (0, 0);
        each_hashed_item(&mut |file, key, node, text, levels| {
            let want: u32 = body_by_hand(node)
                .into_iter()
                .map(|body| leaf_text_bytes(&v2_walk(body, text, None)))
                .sum();
            if levels.body_bytes != want {
                failures.push(format!(
                    "{file}: {key}: body_bytes {}, the body walk's leaves {want}",
                    levels.body_bytes
                ));
            }
            if levels.trivial != (levels.body_bytes < TRIVIAL_BODY_BYTES) {
                failures.push(format!("{file}: {key}: is_trivial disagrees with < 124"));
            }
            if levels.trivial {
                trivial += 1;
            } else {
                not_trivial += 1;
            }
        });
        report("body_bytes by hand", &failures);
        assert!(trivial > 0 && not_trivial > 0, "both sides of 124 occur");
    });
}

/// "The body field": `body_nodes` gives the grammar's body field (value,
/// type, the `macro_rule` children), in source order; none for a signature
/// item, a unit struct, `mod x;`, `impl ... ;`, `const X: T;`, `static X: T;`.
/// Without a body, `body` and `deps` hash the empty stream and count 0.
#[test]
fn recipe_body_nodes_follow_the_body_field_table() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        let mut bodiless = BTreeSet::new();
        each_hashed_item(&mut |file, key, node, _text, levels| {
            let by_hand: Vec<(&str, Range<usize>)> = body_by_hand(node)
                .iter()
                .map(|n| (n.kind(), n.byte_range()))
                .collect();
            let given: Vec<(&str, Range<usize>)> = body_nodes(node)
                .iter()
                .map(|n| (n.kind(), n.byte_range()))
                .collect();
            if by_hand != given {
                failures.push(format!(
                    "{file}: {key}: body_nodes {given:?}, the table {by_hand:?}"
                ));
            }
            if by_hand.is_empty() {
                bodiless.insert(key.kind.clone());
                if *levels.body.as_bytes() != digest_by_hand("body", &[]) {
                    failures.push(format!(
                        "{file}: {key}: no body, yet not the empty body stream"
                    ));
                }
                if *levels.deps.as_bytes() != digest_by_hand("deps", &[]) {
                    failures.push(format!(
                        "{file}: {key}: no body, yet not the empty deps stream"
                    ));
                }
                if levels.body_bytes != 0 {
                    failures.push(format!(
                        "{file}: {key}: no body, body_bytes {}",
                        levels.body_bytes
                    ));
                }
            }
        });
        report("body nodes", &failures);
        let want: BTreeSet<String> = [
            "const_item",
            "function_signature_item",
            "impl_item",
            "mod_item",
            "static_item",
            "struct_item",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(
            bodiless, want,
            "every bodiless form of the table occurs in the fixture"
        );
    });
}

/// File module: `sig` is the v2 walk of the leading inner attributes;
/// without inner attributes and N5 runs, `body` is v2's walk of the file;
/// `body_bytes` counts the file's leaves outside that run.
#[test]
fn recipe_module_sig_body_and_body_bytes() {
    on_big_stack(|| {
        let mut failures = Vec::new();
        let mut bodies_checked = 0;
        for file in base_files() {
            let text = read(&file);
            let tree = parse(&mut new_parser(), &text);
            let snap = snapshot(&text);
            let Some(module) = snap.module.levels() else {
                continue;
            };
            let root = tree.root_node();
            let run = leading_inner_attributes(root);
            let sig: Vec<u8> = run.iter().flat_map(|a| v2_walk(*a, &text, None)).collect();
            if *module.sig.as_bytes() != digest_by_hand("sig", &sig) {
                failures.push(format!(
                    "{file}: module sig is not its inner attributes' walk"
                ));
            }
            let whole = leaf_text_bytes(&v2_walk(root, &text, None));
            let in_sig = leaf_text_bytes(&sig);
            if module.body_bytes != whole - in_sig {
                failures.push(format!(
                    "{file}: module body_bytes {}, the file's leaves outside sig {}",
                    module.body_bytes,
                    whole - in_sig
                ));
            }
            if run.is_empty() && !levels_common::has_n5_run(root) {
                bodies_checked += 1;
                let body = v2_walk(root, &text, None);
                if *module.body.as_bytes() != digest_by_hand("body", &body) {
                    failures.push(format!("{file}: module body is not the file's walk"));
                }
            }
        }
        report("module by hand", &failures);
        assert!(
            bodies_checked >= 5,
            "module bodies checked: {bodies_checked}"
        );
    });
}
