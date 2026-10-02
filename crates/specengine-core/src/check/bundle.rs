//! The layers of a context bundle over the spec graph (task spec
//! `spec-cli-bundle`; 05 §6): which nodes stand around the named targets
//! and in which layer, chosen by link type, link direction and the `[ids]`
//! scope of a prefix only, never by a node's kind (ADR-0008). Pure: it
//! reads the graph, no file; fitting, rendering and hashing are the
//! caller's (`spec bundle`, later MCP `get_context_bundle`).
//!
//! - "Linked": a resolved link with one end on a target or an ID section
//!   within it ([`SpecGraph::within`]), the other on the candidate, written
//!   in a live file or a target's own; the direction is the target's
//!   (`out`: target → candidate). Dangling, `project:` and unchecked links
//!   are not followed. A candidate lives in a live file and is within no
//!   target.
//! - Layer 2, open questions: linked by any type, `mentions` included; it
//!   declares a `working_answer:` and no resolved `answers` written in a
//!   live file lands on it or a section within it.
//! - Layer 3, ancestors: [`SpecGraph::ancestors`] per target, nearest first,
//!   those in live files.
//! - Layer 4, criteria: a source of `verifies` linked in, or an ID section
//!   whose prefix is `scope = "feature"` linked in by any type.
//! - Layers 6, 7, 8: [`BUNDLE_LINK_TYPES`].
//! - Layers 5 and 9 (bindings, tests): none until Phase 3.
//!
//! A node is a candidate once, in its first layer; within a layer by (path,
//! position), ancestors nearest first per target.

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::{Direction, IdScope};

use super::spec_graph::{Endpoint, NodeAt, SpecGraph, Standing};

/// A layer of the bundle, in print order (05 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BundleLayer {
    Targets,
    OpenQuestions,
    Ancestors,
    Criteria,
    /// Phase 3: always empty.
    Bindings,
    Decisions,
    Neighbours,
    Terms,
    /// Phase 3: always empty.
    Tests,
}

impl BundleLayer {
    /// Every layer, in print order.
    pub const ALL: [Self; 9] = [
        Self::Targets,
        Self::OpenQuestions,
        Self::Ancestors,
        Self::Criteria,
        Self::Bindings,
        Self::Decisions,
        Self::Neighbours,
        Self::Terms,
        Self::Tests,
    ];
}

/// The (layer, link type, direction from the target) rules of layers 4, 6,
/// 7 and 8; one table for every project. Layer 2 takes any type, layer 4
/// also the feature-scoped ID sections linked in by any type.
pub const BUNDLE_LINK_TYPES: [(BundleLayer, &str, Direction); 7] = [
    (BundleLayer::Criteria, VERIFIES, Direction::In),
    (BundleLayer::Decisions, "canon", Direction::In),
    (BundleLayer::Neighbours, "depends_on", Direction::Out),
    (BundleLayer::Neighbours, "constrains", Direction::Out),
    (BundleLayer::Neighbours, "constrains", Direction::In),
    (BundleLayer::Neighbours, "derived_from", Direction::Out),
    (BundleLayer::Terms, "uses_term", Direction::Out),
];

const VERIFIES: &str = "verifies";
const WORKING_ANSWER: &str = "working_answer";
const ANSWERS: &str = "answers";

/// A node around the targets and the layer it stands in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleCandidate {
    pub node: NodeAt,
    pub layer: BundleLayer,
    /// The (link type, direction) pairs that put it in its layer, sorted by
    /// type, then `in` before `out`; empty for an ancestor.
    pub via: Vec<(String, Direction)>,
    /// Layer 2: the edge ([`SpecGraph::edges`]) of its first
    /// `working_answer:`, whatever its state.
    pub working_answer: Option<usize>,
}

/// The targets and the candidates of one bundle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BundleLayers {
    /// Each node once, by (path, position); a target within another one
    /// is merged into it.
    pub targets: Vec<NodeAt>,
    /// (merged, into): a target within another target, by the merged one.
    pub merged: Vec<(NodeAt, NodeAt)>,
    /// By layer, then (path, position); ancestors nearest first, target by
    /// target.
    pub candidates: Vec<BundleCandidate>,
}

