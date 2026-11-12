//! The subset read of `spec show`, `show --links` and `spec bundle`
//! (the `store-reads` spec): the rows of the files around
//! the asked nodes and those files' bytes, never the whole corpus.
//!
//! 1. The freshness step (`crate::freshness::step`: the stat-keyed walk,
//!    or a hub's trusted index); its warnings print once.
//! 2. An attempt: one index snapshot (T3) computes the subset ("Data": a
//!    REF's closure, the parent chains and edges of the shown nodes, a
//!    bundle's candidate edges and parent chains) and decodes each of its
//!    files once with its stored BLAKE3; the snapshot ends before any file
//!    is read.
//! 3. Every subset file is read once through the freshness step's
//!    `WorkingTree` and compared with its row (T5, R3): its bytes hash to
//!    the row's BLAKE3, or the row has no hash and the read fails now too.
//! 4. All match: the caller builds its answer over the subset
//!    (`SpecGraph::from_rows`). Some mismatch: `update_paths` of exactly
//!    those paths (its report dropped), then one more attempt. A second
//!    mismatch, a snapshot that is not indexed or any store error is the
//!    walk: the caller's whole read, rerun from the freshness step with
//!    only the messages of its classification. A mismatch, rows that do
//!    not cover the subset, or a store error in an attempt doubts the
//!    project's index (the `read-trust` spec, R13).
//!
//! Nothing here prints: an attempt's answer is built only once it matched,
//! so a dropped attempt leaves no message.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use specengine_core::Paths;
use specengine_core::check::{
    CheckFile, CheckInput, GraphRows, LinkState, NodeAt, Resolution, Resolver, SpecGraph,
    bundle_linked, nodes_within,
};
use specengine_model::{IdScheme, Reference};
use specengine_store::{
    IndexSnapshot, IndexWriter as _, Listing, Probed, Source, SqliteIndex, StatKey, StoreError,
    StoredEdge, StoredParent, WorkingTree,
};

use crate::corpus::{locate, row_matches};
use crate::freshness::{self, step};
use crate::location::{OpenIndex, open_index};
use crate::project::ProjectRoot;
use crate::show::{Target, pick};
use crate::{CliError, Env, Message, store_error};

use seams::Event;

/// The link type a candidate's held edges are kept by (layer 2's working
/// answer).
const WORKING_ANSWER: &str = "working_answer";

/// The link type a candidate's ending edges are kept by (an answered
/// question leaves layer 2).
const ANSWERS: &str = "answers";

/// What a subset read is for.
pub(crate) enum Wanted<'r> {
    /// `spec show REF`, `--links` when `links`.
    Show { target: &'r Target, links: bool },
    /// `spec bundle REF…`: every REF classified, in order.
    Bundle { asked: &'r [(&'r str, Target)] },
}

/// How a subset read ended.
pub(crate) enum Subset {
    /// Every file matched: the answer's input.
    Read(Box<SubsetRead>),
    /// The walk: `fallback` after attempts (else forced, test-only).
    Walk { fallback: bool },
}

/// A matched attempt: its files, its rows, its reads.
pub(crate) struct SubsetRead {
    /// The handle the freshness step updated.
    pub open: OpenIndex,
    /// The subset in path order; each parsed file with the bytes read.
    pub input: CheckInput,
    /// The edges and parents the answer's graph is built from.
    pub rows: GraphRows,
    /// The reads not moved into a parsed file of `input`, by path: a read
    /// error, or the bytes of a file without a parse (`show`'s holder loop
    /// takes them from here, the others from `input`: one read per file).
    pub reads: BTreeMap<String, io::Result<Vec<u8>>>,
    /// The freshness step's warnings.
    pub warnings: Vec<Message>,
}

