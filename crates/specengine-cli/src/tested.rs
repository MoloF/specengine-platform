//! A `[[tested]]` block's bound hash and its state on a read (task specs
//! `tested-evidence`, "Bound hash", "The state", "Plain verify";
//! `tested-reads`): the one hashing function the test run's write and
//! every read call ([`bound_hash`]: store `b3_hash` of core's
//! `bound_bytes`), the `tested` entries of verify and of the reads that
//! show a node's code ([`tested_entries`], each a [`TestedRow`] judged by
//! core's one `tested_state`) and their one line ([`tested_line`]). Never
//! stored, never a finding, a class, a count, a hint, a verdict or an
//! exit; nothing is written.
//!
//! On a read a block's node is the one whose canonical ID is the block's
//! (several: the first in path and source order); its bindings are its
//! marker bindings in the given tree now, sorted by accept's own rows
//! ([`crate::lock::node_bindings`] over [`Source::Disk`], every judged
//! node in one pass): a binding whose entry's `file_blob` is the file's
//! blob on disk takes the entry's digests, any other is hashed by accept's
//! functions from the tree's text, each file parsed once. A failure to
//! judge the blocks is the caller's note, never a run cause.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;

use specengine_code::markers_in;
use specengine_core::check::CheckInput;
use specengine_core::lock::{
    EntryCode, EntryKey, LevelCode, Lock, TestedNode, TestedState, Verified, bound_bytes,
    tested_state,
};
use specengine_core::{norm_input, revision};
use specengine_model::TestedRow;
use specengine_store::{VerifyGit, b3_hash};

use crate::code_walk::CodeTree;
use crate::lock::{Bindings, Facts, NodeAt, NodeFiles, Source, node_bindings};
use crate::project::ProjectRoot;
use crate::refs::{Corpus, NodeKey};
use crate::verify::unprefixed;
use crate::verify_rows::node_id;
use crate::{Message, one_line};

/// A block's `bound_hash` over a node's bindings and their digests: store
/// `b3_hash` of core's `bound_bytes`. The one function the run's write and
/// every read call.
pub(crate) fn bound_hash<'a>(
    bindings: impl IntoIterator<Item = (&'a EntryKey, &'a LevelCode)>,
) -> String {
    b3_hash(bound_bytes(bindings).as_bytes())
}

/// [`bound_hash`] of a node's lockable bindings ([`Bindings::fresh`]).
pub(crate) fn fresh_bound_hash(fresh: &BTreeMap<EntryKey, Verified>) -> String {
    bound_hash(
        fresh
            .iter()
            .filter_map(|(key, verified)| match &verified.code {
                EntryCode::Levels(code) => Some((key, code)),
                EntryCode::Whole(_) => None,
            }),
    )
}

/// A node's first binding not lockable: keyed (in key order), then keyless
/// (in row order), in its words; `None`: every one is lockable.
pub(crate) fn first_refusal(bindings: &Bindings) -> Option<String> {
    bindings.unlockable.values().next().cloned().or_else(|| {
        bindings
            .keyless
            .iter()
            .next()
            .map(|(_, state)| state.clone())
    })
}

/// The one note of a run whose blocks could not be judged (its `tested`
/// `null`): `tested: <cause>`.
pub(crate) fn tested_note(cause: &str) -> Message {
    Message::Note(tested_note_text(cause))
}

/// [`tested_note`]'s text, for a read's one note.
pub(crate) fn tested_note_text(cause: &str) -> String {
    format!("tested: {}", one_line(cause))
}

/// One block's line, verify's and every read's, without an indent or a
/// line end: `tested | <state>[ <category>] | <node>`, no path, date or
/// commit in it.
pub(crate) fn tested_line(row: &TestedRow) -> String {
    let state = match &row.category {
        Some(category) => format!("{} {category}", row.state),
        None => row.state.clone(),
    };
    format!("tested | {} | {}", one_line(&state), one_line(&row.node))
}

