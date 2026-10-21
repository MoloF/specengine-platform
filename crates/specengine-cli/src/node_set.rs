//! The node sets `spec verify` scopes by (task spec `verify-tests`, "The
//! node set"; shared with `verify-task` and `verify-node`): a set of nodes
//! of the corpus the index gave, and the rule a row follows into it.
//!
//! - **A task's set** ([`task_set`]): the task's `targets`, `criteria`
//!   refs and `affected_nodes` columns as stored now, in any state (never
//!   its snapshot), each resolved now as `spec show` resolves a REF, plus
//!   every section each contains; a ref naming nothing now is a note,
//!   `verify: <ref> names nothing now`, and is skipped.
//! - **A row in a set** ([`NodeSet::row_nodes`]): the set nodes a marker of
//!   the row is listed under as `spec refs` lists it (resolved, an alias,
//!   each node of an ambiguous ID, a look-alike under its Latin fix's
//!   node), and the node of its lock entry when that is a set node's
//!   canonical ID. Only the builder adds sections or parents.
//! - **The task named** ([`named_task`]): `--task T` of this repository,
//!   read from the queue `spec task show` opens; every refusal exits 2
//!   with stdout empty (`--tests`, and plain `--task`: task spec
//!   `verify-task`).
//! - **A row naming a node** ([`Named::any`]): the same rule over every
//!   node of the corpus; plain `--task` counts the rows naming none in a
//!   note instead of keeping them (task spec `verify-task`).

use std::collections::{BTreeMap, BTreeSet};

use specengine_core::check::canonical_id;
use specengine_core::task::task_prefix_clash;
use specengine_model::IdScheme;
use specengine_store::Task;

use crate::proposals::QueueContext;
use crate::refs::{Corpus, NodeKey};
use crate::show::{Target, classify};
use crate::task::{find_task, not_a_task, written_task};
use crate::{CliError, Message};

/// Why a stored ref's `@rev` would be ignored (no stored ref has one).
const REV_IGNORED: &str = "a task's node set reads no revision";

/// Nodes of one corpus, each with its canonical cited ID.
#[derive(Debug, Clone, Default)]
pub(crate) struct NodeSet {
    /// Each node's canonical ID (`slug/ID` when feature-scoped); `None`
    /// for a node without one.
    nodes: BTreeMap<NodeKey, Option<String>>,
}

impl NodeSet {
    /// The set of `keys` over `corpus`.
    pub(crate) fn of(
        corpus: &Corpus<'_>,
        scheme: &IdScheme,
        keys: impl IntoIterator<Item = NodeKey>,
    ) -> Self {
        let nodes = keys
            .into_iter()
            .map(|key| (key, canonical_of(corpus, scheme, key)))
            .collect();
        Self { nodes }
    }

    /// Its nodes, by (path, position).
    pub(crate) fn keys(&self) -> impl Iterator<Item = NodeKey> + '_ {
        self.nodes.keys().copied()
    }

    /// The node's canonical ID, when it is in the set and has one.
    pub(crate) fn canonical(&self, key: NodeKey) -> Option<&str> {
        self.nodes.get(&key).and_then(Option::as_deref)
    }

    /// "A row in a set": the set nodes a row belongs to, through any of
    /// `markers` (the IDs its markers write) as `spec refs` lists them, and
    /// through `entry` (its lock entry's node, a canonical ID).
    pub(crate) fn row_nodes(
        &self,
        corpus: &mut Corpus<'_>,
        markers: &[&str],
        entry: Option<&str>,
    ) -> BTreeSet<NodeKey> {
        let mut found = BTreeSet::new();
        for written in markers {
            found.extend(
                corpus
                    .judge(written)
                    .nodes
                    .into_iter()
                    .filter(|key| self.nodes.contains_key(key)),
            );
        }
        if let Some(entry) = entry {
            found.extend(
                self.nodes
                    .iter()
                    .filter(|(_, canonical)| canonical.as_deref() == Some(entry))
                    .map(|(key, _)| *key),
            );
        }
        found
    }
}

/// The canonical cited IDs of every node of a corpus: whether a row names
/// any node now ("A row in a set" with the set every node).
#[derive(Debug)]
pub(crate) struct Named {
    ids: BTreeSet<String>,
}

impl Named {
    /// Every node of `corpus` with an ID, by its canonical cited ID.
    pub(crate) fn of(corpus: &Corpus<'_>, scheme: &IdScheme) -> Self {
        let mut ids = BTreeSet::new();
        for (file, checked) in corpus.files.iter().enumerate() {
            let Some(parsed) = &checked.parsed else {
                continue;
            };
            ids.extend(
                (0..parsed.nodes.len())
                    .filter_map(|ord| canonical_of(corpus, scheme, NodeKey { file, ord })),
            );
        }
        Self { ids }
    }

    /// Whether a row names a node now: one of `markers` names one as
    /// `spec refs` lists it, or `entry` is a node's canonical ID.
    pub(crate) fn any(
        &self,
        corpus: &mut Corpus<'_>,
        markers: &[&str],
        entry: Option<&str>,
    ) -> bool {
        markers
            .iter()
            .any(|written| !corpus.judge(written).nodes.is_empty())
            || entry.is_some_and(|entry| self.ids.contains(entry))
    }
}

