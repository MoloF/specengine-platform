//! A write's introduced findings over the walked rows (ADR-0083,
//! `write-cost` RM-0125): after the call's walk the check takes each file as
//! its row has it, trusting its stat key as the freshness step does, and
//! judges again only the files the change reaches (core's
//! [`affected`]). Equal to [`introduced_findings`], 0.47.0's whole disk
//! check, finding for finding and in its order.
//!
//! 1. **One snapshot**: the walk-state digest and every row the check reads
//!    come from one deferred read transaction.
//! 2. **The memo** in scope ([`check_memo`]): D's findings for the digest,
//!    the path, the new bytes and the login, when held; C's as-is findings
//!    of the set's paths it holds.
//! 3. **The cold path**, 0.47.0's whole disk check: a store error, rows of
//!    another `[ids]` or `[paths]` than the setup's, no index handle, or a
//!    memo whose last whole check ran under another config (the first call
//!    after a start among them).
//! 4. **The set**: P as it is, then the readers and holders of P's keys
//!    (and the second hop), decoded until no file joins; then the files
//!    every set file's lookups read, on both sides, until no holder joins.
//!    Only the index render, when P's rendered projection changes, decodes
//!    every row.
//! 5. **The region**, when a cycle pass reruns: from the set, every file
//!    reached forward over `depends_on` and `parent:`, on either side, each
//!    closed for its resolution. Forward closed, it holds the whole cycle
//!    of each of its files; a cycle finding landing outside it is dropped on
//!    both sides (any such cycle reads no key P gives: never introduced).
//! 6. **The hash net**: every set file (and each index output a render
//!    compares) is read from disk and its BLAKE3 compared with its row's; a
//!    mismatch runs the whole disk check and doubts (D emptied, and the
//!    caller doubts the trust). A row recording the parser's panic on
//!    bytes that read now is the cold path, nothing doubted
//!    ([`Rows::Cold`]).
//! 7. **The cycle line**: an introduced cycle's first member outside the set
//!    is read through the same net and the patched side judged again, so
//!    its line is that file's.
//!
//! Nothing refuses (ADR-0012): a parser panic gives no findings, as the
//! whole check's does.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};

use specengine_core::check::{
    Affected, AsIs, CheckFile, CheckInput, Dependents, EdgeKey, Parent, Problem, ProblemKind,
    affected, depends_targets, given_keys, resolve_files,
};
use specengine_core::patch::PatchCheck;

use rusqlite::Statement;

use crate::INDEX_FORMAT;
use crate::check::{CheckSetup, NamedBytes, PARSER_PANICKED};
use crate::check_memo::{self, Digest, FindingsKey};
use crate::cost::{self, Kind};
use crate::edges::{DEPENDENTS, HOLDERS};
use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::queue::ProposalFinding;
use crate::read::{IndexSnapshot, walk_notes};
use crate::rows::hash_bytes;
use crate::source::Source;
use crate::update::introduced_findings;
use crate::write::{fingerprint, paths_fingerprint};

/// The check's setup files as read: the config and the baseline (`None`:
/// none), whose hashes key the memo.
#[derive(Debug, Clone, Copy)]
pub struct SetupFiles<'a> {
    pub config: &'a NamedBytes,
    pub baseline: Option<&'a NamedBytes>,
}

/// A write's introduced findings, and whether the rows were doubted (a
/// file read differed from its row): the caller doubts the index's trust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Introduced {
    pub findings: Vec<ProposalFinding>,
    pub doubted: bool,
}

/// [`introduced_findings`] over `index`'s rows, as the module documentation
/// says: `index` the handle the call's walk updated (`None`: the whole disk
/// check), `source` the tree that walk read. Never a refusal.
pub fn introduced_from_rows(
    index: Option<&SqliteIndex>,
    source: &dyn Source,
    setup: &CheckSetup,
    files: SetupFiles<'_>,
    today: &str,
    path: &str,
    patched: Vec<u8>,
) -> Introduced {
    let _cost = cost::charge(Kind::Check);
    let whole = |patched: Vec<u8>| Introduced {
        findings: introduced_findings(source, setup, today, path, patched),
        doubted: false,
    };
    let Some(index) = index else {
        return whole(patched);
    };
    let config = setup_hash(Some(files.config));
    let scope = check_memo::current();
    let snapshot = match index.snapshot() {
        Ok(snapshot) => snapshot,
        Err(_) => return whole(patched),
    };
    let digest = match tree_digest(&snapshot, setup, &config, files.baseline, today) {
        Ok(Some(digest)) => digest,
        _ => {
            drop(snapshot);
            return whole(patched);
        }
    };
    // Test builds: another connection may write here, between the digest
    // and the rows (WC20); the snapshot must not see it.
    #[cfg(test)]
    between::run();
    let root = index.root();
    let key = scope.as_ref().map(|(_, login)| FindingsKey {
        root: root.to_path_buf(),
        tree: digest,
        path: path.to_owned(),
        bytes: *blake3::hash(&patched).as_bytes(),
        login: login.clone(),
    });
    if let (Some((memo, _)), Some(key)) = (&scope, &key) {
        if let Some(findings) = memo.findings(key) {
            return Introduced {
                findings,
                doubted: false,
            };
        }
        if !memo.is_warm(root, &config) {
            drop(snapshot);
            let found = whole(patched);
            memo.warmed(root, config);
            memo.keep_findings(key.clone(), found.findings.clone());
            return found;
        }
    }
    let as_is = scope
        .as_ref()
        .and_then(|(memo, _)| memo.take_tree(root, &digest))
        .unwrap_or_default();
    let check = PatchCheck {
        scheme: &setup.project.scheme,
        paths: &setup.project.paths,
        config: &setup.config,
        baseline: &setup.baseline,
        today,
    };
    let run = panic::catch_unwind(AssertUnwindSafe(|| {
        over_rows(&snapshot, source, &check, path, &patched, as_is)
    }));
    drop(snapshot);
    match run {
        Ok(Ok(Rows::Found { findings, as_is })) => {
            let findings: Vec<ProposalFinding> =
                findings.iter().map(ProposalFinding::from).collect();
            if let (Some((memo, _)), Some(key)) = (&scope, key) {
                memo.keep_tree(root, digest, as_is);
                memo.keep_findings(key, findings.clone());
            }
            Introduced {
                findings,
                doubted: false,
            }
        }
        Ok(Ok(Rows::Unparsed)) => Introduced {
            findings: Vec::new(),
            doubted: false,
        },
        Ok(Ok(Rows::Mismatch)) => {
            if let Some((memo, _)) = &scope {
                memo.doubt();
            }
            Introduced {
                doubted: true,
                ..whole(patched)
            }
        }
        Ok(Ok(Rows::Cold) | Err(_)) | Err(_) => whole(patched),
    }
}