/// The read: the freshness step, then at most two attempts.
pub(crate) fn read(
    env: &Env,
    project: &ProjectRoot,
    wanted: Wanted<'_>,
) -> Result<Subset, CliError> {
    if seams::forced_walk() {
        return Ok(Subset::Walk { fallback: false });
    }
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let mut open = open_index(env, project)?;
    let warnings = step(&mut open.index, project, &tree)?;
    seams::fresh();
    for attempt in 1..=2 {
        seams::note(Event::Attempt);
        // A store error, or rows that do not cover the subset (a
        // corrupted row): the walk.
        let Ok(Some(planned)) = plan(&open.index, project, &wanted) else {
            freshness::doubt(&project.root);
            break;
        };
        let paths: Vec<String> = planned
            .input
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect();
        seams::snapshot(attempt, &paths);
        let mut reads = BTreeMap::new();
        let mut mismatched: Vec<String> = Vec::new();
        for (done, path) in paths.iter().enumerate() {
            if done > 0 {
                seams::between(attempt, done, path);
            }
            seams::note(Event::Read(path));
            let read = tree.read(path);
            let stored = planned.blake3.get(path).and_then(Option::as_deref);
            // R3: the bytes hash to the row's BLAKE3, or the row has no
            // hash and the read fails now too.
            let same = match &read {
                Ok(bytes) => row_matches(stored, bytes),
                Err(_) => stored.is_none(),
            };
            if !same {
                mismatched.push(path.clone());
            }
            reads.insert(path.clone(), read);
        }
        if mismatched.is_empty() {
            return Ok(Subset::Read(Box::new(matched(
                open, planned, reads, warnings,
            ))));
        }
        freshness::doubt(&project.root);
        if attempt == 2 {
            break;
        }
        seams::note(Event::Update(&mismatched));
        let named: Vec<&str> = mismatched.iter().map(String::as_str).collect();
        let source = Listed {
            tree: &tree,
            counted: true,
        };
        if open
            .index
            .update_paths(&source, &project.config.scheme, &named)
            .is_err()
        {
            break;
        }
    }
    seams::note(Event::Walk);
    Ok(Subset::Walk { fallback: true })
}

/// Runs the caller's walk, `fallback` marking it a subset read's (no stage
/// hook fires inside it).
pub(crate) fn walk<T>(fallback: bool, run: impl FnOnce() -> T) -> T {
    if fallback { seams::walking(run) } else { run() }
}

/// The matched attempt's answer input: each parsed file with its bytes,
/// moved out of `reads`.
fn matched(
    open: OpenIndex,
    planned: Planned,
    mut reads: BTreeMap<String, io::Result<Vec<u8>>>,
    warnings: Vec<Message>,
) -> SubsetRead {
    let mut input = planned.input;
    for file in &mut input.files {
        if file.parsed.is_none() || !matches!(reads.get(&file.path), Some(Ok(_))) {
            continue;
        }
        if let (Some(Ok(bytes)), Some(parsed)) = (reads.remove(&file.path), file.parsed.take()) {
            let path = std::mem::take(&mut file.path);
            *file = CheckFile::parsed(path, bytes, parsed);
        }
    }
    SubsetRead {
        open,
        input,
        rows: planned.rows,
        reads,
        warnings,
    }
}

/// An attempt's subset: its files decoded (path order) with their hashes,
/// and its rows.
struct Planned {
    input: CheckInput,
    blake3: BTreeMap<String, Option<String>>,
    rows: GraphRows,
}

/// One attempt's subset from one snapshot, ended before it returns;
/// `None` when its rows do not cover it ([`covers`]).
fn plan(
    index: &SqliteIndex,
    project: &ProjectRoot,
    wanted: &Wanted<'_>,
) -> Result<Option<Planned>, StoreError> {
    let snapshot = index.snapshot()?;
    let mut gather = Gather::new(&snapshot, &project.config.scheme, &project.config.paths);
    match wanted {
        Wanted::Show { target, links } => gather.show(target, *links)?,
        Wanted::Bundle { asked } => gather.bundle(asked)?,
    }
    let planned = gather.planned();
    drop(snapshot);
    Ok(covers(&planned).then_some(planned))
}

