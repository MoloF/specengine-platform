//! The store-reads task spec (step 12d of the scale reads), the CLI unit
//! half, behind the
//! `#[cfg(test)]` seams of `crate::subset::seams` (the stage hook, the read
//! counters, the forced walk) and the test-only walk entries `show_walk`
//! and `bundle_walk`:
//! - SR-0 "Differential": the subset read's outcome (text, JSON, messages)
//!   equals the walk's on the same index state, for every node ID and path
//!   of every configured fixture, and for its REF errors.
//! - AC-12: the files a read decodes and reads are exactly the subset the
//!   oracle computes from the whole graph by "Data", and no whole decode,
//!   `Source::list`, whole resolver or whole graph runs after the
//!   freshness step.
//! - AC-7 "The swap": a same-size version swapped in at stage (b), its
//!   mtime restored to the nanosecond, is answered whole, never mixed.
//! - SR-1: five edits at each of the stages (a), (b), (c), each answered as
//!   a second read with no hook, with one `update_paths` of exactly the
//!   mismatched path; an editing hook at every attempt gives two attempts,
//!   then the walk, within a 10 s deadline.
//! - SR-2: SQL at stage (a): another hash and a NULL hash retry once; a
//!   stale stamp and a deleted worktree row walk.
//! - SR-3: warm whole reads parse nothing in `read_bytes`; one file edited
//!   at stage (a) is re-parsed alone and its fresh parse used.
//!
//! Every test works on a scratch copy of a fixture under
//! `std::env::temp_dir()`, with a scratch `HOME`, removed on drop; the
//! repository and the fixtures are only read. Non-Latin characters are
//! Unicode escapes.
//!
//! This file is a CLI source, so the genre rule (`tests/genre.rs`) holds
//! for it too: no project word is written here. Every `spec-a` name the
//! swap, the edits and the SQL cases use ([`Names`]) is read at run time
//! from the copy's `[paths]` and from its files.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use specengine_core::ProjectConfig;
use specengine_core::check::{
    CheckInput, Edge, Endpoint, NodeAt, Parent, Resolver, SpecGraph, bundle_linked, given_keys,
    is_tier3_file,
};
use specengine_model::{Reference, grammar};
use specengine_store::{GitEnv, WorkingTree, check_input};

use crate::subset::seams::{self, Stage};
use crate::{
    BundleRequest, CliError, Env, FlowRequest, Globals, GraphRequest, ImpactRequest, Outcome,
    ShowOutcome, ShowRequest, TreeRequest, VerifiedTree, VerifyRequest, render_json, render_text,
};

// ------------------------------------------------------------------ the scratch

/// The configured fixtures: every `fixtures/*/specengine.toml`.
const FIXTURES: [&str; 21] = [
    "anchor-states",
    "card-press",
    "code-a",
    "code-bevy",
    "code-index",
    "corpus-mini",
    "flow-aside",
    "flow-gaps",
    "flow-hire",
    "flow-relay",
    "impact",
    "parent-ref",
    "roadmap-a",
    "roadmap-b",
    "spec-a",
    "spec-b",
    "spec-c",
    "studio-lamp",
    "trace-almanac",
    "verify-ecs",
    "verify-tests",
];

/// The budget that leaves nothing out.
const UNBOUNDED: i64 = 4_294_967_295;

/// The `spec-a` names the swap, the edits and the SQL cases use, read from
/// the copy: its `[paths]` roles and its files' parses.
#[derive(Clone, Debug)]
struct Names {
    /// The subset file: `movement/sprint.md` under `[paths] spec`.
    sprint: String,
    /// Its document's ID: the REF every case reads.
    target: String,
    /// Its `parent:`, as written.
    parent: String,
    /// The ID of `game.md` under `[paths] spec`: the other version's
    /// `parent:`, shorter than [`Names::parent`].
    other_parent: String,
    /// A file citing the target in its front-matter: `movement/stamina.md`.
    citing: String,
    /// Its document's ID (the REF of SR-3's `graph` and `impact`).
    citing_id: String,
    /// An end of one of the target's mentions: `A/A-101.md` under
    /// `[paths] records`.
    end: String,
}