/// How a run over the rows ended.
enum Rows {
    /// The findings, and C grown by the set's paths.
    Found {
        findings: Vec<specengine_core::check::Finding>,
        as_is: AsIs,
    },
    /// The parser panicked on the new bytes: no findings, as the whole
    /// check gives.
    Unparsed,
    /// A file read differed from its row.
    Mismatch,
    /// A row records the parser's panic on bytes that read: the whole
    /// check judges them as 0.47.0 did, nothing doubted.
    Cold,
}

/// The run over the rows of `snapshot` (see the module documentation,
/// steps 4 and 5).
fn over_rows(
    snapshot: &IndexSnapshot<'_>,
    source: &dyn Source,
    check: &PatchCheck<'_>,
    path: &str,
    patched: &[u8],
    mut as_is: AsIs,
) -> Result<Rows, StoreError> {
    let scheme = check.scheme;
    let paths = check.paths;
    let Ok(patched) = panic::catch_unwind(AssertUnwindSafe(|| {
        CheckFile::parse(path, patched.to_vec(), scheme)
    })) else {
        return Ok(Rows::Unparsed);
    };
    let mut gather = Gather::new(snapshot);
    gather.decode([path])?;
    let dependents = Keyed::new(snapshot)?;
    // The set: decoded until no file joins (the second hop reads the
    // readers' parses).
    let reach = loop {
        let reach = affected(
            &gather.input,
            check.config,
            paths,
            path,
            &patched,
            &dependents,
        );
        dependents.failed()?;
        let fresh = gather.decode(reach.paths.iter().map(String::as_str))?;
        if !fresh {
            break reach;
        }
    };
    let globals = reach.globals;
    // The index render reads every file; a cycle pass, its region alone.
    let regional = (globals.cycles || globals.parents) && !globals.index;
    let mut region = None;
    if globals.index {
        gather.decode_all()?;
    } else {
        gather.close(&reach, &patched, check, &dependents)?;
        if regional {
            region = Some(gather.region(&reach, &patched, check, &dependents)?);
        }
    }
    // A cycle finding outside the region, on either side: dropped (its
    // triple holds its path, so dropping the patched side's drops both).
    let in_region = |finding: &specengine_core::check::Finding| {
        !matches!(finding.code.as_str(), "depends-cycle" | "parent-cycle")
            || region
                .as_ref()
                .is_none_or(|region: &BTreeSet<String>| region.contains(&finding.path))
    };
    // The hash net: the set's files, and the outputs a render compares.
    let mut read: BTreeSet<String> = reach.paths.clone();
    if globals.index
        && let Some(generator) = check.config.index_generator()
    {
        read.extend(paths.index.clone());
        read.extend(generator.shards.iter().map(|shard| shard.path.clone()));
        read.extend(generator.roadmap.as_ref().map(|view| view.path.clone()));
    }
    if let Some(stop) = gather.read(source, &read) {
        return Ok(stop);
    }
    if globals.index {
        gather.input.problems = gather.problems()?;
    }
    // A cycle pass over a region judges that region alone: its as-is side
    // is this call's, never C's (whose passes ran whole), and is not kept.
    let whole_cycles = regional.then(|| (as_is.cycles.take(), as_is.parents.take()));
    let mut findings = check.introduced_in(&gather.input, patched.clone(), &reach, &mut as_is);
    findings.retain(&in_region);
    // A cycle's line is its first member's, read from that file's bytes:
    // a first member outside the set is read now, through the same net,
    // and the patched side judged again (the as-is side is in `as_is`).
    let lined: BTreeSet<String> = findings
        .iter()
        .filter(|finding| matches!(finding.code.as_str(), "depends-cycle" | "parent-cycle"))
        .filter(|finding| !gather.has_bytes(&finding.path))
        .map(|finding| finding.path.clone())
        .collect();
    if !lined.is_empty() {
        if let Some(stop) = gather.read(source, &lined) {
            return Ok(stop);
        }
        findings = check.introduced_in(&gather.input, patched, &reach, &mut as_is);
        findings.retain(&in_region);
    }
    if let Some((cycles, parents)) = whole_cycles {
        as_is.cycles = cycles;
        as_is.parents = parents;
    }
    Ok(Rows::Found { findings, as_is })
}