/// Every row names nodes of the subset's files: each edge's holder in its
/// citing file, each resolved edge's and parent's ends. Always so, since
/// the plan decodes every file a row names and the store keeps the edge
/// rows a function of the file rows; a corrupted row (R7) fails it, and the
/// walk, which resolves again, answers instead of a row no file holds.
fn covers(planned: &Planned) -> bool {
    let files = &planned.input.files;
    let holds = |path: &str, ord: usize| {
        files
            .binary_search_by(|file| file.path.as_str().cmp(path))
            .ok()
            .and_then(|at| files[at].parsed.as_ref())
            .is_some_and(|parsed| ord < parsed.nodes.len())
    };
    let ends = |state: LinkState, ends: &[(String, usize)]| {
        state != LinkState::Resolved
            || (!ends.is_empty() && ends.iter().all(|(path, ord)| holds(path, *ord)))
    };
    planned
        .rows
        .edges
        .iter()
        .all(|edge| holds(&edge.path, edge.holder) && ends(edge.state, &edge.ends))
        && planned
            .rows
            .parents
            .iter()
            .all(|parent| holds(&parent.path, 0) && ends(parent.state, &parent.ends))
}

/// The files an attempt decoded, each once, and the rows it kept.
struct Gather<'s, 'i> {
    snapshot: &'s IndexSnapshot<'i>,
    scheme: &'s IdScheme,
    paths: &'s Paths,
    /// Decode order; sorted by path at the end.
    input: CheckInput,
    /// Path -> its index in `input`.
    at: BTreeMap<String, usize>,
    blake3: BTreeMap<String, Option<String>>,
    /// Every path asked for, stored or not: none is decoded twice.
    tried: BTreeSet<String>,
    edges: BTreeMap<(String, usize), StoredEdge>,
    parents: BTreeMap<String, StoredParent>,
}

impl<'s, 'i> Gather<'s, 'i> {
    fn new(snapshot: &'s IndexSnapshot<'i>, scheme: &'s IdScheme, paths: &'s Paths) -> Self {
        Self {
            snapshot,
            scheme,
            paths,
            input: CheckInput::default(),
            at: BTreeMap::new(),
            blake3: BTreeMap::new(),
            tried: BTreeSet::new(),
            edges: BTreeMap::new(),
            parents: BTreeMap::new(),
        }
    }