/// The task `--task` names, of this repository: exit 2, stdout empty,
/// canon `tasks`, "Commands"' texts, for an `[ids]` taking `T`, an ID that
/// is none, a look-alike, an unknown or another repository's task.
pub(crate) fn named_task(context: &QueueContext, written: &str) -> Result<Task, CliError> {
    if let Some(clash) = task_prefix_clash(&context.project.config.scheme) {
        return Err(CliError::spec(format!(
            "{}: {clash}",
            context.project.config_label
        )));
    }
    let Some(id) = written_task(written)? else {
        return Err(CliError::spec(not_a_task(written)));
    };
    find_task(context, &id)?.map_err(CliError::spec)
}

/// The canonical cited ID of `key`; `None` for a node without an ID.
pub(crate) fn canonical_of(corpus: &Corpus<'_>, scheme: &IdScheme, key: NodeKey) -> Option<String> {
    let path = corpus.resolver.paths().get(key.file)?;
    let id = corpus
        .files
        .get(key.file)?
        .parsed
        .as_ref()?
        .nodes
        .get(key.ord)?
        .id
        .as_deref()?;
    Some(canonical_id(&corpus.resolver, scheme, path, id, None))
}

/// A task's set (see the module documentation): the refs of its columns,
/// in column order, each resolved now; the notes of those naming nothing.
pub(crate) fn task_set(
    task: &Task,
    corpus: &Corpus<'_>,
    scheme: &IdScheme,
    messages: &mut Vec<Message>,
) -> NodeSet {
    let refs = task
        .targets
        .iter()
        .chain(
            task.criteria
                .iter()
                .filter_map(|criterion| criterion.reference.as_ref()),
        )
        .chain(task.affected_nodes.iter());
    let mut keys: BTreeSet<NodeKey> = BTreeSet::new();
    let mut gone: BTreeSet<&str> = BTreeSet::new();
    for written in refs {
        let named = resolved_now(corpus, scheme, written);
        if named.is_empty() {
            if gone.insert(written.as_str()) {
                messages.push(Message::Note(format!(
                    "verify: {written} names nothing now"
                )));
            }
            continue;
        }
        for key in named {
            keys.insert(key);
            keys.extend(contained(corpus, key));
        }
    }
    NodeSet::of(corpus, scheme, keys)
}

/// The nodes `written` names now, as `spec show` resolves a REF; none when
/// it names nothing (or cannot be read as a REF of this project).
fn resolved_now(corpus: &Corpus<'_>, scheme: &IdScheme, written: &str) -> Vec<NodeKey> {
    let written = written.trim();
    let mut ignored = Vec::new();
    let target: Target = match classify(written, scheme, &mut ignored, REV_IGNORED) {
        Ok(Ok(target)) => target,
        Ok(Err(_)) | Err(_) => return Vec::new(),
    };
    match corpus.target_nodes(&target, written) {
        Ok(Ok(nodes)) => nodes,
        Ok(Err(_)) | Err(_) => Vec::new(),
    }
}