/// The rows a run decoded, each file once, as the check's input (in
/// decode order: every reader sorts by path).
struct Gather<'s, 'i> {
    snapshot: &'s IndexSnapshot<'i>,
    input: CheckInput,
    /// Path → its file's position in `input`.
    at: BTreeMap<String, usize>,
    /// Path → its row's BLAKE3 (`None`: a read error or parser panic).
    blake3: BTreeMap<String, Option<String>>,
    /// Every path asked for, stored or not.
    tried: BTreeSet<String>,
    /// Every key whose holders were asked for.
    queried: BTreeSet<EdgeKey>,
    whole: bool,
}

impl<'s, 'i> Gather<'s, 'i> {
    fn new(snapshot: &'s IndexSnapshot<'i>) -> Self {
        Self {
            snapshot,
            input: CheckInput::default(),
            at: BTreeMap::new(),
            blake3: BTreeMap::new(),
            tried: BTreeSet::new(),
            queried: BTreeSet::new(),
            whole: false,
        }
    }

    /// `file` added, unless a file of its path is there.
    fn add(&mut self, file: CheckFile) {
        if !self.at.contains_key(&file.path) {
            self.at.insert(file.path.clone(), self.input.files.len());
            self.input.files.push(file);
        }
    }

    /// The stored files among `paths` not asked for yet; whether any was.
    fn decode<'p>(&mut self, paths: impl IntoIterator<Item = &'p str>) -> Result<bool, StoreError> {
        if self.whole {
            return Ok(false);
        }
        let fresh: Vec<&str> = paths
            .into_iter()
            .filter(|path| self.tried.insert((*path).to_owned()))
            .collect();
        if fresh.is_empty() {
            return Ok(false);
        }
        let decoded = self.snapshot.decode(&fresh)?;
        for file in decoded.input.files {
            self.add(file);
        }
        self.blake3.extend(decoded.blake3);
        Ok(true)
    }

    /// Every stored file; the bytes already attached kept.
    fn decode_all(&mut self) -> Result<(), StoreError> {
        let decoded = self.snapshot.decode_all()?;
        for file in decoded.input.files {
            self.add(file);
        }
        self.blake3.extend(decoded.blake3);
        self.whole = true;
        Ok(())
    }

    /// The file at `path` holds the bytes it was parsed from.
    fn has_bytes(&self, path: &str) -> bool {
        self.at
            .get(path)
            .is_some_and(|&at| !self.input.files[at].bytes.is_empty())
    }

    /// Each stored file of `paths` read from disk and compared with its
    /// row (the hash net), its bytes put beside its row's parse; the end of
    /// the run when one differs ([`Rows::Mismatch`]) or a row records the
    /// parser's panic on bytes that read now ([`Rows::Cold`]).
    fn read(&mut self, source: &dyn Source, paths: &BTreeSet<String>) -> Option<Rows> {
        for path in paths {
            let Some(stored) = self.blake3.get(path) else {
                // No row: absent from the walk (a created file, an output
                // not walked), as the whole check's listing has it.
                continue;
            };
            let bytes = source.read(path);
            let same = match &bytes {
                Ok(bytes) => stored.as_deref() == Some(hash_bytes(bytes).as_str()),
                Err(_) => stored.is_none(),
            };
            if !same {
                let panicked = stored.is_none()
                    && bytes.is_ok()
                    && self.at.get(path).is_some_and(|&at| {
                        self.input.files[at].read_error.as_deref() == Some(PARSER_PANICKED)
                    });
                return Some(if panicked { Rows::Cold } else { Rows::Mismatch });
            }
            if let Ok(bytes) = bytes {
                self.attach(path, bytes);
            }
        }
        None
    }

    /// The bytes read for `path`, beside its row's parse.
    fn attach(&mut self, path: &str, bytes: Vec<u8>) {
        if let Some(&at) = self.at.get(path) {
            let file = &mut self.input.files[at];
            if let Some(parsed) = file.parsed.take() {
                *file = CheckFile::parsed(std::mem::take(&mut file.path), bytes, parsed);
            }
        }
    }

    /// The walk's problems, from its notes (the index comparison's gap).
    fn problems(&self) -> Result<Vec<Problem>, StoreError> {
        let Some(notes) = walk_notes(&self.snapshot.tx, self.snapshot.wt)? else {
            return Ok(Vec::new());
        };
        let mut problems: Vec<Problem> = notes
            .missing_roots
            .into_iter()
            .map(|path| Problem {
                kind: ProblemKind::MissingRoot,
                path,
            })
            .collect();
        problems.extend(notes.unreadable_dirs.into_iter().map(|path| Problem {
            kind: ProblemKind::UnreadableDir,
            path,
        }));
        problems.extend((0..notes.skipped_names).map(|_| Problem {
            kind: ProblemKind::SkippedName,
            path: String::new(),
        }));
        Ok(problems)
    }

    /// The holders of `keys` not asked for yet, decoded; whether any file
    /// joined.
    fn holders_of(
        &mut self,
        keys: impl IntoIterator<Item = EdgeKey>,
        dependents: &Keyed<'_>,
    ) -> Result<bool, StoreError> {
        let mut named: Vec<String> = Vec::new();
        for key in keys {
            if self.queried.insert(key.clone()) {
                named.extend(dependents.holders(&key));
            }
        }
        dependents.failed()?;
        self.decode(named.iter().map(String::as_str))
    }

    /// The set closed over both sides (R4: a resolution over a subset equals
    /// the whole corpus's when every key it read has every holder there):
    /// the set's files and every superseded file cited resolve over the
    /// decoded rows, as they are and with P patched, and the holders of
    /// every key they read join, with the holders of every key a set file
    /// gives (`id-taken`'s first holder), until none joins.
    fn close(
        &mut self,
        reach: &Affected,
        patched: &CheckFile,
        check: &PatchCheck<'_>,
        dependents: &Keyed<'_>,
    ) -> Result<(), StoreError> {
        let mut given: Vec<EdgeKey> = given_keys(patched);
        for path in &reach.paths {
            if let Some(&at) = self.at.get(path) {
                given.extend(given_keys(&self.input.files[at]));
            }
        }
        self.holders_of(given, dependents)?;
        self.close_sources(&reach.paths, true, patched, check, dependents)
    }

    /// `sources` closed over both sides: they (and, with `superseded`,
    /// every superseded file decoded) resolve over the decoded rows, as they
    /// are and with P patched, and the holders of every key they read join,
    /// until none joins.
    fn close_sources(
        &mut self,
        sources: &BTreeSet<String>,
        superseded: bool,
        patched: &CheckFile,
        check: &PatchCheck<'_>,
        dependents: &Keyed<'_>,
    ) -> Result<(), StoreError> {
        loop {
            let (mut keys, after) =
                self.on_both_sides(patched, |gather| gather.reads(sources, superseded, check));
            keys.extend(after);
            if !self.holders_of(keys, dependents)? {
                return Ok(());
            }
        }
    }

    /// `read` over the files decoded as they are, then with P patched in
    /// its place (added when absent), the files put back after.
    fn on_both_sides<T>(&mut self, patched: &CheckFile, read: impl Fn(&Self) -> T) -> (T, T) {
        let before = read(self);
        let at = self.at.get(&patched.path).copied();
        let kept = match at {
            Some(at) => Some(std::mem::replace(
                &mut self.input.files[at],
                patched.clone(),
            )),
            None => {
                self.input.files.push(patched.clone());
                None
            }
        };
        let after = read(self);
        match (at, kept) {
            (Some(at), Some(kept)) => self.input.files[at] = kept,
            _ => {
                self.input.files.pop();
            }
        }
        (before, after)
    }

    /// The keys `sources` (and, with `superseded`, every superseded file
    /// decoded) read, resolved over the files decoded.
    fn reads(
        &self,
        sources: &BTreeSet<String>,
        superseded_too: bool,
        check: &PatchCheck<'_>,
    ) -> BTreeSet<EdgeKey> {
        let only: BTreeSet<&str> = self
            .input
            .files
            .iter()
            .filter(|file| {
                sources.contains(&file.path)
                    || (superseded_too && file.parsed.as_ref().is_some_and(superseded))
            })
            .map(|file| file.path.as_str())
            .collect();
        resolve_files(&self.input, check.scheme, check.paths, Some(&only))
            .files
            .into_iter()
            .flat_map(|file| file.reads)
            .collect()
    }

    /// The region a cycle pass needs, decoded and closed: from the set, every
    /// file reached forward over `depends_on` (when `cycles`) and `parent:`
    /// (when `parents`), on either side, each resolved exactly. Forward
    /// closed, it holds the whole cycle of each of its files; a cycle of
    /// files outside it reads no key P gives, so it is the same on both
    /// sides and never introduced.
    fn region(
        &mut self,
        reach: &Affected,
        patched: &CheckFile,
        check: &PatchCheck<'_>,
        dependents: &Keyed<'_>,
    ) -> Result<BTreeSet<String>, StoreError> {
        let globals = reach.globals;
        let mut region: BTreeSet<String> = reach.paths.clone();
        let mut fresh: BTreeSet<String> = reach.paths.clone();
        // The set is decoded and closed already ([`Gather::close`]).
        let mut closed = true;
        while !fresh.is_empty() {
            if !closed {
                self.decode(fresh.iter().map(String::as_str))?;
                self.close_sources(&fresh, false, patched, check, dependents)?;
            }
            closed = false;
            let (before, after) = self.on_both_sides(patched, |gather| {
                gather.targets(&fresh, globals.cycles, globals.parents, check)
            });
            fresh = before
                .into_iter()
                .chain(after)
                .filter(|path| region.insert(path.clone()))
                .collect();
        }
        Ok(region)
    }

    /// The files `from` reaches in one step: their `depends_on` targets
    /// (`cycles`) and their document's `parent:` (`parents`), as the cycle
    /// passes resolve them over the files decoded.
    fn targets(
        &self,
        from: &BTreeSet<String>,
        cycles: bool,
        parents: bool,
        check: &PatchCheck<'_>,
    ) -> BTreeSet<String> {
        let only: BTreeSet<&str> = from.iter().map(String::as_str).collect();
        let mut out = BTreeSet::new();
        if cycles {
            for reached in
                depends_targets(&self.input, check.scheme, check.paths, &only).into_values()
            {
                out.extend(reached);
            }
        }
        if parents {
            let resolved = resolve_files(&self.input, check.scheme, check.paths, Some(&only));
            for file in &resolved.files {
                if let Some(Parent::Node { node, .. }) = &file.parent {
                    out.insert(resolved.paths[node.file].to_owned());
                }
            }
        }
        out
    }
}