/// Whether a block's state now is `verified`.
pub(crate) fn is_verified(row: &TestedRow) -> bool {
    row.state == TestedState::Verified.as_str()
}

/// The blocks of `lock` whose node `keep` keeps, by node bytes, each
/// judged over `tree` (the walk, or a node set's partial tree that holds
/// every marker file of its nodes) and the spec corpus `input` now.
/// Nothing is read for a node `node_gone`, `rev_unknown` or `hash_null`.
/// The other nodes' marker files are hashed by one `hash-object`, and their
/// bindings sorted in one pass (each file read and parsed once, by one
/// parser; lockability needs their units' levels, `unverifiable` winning
/// over `stale`); a block's bound bytes and hash are made only past its
/// node's lockability and (R, H). `Err`: why the blocks cannot be judged
/// (git refusing `hash-object`, the grammar not loading): the caller's
/// `tested` is `null` with a note, never a run cause.
pub(crate) fn tested_entries(
    project: &ProjectRoot,
    input: &CheckInput,
    tree: &CodeTree,
    lock: &Lock,
    git: &VerifyGit,
    keep: &dyn Fn(&str) -> bool,
) -> Result<Vec<TestedRow>, String> {
    let blocks: Vec<_> = lock.tested.iter().filter(|(node, _)| keep(node)).collect();
    if blocks.is_empty() {
        return Ok(Vec::new());
    }
    let scheme = &project.config.scheme;
    let mut corpus = Corpus::new(input, scheme, &project.config.paths);

    // Each block's node now: its holder, R and H.
    let wanted: BTreeSet<&str> = blocks.iter().map(|(node, _)| node.as_str()).collect();
    let mut holders: BTreeMap<&str, NodeKey> = BTreeMap::new();
    for (file, checked) in corpus.files.iter().enumerate() {
        let Some(parsed) = &checked.parsed else {
            continue;
        };
        for ord in 0..parsed.nodes.len() {
            let at = NodeKey { file, ord };
            if let Some(id) = node_id(&corpus, project, at)
                && let Some(&node) = wanted.get(id.as_str())
            {
                holders.entry(node).or_insert(at);
            }
        }
    }
    let now: BTreeMap<&str, (NodeKey, Option<u32>, Option<String>)> = holders
        .iter()
        .map(|(&node, &at)| {
            let (rev, hash) = node_now(&corpus, at);
            (node, (at, rev, hash))
        })
        .collect();

    // The marker files of each node judged past its revision and hash, and
    // their blobs on disk.
    let judged: BTreeSet<NodeKey> = now
        .values()
        .filter(|(_, rev, hash)| rev.is_some() && hash.is_some())
        .map(|(at, _, _)| *at)
        .collect();
    let marker_files = marker_files(tree, &mut corpus, &judged);
    let paths: Vec<&str> = marker_files
        .values()
        .flatten()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let blobs: BTreeMap<String, String> = if paths.is_empty() {
        BTreeMap::new()
    } else {
        let oids = git.hash_files(&paths).map_err(|cause| cause.message)?;
        paths
            .iter()
            .map(|path| (*path).to_owned())
            .zip(oids)
            .collect()
    };

    // Every judged node's bindings, in one pass over their marker files.
    let judged: Vec<(&str, NodeKey, u32, &str)> = now
        .iter()
        .filter_map(|(&node, (at, rev, hash))| Some((node, *at, (*rev)?, hash.as_deref()?)))
        .collect();
    let holders_at: Vec<NodeAt> = judged
        .iter()
        .map(|&(node, at, _, _)| NodeAt {
            key: at,
            path: corpus
                .resolver
                .paths()
                .get(at.file)
                .map(|path| (*path).to_owned())
                .unwrap_or_default(),
            id: String::new(),
            canonical: node.to_owned(),
        })
        .collect();
    let facts: Vec<Facts<'_>> = judged
        .iter()
        .map(|&(_, _, rev, spec_hash)| Facts {
            rev,
            spec_hash,
            commit: "",
            today: "",
        })
        .collect();
    let none = BTreeSet::new();
    let nodes: Vec<NodeFiles<'_>> = judged
        .iter()
        .zip(&holders_at)
        .zip(&facts)
        .map(|((&(_, at, _, _), node), facts)| NodeFiles {
            node,
            facts,
            files: marker_files.get(&at).unwrap_or(&none),
        })
        .collect();
    let sorted = node_bindings(
        &Source::Disk { blobs: &blobs },
        tree,
        &nodes,
        &mut corpus,
        lock,
    )
    .map_err(|error| unprefixed(&error))?;
    let bindings_of: BTreeMap<&str, &Bindings> = judged
        .iter()
        .map(|&(node, _, _, _)| node)
        .zip(&sorted)
        .collect();

    let mut entries = Vec::with_capacity(blocks.len());
    for (node, block) in blocks {
        let found = now.get(node.as_str());
        let state_node = match found {
            None => TestedNode::Gone,
            Some((_, rev, hash)) => TestedNode::Found {
                rev: *rev,
                norm_hash: hash.as_deref(),
            },
        };
        let bindings = bindings_of.get(node.as_str()).copied();
        // Both asked only past R and H, where the bindings were sorted; the
        // bound hash only past lockability and (R, H).
        let Ok(state) = tested_state::<Infallible>(
            block,
            state_node,
            || Ok(bindings.and_then(first_refusal)),
            || {
                Ok(bindings
                    .map(|bindings| fresh_bound_hash(&bindings.fresh))
                    .unwrap_or_default())
            },
        );
        entries.push(TestedRow {
            node: node.clone(),
            state: state.as_str().to_owned(),
            category: state.category().map(str::to_owned),
            rev: block.rev,
            node_rev: found.and_then(|(_, rev, _)| *rev),
            commit: block.commit.clone(),
            verified_at: block.verified_at.clone(),
        });
    }
    Ok(entries)
}