/// Every section of `key`'s file whose span lies within `key`'s (a
/// document: every section of it).
pub(crate) fn contained(corpus: &Corpus<'_>, key: NodeKey) -> Vec<NodeKey> {
    let Some(parsed) = corpus
        .files
        .get(key.file)
        .and_then(|file| file.parsed.as_ref())
    else {
        return Vec::new();
    };
    let Some(span) = parsed.nodes.get(key.ord).map(|node| node.span) else {
        return Vec::new();
    };
    parsed
        .nodes
        .iter()
        .enumerate()
        .skip(1)
        .filter(|&(ord, section)| ord != key.ord && span.contains(section.span))
        .map(|(ord, _)| NodeKey {
            file: key.file,
            ord,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Task spec `verify-tests` (8a-1) RD-11, "A row in a set" (canon
    //! `spec-verify-tests`, "The node set"), the lock-entry half
    //! `--tests` never reaches (its rows pass no entry): a row belongs to
    //! the set node whose canonical ID its lock entry's `node` is (a
    //! feature-scoped node by `slug/ID`), never by a bare, alias or unknown
    //! ID, never to a node outside the set; the marker half and the entry
    //! half add up. Over a copy of `fixtures/verify-tests` indexed in a
    //! scratch `HOME` (no git needed). The look-alike is written from an
    //! escape (sources are Latin-only). M: the lock-entry half dropped (an
    //! entry adds no node), or an entry resolved as a marker is.

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use specengine_core::check::CheckInput;

    use super::{NodeSet, canonical_of};
    use crate::corpus::indexed;
    use crate::project::{ProjectRoot, discover_with_text};
    use crate::refs::{Corpus, NodeKey};
    use crate::{Env, Globals};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-node-set-{name}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(fs::canonicalize(&path).expect("canonical scratch"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("copy target");
        for entry in fs::read_dir(from).expect("fixture").flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().expect("type").is_dir() {
                copy_dir(&entry.path(), &target);
            } else if entry.file_name() != "expected.json" {
                fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
            }
        }
    }

    /// The fixture's copy, its project and its indexed corpus.
    struct Indexed {
        _scratch: Scratch,
        project: ProjectRoot,
        input: CheckInput,
    }

    impl Indexed {
        fn new(name: &str) -> Self {
            let scratch = Scratch::new(name);
            let root = scratch.0.join("repo");
            let home = scratch.0.join("home");
            fs::create_dir_all(&home).expect("home");
            copy_dir(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/verify-tests"),
                &root,
            );
            let env = Env {
                cwd: root.clone(),
                home: Some(home.into_os_string()),
                xdg_data_home: None,
            };
            let globals = Globals {
                root: Some(root),
                config: None,
            };
            let (project, _) = discover_with_text(&env, &globals).expect("the project");
            let mut messages = Vec::new();
            let input = indexed(&env, &project, &mut messages, false).expect("the index");
            Self {
                _scratch: scratch,
                project,
                input,
            }
        }
    }

    /// Every node of the corpus that has a canonical ID, by it.
    fn by_canonical(corpus: &Corpus<'_>, fixture: &Indexed) -> BTreeMap<String, NodeKey> {
        let scheme = &fixture.project.config.scheme;
        let mut found = BTreeMap::new();
        for (file, checked) in corpus.files.iter().enumerate() {
            let count = checked
                .parsed
                .as_ref()
                .map_or(0, |parsed| parsed.nodes.len());
            for ord in 0..count {
                let key = NodeKey { file, ord };
                if let Some(id) = canonical_of(corpus, scheme, key) {
                    assert!(found.insert(id.clone(), key).is_none(), "{id} twice");
                }
            }
        }
        found
    }

    /// The document node of the feature file (no ID).
    fn feature_document(corpus: &Corpus<'_>) -> NodeKey {
        let file = corpus
            .resolver
            .paths()
            .iter()
            .position(|path| path.ends_with("glaze-check.md"))
            .expect("the feature file");
        NodeKey { file, ord: 0 }
    }

    #[test]
    fn lock_entry_row_follows_its_canonical_node() {
        let fixture = Indexed::new("entry");
        let scheme = &fixture.project.config.scheme;
        let mut corpus = Corpus::new(&fixture.input, scheme, &fixture.project.config.paths);
        let all = by_canonical(&corpus, &fixture);
        let glaze_id = all
            .keys()
            .find(|id| id.ends_with("/GLZ-01"))
            .cloned()
            .unwrap_or_else(|| panic!("a feature-scoped GLZ-01: {all:?}"));
        assert_eq!(glaze_id, "glaze-check/GLZ-01", "its canonical ID");
        let (hold, glaze, cone, ramp) = (
            all["KILN-HOLD"],
            all[&glaze_id],
            all["CONE-01"],
            all["KILN-RAMP"],
        );
        let document = feature_document(&corpus);
        assert_eq!(canonical_of(&corpus, scheme, document), None, "no ID");
        let set = NodeSet::of(&corpus, scheme, [hold, glaze, cone, document]);
        let mut rows =
            |markers: &[&str], entry: Option<&str>| set.row_nodes(&mut corpus, markers, entry);
        let keys = |keys: &[NodeKey]| keys.iter().copied().collect::<BTreeSet<_>>();

        // The entry half alone.
        assert_eq!(
            rows(&[], Some("KILN-HOLD")),
            keys(&[hold]),
            "a set node's ID"
        );
        assert_eq!(rows(&[], Some(&glaze_id)), keys(&[glaze]), "slug/ID");
        assert_eq!(rows(&[], Some("GLZ-01")), keys(&[]), "a bare scoped ID");
        assert_eq!(
            rows(&[], Some("PYRO-01")),
            keys(&[]),
            "an alias is no entry"
        );
        assert_eq!(rows(&[], Some("KILN-RAMP")), keys(&[]), "outside the set");
        assert_eq!(rows(&[], Some("KILN-NONE")), keys(&[]), "unknown");
        assert_eq!(rows(&[], Some("glaze-check")), keys(&[]), "a slug");
        assert_eq!(rows(&[], Some("")), keys(&[]), "empty");
        assert_eq!(rows(&[], None), keys(&[]), "nothing");

        // The marker half and the entry half add up.
        assert_eq!(
            rows(&["PYRO-01"], Some("KILN-HOLD")),
            keys(&[cone, hold]),
            "an alias marker and an entry"
        );
        assert_eq!(
            rows(&["KILN-H\u{041E}LD"], Some("KILN-HOLD")),
            keys(&[hold]),
            "a look-alike marker and its fix's entry: one node"
        );
        assert_eq!(
            rows(&["KILN-RAMP"], Some(&glaze_id)),
            keys(&[glaze]),
            "a marker outside the set adds nothing"
        );
        assert!(!set.keys().any(|key| key == ramp), "the scenario");
    }
}
