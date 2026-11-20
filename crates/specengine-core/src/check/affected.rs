//! The files a write's check judges again (`write-cost`, RM-0125, "E: the
//! affected set"), in model terms alone: keys a file gives or reads, never
//! a directory, prefix, kind or class (ADR-0008).
//!
//! - **N**: the keys the changed file P gives before and after the change
//!   ([`super::given_keys`]): its path, its `id:`, `{#ID}` sections and
//!   `aliases:` entries.
//! - **The set**: P; every reader of a key in N (a file whose recorded
//!   resolution looked it up, hit or miss: a reference dangling today, a
//!   name-fallback candidate, a `slug/ID`'s feature path, a link candidate);
//!   every holder of a key in N (`id-taken`); and, for a reader of N whose
//!   `status:` is `superseded-by`, every reader of its own keys (the
//!   superseded-date exemption's second hop).
//! - **The global passes** ([`Globals`]): `depends-cycle` when a file of the
//!   set has a `depends_on` list on either side, `parent-cycle` when one has
//!   a `parent:`, the index outputs when an index entry is registered and
//!   P's rendered projection (its fields, ID, title, front-matter state
//!   and given keys) or its path's role as an output changes. Otherwise
//!   each gives the same findings on both sides, so none of them is
//!   introduced.
//!
//! A file outside the set resolves alike on both sides and gives equal
//! findings in both runs: never introduced.

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::{ParsedFile, grammar};

use super::config::CheckConfig;
use super::engine::Globals;
use super::input::{CheckFile, CheckInput};
use super::render::readable_fields;
use super::report::Finding;
use super::resolve::{EdgeKey, given_keys_of};
use super::text::front_matter_failed;
use crate::Paths;

/// The link type of the cycle rule and of the roadmap view's order.
const DEPENDS_ON: &str = "depends_on";

/// Who reads and who gives a key, over one snapshot of a corpus
/// (`edge_deps` and `edge_names`; in a test, `resolve_files` with recording
/// on and `given_keys`). Each answer is by path; order does not matter.
pub trait Dependents {
    /// The files whose resolution read `key`, hit or miss.
    fn readers(&self, key: &EdgeKey) -> Vec<String>;
    /// The files that give `key`.
    fn holders(&self, key: &EdgeKey) -> Vec<String>;
}

/// The set a write's check judges again, and the global passes it reruns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Affected {
    /// Root-relative paths, P among them.
    pub paths: BTreeSet<String>,
    /// The global passes run whole, on both sides.
    pub globals: Globals,
}

/// The as-is findings of one tree state (`write-cost` memo C): by path, for
/// the paths judged so far, and each global pass's once run. Holds the
/// corpus's own findings, no write's, no login's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AsIs {
    /// Every finding landing on the path, the global passes' aside; an entry
    /// with no finding is a path judged clean.
    pub by_path: BTreeMap<String, Vec<Finding>>,
    /// `depends-cycle`, once run.
    pub cycles: Option<Vec<Finding>>,
    /// `parent-cycle`, once run.
    pub parents: Option<Vec<Finding>>,
    /// `index-drift` and `index-missing`, once run.
    pub index: Option<Vec<Finding>>,
}

impl AsIs {
    /// The global passes `wanted` names that are not held yet.
    pub(crate) fn lacking(&self, wanted: Globals) -> Globals {
        Globals {
            cycles: wanted.cycles && self.cycles.is_none(),
            parents: wanted.parents && self.parents.is_none(),
            index: wanted.index && self.index.is_none(),
        }
    }

    /// `found`, a run over `paths` with `globals`, put in place: each path
    /// gets its findings (none: an empty entry), each global pass run its
    /// own.
    pub(crate) fn absorb(
        &mut self,
        found: Vec<Finding>,
        paths: &BTreeSet<String>,
        globals: Globals,
    ) {
        for path in paths {
            self.by_path.entry(path.clone()).or_default();
        }
        if globals.cycles {
            self.cycles.get_or_insert_with(Vec::new);
        }
        if globals.parents {
            self.parents.get_or_insert_with(Vec::new);
        }
        if globals.index {
            self.index.get_or_insert_with(Vec::new);
        }
        for finding in found {
            let bucket = match global_of(&finding.code) {
                Some(Global::Cycles) => self.cycles.get_or_insert_with(Vec::new),
                Some(Global::Parents) => self.parents.get_or_insert_with(Vec::new),
                Some(Global::Index) => self.index.get_or_insert_with(Vec::new),
                None => self.by_path.entry(finding.path.clone()).or_default(),
            };
            bucket.push(finding);
        }
    }

