//! What the whole reads (`spec tree`, `spec graph`, the walk of `spec show
//! --links` and `spec bundle`) read: the index, updated first (freshness,
//! the file list), as the check's input; then every parsed file read again
//! from the working tree, the source of lines and written forms: a file
//! whose bytes hash to its row keeps its decoded parse, any other is parsed
//! again by the check's parser (a file that cannot be read or parsed keeps
//! its indexed parse, its lines 1, with a warning). The live rule, the
//! count of what it left out, and a `REF` or `ROOT` located in the graph.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};

use serde::Serialize;
use specengine_core::check::{CheckFile, CheckInput, Endpoint, NodeAt, SpecGraph, Standing};
use specengine_store::{Decoded, Source as _, SpecIndex as _, SqliteIndex, WorkingTree, b3_hash};

use crate::location::open_index;
use crate::project::ProjectRoot;
use crate::refresh::refresh_in;
use crate::show::{Target, project_qualified};
use crate::subset::seams::{self, Event};
use crate::{CliError, Env, Message, store_error};

/// The index after its update, as the check's input; `with_bytes`: each
/// parsed file read and parsed again as it is now (the graph commands'
/// lines and written forms).
pub(crate) fn indexed(
    env: &Env,
    project: &ProjectRoot,
    messages: &mut Vec<Message>,
    with_bytes: bool,
) -> Result<CheckInput, CliError> {
    let mut open = open_index(env, project)?;
    indexed_in(&mut open.index, project, messages, with_bytes)
}

/// [`indexed`] over a handle the caller opened (and reads again: the
/// code index's refresh after it, one database handle for both).
pub(crate) fn indexed_in(
    index: &mut SqliteIndex,
    project: &ProjectRoot,
    messages: &mut Vec<Message>,
    with_bytes: bool,
) -> Result<CheckInput, CliError> {
    if with_bytes {
        return Ok(indexed_read(index, project, messages)?.0);
    }
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let (_, warnings) = refresh_in(index, project, &tree, false)?;
    messages.extend(warnings);
    seams::fresh();
    seams::note(Event::WholeDecode);
    index.indexed_input().map_err(store_error)
}

/// [`indexed_in`] with bytes, and the paths whose bytes it attached: each
/// read once, so a reader that prints a file of the input takes its bytes
/// from there and prints what the graph was built from.
pub(crate) fn indexed_read(
    index: &mut SqliteIndex,
    project: &ProjectRoot,
    messages: &mut Vec<Message>,
) -> Result<(CheckInput, BTreeSet<String>), CliError> {
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let (_, warnings) = refresh_in(index, project, &tree, false)?;
    messages.extend(warnings);
    seams::fresh();
    let Decoded { mut input, blake3 } = decoded_whole(index)?;
    let read = read_bytes(&mut input, &blake3, &tree, project, messages);
    Ok((input, read))
}

/// [`indexed`] without bytes, with each file's stored hash from the same
/// snapshot: what [`add_bytes`] needs later (`spec task`'s package).
pub(crate) fn indexed_decoded(
    env: &Env,
    project: &ProjectRoot,
    messages: &mut Vec<Message>,
) -> Result<Decoded, CliError> {
    let mut open = open_index(env, project)?;
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let (_, warnings) = refresh_in(&mut open.index, project, &tree, false)?;
    messages.extend(warnings);
    seams::fresh();
    decoded_whole(&open.index)
}

/// Every stored file with its hash, from one snapshot.
fn decoded_whole(index: &SqliteIndex) -> Result<Decoded, CliError> {
    let snapshot = index.snapshot().map_err(store_error)?;
    seams::note(Event::WholeDecode);
    snapshot.decode_all().map_err(store_error)
}

/// What `with_bytes` adds to an input [`indexed_decoded`] gave without it,
/// `blake3` its hashes: each parsed file read and parsed again as it is
/// now, unless its bytes match its row (Q7), so the two reads give one
/// input (the package's plan is `spec roadmap`'s).
pub(crate) fn add_bytes(
    input: &mut CheckInput,
    blake3: &BTreeMap<String, Option<String>>,
    project: &ProjectRoot,
    messages: &mut Vec<Message>,
) -> Result<(), CliError> {
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    read_bytes(input, blake3, &tree, project, messages);
    Ok(())
}

/// The bytes read now hash to the row's BLAKE3 (`stored`, lower-case hex;
/// `None`: the row has none): the row's parse is the parse of `bytes`.
pub(crate) fn row_matches(stored: Option<&str>, bytes: &[u8]) -> bool {
    stored.is_some_and(|stored| b3_hash(bytes).strip_prefix("b3:") == Some(stored))
}