/// A file whose `status:` reads `superseded-by`: its successor's lookups
/// decide the exemption of every file citing it.
fn superseded(parsed: &specengine_model::ParsedFile) -> bool {
    parsed
        .document()
        .and_then(|document| document.fields.as_ref())
        .and_then(|fields| fields.status.as_deref())
        .is_some_and(|status| specengine_model::grammar::split_superseded_by(status).is_some())
}

/// [`Dependents`] by the snapshot's `edge_deps` and `edge_names`, each
/// statement prepared once; a store error answers nothing and is kept for
/// [`Keyed::failed`].
struct Keyed<'s> {
    wt: i64,
    readers: RefCell<Statement<'s>>,
    holders: RefCell<Statement<'s>>,
    error: RefCell<Option<StoreError>>,
}

impl<'s> Keyed<'s> {
    fn new(snapshot: &'s IndexSnapshot<'_>) -> Result<Self, StoreError> {
        Ok(Self {
            wt: snapshot.wt,
            readers: RefCell::new(snapshot.tx.prepare(DEPENDENTS).db()?),
            holders: RefCell::new(snapshot.tx.prepare(HOLDERS).db()?),
            error: RefCell::new(None),
        })
    }

    fn paths(&self, statement: &RefCell<Statement<'s>>, key: &EdgeKey) -> Vec<String> {
        let mut statement = statement.borrow_mut();
        let found = statement
            .query_map((self.wt, key.text()), |row| row.get::<_, String>(0))
            .db()
            .and_then(|rows| rows.collect::<Result<Vec<_>, _>>().db());
        match found {
            Ok(paths) => paths,
            Err(error) => {
                self.error.borrow_mut().get_or_insert(error);
                Vec::new()
            }
        }
    }