    /// Decodes the stored files among `paths` not decoded yet.
    fn decode<'p>(&mut self, paths: impl IntoIterator<Item = &'p str>) -> Result<(), StoreError> {
        let fresh: Vec<&str> = paths
            .into_iter()
            .filter(|path| self.tried.insert((*path).to_owned()))
            .collect();
        if fresh.is_empty() {
            return Ok(());
        }
        let decoded = self.snapshot.decode(&fresh)?;
        for file in decoded.input.files {
            seams::note(Event::Decoded(&file.path));
            self.at.insert(file.path.clone(), self.input.files.len());
            self.input.files.push(file);
        }
        self.blake3.extend(decoded.blake3);
        Ok(())
    }

    fn file(&self, path: &str) -> Option<&CheckFile> {
        self.at.get(path).map(|&at| &self.input.files[at])
    }

    /// A REF's closure C(REF), decoded: rounds from the empty set, each
    /// resolving over the set and adding the holders of every key read,
    /// until no file joins. The resolution's holder files, by path.
    fn closure(&mut self, reference: &Reference) -> Result<Vec<String>, StoreError> {
        let mut members: BTreeSet<String> = BTreeSet::new();
        let mut queried: BTreeSet<String> = BTreeSet::new();
        loop {
            let named: Vec<String> = members.iter().cloned().collect();
            self.decode(named.iter().map(String::as_str))?;
            let round = CheckInput {
                files: named
                    .iter()
                    .filter_map(|path| self.file(path))
                    .cloned()
                    .collect(),
                problems: Vec::new(),
            };
            let resolver = Resolver::new(&round, self.scheme, self.paths);
            let (resolution, keys) = resolver.resolve_detached_reading(reference);
            let mut grown = false;
            for key in keys {
                let text = key.text();
                if !queried.insert(text.clone()) {
                    continue;
                }
                for holder in self.snapshot.holders(&text)? {
                    grown |= members.insert(holder);
                }
            }
            if !grown {
                return Ok(match resolution {
                    Resolution::Resolved(files) => files
                        .iter()
                        .map(|&file| resolver.paths()[file].to_owned())
                        .collect(),
                    Resolution::Dangling(_) | Resolution::Skipped => Vec::new(),
                });
            }
        }
    }

    /// The files `target` names: a path's own when stored, else its
    /// closure's holders.
    fn holders(&mut self, target: &Target) -> Result<Vec<String>, StoreError> {
        match target {
            Target::Path(path) => {
                self.decode([path.as_str()])?;
                Ok(self
                    .file(path)
                    .map(|_| vec![path.clone()])
                    .unwrap_or_default())
            }
            Target::Reference(reference) => self.closure(reference),
        }
    }

    /// Keeps `edges` (each whole) whose type `keep` takes.
    fn keep(&mut self, edges: Vec<StoredEdge>, keep: impl Fn(&str) -> bool) {
        for edge in edges.into_iter().filter(|edge| keep(&edge.link_type)) {
            self.edges.insert((edge.path.clone(), edge.ord), edge);
        }
    }

    /// The edges held by or ending on each node within `(path, ord)`.
    fn node_edges(&mut self, path: &str, within: &[usize]) -> Result<(), StoreError> {
        for &node in within {
            let held = self.snapshot.held(path, node)?;
            self.keep(held, |_| true);
            let ending = self.snapshot.ending(path, node)?;
            self.keep(ending, |_| true);
        }
        Ok(())
    }

    /// Decodes every edge's citing file and every end.
    fn decode_edge_files(&mut self) -> Result<(), StoreError> {
        let named: BTreeSet<String> = self
            .edges
            .values()
            .flat_map(|edge| {
                std::iter::once(edge.path.clone())
                    .chain(edge.ends.iter().map(|(path, _)| path.clone()))
            })
            .collect();
        self.decode(named.iter().map(String::as_str))
    }

    /// The stored parents of `path`'s chain, by file, as `ancestors`
    /// climbs it: a section climbs to its document, a document goes to its
    /// stored parent's first end, until none or one repeats; every end's
    /// file decoded.
    fn chain(&mut self, path: &str) -> Result<(), StoreError> {
        let mut seen = BTreeSet::from([path.to_owned()]);
        let mut file = path.to_owned();
        while let Some(parent) = self.snapshot.parent(&file)? {
            let next = (parent.state == LinkState::Resolved)
                .then(|| parent.ends.first().map(|(path, _)| path.clone()))
                .flatten();
            let ends: Vec<String> = parent.ends.iter().map(|(path, _)| path.clone()).collect();
            self.parents.insert(file.clone(), parent);
            self.decode(ends.iter().map(String::as_str))?;
            match next {
                Some(next) if seen.insert(next.clone()) => file = next,
                _ => break,
            }
        }
        Ok(())
    }

    /// `show REF [--links]`: C(REF); for each holder with a shown node, its
    /// chain of stored parents; with links, for each shown node n and each
    /// m within it, the edges m holds and those ending on m, and their
    /// files.
    fn show(&mut self, target: &Target, links: bool) -> Result<(), StoreError> {
        let holders = self.holders(target)?;
        for path in &holders {
            let Some(parsed) = self.file(path).and_then(|file| file.parsed.as_ref()) else {
                continue;
            };
            let picked = if parsed.nodes.is_empty() {
                Vec::new()
            } else {
                pick(parsed, target)
            };
            if links {
                let within: BTreeSet<usize> = picked
                    .iter()
                    .flat_map(|&ord| nodes_within(&parsed.nodes, ord))
                    .collect();
                let within: Vec<usize> = within.into_iter().collect();
                self.node_edges(path, &within)?;
            }
            if !picked.is_empty() {
                self.chain(path)?;
            }
        }
        if links {
            self.decode_edge_files()?;
        }
        Ok(())
    }

    /// `bundle REF…`: the union of the closures; for each kept target, the
    /// rows of `--links`; for each linked candidate, its held
    /// `working_answer` edges and the `answers` edges ending within it; for
    /// each kept target, the stored parents of its chain. Stops after the
    /// closures when a REF names nothing (the answer is that REF's reason).
    fn bundle(&mut self, asked: &[(&str, Target)]) -> Result<(), StoreError> {
        for (_, target) in asked {
            self.holders(target)?;
        }
        // The targets, located as the answer locates them.
        let (targets, kept) = {
            let graph =
                SpecGraph::from_rows(&self.input, self.scheme, self.paths, GraphRows::default());
            let mut targets: Vec<NodeAt> = Vec::new();
            for (written, target) in asked {
                match locate(&graph, target, written) {
                    Ok(Ok(nodes)) => targets.extend(nodes),
                    _ => return Ok(()),
                }
            }
            let (kept, _) = bundle_linked(&graph, &targets);
            (named_nodes(&graph, &targets), within_of(&graph, &kept))
        };
        for (path, _, within) in &kept {
            self.node_edges(path, within)?;
        }
        self.decode_edge_files()?;

        // The candidates linked to the kept targets, over their rows.
        let linked = {
            let rows = GraphRows {
                edges: self.edges.values().cloned().collect(),
                parents: Vec::new(),
            };
            let graph = SpecGraph::from_rows(&self.input, self.scheme, self.paths, rows);
            let targets: Vec<NodeAt> = targets
                .iter()
                .filter_map(|(path, ord)| {
                    graph.file_of(path).map(|file| NodeAt { file, ord: *ord })
                })
                .collect();
            let (_, linked) = bundle_linked(&graph, &targets);
            within_of(&graph, &linked)
        };
        for (path, ord, within) in &linked {
            let held = self.snapshot.held(path, *ord)?;
            self.keep(held, |link_type| link_type == WORKING_ANSWER);
            for &node in within {
                let ending = self.snapshot.ending(path, node)?;
                self.keep(ending, |link_type| link_type == ANSWERS);
            }
        }
        self.decode_edge_files()?;

        // Each kept target's chain.
        for (path, _, _) in &kept {
            self.chain(path)?;
        }
        Ok(())
    }

    /// The subset, path order, and its rows.
    fn planned(self) -> Planned {
        let mut input = self.input;
        input.files.sort_by(|a, b| a.path.cmp(&b.path));
        Planned {
            input,
            blake3: self.blake3,
            rows: GraphRows {
                edges: self.edges.into_values().collect(),
                parents: self.parents.into_values().collect(),
            },
        }
    }
}

