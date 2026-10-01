//! What `spec tree`, `spec graph` and `spec show --links` read: the index,
//! updated first (freshness, the file list), as the check's input; then
//! every parsed file read again from the working tree and parsed again by
//! the check's parser, the source of lines and written forms (a file that
//! cannot be read or parsed keeps its indexed parse, its lines 1, with a
//! warning). The live rule, the count of what it left out, and a `REF` or
//! `ROOT` located in the graph.

use std::collections::BTreeSet;
use std::panic::{self, AssertUnwindSafe};

use serde::Serialize;
use specengine_core::check::{CheckFile, CheckInput, Endpoint, NodeAt, SpecGraph, Standing};
use specengine_store::{Source as _, SpecIndex as _, WorkingTree};

use crate::location::open_index;
use crate::project::ProjectRoot;
use crate::refresh::refresh;
use crate::show::{Target, project_qualified};
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
    let (_, warnings) = refresh(&mut open.index, project, false)?;
    messages.extend(warnings);
    let mut input = open.index.indexed_input().map_err(store_error)?;
    if with_bytes {
        let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
        read_bytes(&mut input, &tree, project, messages);
    }
    Ok(input)
}

/// Reads every parsed file again and parses it as it is now, whatever its
/// size: the parse, size and bytes then agree, so no edit made after the
/// update leaves a stale span. A file that cannot be read or parsed keeps
/// its indexed parse, its lines shown as 1.
fn read_bytes(
    input: &mut CheckInput,
    tree: &WorkingTree,
    project: &ProjectRoot,
    messages: &mut Vec<Message>,
) {
    let scheme = &project.config.scheme;
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
        let path = file.path.clone();
        let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(&path, &bytes, scheme)
        }));
        match parsed {
            Ok(parsed) => *file = CheckFile::parsed(path, bytes, parsed),
            Err(_) => messages.push(Message::Warning(format!(
                "the spec parser failed on `{path}`; its lines show as 1"
            ))),
        }
    }
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