    /// The first error a lookup met.
    fn failed(&self) -> Result<(), StoreError> {
        match self.error.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Dependents for Keyed<'_> {
    fn readers(&self, key: &EdgeKey) -> Vec<String> {
        self.paths(&self.readers, key)
    }

    fn holders(&self, key: &EdgeKey) -> Vec<String> {
        self.paths(&self.holders, key)
    }
}

/// BLAKE3 of a setup file's bytes, or of "absent" (none given) or its read
/// error.
fn setup_hash(file: Option<&NamedBytes>) -> Digest {
    let mut hasher = blake3::Hasher::new();
    match file.map(|file| &file.bytes) {
        None => {
            hasher.update(b"absent");
        }
        Some(Ok(bytes)) => {
            hasher.update(b"bytes");
            hasher.update(bytes);
        }
        Some(Err(error)) => {
            hasher.update(b"error");
            hasher.update(error.to_string().as_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

/// One field of a digest, length-prefixed.
fn field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

/// The walk-state digest of `snapshot` under `setup` (`write-cost` "Memo
/// keys", the tree key), or `None` when the worktree's fingerprints are not
/// the setup's (the cold path).
fn tree_digest(
    snapshot: &IndexSnapshot<'_>,
    setup: &CheckSetup,
    config: &Digest,
    baseline: Option<&NamedBytes>,
    today: &str,
) -> Result<Option<Digest>, StoreError> {
    let tx = &snapshot.tx;
    let wt = snapshot.wt;
    let fingerprints: Option<(String, String)> =
        rusqlite::OptionalExtension::optional(tx.query_row(
            "SELECT scheme_fp, paths_fp FROM worktrees WHERE wt = ?1",
            [wt],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ))
        .db()?;
    let Some((scheme_fp, paths_fp)) = fingerprints else {
        return Ok(None);
    };
    if scheme_fp != fingerprint(&setup.project.scheme)?
        || paths_fp != paths_fingerprint(&setup.project.paths)
    {
        return Ok(None);
    }
    let mut hasher = blake3::Hasher::new();
    field(&mut hasher, INDEX_FORMAT.to_string().as_bytes());
    field(&mut hasher, scheme_fp.as_bytes());
    field(&mut hasher, paths_fp.as_bytes());
    let mut rows = blake3::Hasher::new();
    let mut statement = tx
        .prepare("SELECT path, blake3, read_error, size FROM files WHERE wt = ?1 ORDER BY path")
        .db()?;
    let mut found = statement.query([wt]).db()?;
    while let Some(row) = found.next().db()? {
        let path: String = row.get(0).db()?;
        let hash: Option<String> = row.get(1).db()?;
        let error: Option<String> = row.get(2).db()?;
        let size: i64 = row.get(3).db()?;
        field(&mut rows, path.as_bytes());
        match (&hash, &error) {
            (Some(hash), _) => field(&mut rows, hash.as_bytes()),
            (None, error) => {
                field(&mut rows, b"unread");
                field(&mut rows, error.as_deref().unwrap_or_default().as_bytes());
            }
        }
        field(&mut rows, &size.to_le_bytes());
    }
    field(&mut hasher, rows.finalize().as_bytes());
    match walk_notes(tx, wt)? {
        Some(notes) => {
            field(&mut hasher, b"notes");
            for root in &notes.missing_roots {
                field(&mut hasher, root.as_bytes());
            }
            field(&mut hasher, b"unlisted");
            for dir in &notes.unreadable_dirs {
                field(&mut hasher, dir.as_bytes());
            }
            field(&mut hasher, &notes.skipped_names.to_le_bytes());
        }
        None => field(&mut hasher, b"no notes"),
    }
    field(&mut hasher, config);
    field(&mut hasher, &setup_hash(baseline));
    field(&mut hasher, today.as_bytes());
    Ok(Some(*hasher.finalize().as_bytes()))
}

/// A test's hook between the digest and the row reads.
#[cfg(test)]
pub(crate) mod between {
    use std::cell::RefCell;

    thread_local! {
        static HOOK: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
    }

    /// `hook` runs once, at the next call's point between the two reads.
    pub(crate) fn set(hook: impl FnOnce() + 'static) {
        HOOK.with_borrow_mut(|held| *held = Some(Box::new(hook)));
    }

    pub(crate) fn run() {
        if let Some(hook) = HOOK.with_borrow_mut(Option::take) {
            hook();
        }
    }
}

#[cfg(test)]
mod tests {
    //! The rows' findings against the whole disk check (0.47.0's), over
    //! every fixture corpus: for files sampled from each, edits that reach
    //! other files through their keys (an ID taken, dropped or given
    //! twice, a section, a mention, a file link, a status, a `depends_on`,
    //! broken front-matter, a created file). Equal, order included.

    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use super::{SetupFiles, introduced_from_rows};
    use crate::check::{NamedBytes, default_baseline, load_check};
    use crate::check_memo::{CheckMemo, scoped};
    use crate::index::SqliteIndex;
    use crate::source::{Source, WorkingTree};
    use crate::update::introduced_findings;
    use crate::{IndexWriter as _, SpecIndex as _};

    const TODAY: &str = "2026-11-15";

    fn corpora() -> Vec<PathBuf> {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut found = Vec::new();
        let mut stack = vec![(fixtures, 0)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && depth < 2 {
                    if path.join("specengine.toml").is_file() {
                        found.push(path.clone());
                    }
                    stack.push((path, depth + 1));
                }
            }
        }
        found.sort();
        found
    }

    /// Edits of `text` (the file at `path`), each a new text: `ids` are the
    /// corpus's document IDs, `other` another file's path.
    fn edits(text: &str, own: Option<&str>, ids: &[String], other: &str) -> Vec<String> {
        let foreign = ids.iter().find(|id| Some(id.as_str()) != own).cloned();
        let mut out = Vec::new();
        out.push(format!(
            "{text}\nSee [there]({other}#nowhere) and [it]({other}).\n"
        ));
        if let Some(id) = &foreign {
            out.push(format!("{text}\nAs {id} says, and ZZ-404.\n"));
            out.push(format!("{text}\n## Again {{#{id}}}\n\nOnce more.\n"));
            if let Some(own) = own {
                out.push(text.replacen(&format!("id: {own}"), &format!("id: {id}"), 1));
            }
            out.push(text.replacen("links:\n", &format!("links:\n  depends_on: [{id}]\n"), 1));
            let status = text
                .lines()
                .find(|line| line.starts_with("status:"))
                .map(str::to_owned);
            if let Some(status) = status {
                out.push(text.replacen(&status, &format!("status: superseded-by {id}"), 1));
            }
        }
        if let Some(own) = own {
            out.push(text.replacen(&format!("id: {own}\n"), "", 1));
            out.push(text.replacen(&format!("id: {own}"), &format!("id: {own}X"), 1));
        }
        out.push(text.replacen("---\n", "---\nbroken: [\n", 1));
        out.push(text.replacen("title:", "title: Renamed", 1));
        out.retain(|edited| edited != text);
        out
    }

    #[test]
    fn rows_equal_the_whole_disk_check_on_every_fixture() {
        let scratch = std::env::temp_dir().join(format!(
            "specengine-store-check-rows-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).expect("scratch");
        let mut cases = 0;
        let mut differ = Vec::new();
        for (at, root) in corpora().iter().enumerate() {
            let config = NamedBytes::read("specengine.toml", &root.join("specengine.toml"));
            let baseline = default_baseline(root);
            let Ok(setup) = load_check(&config, baseline.as_ref()) else {
                continue;
            };
            let slug = setup.project.slug().unwrap_or("corpus").to_owned();
            let Ok(tree) = WorkingTree::new(root, &setup.project.paths) else {
                continue;
            };
            let mut index =
                SqliteIndex::open(scratch.join(format!("{at}.db")), &slug, root).expect("open");
            index.update(&tree, &setup.project.scheme).expect("update");
            let input = index.indexed_input().expect("decode");
            let ids: Vec<String> = input
                .files
                .iter()
                .filter_map(|file| file.parsed.as_ref()?.document()?.id.clone())
                .collect();
            let files: Vec<String> = input.files.iter().map(|file| file.path.clone()).collect();
            let memo = Arc::new(CheckMemo::new());
            let setup_files = SetupFiles {
                config: &config,
                baseline: baseline.as_ref(),
            };
            for (n, path) in files.iter().enumerate().take(14) {
                let Ok(bytes) = tree.read(path) else {
                    continue;
                };
                let Ok(text) = String::from_utf8(bytes) else {
                    continue;
                };
                let own = input.files[n]
                    .parsed
                    .as_ref()
                    .and_then(|parsed| parsed.document()?.id.clone());
                let other = files
                    .iter()
                    .find(|other| *other != path)
                    .and_then(|other| other.rsplit('/').next())
                    .unwrap_or("x.md")
                    .to_owned();
                let mut texts = edits(&text, own.as_deref(), &ids, &other);
                let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
                let created = if dir.is_empty() {
                    "zz-created.md".to_owned()
                } else {
                    format!("{dir}/zz-created.md")
                };
                let mut targets: Vec<(String, String)> = texts
                    .drain(..)
                    .map(|edited| (path.clone(), edited))
                    .collect();
                if let Some(id) = ids.first() {
                    targets.push((
                        created.clone(),
                        format!("---\nid: {id}\nclass: decision\n---\n\n# Made\n\nCites {id}.\n"),
                    ));
                }
                for (target, edited) in targets {
                    let whole = introduced_findings(
                        &tree,
                        &setup,
                        TODAY,
                        &target,
                        edited.clone().into_bytes(),
                    );
                    for memo_on in [false, true] {
                        let run = || {
                            introduced_from_rows(
                                Some(&index),
                                &tree,
                                &setup,
                                setup_files,
                                TODAY,
                                &target,
                                edited.clone().into_bytes(),
                            )
                        };
                        let rows = if memo_on {
                            scoped(&memo, "-", || {
                                // Warm: the first call is the cold path.
                                run();
                                run()
                            })
                        } else {
                            run()
                        };
                        cases += 1;
                        if rows.findings != whole || rows.doubted {
                            differ.push(format!(
                                "{}: {target} (memo {memo_on}): rows {:?}\nwhole {:?}",
                                root.display(),
                                rows.findings,
                                whole
                            ));
                        }
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&scratch);
        assert!(cases > 300, "{cases} cases");
        assert!(
            differ.is_empty(),
            "{} of {cases} differ:\n{}",
            differ.len(),
            differ.join("\n\n")
        );
    }

    /// A neutral scheme, one name-shaped prefix.
    const KNOTS: &str = "[project]\nname = \"knots\"\nslug = \"knots\"\n\n[ids]\n\
                         KNT = { kind = \"knot\", shape = \"name\" }\n";

    /// A document `id` with a title, `extra` front-matter lines after it.
    fn knot(id: &str, extra: &str) -> String {
        format!("---\nid: {id}\ntitle: {id}\n{extra}---\n\n# {id}\n\nA knot.\n")
    }

    /// A cycle closed by P whose first member lies outside the set: its line
    /// is that member's, read from its bytes, as the whole check reads it.
    /// `depends_on` A→B→C, C (P) adds →A: the set is {C, B}, A first by
    /// name. `parent:` one→two→three, three (P) adds →one: the set is
    /// {three, two}, one first by path. Equal to the whole disk check, with
    /// the memo and without; the cycle's line is past the first. M: the
    /// first member's bytes left unread (its line 1).
    #[test]
    fn a_cycle_whose_first_member_is_outside_the_set_takes_its_line() {
        let scratch = std::env::temp_dir().join(format!(
            "specengine-store-check-rows-cycles-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        let root = scratch.join("wt");
        let spec = root.join("docs/spec");
        std::fs::create_dir_all(&spec).expect("scratch");
        std::fs::write(root.join("specengine.toml"), KNOTS).expect("config");
        let files = [
            (
                "a-alpha.md",
                knot("KNT-ALPHA", "links:\n  depends_on: [KNT-BETA]\n"),
            ),
            (
                "b-beta.md",
                knot("KNT-BETA", "links:\n  depends_on: [KNT-GAMMA]\n"),
            ),
            ("c-gamma.md", knot("KNT-GAMMA", "")),
            ("p-one.md", knot("KNT-ONE", "parent: KNT-TWO\n")),
            ("p-two.md", knot("KNT-TWO", "parent: KNT-THREE\n")),
            ("p-three.md", knot("KNT-THREE", "")),
        ];
        for (file, text) in &files {
            std::fs::write(spec.join(file), text).expect("a document");
        }
        let root = root.canonicalize().expect("canonical");
        let config = NamedBytes::read("specengine.toml", &root.join("specengine.toml"));
        let setup = load_check(&config, None).expect("the setup");
        let tree = WorkingTree::new(&root, &setup.project.paths).expect("a tree");
        let mut index = SqliteIndex::open(scratch.join("index.db"), "knots", &root).expect("open");
        index.update(&tree, &setup.project.scheme).expect("update");
        let files = SetupFiles {
            config: &config,
            baseline: None,
        };
        let memo = Arc::new(CheckMemo::new());
        let cases = [
            (
                "docs/spec/c-gamma.md",
                knot("KNT-GAMMA", "links:\n  depends_on: [KNT-ALPHA]\n"),
                "depends-cycle",
                "docs/spec/a-alpha.md",
            ),
            (
                "docs/spec/p-three.md",
                knot("KNT-THREE", "parent: KNT-ONE\n"),
                "parent-cycle",
                "docs/spec/p-one.md",
            ),
        ];
        for (path, text, code, first) in cases {
            let whole = introduced_findings(&tree, &setup, TODAY, path, text.clone().into_bytes());
            let cycle = whole
                .iter()
                .find(|finding| finding.code == code)
                .unwrap_or_else(|| panic!("{code} introduced: {whole:?}"));
            assert_eq!(cycle.path, first, "{whole:?}");
            assert!(cycle.line > 1, "{code}'s line is its member's: {whole:?}");
            let run = || {
                introduced_from_rows(
                    Some(&index),
                    &tree,
                    &setup,
                    files,
                    TODAY,
                    path,
                    text.clone().into_bytes(),
                )
            };
            let bare = run();
            assert_eq!(bare.findings, whole, "{code}, no memo");
            assert!(!bare.doubted);
            let (warm, hit) = scoped(&memo, "-", || {
                run();
                (run(), run())
            });
            assert_eq!(warm.findings, whole, "{code}, the memo's miss");
            assert_eq!(hit.findings, whole, "{code}, the memo's hit");
        }
        let _ = std::fs::remove_dir_all(&scratch);
    }

    /// WC20, one snapshot: another connection commits rows between the
    /// digest and the row reads (a file defining the ID P mentions). The
    /// check never mixes the two states: it answers as the state its digest
    /// names (the mention dangling), and the next call, whose walk saw the
    /// new file, as that one. M: the rows read in another transaction than
    /// the digest's (the holder found, the mention resolved): red.
    #[test]
    fn wc20_the_digest_and_the_rows_are_one_snapshot() {
        let scratch = std::env::temp_dir().join(format!(
            "specengine-store-check-rows-snapshot-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        let root = scratch.join("wt");
        let spec = root.join("docs/spec");
        std::fs::create_dir_all(&spec).expect("scratch");
        std::fs::write(root.join("specengine.toml"), KNOTS).expect("config");
        std::fs::write(spec.join("a-alpha.md"), knot("KNT-ALPHA", "")).expect("a document");
        let root = root.canonicalize().expect("canonical");
        let config = NamedBytes::read("specengine.toml", &root.join("specengine.toml"));
        let setup = load_check(&config, None).expect("the setup");
        let tree = WorkingTree::new(&root, &setup.project.paths).expect("a tree");
        let db = scratch.join("index.db");
        let mut index = SqliteIndex::open(&db, "knots", &root).expect("open");
        index.update(&tree, &setup.project.scheme).expect("update");
        let files = SetupFiles {
            config: &config,
            baseline: None,
        };
        let path = "docs/spec/b-beta.md";
        let text = format!("{}\nSee KNT-LATE.\n", knot("KNT-BETA", ""));
        let before = introduced_findings(&tree, &setup, TODAY, path, text.clone().into_bytes());
        assert!(
            before.iter().any(|finding| finding.subject == "KNT-LATE"),
            "premise: the mention dangles: {before:?}"
        );
        // Between the digest and the rows: the file lands and another
        // connection indexes it, committed.
        let (late_root, late_db, scheme) = (root.clone(), db.clone(), setup.project.scheme.clone());
        let paths = setup.project.paths.clone();
        super::between::set(move || {
            std::fs::write(late_root.join("docs/spec/c-late.md"), knot("KNT-LATE", ""))
                .expect("the late file");
            let mut other = SqliteIndex::open(&late_db, "knots", &late_root).expect("open");
            let tree = WorkingTree::new(&late_root, &paths).expect("a tree");
            let report = other.update(&tree, &scheme).expect("the other commit");
            assert_eq!(report.parsed, 1, "{report:?}");
        });
        let during = introduced_from_rows(
            Some(&index),
            &tree,
            &setup,
            files,
            TODAY,
            path,
            text.clone().into_bytes(),
        );
        assert_eq!(during.findings, before, "the digest's state, whole");
        let after = introduced_from_rows(
            Some(&index),
            &tree,
            &setup,
            files,
            TODAY,
            path,
            text.clone().into_bytes(),
        );
        let now = introduced_findings(&tree, &setup, TODAY, path, text.into_bytes());
        assert!(!now.iter().any(|finding| finding.subject == "KNT-LATE"));
        assert_eq!(after.findings, now, "the next state, whole");
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