/// Graph nodes as (path, position).
fn named_nodes(graph: &SpecGraph<'_>, nodes: &[NodeAt]) -> Vec<(String, usize)> {
    nodes
        .iter()
        .map(|at| (graph.paths()[at.file].to_owned(), at.ord))
        .collect()
}

/// Graph nodes as (path, position, the positions within each).
fn within_of(graph: &SpecGraph<'_>, nodes: &[NodeAt]) -> Vec<(String, usize, Vec<usize>)> {
    nodes
        .iter()
        .map(|&at| {
            (
                graph.paths()[at.file].to_owned(),
                at.ord,
                graph.within(at).iter().map(|node| node.ord).collect(),
            )
        })
        .collect()
}

/// A `WorkingTree` as the store reads it, counting its listings for the
/// tests (`counted`: not a read's first freshness step), and every read
/// and stat it gives the store.
pub(crate) struct Listed<'t> {
    pub tree: &'t WorkingTree,
    pub counted: bool,
}

impl Source for Listed<'_> {
    fn root(&self) -> &Path {
        self.tree.root()
    }

    fn paths(&self) -> &Paths {
        self.tree.paths()
    }

    fn list(&self) -> io::Result<Listing> {
        if self.counted {
            seams::note(Event::List);
        }
        self.tree.list()
    }

    fn probe(&self, path: &str) -> bool {
        self.tree.probe(path)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        seams::note(Event::StoreRead(path));
        self.tree.read(path)
    }

    fn stat(&self, path: &str) -> Option<StatKey> {
        seams::note(Event::Stat);
        self.tree.stat(path)
    }

    fn is_dir(&self, path: &str) -> bool {
        self.tree.is_dir(path)
    }

    fn probe_paths(&self, paths: &[&str]) -> Vec<Probed> {
        self.tree.probe_paths(paths)
    }
}

/// The test seams of the reads (the `store-reads` spec, "CLI"): a
/// thread-local stage hook and counters, both `#[cfg(test)]`; outside the
/// tests every call here is empty.
pub(crate) mod seams {
    #[cfg(test)]
    use std::cell::{Cell, RefCell};