impl Names {
    fn of(place: &Place) -> Self {
        let config = place.config();
        let spec = config.paths.spec.clone();
        let records = config.paths.records.clone();
        let id_of = |relative: &str| -> String {
            let bytes = fs::read(place.path(relative)).expect("a named file");
            specengine_core::parse(relative, &bytes, &config.scheme)
                .document()
                .and_then(|document| document.id.clone())
                .unwrap_or_else(|| panic!("premise: {relative} has a document ID"))
        };
        let sprint = format!("{spec}/movement/sprint.md");
        let citing = format!("{spec}/movement/stamina.md");
        let parent = place
            .text(&sprint)
            .lines()
            .find_map(|line| line.strip_prefix("parent: ").map(str::to_owned))
            .expect("premise: the target has a `parent:`");
        let names = Self {
            target: id_of(&sprint),
            other_parent: id_of(&format!("{spec}/game.md")),
            citing_id: id_of(&citing),
            end: format!("{records}/A/A-101.md"),
            sprint,
            parent,
            citing,
        };
        assert!(
            names.other_parent.len() < names.parent.len(),
            "premise: the other parent is shorter"
        );
        names
    }
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A scratch directory, canonical, removed on drop (permissions restored
/// first).
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-cli-store-reads-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(fs::canonicalize(&dir).expect("canonical"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        restore_permissions(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn restore_permissions(dir: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if is_dir {
            restore_permissions(&path);
        } else {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
        }
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a copy directory");
    for entry in fs::read_dir(from).expect("a fixture directory").flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a fixture file copied");
        }
    }
}

/// A scratch copy of one fixture with its own `HOME`, and what every call
/// over it takes.
struct Place {
    _scratch: Scratch,
    root: PathBuf,
    env: Env,
    globals: Globals,
    git: GitEnv,
}

impl Place {
    fn new(fixture: &str) -> Self {
        let scratch = Scratch::new(fixture);
        let root = scratch.0.join("copy");
        copy_dir(&fixtures_dir().join(fixture), &root);
        let root = fs::canonicalize(&root).expect("the copy");
        let home = scratch.0.join("home");
        fs::create_dir_all(&home).expect("a home");
        Self {
            env: Env {
                cwd: root.clone(),
                home: Some(home.as_os_str().to_owned()),
                xdg_data_home: None,
            },
            globals: Globals {
                root: Some(root.clone()),
                config: None,
            },
            git: GitEnv::new(root.clone(), [("HOME", home.as_os_str().to_owned())]),
            root,
            _scratch: scratch,
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn text(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).expect("a copy file")
    }

    fn config(&self) -> ProjectConfig {
        ProjectConfig::from_toml(&self.text("specengine.toml")).expect("the config")
    }

    /// The fixture has a `[code]` table: its bundles are D7's whole reads.
    fn has_code(&self) -> bool {
        self.text("specengine.toml")
            .lines()
            .any(|line| line.trim() == "[code]")
    }

    /// The fixture opens an index (it has a `[project] slug`).
    fn indexable(&self) -> bool {
        self.config().slug().is_ok()
    }

    /// The index database.
    fn db(&self) -> PathBuf {
        let slug = self.config().slug().expect("a slug").to_owned();
        crate::location::data_dir(&self.env)
            .expect("the data directory")
            .join(format!("{slug}.db"))
    }

    fn show_request(&self, reference: &str, links: bool, archive: bool) -> ShowRequest {
        ShowRequest {
            reference: reference.to_owned(),
            links,
            archive,
            bindings: false,
            history: false,
            git: self.git.clone(),
        }
    }

    fn bundle_request(&self, reference: &str, budget: Option<i64>) -> BundleRequest {
        BundleRequest {
            references: vec![reference.to_owned()],
            budget,
            git: self.git.clone(),
        }
    }
}

/// `statements` by the system's `sqlite3` on `db`; its stdout.
fn sql(db: &Path, statements: &str) -> String {
    let program = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(program)
        .env_clear()
        .args(["-cmd", ".timeout 5000"])
        .arg(db)
        .arg(statements)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {statements}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 sqlite3 output")
}

/// The hook installed until drop.
struct Hooked;

impl Hooked {
    fn new(hook: impl FnMut(Stage) + 'static) -> Self {
        seams::set_read_hook(Some(Box::new(hook)));
        Self
    }
}

impl Drop for Hooked {
    fn drop(&mut self) {
        seams::set_read_hook(None);
    }
}

/// The counters cleared, the hook and the forced walk off.
fn clean_seams() {
    seams::set_read_hook(None);
    seams::set_force_walk(false);
    let _ = seams::take_read_counts();
}

// ------------------------------------------------------------------ the reads

/// One read of SR-0's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Read {
    Show { links: bool, archive: bool },
    Bundle { budget: Option<i64> },
}

impl Read {
    /// The reads of one REF: `show`, `--links`, `--links --archive` where
    /// the fixture has a Tier 3 file, `bundle` at the default and at
    /// `u32::MAX`.
    fn all(tier3: bool) -> Vec<Self> {
        let mut out = vec![
            Self::Show {
                links: false,
                archive: false,
            },
            Self::Show {
                links: true,
                archive: false,
            },
        ];
        if tier3 {
            out.push(Self::Show {
                links: true,
                archive: true,
            });
        }
        out.push(Self::Bundle { budget: None });
        out.push(Self::Bundle {
            budget: Some(UNBOUNDED),
        });
        out
    }

    fn is_bundle(self) -> bool {
        matches!(self, Self::Bundle { .. })
    }
}

/// An outcome as the CLI prints it: the text, the JSON and every message,
/// or the error's exit and line.
fn shown(result: Result<Outcome, CliError>) -> String {
    match result {
        Ok(outcome) => {
            let messages = match &outcome {
                Outcome::Show(show) => show.messages.clone(),
                Outcome::Bundle(bundle) => bundle.messages.clone(),
                _ => Vec::new(),
            };
            let lines: Vec<String> = messages.iter().map(|message| message.line()).collect();
            format!(
                "{}--- json\n{}\n--- messages\n{}",
                render_text(&outcome),
                render_json(&outcome),
                lines.join("\n")
            )
        }
        Err(error) => format!("error {:?}: {}", error.exit, error.message),
    }
}

/// `read` of `reference` through the production entry (the subset read
/// where D7 does not apply).
fn answer(place: &Place, read: Read, reference: &str) -> String {
    shown(match read {
        Read::Show { links, archive } => crate::show(
            &place.env,
            &place.globals,
            &place.show_request(reference, links, archive),
        )
        .map(Outcome::Show),
        Read::Bundle { budget } => crate::bundle(
            &place.env,
            &place.globals,
            &place.bundle_request(reference, budget),
        )
        .map(Outcome::Bundle),
    })
}

/// `read` of `reference` as the walk (D5) answers it.
fn walk_answer(place: &Place, read: Read, reference: &str) -> String {
    shown(match read {
        Read::Show { links, archive } => crate::show_walk(
            &place.env,
            &place.globals,
            &place.show_request(reference, links, archive),
        )
        .map(Outcome::Show),
        Read::Bundle { budget } => crate::bundle_walk(
            &place.env,
            &place.globals,
            &place.bundle_request(reference, budget),
        )
        .map(Outcome::Bundle),
    })
}

/// The first line where two answers part.
fn first_difference(left: &str, right: &str) -> String {
    for (number, (a, b)) in left.lines().zip(right.lines()).enumerate() {
        if a != b {
            return format!("line {}:\n  got  {a:?}\n  want {b:?}", number + 1);
        }
    }
    format!(
        "{} lines got, {} wanted",
        left.lines().count(),
        right.lines().count()
    )
}

// ------------------------------------------------------------------ the corpus

/// The whole input of a copy, with bytes, and its config.
struct Whole {
    config: ProjectConfig,
    input: CheckInput,
}

impl Whole {
    fn of(place: &Place) -> Self {
        let config = place.config();
        let tree = WorkingTree::new(&place.root, &config.paths).expect("the working tree");
        let input = check_input(&tree, &config.scheme);
        Self { config, input }
    }

    fn graph(&self) -> SpecGraph<'_> {
        SpecGraph::new(&self.input, &self.config.scheme, &self.config.paths)
    }

    /// Every node ID, each once, then every path; then the REF errors,
    /// built on the first ID.
    fn references(&self) -> (Vec<String>, Vec<String>) {
        let mut ids: Vec<String> = Vec::new();
        let mut seen = BTreeSet::new();
        for file in &self.input.files {
            for node in file.parsed.iter().flat_map(|parsed| &parsed.nodes) {
                if let Some(id) = &node.id
                    && seen.insert(id.clone())
                {
                    ids.push(id.clone());
                }
            }
        }
        let mut good = ids.clone();
        good.extend(self.input.files.iter().map(|file| file.path.clone()));
        let first = ids.first().cloned().unwrap_or_else(|| "ZR-001".to_owned());
        let mut errors = vec![
            format!("{first}9"),
            format!("other:{first}"),
            "store-reads-missing.md".to_owned(),
            format!("{first}@3"),
        ];
        // A look-alike: the first Latin capital with a Cyrillic twin.
        let twins = [
            ('A', '\u{0410}'),
            ('C', '\u{0421}'),
            ('E', '\u{0415}'),
            ('M', '\u{041c}'),
            ('O', '\u{041e}'),
            ('P', '\u{0420}'),
            ('T', '\u{0422}'),
        ];
        if let Some(id) = ids.iter().find(|id| {
            id.chars()
                .any(|letter| twins.iter().any(|(latin, _)| *latin == letter))
        }) {
            let mut done = false;
            let twin: String = id
                .chars()
                .map(
                    |letter| match twins.iter().find(|(latin, _)| *latin == letter) {
                        Some((_, cyrillic)) if !done => {
                            done = true;
                            *cyrillic
                        }
                        _ => letter,
                    },
                )
                .collect();
            errors.push(twin);
        }
        (good, errors)
    }

    fn tier3(&self) -> bool {
        self.input
            .files
            .iter()
            .filter_map(|file| file.parsed.as_ref())
            .any(is_tier3_file)
    }

    /// Each file's keys ([`given_keys`]): what HOLDERS indexes.
    fn keys(&self) -> Vec<BTreeSet<String>> {
        self.input
            .files
            .iter()
            .map(|file| given_keys(file).iter().map(|key| key.text()).collect())
            .collect()
    }

    /// The reference `text` names, as `show` classifies it; `None` for a
    /// path or no reference.
    fn reference(&self, text: &str) -> Option<Reference> {
        if text.ends_with(".md") {
            return None;
        }
        grammar::parse_reference(text, 0, &self.config.scheme).map(|found| found.reference)
    }

    /// C(REF) by "Data": rounds from the empty set, each resolving over
    /// the set and adding the holders of every key read, until no file
    /// joins; a path REF's is its path, when it is stored.
    fn closure(&self, text: &str) -> BTreeSet<String> {
        let Some(reference) = self.reference(text) else {
            return self
                .input
                .files
                .iter()
                .filter(|file| file.path == text)
                .map(|file| file.path.clone())
                .collect();
        };
        let keys = self.keys();
        let mut set: BTreeSet<String> = BTreeSet::new();
        loop {
            let round = CheckInput {
                files: self
                    .input
                    .files
                    .iter()
                    .filter(|file| set.contains(&file.path))
                    .cloned()
                    .collect(),
                problems: Vec::new(),
            };
            let resolver = Resolver::new(&round, &self.config.scheme, &self.config.paths);
            let (_, read) = resolver.resolve_detached_reading(&reference);
            let mut grown = set.clone();
            for key in read {
                let key = key.text();
                for (at, file) in self.input.files.iter().enumerate() {
                    if keys[at].contains(&key) {
                        grown.insert(file.path.clone());
                    }
                }
            }
            if grown == set {
                return set;
            }
            set = grown;
        }
    }
}

/// The edge is held by `m`.
fn held_by(edge: &Edge, m: NodeAt) -> bool {
    edge.holder == m
}

/// The edge's written end holds `m` (a by-end row whose end is `m`).
fn ends_at(edge: &Edge, m: NodeAt) -> bool {
    matches!(edge.written_end(), Endpoint::Nodes(ends) if ends.contains(&m))
}

/// The edge's citing file and every end of its written end.
fn edge_files(graph: &SpecGraph<'_>, edge: &Edge, out: &mut BTreeSet<String>) {
    out.insert(graph.paths()[edge.file].to_owned());
    if let Endpoint::Nodes(ends) = edge.written_end() {
        for end in ends {
            out.insert(graph.paths()[end.file].to_owned());
        }
    }
}

/// `--links`' files of `n`: for each m in W(n), HELD(m) and ENDING(m).
fn links_files(graph: &SpecGraph<'_>, n: NodeAt, out: &mut BTreeSet<String>) {
    for m in graph.within(n) {
        for edge in graph.edges() {
            if held_by(edge, m) || ends_at(edge, m) {
                edge_files(graph, edge, out);
            }
        }
    }
}

/// The nodes `outcome` shows, in the whole graph.
fn shown_nodes(graph: &SpecGraph<'_>, outcome: &ShowOutcome) -> Vec<NodeAt> {
    outcome
        .nodes
        .iter()
        .map(|node| {
            let file = graph
                .file_of(&node.path)
                .unwrap_or_else(|| panic!("{} is in the whole graph", node.path));
            (0..graph.nodes(file).len())
                .map(|ord| NodeAt { file, ord })
                .find(|&at| {
                    graph.line(at) == node.line
                        && graph.node(at).and_then(|found| found.id.as_ref()) == node.id.as_ref()
                })
                .unwrap_or_else(|| panic!("{}:{} is a node", node.path, node.line))
        })
        .collect()
}

/// The chain's files of `target` by "Data": its file, then each
/// document's `parent:` ends, until there is none or one repeats.
fn chain_files(graph: &SpecGraph<'_>, target: NodeAt, out: &mut BTreeSet<String>) {
    let mut file = target.file;
    let mut seen = BTreeSet::new();
    while seen.insert(file) {
        out.insert(graph.paths()[file].to_owned());
        let Some(document) = graph.document(file) else {
            break;
        };
        match graph.parent(document) {
            Some(Parent::Node { node, others }) => {
                out.insert(graph.paths()[node.file].to_owned());
                for other in others {
                    out.insert(graph.paths()[other.file].to_owned());
                }
                file = node.file;
            }
            Some(Parent::Cycle { members }) => {
                for member in members {
                    out.insert(graph.paths()[member.file].to_owned());
                }
                break;
            }
            _ => break,
        }
    }
}

/// The bundle's targets of `text` in the whole graph.
fn targets(whole: &Whole, graph: &SpecGraph<'_>, text: &str) -> Vec<NodeAt> {
    match whole.reference(text) {
        None => graph
            .file_of(text)
            .and_then(|file| graph.document(file))
            .into_iter()
            .collect(),
        Some(reference) => match graph.locate(&reference, text) {
            Endpoint::Nodes(ends) => ends,
            _ => Vec::new(),
        },
    }
}

/// The subset of `read` of `text` by "Data", from the whole graph. Since
/// the flow-scenario slice (its D5), a `show` read's plan adds each
/// shown node's chain of `PARENT` rows, file by file, as `bundle` does
/// ([`chain_files`]): its `ancestors`.
fn oracle(
    whole: &Whole,
    read: Read,
    text: &str,
    outcome: Option<&ShowOutcome>,
) -> BTreeSet<String> {
    let graph = whole.graph();
    let mut out = whole.closure(text);
    match read {
        Read::Show { links, .. } => {
            for n in shown_nodes(&graph, outcome.expect("the show's outcome")) {
                chain_files(&graph, n, &mut out);
                if links {
                    links_files(&graph, n, &mut out);
                }
            }
        }
        Read::Bundle { .. } => {
            let targets = targets(whole, &graph, text);
            if targets.is_empty() {
                return out;
            }
            let (kept, linked) = bundle_linked(&graph, &targets);
            for &target in &kept {
                links_files(&graph, target, &mut out);
                chain_files(&graph, target, &mut out);
            }
            for c in linked {
                for edge in graph.edges() {
                    if held_by(edge, c) && edge.link_type == "working_answer" {
                        edge_files(&graph, edge, &mut out);
                    }
                }
                for m in graph.within(c) {
                    for edge in graph.edges() {
                        if ends_at(edge, m) && edge.link_type == "answers" {
                            edge_files(&graph, edge, &mut out);
                        }
                    }
                }
            }
        }
    }
    out
}

// ------------------------------------------------------------------ SR-0 Differential

/// SR-0 "Differential" over one fixture: every node ID and path, and the
/// REF errors, through every read; the subset read's answer equals the
/// walk's. Where D7 does not apply and the REF is the fixture's, the
/// answer came from one attempt with no walk.
fn differential(fixture: &str) {
    clean_seams();
    let place = Place::new(fixture);
    let whole = Whole::of(&place);
    let (good, errors) = whole.references();
    let tier3 = whole.tier3();
    let indexable = place.indexable();
    let mut failed: Vec<String> = Vec::new();
    let mut compared = 0usize;
    for (reference, own) in good
        .iter()
        .map(|reference| (reference, true))
        .chain(errors.iter().map(|reference| (reference, false)))
    {
        for read in Read::all(tier3) {
            let _ = seams::take_read_counts();
            let got = answer(&place, read, reference);
            let counts = seams::take_read_counts();
            let want = walk_answer(&place, read, reference);
            compared += 1;
            if got != want {
                failed.push(format!(
                    "{reference} {read:?}: {}",
                    first_difference(&got, &want)
                ));
                continue;
            }
            if !(own && indexable) {
                continue;
            }
            let whole_read = read.is_bundle() && place.has_code();
            let expected_attempts = usize::from(!whole_read);
            if counts.attempts != expected_attempts || counts.walks != 0 {
                failed.push(format!(
                    "{reference} {read:?}: {} attempts and {} walks, {expected_attempts} and 0 \
                     expected (D7 whole read: {whole_read}): {counts:?}",
                    counts.attempts, counts.walks
                ));
            }
        }
    }
    assert!(compared > 0, "{fixture}: premise: reads compared");
    assert!(
        failed.is_empty(),
        "{fixture}: {} of {compared} reads differ from the walk's:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

/// SR-0 "Differential" over alias forms, which no fixture REF reaches: a
/// `spec-a` copy whose feature document declares the legacy form `QST-31`
/// as an `aliases:` entry and whose `A-102` record declares `Q-031`, the
/// form's own ID, as one. The answers of the legacy form bare and scoped
/// by the feature's slug, the zero-padded legacy form, the ID itself, a
/// scoped criterion and a record's own alias are the walk's, through every read, each from one
/// attempt. M: a scoped REF's `path:` key dropped from the closure. (C(REF)
/// cut to one round and the bare form's key dropped are equivalent here: a
/// CLI REF never has a bare form other than its ID, look-alikes being
/// refused, so the first round, over no file, already reads every key.)
#[test]
fn sr0_differential_over_alias_forms() {
    clean_seams();
    let place = Place::new("spec-a");
    let config = place.config();
    let feature = format!("{}/stamina-tuning.md", config.paths.features);
    let record = format!("{}/A/A-102.md", config.paths.records);
    for (relative, from, to) in [
        (
            &feature,
            "class: spec\n",
            "class: spec\naliases: [QST-31]\n",
        ),
        (&record, "id: A-102\n", "id: A-102\naliases: [Q-031]\n"),
    ] {
        let text = place.text(relative);
        assert_eq!(
            text.matches(from).count(),
            1,
            "premise: {relative}: {from:?}"
        );
        fs::write(place.path(relative), text.replacen(from, to, 1)).expect("the edit");
    }
    let whole = Whole::of(&place);
    let tier3 = whole.tier3();
    // A record's own `aliases:` entry, as the fixture writes it.
    let own_alias = whole
        .input
        .files
        .iter()
        .filter(|file| file.path != feature && file.path != record)
        .filter_map(|file| {
            file.parsed
                .as_ref()?
                .document()?
                .fields
                .as_ref()?
                .aliases
                .clone()
        })
        .flatten()
        .next()
        .expect("premise: a record declares an alias");
    let references = [
        "QST-31".to_owned(),
        "QST-031".to_owned(),
        "stamina-tuning/QST-31".to_owned(),
        "stamina-tuning/QST-031".to_owned(),
        "Q-031".to_owned(),
        "stamina-tuning/Q-031".to_owned(),
        "stamina-tuning/AC-07".to_owned(),
        own_alias,
    ];
    let mut failed: Vec<String> = Vec::new();
    for reference in &references {
        for read in Read::all(tier3) {
            let _ = seams::take_read_counts();
            let got = answer(&place, read, reference);
            let counts = seams::take_read_counts();
            let want = walk_answer(&place, read, reference);
            if got != want {
                failed.push(format!(
                    "{reference} {read:?}: {}",
                    first_difference(&got, &want)
                ));
            }
            if counts.attempts != 1 || counts.walks != 0 {
                failed.push(format!(
                    "{reference} {read:?}: one attempt, no walk expected: {counts:?}"
                ));
            }
        }
    }
    assert!(
        failed.is_empty(),
        "{} alias reads differ from the walk's:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

/// SR-0 "Differential" over IDs with two holders, which no fixture has
/// (R6: the `has N holders` warning, the holder order): a `spec-a` copy
/// with byte twins of the end record of [`Names`] and of the target's
/// `parent:` document. The target, its parent's ID, the end's ID and the
/// twins' paths answer as the walk through every read, each from one
/// attempt. M: ENDING not completed by `EDGE` (a by-end row's single end).
#[test]
fn sr0_differential_over_two_holder_ids() {
    clean_seams();
    let place = Place::new("spec-a");
    let names = Names::of(&place);
    let spec = place.config().paths.spec;
    let parent_file = format!("{spec}/movement/README.md");
    let mut twins = Vec::new();
    for from in [names.end.clone(), parent_file] {
        let to = from.replace(".md", "-twin.md");
        fs::copy(place.path(&from), place.path(&to)).expect("a twin");
        twins.push(to);
    }
    let end_id = specengine_core::parse(
        &names.end,
        &fs::read(place.path(&names.end)).expect("the end"),
        &place.config().scheme,
    )
    .document()
    .and_then(|document| document.id.clone())
    .expect("premise: the end's ID");
    let whole = Whole::of(&place);
    let tier3 = whole.tier3();
    let mut references = vec![
        names.target.clone(),
        names.parent.clone(),
        end_id,
        names.end.clone(),
    ];
    references.extend(twins);
    let mut failed: Vec<String> = Vec::new();
    for reference in &references {
        for read in Read::all(tier3) {
            let _ = seams::take_read_counts();
            let got = answer(&place, read, reference);
            let counts = seams::take_read_counts();
            let want = walk_answer(&place, read, reference);
            if got != want {
                failed.push(format!(
                    "{reference} {read:?}: {}",
                    first_difference(&got, &want)
                ));
            }
            if counts.attempts != 1 || counts.walks != 0 {
                failed.push(format!(
                    "{reference} {read:?}: one attempt, no walk expected: {counts:?}"
                ));
            }
        }
    }
    assert!(
        failed.is_empty(),
        "{} two-holder reads differ from the walk's:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

// ------------------------------------------------------------------ AC-12 the subset

/// AC-12 "On every fixture node": for every node ID and path, `show`,
/// `show --links` and (where D7 does not apply) `bundle` decode and read
/// exactly the oracle's subset ("Data"), each file once, the reads in path
/// order; the whole-decode, `Source::list`, whole-resolver and whole-graph
/// counters stay 0, no `update_paths` runs. M: the resolver over
/// `indexed_input`; the subset widened by the citing files' own closures
/// (the `impact` fixture's generated `departures.md` pulling its
/// corpus).
fn subset_is_the_oracles(fixture: &str) {
    clean_seams();
    let place = Place::new(fixture);
    if !place.indexable() {
        return;
    }
    let whole = Whole::of(&place);
    let (good, _) = whole.references();
    let mut reads = vec![
        Read::Show {
            links: false,
            archive: false,
        },
        Read::Show {
            links: true,
            archive: false,
        },
    ];
    if !place.has_code() {
        reads.push(Read::Bundle { budget: None });
    }
    // The freshness step's full pass, once, so every read below is warm.
    let _ = answer(&place, reads[0], &good[0]);
    let mut failed: Vec<String> = Vec::new();
    for reference in &good {
        for &read in &reads {
            let _ = seams::take_read_counts();
            let outcome = match read {
                Read::Show { links, archive } => Some(
                    crate::show(
                        &place.env,
                        &place.globals,
                        &place.show_request(reference, links, archive),
                    )
                    .unwrap_or_else(|error| panic!("{reference}: {}", error.message)),
                ),
                Read::Bundle { budget } => {
                    let _ = crate::bundle(
                        &place.env,
                        &place.globals,
                        &place.bundle_request(reference, budget),
                    );
                    None
                }
            };
            let counts = seams::take_read_counts();
            let want = oracle(&whole, read, reference, outcome.as_ref());
            let decoded: BTreeSet<String> = counts.decoded.iter().cloned().collect();
            let read_set: BTreeSet<String> = counts.read.iter().cloned().collect();
            let mut problems: Vec<String> = Vec::new();
            if decoded != want {
                problems.push(format!(
                    "decoded {:?} (extra {:?}, missing {:?})",
                    counts.decoded,
                    decoded.difference(&want).collect::<Vec<_>>(),
                    want.difference(&decoded).collect::<Vec<_>>()
                ));
            }
            if decoded.len() != counts.decoded.len() {
                problems.push(format!("a file decoded twice: {:?}", counts.decoded));
            }
            if read_set != want || read_set.len() != counts.read.len() {
                problems.push(format!("read {:?}", counts.read));
            }
            let mut sorted = counts.read.clone();
            sorted.sort();
            if sorted != counts.read {
                problems.push(format!("not read in path order: {:?}", counts.read));
            }
            if counts.whole_decodes != 0
                || counts.lists != 0
                || counts.whole_resolvers != 0
                || counts.whole_graphs != 0
                || !counts.updates.is_empty()
                || counts.attempts != 1
                || counts.walks != 0
            {
                problems.push(format!("counters {counts:?}"));
            }
            if !problems.is_empty() {
                failed.push(format!(
                    "{reference} {read:?}: oracle {want:?}: {}",
                    problems.join("; ")
                ));
            }
        }
    }
    assert!(
        failed.is_empty(),
        "{fixture}: {} reads are not the oracle's subset:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

macro_rules! per_fixture {
    ($($differential:ident, $subset:ident => $name:literal;)*) => {
        $(
            #[test]
            fn $differential() {
                differential($name);
            }

            #[test]
            fn $subset() {
                subset_is_the_oracles($name);
            }
        )*
    };
}

per_fixture! {
    sr0_differential_anchor_states, ac12_subset_anchor_states => "anchor-states";
    sr0_differential_card_press, ac12_subset_card_press => "card-press";
    sr0_differential_code_a, ac12_subset_code_a => "code-a";
    sr0_differential_code_bevy, ac12_subset_code_bevy => "code-bevy";
    sr0_differential_code_index, ac12_subset_code_index => "code-index";
    sr0_differential_corpus_mini, ac12_subset_corpus_mini => "corpus-mini";
    sr0_differential_flow_aside, ac12_subset_flow_aside => "flow-aside";
    sr0_differential_flow_gaps, ac12_subset_flow_gaps => "flow-gaps";
    sr0_differential_flow_hire, ac12_subset_flow_hire => "flow-hire";
    sr0_differential_flow_relay, ac12_subset_flow_relay => "flow-relay";
    sr0_differential_impact, ac12_subset_impact => "impact";
    sr0_differential_parent_ref, ac12_subset_parent_ref => "parent-ref";
    sr0_differential_roadmap_a, ac12_subset_roadmap_a => "roadmap-a";
    sr0_differential_roadmap_b, ac12_subset_roadmap_b => "roadmap-b";
    sr0_differential_spec_a, ac12_subset_spec_a => "spec-a";
    sr0_differential_spec_b, ac12_subset_spec_b => "spec-b";
    sr0_differential_spec_c, ac12_subset_spec_c => "spec-c";
    sr0_differential_studio_lamp, ac12_subset_studio_lamp => "studio-lamp";
    sr0_differential_trace_almanac, ac12_subset_trace_almanac => "trace-almanac";
    sr0_differential_verify_ecs, ac12_subset_verify_ecs => "verify-ecs";
    sr0_differential_verify_tests, ac12_subset_verify_tests => "verify-tests";
}

/// The configured fixtures are the list above.
#[test]
fn the_fixture_list_is_every_configured_fixture() {
    let mut found: Vec<String> = fs::read_dir(fixtures_dir())
        .expect("fixtures/")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("specengine.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    assert_eq!(found, FIXTURES.map(str::to_owned).to_vec());
}

// ------------------------------------------------------------------ flow-scenario FS-9

/// The one node of the whole graph whose ID is `id`.
fn node_with_id(graph: &SpecGraph<'_>, id: &str) -> NodeAt {
    let mut found = Vec::new();
    for file in 0..graph.paths().len() {
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            if graph.node(at).and_then(|node| node.id.as_deref()) == Some(id) {
                found.push(at);
            }
        }
    }
    assert_eq!(found.len(), 1, "{id}: {found:?}");
    found[0]
}

/// The flow-scenario slice's FS-9, from the subset, on `flow-aside`:
/// a plain `show` of every document two `parent:` levels under the top (the
/// 142 records among them) decodes C(REF) and its two chain files, nothing
/// else, from one attempt with no whole decode, no whole graph and no walk;
/// its `ancestors` are not empty and are the walk's. The REFs are read from
/// the copy, so no project word is written here. M: the chain from a whole
/// decode; the chain left out of the plan (`[]` against the walk's list).
#[test]
fn fs9_a_show_reads_its_chain_from_the_subset() {
    clean_seams();
    let place = Place::new("flow-aside");
    let whole = Whole::of(&place);
    let graph = whole.graph();
    let (good, _) = whole.references();
    let read = Read::Show {
        links: false,
        archive: false,
    };
    // The freshness step's full pass, once, so every read below is warm.
    let _ = answer(&place, read, &good[0]);
    let mut two_deep = 0usize;
    let mut failed: Vec<String> = Vec::new();
    for reference in good.iter().filter(|reference| !reference.ends_with(".md")) {
        let at = node_with_id(&graph, reference);
        let own = graph.paths()[at.file].to_owned();
        let chain: BTreeSet<String> = graph
            .ancestors(at)
            .into_iter()
            .map(|node| graph.paths()[node.file].to_owned())
            .filter(|path| *path != own)
            .collect();
        if at.ord != 0 || chain.len() != 2 {
            continue;
        }
        two_deep += 1;
        let _ = seams::take_read_counts();
        let got = answer(&place, read, reference);
        let counts = seams::take_read_counts();
        let mut want = whole.closure(reference);
        want.insert(own);
        want.extend(chain);
        let decoded: BTreeSet<String> = counts.decoded.iter().cloned().collect();
        if decoded != want || decoded.len() != counts.decoded.len() {
            failed.push(format!(
                "{reference}: decoded {:?}, want {want:?}",
                counts.decoded
            ));
        }
        if counts.whole_decodes != 0
            || counts.whole_graphs != 0
            || counts.walks != 0
            || counts.attempts != 1
        {
            failed.push(format!("{reference}: counters {counts:?}"));
        }
        if !got.contains("\"ancestors\":[{\"id\":") {
            failed.push(format!("{reference}: an empty or missing list: {got}"));
        }
        let walked = walk_answer(&place, read, reference);
        if got != walked {
            failed.push(format!("{reference}: {}", first_difference(&got, &walked)));
        }
    }
    assert!(
        two_deep >= 142,
        "premise: the records stand two deep: {two_deep}"
    );
    assert!(
        failed.is_empty(),
        "{} of {two_deep} reads:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

// ------------------------------------------------------------------ AC-7 the swap

/// `original` with another `parent:`, another mention end and its sections
/// four lines lower at the same byte offsets: the same size.
fn sprint_moved(original: &str, names: &Names) -> String {
    let parent = format!("parent: {}\n", names.parent);
    let other = format!("parent: {}\n", names.other_parent);
    let lower = "\n".repeat(names.parent.len() - names.other_parent.len());
    for from in [parent.as_str(), "\n# Sprint\n", "bounded by A-101."] {
        assert_eq!(original.matches(from).count(), 1, "premise: {from:?}");
    }
    let moved = original
        .replacen(&parent, &other, 1)
        .replacen("\n# Sprint\n", &format!("\n{lower}# Sprint\n"), 1)
        .replacen("bounded by A-101.", "bounded by A-102.", 1);
    assert_eq!(moved.len(), original.len(), "premise: the same size");
    moved
}

/// `path` replaced (atomic rename) by `text` of the same size, its mtime
/// and atime set back to `path`'s before, to the nanosecond.
fn swap_keeping_times(path: &Path, text: &str) {
    let before = fs::metadata(path).expect("the file");
    assert_eq!(before.len(), text.len() as u64, "premise: the same size");
    let modified = before.modified().expect("an mtime");
    let accessed = before.accessed().expect("an atime");
    let temp = path.with_extension("swap");
    fs::write(&temp, text).expect("the other version");
    let file = fs::File::options()
        .write(true)
        .open(&temp)
        .expect("the other version");
    file.set_times(
        fs::FileTimes::new()
            .set_modified(modified)
            .set_accessed(accessed),
    )
    .expect("the times restored");
    drop(file);
    fs::rename(&temp, path).expect("the swap");
    let after = fs::metadata(path).expect("the swapped file");
    assert_eq!(
        after.modified().expect("an mtime"),
        modified,
        "premise: the mtime"
    );
    assert_eq!(after.len(), before.len(), "premise: the size");
}

/// AC-7 "The swap": for `show REF`, `show REF --links` and `bundle REF`
/// (the target of [`Names`] in `spec-a`), each direction between two same-size
/// versions of sprint.md: the hook, at stage (b) of the first attempt,
/// swaps the file for the other version with its mtime restored to the
/// nanosecond. The answer is exactly the swapped-in version's at-rest
/// answer (one `update_paths` of sprint.md, a second attempt, no walk),
/// never one version's structure with the other's text. M: bytes printed
/// without their BLAKE3 compared; the attempt's rows kept with the swapped
/// bytes attached.
#[test]
fn ac7_a_same_size_swap_at_stage_b_is_answered_whole() {
    clean_seams();
    let reads = [
        Read::Show {
            links: false,
            archive: false,
        },
        Read::Show {
            links: true,
            archive: false,
        },
        Read::Bundle { budget: None },
    ];
    let place = Place::new("spec-a");
    let names = Names::of(&place);
    let target = names.target.as_str();
    let path = place.path(&names.sprint);
    let original = place.text(&names.sprint);
    let moved = sprint_moved(&original, &names);
    let versions = [original, moved];
    let mut at_rest: Vec<[String; 2]> = Vec::new();
    for read in reads {
        let mut both: [String; 2] = Default::default();
        for (at, version) in versions.iter().enumerate() {
            fs::write(&path, version).expect("a version");
            both[at] = answer(&place, read, target);
        }
        assert_ne!(
            both[0], both[1],
            "premise: {read:?} answers each version differently"
        );
        at_rest.push(both);
    }
    let mut failed: Vec<String> = Vec::new();
    for (at, read) in reads.into_iter().enumerate() {
        for from in 0..2 {
            let to = 1 - from;
            fs::write(&path, &versions[from]).expect("the version at rest");
            let _ = answer(&place, read, target);
            let _ = seams::take_read_counts();
            let swapped = Rc::new(Cell::new(false));
            let hook = {
                let (swapped, path, text) =
                    (Rc::clone(&swapped), path.clone(), versions[to].clone());
                Hooked::new(move |stage| {
                    if let Stage::Snapshot { attempt: 1, .. } = stage {
                        swap_keeping_times(&path, &text);
                        swapped.set(true);
                    }
                })
            };
            let got = answer(&place, read, target);
            drop(hook);
            let counts = seams::take_read_counts();
            assert!(
                swapped.get(),
                "premise: {read:?}: the hook swapped at stage (b)"
            );
            if got != at_rest[at][to] {
                let other = if got == at_rest[at][from] {
                    "the version swapped out".to_owned()
                } else {
                    format!(
                        "neither version: {}",
                        first_difference(&got, &at_rest[at][to])
                    )
                };
                failed.push(format!("{read:?} version {from} -> {to}: {other}"));
            }
            if counts.updates != vec![vec![names.sprint.clone()]]
                || counts.attempts != 2
                || counts.walks != 0
            {
                failed.push(format!(
                    "{read:?} version {from} -> {to}: one update_paths of {}, two attempts, \
                     no walk expected: {counts:?}",
                    names.sprint
                ));
            }
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

// ------------------------------------------------------------------ SR-1

/// The stage an edit is made at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    /// (a) after the freshness step.
    Fresh,
    /// (b) the snapshot closed, no file read yet.
    Snapshot,
    /// (c) between two file reads, just before the edited file's.
    Between,
}

/// One of SR-1's edits of `spec-a` around the target of [`Names`]: the file it
/// touches and what it does.
#[derive(Clone, Copy, Debug)]
enum Edit {
    /// A holder's text changed.
    HolderText,
    /// A citing file dropping its link (`stamina.md`'s `depends_on`).
    DropLink,
    /// An end's ID renamed (`A-101`).
    RenameEnd,
    /// A subset file deleted (`A-101.md`).
    Delete,
    /// A subset file made unreadable (mode 000).
    Unreadable,
}

impl Edit {
    const ALL: [Self; 5] = [
        Self::HolderText,
        Self::DropLink,
        Self::RenameEnd,
        Self::Delete,
        Self::Unreadable,
    ];

    fn path(self, names: &Names) -> String {
        match self {
            Self::HolderText => names.sprint.clone(),
            Self::DropLink => names.citing.clone(),
            Self::RenameEnd | Self::Delete | Self::Unreadable => names.end.clone(),
        }
    }

    fn apply(self, root: &Path, names: &Names) {
        use std::os::unix::fs::PermissionsExt as _;
        let relative = self.path(names);
        let path = root.join(&relative);
        let replace = |from: &str, to: &str| {
            let text = fs::read_to_string(&path).expect("the edited file");
            assert_eq!(text.matches(from).count(), 1, "{relative}: {from:?}");
            fs::write(&path, text.replacen(from, to, 1)).expect("the edit");
        };
        match self {
            Self::HolderText => replace("Hold the sprint key to run", "Hold the dash key to run"),
            Self::DropLink => replace(&format!("  depends_on: [{}]\n", names.target), ""),
            Self::RenameEnd => replace("id: A-101\n", "id: A-109\n"),
            Self::Delete => fs::remove_file(&path).expect("the file deleted"),
            Self::Unreadable => {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("mode 000");
            }
        }
    }
}

/// SR-1 "T5 under concurrent edits": each edit at each stage, for `show`,
/// `show --links` and `bundle` of the target. The answer equals a second
/// read with no hook and the walk's answer (no stale text, ID or link; no message of the first
/// attempt); `update_paths` runs once with exactly the edited path when the
/// read's subset holds it (none otherwise), then a second attempt, no walk,
/// no `Source::list`. Stage (c) applies only where the edited file is read
/// after another. M: no compare; `update` in place of `update_paths` (the
/// counter); a retry on the old snapshot (a second mismatch: a walk);
/// messages kept from attempt 1.
#[test]
fn sr1_an_edit_at_each_stage_is_answered_as_a_second_read() {
    clean_seams();
    let reads = [
        Read::Show {
            links: false,
            archive: false,
        },
        Read::Show {
            links: true,
            archive: false,
        },
        Read::Bundle { budget: None },
    ];
    let mut failed: Vec<String> = Vec::new();
    let mut cases = 0usize;
    for edit in Edit::ALL {
        for at in [At::Fresh, At::Snapshot, At::Between] {
            for read in reads {
                let place = Place::new("spec-a");
                let names = Names::of(&place);
                let id = names.target.clone();
                // At rest: what the read reads, in order.
                let _ = answer(&place, read, &id);
                let _ = seams::take_read_counts();
                let _ = answer(&place, read, &id);
                let rest = seams::take_read_counts();
                let target = edit.path(&names);
                let position = rest.read.iter().position(|path| *path == target);
                if at == At::Between && !matches!(position, Some(1..)) {
                    continue;
                }
                cases += 1;
                let done = Rc::new(Cell::new(false));
                let hook = {
                    let (done, root, target, names) = (
                        Rc::clone(&done),
                        place.root.clone(),
                        target.clone(),
                        names.clone(),
                    );
                    Hooked::new(move |stage| {
                        if done.get() {
                            return;
                        }
                        let now = match (&stage, at) {
                            (Stage::Fresh, At::Fresh) => true,
                            (Stage::Snapshot { attempt: 1, .. }, At::Snapshot) => true,
                            (
                                Stage::Between {
                                    attempt: 1, next, ..
                                },
                                At::Between,
                            ) => *next == target,
                            _ => false,
                        };
                        if now {
                            edit.apply(&root, &names);
                            done.set(true);
                        }
                    })
                };
                let got = answer(&place, read, &id);
                drop(hook);
                let counts = seams::take_read_counts();
                let case = format!("{edit:?} at {at:?}, {read:?}");
                if !done.get() {
                    failed.push(format!("{case}: premise: the hook never edited"));
                    continue;
                }
                let want = answer(&place, read, &id);
                if got != want {
                    failed.push(format!(
                        "{case}: not a second read's answer: {}",
                        first_difference(&got, &want)
                    ));
                }
                // The second read reads the rows `update_paths` rewrote; the
                // walk resolves the tree afresh, so a stale row the
                // dependents pass left behind shows here.
                let walked = walk_answer(&place, read, &id);
                if got != walked {
                    failed.push(format!(
                        "{case}: not the walk's answer: {}",
                        first_difference(&got, &walked)
                    ));
                }
                let (updates, attempts) = if position.is_some() {
                    (vec![vec![target.clone()]], 2)
                } else {
                    (Vec::new(), 1)
                };
                if counts.updates != updates
                    || counts.attempts != attempts
                    || counts.walks != 0
                    || counts.lists != 0
                {
                    failed.push(format!(
                        "{case}: {updates:?} updated, {attempts} attempts, no walk and no listing \
                         expected: {counts:?}"
                    ));
                }
            }
        }
    }
    assert!(cases >= 35, "premise: {cases} cases run");
    assert!(
        failed.is_empty(),
        "{} cases:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

/// SR-1 "A hook that edits at every attempt": sprint.md flipped at stage
/// (b) of each attempt gives two attempts, one `update_paths`, then the
/// walk, whose answer is the walk's on the tree it saw; within a 10 s
/// deadline. M: unbounded retries (the editing hook never stops).
#[test]
fn sr1_an_edit_at_every_attempt_gives_two_attempts_then_the_walk() {
    clean_seams();
    let reads = [
        Read::Show {
            links: false,
            archive: false,
        },
        Read::Show {
            links: true,
            archive: false,
        },
        Read::Bundle { budget: None },
    ];
    for read in reads {
        let place = Place::new("spec-a");
        let names = Names::of(&place);
        let id = names.target.clone();
        let path = place.path(&names.sprint);
        let original = place.text(&names.sprint);
        let moved = sprint_moved(&original, &names);
        let _ = answer(&place, read, &id);
        let reference = id.clone();
        let (env, globals, git) = (place.env.clone(), place.globals.clone(), place.git.clone());
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            clean_seams();
            let flips = Rc::new(Cell::new(0usize));
            let hook = {
                let flips = Rc::clone(&flips);
                Hooked::new(move |stage| {
                    if let Stage::Snapshot { .. } = stage {
                        let text = if flips.get().is_multiple_of(2) {
                            &moved
                        } else {
                            &original
                        };
                        let temp = path.with_extension("swap");
                        fs::write(&temp, text).expect("the next version");
                        fs::rename(&temp, &path).expect("the flip");
                        flips.set(flips.get() + 1);
                    }
                })
            };
            let request_place = (env, globals, git);
            let got = shown(match read {
                Read::Show { links, archive } => crate::show(
                    &request_place.0,
                    &request_place.1,
                    &ShowRequest {
                        reference: reference.clone(),
                        links,
                        archive,
                        bindings: false,
                        history: false,
                        git: request_place.2.clone(),
                    },
                )
                .map(Outcome::Show),
                Read::Bundle { budget } => crate::bundle(
                    &request_place.0,
                    &request_place.1,
                    &BundleRequest {
                        references: vec![reference.clone()],
                        budget,
                        git: request_place.2.clone(),
                    },
                )
                .map(Outcome::Bundle),
            });
            drop(hook);
            let counts = seams::take_read_counts();
            let _ = sender.send((got, counts, flips.get()));
        });
        let (got, counts, flips) = receiver
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|_| panic!("{read:?}: no answer in 10 s: the retries are unbounded"));
        assert_eq!(
            flips, 2,
            "{read:?}: the hook edited at both attempts: {counts:?}"
        );
        assert!(
            counts.attempts == 2
                && counts.walks == 1
                && counts.updates == vec![vec![names.sprint.clone()]],
            "{read:?}: two attempts, one update_paths of {}, then the walk: {counts:?}",
            names.sprint
        );
        let want = walk_answer(&place, read, &id);
        assert!(
            got == want,
            "{read:?}: the walk's answer on the tree it saw: {}",
            first_difference(&got, &want)
        );
    }
}

// ------------------------------------------------------------------ SR-2

/// One of SR-2's SQL changes at stage (a).
#[derive(Clone, Copy, Debug)]
enum Corruption {
    /// sprint.md's `files.blake3` set to another hash: one `update_paths`.
    OtherHash,
    /// sprint.md's `blake3` set to NULL with a `read_error`: the same.
    NullHash,
    /// The format stamp set to 15: `NotIndexed`, the walk.
    Stamp,
    /// The worktree row deleted: `NotIndexed`, the walk.
    Worktree,
}

impl Corruption {
    fn sql(self, sprint: &str) -> String {
        match self {
            Self::OtherHash => format!(
                "UPDATE files SET blake3 = '{}' WHERE path = '{sprint}';",
                "0".repeat(64)
            ),
            Self::NullHash => format!(
                "UPDATE files SET blake3 = NULL, read_error = 'injected' WHERE path = '{sprint}';"
            ),
            Self::Stamp => "UPDATE index_meta SET value = '15' WHERE key = 'format';".to_owned(),
            Self::Worktree => "PRAGMA foreign_keys = ON; DELETE FROM worktrees;".to_owned(),
        }
    }

    /// One retry of sprint.md, else the walk.
    fn retries(self) -> bool {
        matches!(self, Self::OtherHash | Self::NullHash)
    }
}

/// SR-2 "Corrupted, stale and missing rows": SQL on the scratch DB at stage
/// (a), for `show`, `show --links` and `bundle` of the target. Another
/// hash or a NULL hash with a `read_error`: one `update_paths` of sprint.md,
/// then the answer at rest. A stale stamp or a deleted worktree row:
/// `NotIndexed`, the walk, the answer at rest. (The answer at rest is the
/// golden's: SR-0's replay.) M: `NotIndexed` surfaced as exit 2; a NULL
/// hash taken as a match.
#[test]
fn sr2_corrupted_stale_and_missing_rows_at_stage_a() {
    clean_seams();
    let reads = [
        Read::Show {
            links: false,
            archive: false,
        },
        Read::Show {
            links: true,
            archive: false,
        },
        Read::Bundle { budget: None },
    ];
    let mut failed: Vec<String> = Vec::new();
    for corruption in [
        Corruption::OtherHash,
        Corruption::NullHash,
        Corruption::Stamp,
        Corruption::Worktree,
    ] {
        for read in reads {
            let place = Place::new("spec-a");
            let names = Names::of(&place);
            let id = names.target.clone();
            let want = answer(&place, read, &id);
            let db = place.db();
            let statements = corruption.sql(&names.sprint);
            let _ = seams::take_read_counts();
            let done = Rc::new(RefCell::new(None::<String>));
            let hook = {
                let (done, db) = (Rc::clone(&done), db.clone());
                Hooked::new(move |stage| {
                    if matches!(stage, Stage::Fresh) && done.borrow().is_none() {
                        sql(&db, &statements);
                        *done.borrow_mut() = Some(sql(&db, "SELECT count(*) FROM worktrees;"));
                    }
                })
            };
            let got = answer(&place, read, &id);
            drop(hook);
            let counts = seams::take_read_counts();
            let case = format!("{corruption:?}, {read:?}");
            if done.borrow().is_none() {
                failed.push(format!("{case}: premise: the hook never ran its SQL"));
                continue;
            }
            if got != want {
                failed.push(format!(
                    "{case}: not the answer at rest: {}",
                    first_difference(&got, &want)
                ));
            }
            let fine = if corruption.retries() {
                counts.updates == vec![vec![names.sprint.clone()]]
                    && counts.attempts == 2
                    && counts.walks == 0
            } else {
                counts.updates.is_empty() && counts.walks == 1
            };
            if !fine {
                failed.push(format!("{case}: counters {counts:?}"));
            }
        }
    }
    assert!(
        failed.is_empty(),
        "{} cases:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

/// One corrupted row of R7 that the file rows do not show: every
/// `files.blake3` still matches, so the freshness step keeps it.
#[derive(Clone, Copy, Debug)]
enum BadRow {
    /// The target's resolved edges end on a position its end file lacks.
    EdgeOrd,
    /// The target's resolved edges end on a file the index does not hold.
    EdgePath,
    /// The target's `parent:` row ends on a position its end file lacks.
    ParentOrd,
    /// The target's `parent:` row ends on a file the index does not hold.
    ParentPath,
}

impl BadRow {
    fn sql(self, sprint: &str) -> String {
        let file = format!("(SELECT file_id FROM files WHERE path = '{sprint}')");
        match self {
            Self::EdgeOrd => format!(
                "UPDATE edges SET end_ord = 99 WHERE state = 'resolved' AND file_id = {file};"
            ),
            Self::EdgePath => format!(
                "UPDATE edges SET end_path = 'store-reads-nowhere.md' \
                 WHERE state = 'resolved' AND file_id = {file};"
            ),
            Self::ParentOrd => {
                format!("UPDATE edge_parents SET end_ord = 99 WHERE file_id = {file};")
            }
            Self::ParentPath => format!(
                "UPDATE edge_parents SET end_path = 'store-reads-nowhere.md' WHERE file_id = {file};"
            ),
        }
    }

    /// The table and the rows it changes, for the premise.
    fn count(self, sprint: &str) -> String {
        let file = format!("(SELECT file_id FROM files WHERE path = '{sprint}')");
        match self {
            Self::EdgeOrd | Self::EdgePath => {
                format!("SELECT count(*) FROM edges WHERE state = 'resolved' AND file_id = {file};")
            }
            Self::ParentOrd | Self::ParentPath => format!(
                "SELECT count(*) FROM edge_parents WHERE state = 'resolved' AND file_id = {file};"
            ),
        }
    }

    /// The reads whose subset uses the row: `--links` and `bundle` hold
    /// the target's edges; only `bundle` climbs its `parent:`.
    fn reads(self) -> Vec<Read> {
        let links = Read::Show {
            links: true,
            archive: false,
        };
        let bundle = Read::Bundle { budget: None };
        match self {
            Self::EdgeOrd | Self::EdgePath => vec![links, bundle],
            Self::ParentOrd | Self::ParentPath => vec![bundle],
        }
    }
}

/// SR-2, R7: an edge or `parent:` row corrupted by SQL so a resolved end
/// names a position or a file outside the subset, every `files.blake3`
/// still matching (the freshness step keeps the row): the plan does not
/// cover its rows (`covers`), so the read walks, with no `update_paths`,
/// and answers as at rest (the golden's answer, by SR-0's replay) and as
/// the walk. M: `covers` always true.
#[test]
fn sr2_a_corrupted_edge_or_parent_row_walks() {
    clean_seams();
    let mut failed: Vec<String> = Vec::new();
    let mut cases = 0usize;
    for bad in [
        BadRow::EdgeOrd,
        BadRow::EdgePath,
        BadRow::ParentOrd,
        BadRow::ParentPath,
    ] {
        for read in bad.reads() {
            let place = Place::new("spec-a");
            let names = Names::of(&place);
            let id = names.target.clone();
            let want = answer(&place, read, &id);
            let db = place.db();
            let rows: usize = sql(&db, &bad.count(&names.sprint))
                .trim()
                .parse()
                .expect("a count");
            assert!(rows > 0, "premise: {bad:?}: the target has such rows");
            let hashes = sql(&db, "SELECT path, blake3 FROM files ORDER BY path;");
            sql(&db, &bad.sql(&names.sprint));
            assert_eq!(
                sql(&db, "SELECT path, blake3 FROM files ORDER BY path;"),
                hashes,
                "premise: {bad:?}: the file rows are untouched"
            );
            let _ = seams::take_read_counts();
            let got = answer(&place, read, &id);
            let counts = seams::take_read_counts();
            cases += 1;
            let case = format!("{bad:?}, {read:?}");
            if got != want {
                failed.push(format!(
                    "{case}: not the answer at rest: {}",
                    first_difference(&got, &want)
                ));
            }
            if counts.walks != 1 || !counts.updates.is_empty() {
                failed.push(format!(
                    "{case}: one walk and no update_paths expected: {counts:?}"
                ));
            }
            let walked = walk_answer(&place, read, &id);
            if got != walked {
                failed.push(format!(
                    "{case}: not the walk's answer: {}",
                    first_difference(&got, &walked)
                ));
            }
        }
    }
    assert_eq!(cases, 6, "premise: the cases run");
    assert!(
        failed.is_empty(),
        "{} cases:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

// ------------------------------------------------------------------ SR-3

/// One whole read of SR-3, rendered; `graph` and `impact` read `reference`.
fn whole_read(place: &Place, command: &str, reference: &str) -> String {
    let result = match command {
        "tree" => crate::tree(
            &place.env,
            &place.globals,
            &TreeRequest {
                root: None,
                depth: None,
                kinds: Vec::new(),
                archive: false,
            },
        )
        .map(Outcome::Tree),
        "graph" => crate::graph(
            &place.env,
            &place.globals,
            &GraphRequest {
                reference: reference.to_owned(),
                impact: false,
                types: Vec::new(),
                depth: None,
                archive: false,
            },
        )
        .map(Outcome::Graph),
        "flow" => crate::flow(
            &place.env,
            &place.globals,
            &FlowRequest {
                root: None,
                depth: None,
                stage: None,
                trace: false,
                in_flow: false,
            },
        )
        .map(|outcome| Outcome::Flow(Box::new(outcome))),
        "impact" => crate::impact(
            &place.env,
            &place.globals,
            &ImpactRequest {
                reference: Some(reference.to_owned()),
                symbol: None,
                since: None,
                depth: None,
                git: place.git.clone(),
            },
        )
        .map(|outcome| Outcome::Impact(Box::new(outcome))),
        "verify" => crate::verify(
            &place.env,
            &place.globals,
            &VerifyRequest {
                tree: VerifiedTree::WorkingTree,
                task: None,
                node: None,
                symbol: None,
                git: place.git.clone(),
            },
        )
        .map(|outcome| Outcome::Verify(Box::new(outcome))),
        other => panic!("no whole read {other}"),
    };
    shown(result)
}

/// The REF `graph` and `impact` read: the citing document of [`Names`]
/// in `spec-a`; none for `verify`.
fn whole_reference(place: &Place, fixture: &str) -> String {
    if fixture == "spec-a" {
        Names::of(place).citing_id
    } else {
        String::new()
    }
}

/// The whole reads of SR-3 and the fixture each runs over: `verify` needs
/// a `[code]` table.
const WHOLE_READS: [(&str, &str); 5] = [
    ("tree", "spec-a"),
    ("graph", "spec-a"),
    ("flow", "spec-a"),
    ("impact", "spec-a"),
    ("verify", "code-a"),
];

/// SR-3 "Whole reads keep a matching row's parse (Q7)": a warm `tree`,
/// `graph`, `flow`, `impact` and `verify` over an unchanged copy parse 0
/// files in `read_bytes`.
#[test]
fn sr3_warm_whole_reads_parse_nothing() {
    clean_seams();
    for (command, fixture) in WHOLE_READS {
        let place = Place::new(fixture);
        let reference = whole_reference(&place, fixture);
        let first = whole_read(&place, command, &reference);
        let _ = seams::take_read_counts();
        let second = whole_read(&place, command, &reference);
        let counts = seams::take_read_counts();
        assert_eq!(
            first, second,
            "{command} on {fixture}: premise: a stable answer"
        );
        assert!(
            counts.parses.is_empty(),
            "{command} on {fixture}: a warm whole read parses nothing: {counts:?}"
        );
    }
}

/// SR-3: one file edited at stage (a) of a whole read is re-parsed alone,
/// and its fresh parse is used: the answer is a second read's. M: the
/// decoded parse kept without a compare (the edit's stale parse printed).
#[test]
fn sr3_a_file_edited_at_stage_a_is_reparsed_alone() {
    clean_seams();
    for (command, fixture) in WHOLE_READS {
        let place = Place::new(fixture);
        let whole = Whole::of(&place);
        let reference = whole_reference(&place, fixture);
        // The file edited: sprint.md in spec-a, else the first document
        // with a `# ` heading.
        let (target, from, to) = if fixture == "spec-a" {
            (
                Names::of(&place).sprint,
                "\n# Sprint\n".to_owned(),
                "\n# Sprunt\n".to_owned(),
            )
        } else {
            let file = whole
                .input
                .files
                .iter()
                .find(|file| String::from_utf8_lossy(&file.bytes).contains("\n# "))
                .expect("premise: a document with a heading");
            let text = String::from_utf8_lossy(&file.bytes).into_owned();
            let line = text
                .lines()
                .find(|line| line.starts_with("# "))
                .expect("a heading")
                .to_owned();
            (
                file.path.clone(),
                format!("\n{line}\n"),
                format!("\n{line} edited\n"),
            )
        };
        let _ = whole_read(&place, command, &reference);
        let _ = seams::take_read_counts();
        let done = Rc::new(Cell::new(false));
        let hook = {
            let (done, root, target, from, to) = (
                Rc::clone(&done),
                place.root.clone(),
                target.clone(),
                from.clone(),
                to.clone(),
            );
            Hooked::new(move |stage| {
                if matches!(stage, Stage::Fresh) && !done.get() {
                    let path = root.join(&target);
                    let text = fs::read_to_string(&path).expect("the edited file");
                    assert_eq!(text.matches(&from).count(), 1, "{target}: {from:?}");
                    fs::write(&path, text.replacen(&from, &to, 1)).expect("the edit");
                    done.set(true);
                }
            })
        };
        let got = whole_read(&place, command, &reference);
        drop(hook);
        let counts = seams::take_read_counts();
        assert!(
            done.get(),
            "{command}: premise: the hook edited at stage (a)"
        );
        assert_eq!(
            counts.parses,
            vec![target.clone()],
            "{command} on {fixture}: the edited file alone is re-parsed: {counts:?}"
        );
        let want = whole_read(&place, command, &reference);
        assert!(
            got == want,
            "{command} on {fixture}: the fresh parse is used: {}",
            first_difference(&got, &want)
        );
    }
}