    /// The findings a write over `affected` is judged against: the set's
    /// paths' and the global passes' it reruns.
    pub(crate) fn judged<'s>(
        &'s self,
        affected: &'s Affected,
    ) -> impl Iterator<Item = &'s Finding> {
        let paths = affected
            .paths
            .iter()
            .filter_map(|path| self.by_path.get(path))
            .flatten();
        let globals = affected.globals;
        let cycles = self.cycles.iter().flatten().filter(move |_| globals.cycles);
        let parents = self
            .parents
            .iter()
            .flatten()
            .filter(move |_| globals.parents);
        let index = self.index.iter().flatten().filter(move |_| globals.index);
        paths.chain(cycles).chain(parents).chain(index)
    }
}

/// A global pass, by the codes it emits.
enum Global {
    Cycles,
    Parents,
    Index,
}

fn global_of(code: &str) -> Option<Global> {
    match code {
        "depends-cycle" => Some(Global::Cycles),
        "parent-cycle" => Some(Global::Parents),
        "index-drift" | "index-missing" => Some(Global::Index),
        _ => None,
    }
}

/// The set a change of the file at `path` to `patched` reaches, over the
/// corpus `input` (which holds P as it is, when it exists, and every reader
/// of N with its parse) and its `dependents`.
pub fn affected(
    input: &CheckInput,
    config: &CheckConfig,
    paths: &Paths,
    path: &str,
    patched: &CheckFile,
    dependents: &dyn Dependents,
) -> Affected {
    let by_path: BTreeMap<&str, &CheckFile> = input
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let before = by_path.get(path).copied();
    let mut given: BTreeSet<EdgeKey> =
        given_keys_of(path, before.and_then(|file| file.parsed.as_ref()))
            .into_iter()
            .collect();
    given.extend(given_keys_of(path, patched.parsed.as_ref()));

    let mut set: BTreeSet<String> = BTreeSet::from([path.to_owned()]);
    let mut readers: BTreeSet<String> = BTreeSet::new();
    for key in &given {
        readers.extend(dependents.readers(key));
        set.extend(dependents.holders(key));
    }
    set.extend(readers.iter().cloned());
    // Hop 2: a reader of N superseded by a name it reads may now have
    // another successor, or one dated otherwise: whoever cites it.
    for reader in &readers {
        let Some(file) = by_path.get(reader.as_str()) else {
            continue;
        };
        if !file.parsed.as_ref().is_some_and(superseded) {
            continue;
        }
        for key in given_keys_of(&file.path, file.parsed.as_ref()) {
            set.extend(dependents.readers(&key));
        }
    }

    let sides = |at: &str| -> Vec<&ParsedFile> {
        let mut parses = Vec::new();
        if at == path {
            parses.extend(patched.parsed.as_ref());
        }
        if let Some(file) = by_path.get(at) {
            parses.extend(file.parsed.as_ref());
        }
        parses
    };
    let cycles = set
        .iter()
        .any(|at| sides(at).into_iter().any(has_depends_on));
    let parents = set.iter().any(|at| sides(at).into_iter().any(has_parent));
    let index = config.index_generator().is_some_and(|generator| {
        let output = paths.index.as_deref() == Some(path)
            || generator.shards.iter().any(|shard| shard.path == path)
            || generator
                .roadmap
                .as_ref()
                .is_some_and(|view| view.path == path);
        // The render reads every file's projection and, for the roadmap
        // view, its items' `depends_on` resolved through the keys files
        // give: P's keys are in its projection, so an unchanged projection
        // leaves every output as it was.
        output || rendered(before) != rendered(Some(patched))
    });
    Affected {
        paths: set,
        globals: Globals {
            cycles,
            parents,
            index,
        },
    }
}

/// `status: superseded-by X`, read.
fn superseded(parsed: &ParsedFile) -> bool {
    readable_fields(parsed)
        .and_then(|fields| fields.status.as_deref())
        .and_then(grammar::split_superseded_by)
        .is_some()
}

/// A `depends_on` list in a read front-matter.
fn has_depends_on(parsed: &ParsedFile) -> bool {
    readable_fields(parsed)
        .and_then(|fields| fields.links.as_ref())
        .and_then(|links| links.get(DEPENDS_ON))
        .is_some_and(|list| !list.is_empty())
}

/// A `parent:` in a read front-matter.
fn has_parent(parsed: &ParsedFile) -> bool {
    !front_matter_failed(parsed)
        && parsed
            .document()
            .is_some_and(|document| document.parent.is_some())
}

/// What the index outputs read of a file: whether it is there and read,
/// its front-matter's state, its document's ID, title, typed and kept keys,
/// and the keys it gives. Equal on both sides: the render is the same.
fn rendered(file: Option<&CheckFile>) -> String {
    let Some(file) = file else {
        return String::new();
    };
    let parsed = file.parsed.as_ref();
    let document = parsed.and_then(|parsed| parsed.document());
    format!(
        "{:?}",
        (
            file.read_error.is_some(),
            parsed.map(front_matter_failed),
            document.map(|document| (
                &document.id,
                &document.title,
                &document.fields,
                &document.extra
            )),
            given_keys_of(&file.path, parsed),
        )
    )
}