    /// What the counters count.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) enum Event<'e> {
        /// `indexed_input` or `decode_all` in `corpus`.
        WholeDecode,
        /// A `Source::list`, but a read's first freshness step's.
        List,
        /// A resolver over the whole input.
        WholeResolver,
        /// A graph over the whole input.
        WholeGraph,
        /// A file a subset attempt decoded.
        Decoded(&'e str),
        /// A file a subset attempt read.
        Read(&'e str),
        /// A subset `update_paths` call's paths.
        Update(&'e [String]),
        Attempt,
        /// A subset read fell back to the walk.
        Walk,
        /// A file `read_bytes` parsed again.
        Parse(&'e str),
        /// A file `Listed` read for the store (a freshness step's update,
        /// or a subset read's `update_paths`).
        StoreRead(&'e str),
        /// A `Listed::stat`.
        Stat,
        /// An upstream read's `events_of_types` (task spec `upstream-signal`
        /// US-9).
        UpstreamEvents,
        /// An upstream read's applied items (US-9).
        UpstreamItems,
        /// A delivery read's card list (task spec `delivery-analytics`
        /// DA-10).
        DeliveryCards,
        /// A delivery read's `events_of_types` (DA-10).
        DeliveryEvents,
        /// A delivery read's item list (DA-10).
        DeliveryItems,
        /// A delivery read's plan groups built from the walked files
        /// (DA-10).
        DeliveryPlan,
        /// A catalog read's trace reading (task spec `direction-catalog`
        /// DC-8).
        CatalogTrace,
        /// A catalog read's open proposal list (DC-8).
        CatalogItems,
        /// A catalog read's git walk, for these top-relative paths (DC-8).
        CatalogWalk(&'e [String]),
    }

    /// Where the stage hook is called.
    #[cfg(test)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum Stage {
        /// (a) Once per read, after its first freshness step (a subset
        /// read's, or a whole read's in `corpus::indexed_in`); never inside
        /// a subset read's walk.
        Fresh,
        /// (b) The attempt's snapshot ended; no file read yet. `paths`: the
        /// attempt's subset in read order (path order).
        Snapshot { attempt: usize, paths: Vec<String> },
        /// (c) Between two file reads: `done` files read, `next` read next.
        Between {
            attempt: usize,
            done: usize,
            next: String,
        },
    }

    /// What this thread's reads did since the last take.
    #[cfg(test)]
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(crate) struct ReadCounts {
        pub whole_decodes: usize,
        pub lists: usize,
        pub whole_resolvers: usize,
        pub whole_graphs: usize,
        pub decoded: Vec<String>,
        pub read: Vec<String>,
        pub updates: Vec<Vec<String>>,
        pub attempts: usize,
        /// Fallback walks only.
        pub walks: usize,
        pub parses: Vec<String>,
        /// The files `Listed` read for the store, in order.
        pub store_reads: Vec<String>,
        /// `Listed::stat` calls.
        pub stats: usize,
        /// Upstream `events_of_types` calls.
        pub upstream_events: usize,
        /// Upstream applied-item reads.
        pub upstream_items: usize,
        /// Delivery card lists.
        pub delivery_cards: usize,
        /// Delivery `events_of_types` calls.
        pub delivery_events: usize,
        /// Delivery item lists.
        pub delivery_items: usize,
        /// Delivery plan builds.
        pub delivery_plans: usize,
        /// Catalog trace readings.
        pub catalog_traces: usize,
        /// Catalog open proposal lists.
        pub catalog_items: usize,
        /// Catalog git walks, each its paths.
        pub catalog_walks: Vec<Vec<String>>,
    }

    /// The stage hook, if any.
    #[cfg(test)]
    type Hook = Option<Box<dyn FnMut(Stage)>>;

    #[cfg(test)]
    thread_local! {
        static HOOK: RefCell<Hook> = const { RefCell::new(None) };
        static COUNTS: RefCell<ReadCounts> = RefCell::new(ReadCounts::default());
        static WALKING: Cell<bool> = const { Cell::new(false) };
        static FORCE_WALK: Cell<bool> = const { Cell::new(false) };
    }

    /// Sets (or, `None`, clears) this thread's stage hook.
    #[cfg(test)]
    #[allow(dead_code)] // The store-reads tests set it.
    pub(crate) fn set_read_hook(hook: Hook) {
        HOOK.with_borrow_mut(|slot| *slot = hook);
    }

    /// This thread's counts since the last take; they start again at 0.
    #[cfg(test)]
    #[allow(dead_code)] // The store-reads tests take them.
    pub(crate) fn take_read_counts() -> ReadCounts {
        COUNTS.with_borrow_mut(std::mem::take)
    }

    /// Subset reads on this thread go straight to the walk (no freshness
    /// step of their own, no attempt): the differential's oracle.
    #[cfg(test)]
    #[allow(dead_code)] // The store-reads tests set it.
    pub(crate) fn set_force_walk(on: bool) {
        FORCE_WALK.set(on);
    }

    #[cfg(test)]
    pub(crate) fn forced_walk() -> bool {
        FORCE_WALK.get()
    }

    #[cfg(not(test))]
    pub(crate) fn forced_walk() -> bool {
        false
    }

    /// Inside a subset read's walk.
    #[cfg(test)]
    pub(crate) fn in_walk() -> bool {
        WALKING.get()
    }

    #[cfg(not(test))]
    pub(crate) fn in_walk() -> bool {
        false
    }

    /// Runs a subset read's walk, marked as one.
    #[cfg(test)]
    pub(crate) fn walking<T>(run: impl FnOnce() -> T) -> T {
        let before = WALKING.replace(true);
        let value = run();
        WALKING.set(before);
        value
    }

    #[cfg(not(test))]
    pub(crate) fn walking<T>(run: impl FnOnce() -> T) -> T {
        run()
    }

    /// Counts `event`.
    #[cfg(test)]
    pub(crate) fn note(event: Event<'_>) {
        COUNTS.with_borrow_mut(|counts| match event {
            Event::WholeDecode => counts.whole_decodes += 1,
            Event::List => counts.lists += 1,
            Event::WholeResolver => counts.whole_resolvers += 1,
            Event::WholeGraph => counts.whole_graphs += 1,
            Event::Decoded(path) => counts.decoded.push(path.to_owned()),
            Event::Read(path) => counts.read.push(path.to_owned()),
            Event::Update(paths) => counts.updates.push(paths.to_vec()),
            Event::Attempt => counts.attempts += 1,
            Event::Walk => counts.walks += 1,
            Event::Parse(path) => counts.parses.push(path.to_owned()),
            Event::StoreRead(path) => counts.store_reads.push(path.to_owned()),
            Event::Stat => counts.stats += 1,
            Event::UpstreamEvents => counts.upstream_events += 1,
            Event::UpstreamItems => counts.upstream_items += 1,
            Event::DeliveryCards => counts.delivery_cards += 1,
            Event::DeliveryEvents => counts.delivery_events += 1,
            Event::DeliveryItems => counts.delivery_items += 1,
            Event::DeliveryPlan => counts.delivery_plans += 1,
            Event::CatalogTrace => counts.catalog_traces += 1,
            Event::CatalogItems => counts.catalog_items += 1,
            Event::CatalogWalk(paths) => counts.catalog_walks.push(paths.to_vec()),
        });
    }

    #[cfg(not(test))]
    pub(crate) fn note(_: Event<'_>) {}

    /// Calls the hook, taken out of its slot meanwhile (it may set
    /// another).
    #[cfg(test)]
    fn fire(stage: Stage) {
        let Some(mut hook) = HOOK.with_borrow_mut(Option::take) else {
            return;
        };
        hook(stage);
        HOOK.with_borrow_mut(|slot| {
            if slot.is_none() {
                *slot = Some(hook);
            }
        });
    }

    /// Stage (a).
    #[cfg(test)]
    pub(crate) fn fresh() {
        if !WALKING.get() {
            fire(Stage::Fresh);
        }
    }

    #[cfg(not(test))]
    pub(crate) fn fresh() {}

    /// Stage (b).
    #[cfg(test)]
    pub(crate) fn snapshot(attempt: usize, paths: &[String]) {
        fire(Stage::Snapshot {
            attempt,
            paths: paths.to_vec(),
        });
    }

    #[cfg(not(test))]
    pub(crate) fn snapshot(_: usize, _: &[String]) {}

    /// Stage (c).
    #[cfg(test)]
    pub(crate) fn between(attempt: usize, done: usize, next: &str) {
        fire(Stage::Between {
            attempt,
            done,
            next: next.to_owned(),
        });
    }

    #[cfg(not(test))]
    pub(crate) fn between(_: usize, _: usize, _: &str) {}
}