/// R and H of the node at `at` now: its revision, and the `norm_hash` of
/// the parse of the bytes read.
fn node_now(corpus: &Corpus<'_>, at: NodeKey) -> (Option<u32>, Option<String>) {
    let Some(file) = corpus.files.get(at.file) else {
        return (None, None);
    };
    let Some(parsed) = &file.parsed else {
        return (None, None);
    };
    let rev = revision(parsed, at.ord).number();
    let hash = norm_input(parsed, &file.bytes, at.ord).map(|input| b3_hash(&input));
    (rev, hash)
}

/// Every file of `tree` holding a marker (any relation) that names one of
/// `nodes`, per node, as accept finds them.
fn marker_files(
    tree: &CodeTree,
    corpus: &mut Corpus<'_>,
    nodes: &BTreeSet<NodeKey>,
) -> BTreeMap<NodeKey, BTreeSet<String>> {
    let mut files: BTreeMap<NodeKey, BTreeSet<String>> = BTreeMap::new();
    if nodes.is_empty() {
        return files;
    }
    let comments = tree
        .rust
        .iter()
        .flat_map(|file| {
            file.binding
                .comments
                .iter()
                .map(move |comment| (file.path.as_str(), file.text.get(comment.range.clone())))
        })
        .chain(tree.ron.iter().flat_map(|file| {
            file.anchors
                .iter()
                .map(move |anchor| (file.path.as_str(), file.text.get(anchor.comment.clone())))
        }));
    for (path, text) in comments {
        let Some(text) = text else {
            continue;
        };
        for marker in markers_in(text) {
            for key in corpus.judge(&marker.id).nodes {
                if nodes.contains(&key) {
                    files.entry(key).or_default().insert(path.to_owned());
                }
            }
        }
    }
    files
}