/// The layers around `targets` (see the module documentation).
pub fn bundle_layers(graph: &SpecGraph<'_>, targets: &[NodeAt]) -> BundleLayers {
    let mut named: Vec<NodeAt> = targets
        .iter()
        .copied()
        .filter(|&at| graph.node(at).is_some())
        .collect();
    named.sort_unstable();
    named.dedup();
    let mut kept = Vec::new();
    let mut merged = Vec::new();
    for &at in &named {
        // The first enclosing target in (path, position) order: the
        // outermost one.
        let outer = named
            .iter()
            .copied()
            .find(|&other| other != at && graph.within(other).contains(&at));
        match outer {
            Some(outer) => merged.push((at, outer)),
            None => kept.push(at),
        }
    }

    let inside: BTreeSet<NodeAt> = kept.iter().flat_map(|&at| graph.within(at)).collect();
    let target_files: BTreeSet<usize> = kept.iter().map(|at| at.file).collect();
    let admits =
        |file: usize| graph.standing(file) == Standing::Live || target_files.contains(&file);
    let candidate = |at: NodeAt| graph.standing(at.file) == Standing::Live && !inside.contains(&at);

    // Every node linked to a target, with the (type, direction) pairs.
    let edges = graph.edges();
    let mut linked: BTreeMap<NodeAt, BTreeSet<(String, Direction)>> = BTreeMap::new();
    for &target in &kept {
        let links = graph.links(target, admits);
        for &(index, _) in &links.outgoing {
            let edge = &edges[index];
            if !admits(edge.file) {
                continue;
            }
            if let Endpoint::Nodes(nodes) = &edge.target {
                for &node in nodes.iter().filter(|&&node| candidate(node)) {
                    linked
                        .entry(node)
                        .or_default()
                        .insert((edge.link_type.clone(), Direction::Out));
                }
            }
        }
        for &(index, _) in &links.incoming {
            let edge = &edges[index];
            if !admits(edge.file) {
                continue;
            }
            if let Endpoint::Nodes(nodes) = &edge.source {
                for &node in nodes.iter().filter(|&&node| candidate(node)) {
                    linked
                        .entry(node)
                        .or_default()
                        .insert((edge.link_type.clone(), Direction::In));
                }
            }
        }
    }

    // Declared working answers (the first per node) and the nodes a live,
    // resolved `answers` lands on.
    let mut working_answers: BTreeMap<NodeAt, usize> = BTreeMap::new();
    let mut answered: BTreeSet<NodeAt> = BTreeSet::new();
    for (index, edge) in edges.iter().enumerate() {
        if edge.link_type == WORKING_ANSWER {
            working_answers.entry(edge.holder).or_insert(index);
        }
        if edge.link_type == ANSWERS
            && graph.standing(edge.file) == Standing::Live
            && let Endpoint::Nodes(nodes) = &edge.target
        {
            answered.extend(nodes.iter().copied());
        }
    }

    let mut placed: BTreeSet<NodeAt> = BTreeSet::new();
    let mut candidates = Vec::new();

    // Layer 2.
    for (&node, pairs) in &linked {
        let Some(&working_answer) = working_answers.get(&node) else {
            continue;
        };
        if graph.within(node).iter().any(|at| answered.contains(at)) {
            continue;
        }
        placed.insert(node);
        candidates.push(BundleCandidate {
            node,
            layer: BundleLayer::OpenQuestions,
            via: sorted(pairs.iter().cloned()),
            working_answer: Some(working_answer),
        });
    }

    // Layer 3.
    for &target in &kept {
        for ancestor in graph.ancestors(target) {
            if candidate(ancestor) && placed.insert(ancestor) {
                candidates.push(BundleCandidate {
                    node: ancestor,
                    layer: BundleLayer::Ancestors,
                    via: Vec::new(),
                    working_answer: None,
                });
            }
        }
    }

    // Layer 4: `verifies` in, or a feature-scoped ID section linked in.
    for (&node, pairs) in &linked {
        if placed.contains(&node) {
            continue;
        }
        let feature_section = is_feature_section(graph, node);
        let via = sorted(
            pairs
                .iter()
                .filter(|(link_type, direction)| {
                    *direction == Direction::In && (feature_section || link_type == VERIFIES)
                })
                .cloned(),
        );
        if via.is_empty() {
            continue;
        }
        placed.insert(node);
        candidates.push(BundleCandidate {
            node,
            layer: BundleLayer::Criteria,
            via,
            working_answer: None,
        });
    }

    // Layers 6, 7, 8 by the table.
    for layer in [
        BundleLayer::Decisions,
        BundleLayer::Neighbours,
        BundleLayer::Terms,
    ] {
        for (&node, pairs) in &linked {
            if placed.contains(&node) {
                continue;
            }
            let via = sorted(
                pairs
                    .iter()
                    .filter(|(link_type, direction)| {
                        BUNDLE_LINK_TYPES.iter().any(|&(rule, wanted, way)| {
                            rule == layer && wanted == link_type && way == *direction
                        })
                    })
                    .cloned(),
            );
            if via.is_empty() {
                continue;
            }
            placed.insert(node);
            candidates.push(BundleCandidate {
                node,
                layer,
                via,
                working_answer: None,
            });
        }
    }

    BundleLayers {
        targets: kept,
        merged,
        candidates,
    }
}

/// An ID section whose prefix the scheme scopes to a feature.
fn is_feature_section(graph: &SpecGraph<'_>, at: NodeAt) -> bool {
    if at.ord == 0 {
        return false;
    }
    graph
        .node(at)
        .and_then(|node| node.id.as_deref())
        .and_then(|id| id.split_once('-'))
        .and_then(|(prefix, _)| graph.scheme().prefix(prefix))
        .is_some_and(|spec| spec.scope == IdScope::Feature)
}

/// By type, then `in` before `out`; each pair once.
fn sorted(pairs: impl Iterator<Item = (String, Direction)>) -> Vec<(String, Direction)> {
    let mut pairs: Vec<(String, Direction)> = pairs.collect();
    pairs.sort_by(|a, b| (&a.0, a.1.as_str()).cmp(&(&b.0, b.1.as_str())));
    pairs.dedup();
    pairs
}