/// Reads every parsed file again, whatever its size, so the parse, size and
/// bytes agree and no edit made after the update leaves a stale span: a
/// file whose bytes hash to its row (`blake3`, the input's own snapshot)
/// keeps its decoded parse, which is the parse of those bytes (the store's
/// projection contract; the `store-reads` spec, Q7); any other is
/// parsed again as it is now. A file that cannot be read or parsed keeps
/// its indexed parse, its lines shown as 1. The paths given their bytes.
fn read_bytes(
    input: &mut CheckInput,
    blake3: &BTreeMap<String, Option<String>>,
    tree: &WorkingTree,
    project: &ProjectRoot,
    messages: &mut Vec<Message>,
) -> BTreeSet<String> {
    let scheme = &project.config.scheme;
    let mut read = BTreeSet::new();
    for file in &mut input.files {
        if file.parsed.is_none() {
            continue;
        }
        let bytes = match tree.read(&file.path) {
            Ok(bytes) => bytes,
            Err(error) => {
                messages.push(Message::Warning(format!(
                    "cannot read `{}`: {error}; its lines show as 1",
                    file.path
                )));
                continue;
            }
        };
        let stored = blake3.get(&file.path).and_then(Option::as_deref);
        if row_matches(stored, &bytes)
            && let Some(parsed) = file.parsed.take()
        {
            let path = std::mem::take(&mut file.path);
            read.insert(path.clone());
            *file = CheckFile::parsed(path, bytes, parsed);
            continue;
        }
        seams::note(Event::Parse(&file.path));
        let path = file.path.clone();
        let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(&path, &bytes, scheme)
        }));
        match parsed {
            Ok(parsed) => {
                read.insert(path.clone());
                *file = CheckFile::parsed(path, bytes, parsed);
            }
            Err(_) => messages.push(Message::Warning(format!(
                "the spec parser failed on `{path}`; its lines show as 1"
            ))),
        }
    }
    read
}

/// What the live rule left out: generated and Tier 3 files' nodes or
/// links.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct LeftOut {
    pub generated: usize,
    pub tier3: usize,
}

impl LeftOut {
    pub(crate) fn add(&mut self, standing: Standing) {
        match standing {
            Standing::Generated => self.generated += 1,
            Standing::Tier3 => self.tier3 += 1,
            Standing::Live => {}
        }
    }

    /// `; left out: <g> generated, <t> archived (--archive)`, zero parts
    /// omitted; empty when nothing was left out.
    pub(crate) fn suffix(&self) -> String {
        let mut parts = Vec::new();
        if self.generated > 0 {
            parts.push(format!("{} generated", self.generated));
        }
        if self.tier3 > 0 {
            parts.push(format!("{} archived (--archive)", self.tier3));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("; left out: {}", parts.join(", "))
        }
    }
}

/// The live rule of one call: live files; Tier 3 ones with `--archive`;
/// never generated ones; always the files of the nodes asked for.
pub(crate) struct Admission<'g, 'a> {
    graph: &'g SpecGraph<'a>,
    archive: bool,
    asked: BTreeSet<usize>,
}

impl<'g, 'a> Admission<'g, 'a> {
    pub(crate) fn new(
        graph: &'g SpecGraph<'a>,
        archive: bool,
        asked: impl IntoIterator<Item = usize>,
    ) -> Self {
        Self {
            graph,
            archive,
            asked: asked.into_iter().collect(),
        }
    }

    pub(crate) fn admits(&self, file: usize) -> bool {
        if self.asked.contains(&file) {
            return true;
        }
        match self.graph.standing(file) {
            Standing::Live => true,
            Standing::Tier3 => self.archive,
            Standing::Generated => false,
        }
    }
}

/// The nodes `target` names in the graph, by (path, position); `Ok(Err)`:
/// none, with the reason (exit 1).
pub(crate) fn locate(
    graph: &SpecGraph<'_>,
    target: &Target,
    written: &str,
) -> Result<Result<Vec<NodeAt>, String>, CliError> {
    match target {
        Target::Path(path) => Ok(match graph.file_of(path) {
            Some(file) => match graph.document(file) {
                Some(document) => Ok(vec![document]),
                None => Err(format!(
                    "`{path}` has no node: it could not be read or is not UTF-8"
                )),
            },
            None => Err(format!(
                "`{path}` is no indexed document: not under the `[paths]` roots, \
                 excluded, or missing"
            )),
        }),
        Target::Reference(reference) => match graph.locate(reference, written) {
            Endpoint::Nodes(mut nodes) => {
                nodes.sort_unstable();
                nodes.dedup();
                Ok(Ok(nodes))
            }
            Endpoint::Dangling(reason) => Ok(Err(format!("`{written}` {reason}"))),
            Endpoint::Skipped | Endpoint::Unchecked => Err(project_qualified(written)),
        },
    }
}

/// The warning for a `REF` or `ROOT` with several holders, all taken.
pub(crate) fn holders_warning(
    graph: &SpecGraph<'_>,
    written: &str,
    nodes: &[NodeAt],
    taken: &str,
) -> Option<Message> {
    if nodes.len() < 2 {
        return None;
    }
    let cited: Vec<String> = nodes
        .iter()
        .map(|&at| format!("{}:{}", graph.paths()[at.file], graph.line(at)))
        .collect();
    Some(Message::Warning(format!(
        "`{written}` has {} holders, {taken}: {}",
        nodes.len(),
        cited.join(", ")
    )))
}

/// `--depth N`: an integer of 0 or more.
pub(crate) fn depth_of(depth: Option<i64>) -> Result<Option<usize>, CliError> {
    depth
        .map(|depth| {
            usize::try_from(depth).map_err(|_| {
                CliError::spec(format!(
                    "--depth {depth}: the depth is an integer of 0 or more"
                ))
            })
        })
        .transpose()
}
