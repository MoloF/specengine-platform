//! `spec lock rebase` (task spec `lock-rebase`; its writers: canon
//! `architecture-code`, "`spec.lock` writers"): a lock of format 1, or of
//! another `code_recipe`, carried into this build's format and recipes on
//! the owner's terminal, each entry re-derived from its own `file_blob`.
//!
//! In order (`main` refuses a stdin that is no terminal before anything is
//! read):
//!
//! 1. the project, its root `specengine.toml` (`--config` naming another
//!    is refused), `[code]` (none: exit 2, `spec refs`' text);
//! 2. git: a worktree, `HEAD` born and on a branch, no operation in
//!    progress, `spec.lock` not unmerged (a merge in progress), else exit 2;
//! 3. the inputs `spec.lock` and `specengine.toml` tracked and equal to
//!    `HEAD`, else each named, exit 1; no lock: exit 1. No walk: dirty code
//!    is never read;
//! 4. the lock: not UTF-8, unreadable, newer, or of another `norm_recipe`
//!    (no rebase crosses it): exit 2 with the reader's text; already this
//!    build's: exit 0 `nothing to change`, no question;
//! 5. every entry's blob read by its OID in one session
//!    ([`VerifyGit::read_blobs`]: raw, no filter, never fetched), never
//!    `HEAD`'s tree, the index, the working tree or the entry's `commit`; a
//!    blob parsed once however many entries name it (a parser panic or no
//!    tree: exit 2 naming the file and the OID, nothing written);
//! 6. each entry, in key order, located in its blob ("Location" below),
//!    given its levels and hashed into format 2, or dropped with the first
//!    reason that applies (core [`DropReason`]); core's [`plan_rebase`]
//!    keeps every key, `rev`, `spec_hash`, `file_blob`, `commit` and
//!    `verified_at` as they were;
//! 7. the rows, the committer identity, the one `[y/N]`, `HEAD`, the branch
//!    and the lock's bytes read again, then the lock commit accept's (canon
//!    `spec-lock-accept`, "The lock commit") under its rebase subject:
//!    `spec: lock rebase`, `Lock-mode: rebase`, `Decided-by:`, no `Lock:`
//!    and no `Proposal:` (no queue item runs a rebase, so it completes
//!    none).
//!
//! **Location**, in the blob alone: names by `placed_name_file` over a file
//! in no package, so an item's in-file path is every segment of its path
//! but the package (a twin's discriminator kept). An `item` entry's
//! candidates: the items of its `item_kind` whose in-file path is the
//! symbol or ends it after a `::`; a `module` entry's: the file's module,
//! always, and each inline `mod` whose in-file path so ends it; a `ron`
//! entry's: the comments anchored to its field path (an ambiguous anchor:
//! `ambiguous`). Of several candidates, those a marker of the blob binds
//! for the entry's node and relation are kept; one left is located, none
//! or several `ambiguous`. A marker names the node by its ID's bytes, or
//! by that ID's resolution in the working-tree corpus.
//!
//! **Levels**: a format-2 entry keeps its own; a format-1 entry takes the
//! union of the effective levels of the blob's markers giving its key, a
//! malformed list dropping it `bad_levels`, none at all (an alias gone from
//! the corpus) `sig, body`, what its whole-item digest compared.
//!
//! **Hashed** as accept hashes: an item by `item_levels(`, an inline `mod`
//! by its `mod` item's, the file's module by `module_levels(`, a RON value
//! by `value_hashes` paired with its comment by range (canon `code-hash-ron`,
//! "RON value"). A format-1 lock at this build's whole-item recipe
//! cross-checks each `item` entry: `hash_bound_items(` over the located
//! item must give its `ast_hash` again (read by `.digest()`), else
//! `hash_mismatch`; any other lock is located without it, under one note.
//!
//! Writes the data directory (the index refresh, the commit message's
//! scratch file), `spec.lock` and one commit; no queue row or event.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use serde::Serialize;
use specengine_code::{
    Anchor, DEFAULT_LEVELS, LevelState, Levels, MOD_KIND, NamedFile, RECIPE, RonValues, RustParser,
    bind_tree, comment_anchors, grammar_info, hash_bound_items, item_levels, markers_in,
    module_levels, placed_name_file, value_hashes,
};
use specengine_core::lock::{
    DropReason, EntryCode, EntryKey, LOCK_FILE, LOCK_FORMAT, LevelCode, Lock, LockFormat, LockKind,
    LockLevel, LockRelation, Recipes, Rederived, Verified, parse_lock, plan_rebase,
};
use specengine_store::{GitEnv, ObjectBlob, VerifyGit};

use crate::corpus::indexed;
use crate::lock::{
    Hashed, LockConsent, LockOutcome, LockSubject, LockWritten, ReadLock, Repo, Target,
    build_recipes, dirty_inputs, dirty_reason, git_error, level_code, lock_digest, lock_level,
    read_lock_bytes, relation_of, write_and_commit,
};
use crate::project::{CONFIG_FILE, ProjectRoot, discover_with_text};
use crate::proposals::require_root_config;
use crate::refs::{Corpus, code_config};
use crate::verify_rows::node_id;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// Step 3's refusal when there is no lock.
const NO_LOCK: &str = "no spec.lock: nothing to rebase";

/// A row's change.
const REBASED: &str = "rebased";
const DROPPED: &str = "dropped";

/// `spec lock rebase` options.
#[derive(Debug, Clone)]
pub struct LockRebaseRequest {
    pub git: GitEnv,
}

/// A lock's format and `code_recipe`, as `--json` shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebaseSide {
    pub format: i64,
    pub code_recipe: String,
}

/// Entries kept and dropped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RebaseCounts {
    pub rebased: usize,
    pub dropped: usize,
}

/// One entry: `rebased|dropped | <node> | <relation> | <file> | <kind>
/// <symbol>[ <item_kind>][ | <reason>]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebaseRow {
    /// `rebased`, `dropped`.
    pub change: &'static str,
    pub node: String,
    pub relation: &'static str,
    pub file: String,
    /// `item`, `module`, `ron`.
    pub kind: &'static str,
    pub symbol: String,
    pub item_kind: Option<String>,
    /// A rebased entry's levels, in canonical order; `None` when dropped.
    pub levels: Option<Vec<&'static str>>,
    /// Why it is dropped (`blob_missing`, …, `cannot_verify <category>`,
    /// `hash_mismatch`); `None` when rebased.
    pub reason: Option<String>,
}

/// What `spec lock rebase` did; `--json` prints its seven keys, every one
/// present, an absent value null.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebaseOutcome {
    /// The lock read (`None` before it is: a dirty input, no lock).
    pub from: Option<RebaseSide>,
    /// This build's.
    pub to: RebaseSide,
    pub counts: RebaseCounts,
    /// In key order.
    pub rows: Vec<RebaseRow>,
    /// The lock's commit, when one was made and named: also when it fails
    /// a check or cannot be verified (exit 1, the refusal naming it).
    pub commit: Option<String>,
    /// Exit 0 with nothing written: the lock is already this build's.
    pub unchanged: bool,
    /// Why nothing was written, or the commit is unverified (exit 1).
    pub refusal: Option<String>,
    #[serde(skip)]
    pub messages: Vec<Message>,
    /// The rows' text still to print; `None` once the prompt showed it.
    #[serde(skip)]
    pub report: Option<String>,
    /// What the run left written.
    #[serde(skip)]
    pub written: LockWritten,
}

impl RebaseOutcome {
    fn new(build: &Recipes) -> Self {
        Self {
            from: None,
            to: RebaseSide {
                format: LOCK_FORMAT,
                code_recipe: build.code.clone(),
            },
            counts: RebaseCounts::default(),
            rows: Vec::new(),
            commit: None,
            unchanged: false,
            refusal: None,
            messages: Vec::new(),
            report: None,
            written: LockWritten::None,
        }
    }

    /// Exit 1 with `reason`, nothing written.
    fn refused(mut self, reason: &str) -> Self {
        self.refusal = Some(one_line(reason));
        self
    }

    /// 1 when refused, else 0.
    pub fn exit(&self) -> Exit {
        if self.refusal.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }
}

impl RebaseSide {
    fn of(lock: &Lock) -> Self {
        Self {
            format: lock.format.number(),
            code_recipe: lock.recipes.code.clone(),
        }
    }
}

/// The `code_recipe` a format-1 lock of this build would name: the
/// whole-item recipe and this build's grammar. Only such a lock's items
/// are cross-checked.
fn whole_item_recipe() -> String {
    let grammar = grammar_info();
    format!(
        "{RECIPE} tree-sitter-rust/{} abi/{}",
        grammar.tree_sitter_rust, grammar.abi
    )
}

/// `spec lock rebase` (see the module documentation).
pub fn lock_rebase(
    env: &Env,
    globals: &Globals,
    request: &LockRebaseRequest,
    consent: LockConsent<'_>,
) -> Result<RebaseOutcome, CliError> {
    let build = build_recipes();
    let mut outcome = RebaseOutcome::new(&build);

    // The project, its config, `[code]`.
    let (project, text) = discover_with_text(env, globals)?;
    require_root_config(env, globals, &project)?;
    project.slug()?;
    code_config(&project, &text)?;

    // Git: no operation in progress, the lock not unmerged.
    let repo = Repo::open(&project, &request.git)?;
    let top_path = repo.top(LOCK_FILE);
    if !repo
        .git
        .unmerged_stages(&top_path)
        .map_err(git_error)?
        .is_empty()
    {
        return Err(CliError::spec(format!(
            "a merge is in progress in {}: finish or abort it first; nothing written",
            repo.worktree
        )));
    }

    // The inputs, tracked and clean; the lock.
    let bytes = read_lock_bytes(&project.root)?;
    let inputs: BTreeSet<String> = [CONFIG_FILE, LOCK_FILE]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let dirty = dirty_inputs(&repo, &inputs, bytes.is_some())?;
    if !dirty.is_empty() {
        return Ok(outcome.refused(&dirty_reason(&dirty)));
    }
    let Some(bytes) = bytes else {
        return Ok(outcome.refused(NO_LOCK));
    };
    let old = match read_old(&bytes, &build)? {
        Read::Old(old) => old,
        Read::Current(current) => {
            outcome.from = Some(RebaseSide::of(&current));
            outcome.unchanged = true;
            return Ok(outcome);
        }
    };
    outcome.from = Some(RebaseSide::of(&old));
    let cross_check = old.format == LockFormat::One && old.recipes.code == whole_item_recipe();
    if !cross_check {
        outcome.messages.push(Message::Note(format!(
            "rebase: \"{}\" is not this build's: located without a hash cross-check",
            old.recipes.code
        )));
    }

    // Every entry's blob, by its OID alone, in one session.
    let objects = read_objects(&project.root, &request.git, &old)?;

    // The working-tree corpus: which node a marker's ID names.
    let input = indexed(env, &project, &mut outcome.messages, false)?;
    let mut corpus = Corpus::new(&input, &project.config.scheme, &project.config.paths);
    let mut names = NodeNames {
        corpus: &mut corpus,
        project: &project,
    };

    // Each blob read once, every entry naming it re-derived from it.
    let mut groups: BTreeMap<(&str, Language), Vec<(&EntryKey, &Verified)>> = BTreeMap::new();
    for (key, verified) in &old.entries {
        let language = match key.kind {
            LockKind::Item | LockKind::Module => Language::Rust,
            LockKind::Ron => Language::Ron,
        };
        groups
            .entry((verified.file_blob.as_str(), language))
            .or_default()
            .push((key, verified));
    }
    let mut parser = None;
    let mut answers: BTreeMap<EntryKey, Rederived> = BTreeMap::new();
    for ((oid, language), entries) in &groups {
        // No object of that OID, or another type: `blob_missing`.
        let Some(ObjectBlob::Bytes(bytes)) = objects.get(*oid) else {
            for (key, _) in entries {
                answers.insert((*key).clone(), Err(DropReason::BlobMissing));
            }
            continue;
        };
        let Ok(text) = std::str::from_utf8(bytes) else {
            for (key, _) in entries {
                answers.insert((*key).clone(), Err(DropReason::NotUtf8));
            }
            continue;
        };
        let blob = Blob {
            oid,
            file: entries.first().map_or("", |(key, _)| key.file.as_str()),
            text,
            cross_check,
        };
        let rederived = match language {
            Language::Rust => {
                let parser = match &mut parser {
                    Some(parser) => parser,
                    None => parser.insert(RustParser::new().map_err(|error| {
                        CliError::spec(format!("the Rust grammar cannot be loaded: {error}"))
                    })?),
                };
                rederive_rust(&blob, parser, entries, &mut names)?
            }
            Language::Ron => rederive_ron(&blob, entries, &mut names)?,
        };
        answers.extend(rederived);
    }

    // The plan: every entry answered above, in key order.
    let plan = plan_rebase(&old, &build, |key, _| {
        answers.remove(key).ok_or_else(|| {
            CliError::spec(format!(
                "`{}` {} `{}` was not re-derived; nothing written",
                key.node,
                key.relation.as_str(),
                key.symbol
            ))
        })
    })?;
    outcome.rows = plan
        .rows
        .iter()
        .map(|row| shown(&row.key, &row.result))
        .collect();
    outcome.counts = RebaseCounts {
        rebased: plan.rebased(),
        dropped: plan.dropped(),
    };
    let rows = rows_text(&outcome.rows);
    let question = format!(
        "rebase {LOCK_FILE} (format {} -> {LOCK_FORMAT}, \"{}\" -> \"{}\": {} rebased, {} dropped) \
         at {} on {} in {}? [y/N]",
        old.format.number(),
        old.recipes.code,
        build.code,
        outcome.counts.rebased,
        outcome.counts.dropped,
        repo.head,
        repo.branch,
        repo.worktree
    );
    let read = ReadLock {
        bytes: Some(bytes),
        lock: old,
    };
    let committed = write_and_commit(
        env,
        &project,
        &repo,
        &read,
        &plan.lock,
        &LockSubject::Rebase { rows: &rows },
        question,
        LockOutcome::of_rebase(),
        consent,
        &[],
    )?;
    outcome.commit = committed.commit;
    outcome.refusal = committed.refusal;
    outcome.report = committed.report;
    outcome.written = committed.written;
    outcome.messages.extend(committed.messages);
    Ok(outcome)
}

/// The lock as step 4 reads it.
enum Read {
    /// Already this build's format and recipes: nothing to change.
    Current(Lock),
    /// Format 1 at any `code_recipe`, or format 2 at another, of this
    /// build's `norm_recipe`.
    Old(Lock),
}

/// The lock read: with the build's recipes first, then without them; exit
/// 2 when it is not UTF-8 (`read_lock`'s text), cannot be read without the
/// build's recipes either (that reader's text), or names another
/// `norm_recipe` (the strict reader's text, verbatim: no rebase crosses
/// it).
fn read_old(bytes: &[u8], build: &Recipes) -> Result<Read, CliError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| CliError::cannot(format!("{LOCK_FILE}: not UTF-8: SpecEngine writes it")))?;
    let strict = match parse_lock(text, Some(build)) {
        Ok(current) => return Ok(Read::Current(current)),
        Err(error) => error,
    };
    let old = parse_lock(text, None).map_err(|error| CliError::cannot(error.to_string()))?;
    if old.recipes.norm != build.norm {
        return Err(CliError::cannot(strict.to_string()));
    }
    Ok(Read::Old(old))
}

/// Every entry's `file_blob`, read by OID in one session that ends
/// cleanly; a git failure: exit 2.
fn read_objects(
    root: &Path,
    env: &GitEnv,
    old: &Lock,
) -> Result<BTreeMap<String, ObjectBlob>, CliError> {
    let cause = |cause: specengine_core::check::Cause| {
        CliError::spec(format!("git: {}; nothing written", cause.message))
    };
    let mut git = VerifyGit::open(root, env).map_err(cause)?;
    let read = git.read_blobs(
        old.entries
            .values()
            .map(|verified| verified.file_blob.as_str()),
    );
    let finished = git.finish();
    let read = read.map_err(cause)?;
    finished.map_err(cause)?;
    Ok(read)
}

/// Which reader a blob takes, by its entries' kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Language {
    Rust,
    Ron,
}

/// One blob, UTF-8, and what its entries share.
struct Blob<'a> {
    oid: &'a str,
    /// The first entry's file (key order): what a refusal names.
    file: &'a str,
    text: &'a str,
    /// A format-1 lock at this build's whole-item recipe.
    cross_check: bool,
}

/// What a marker of the blob is read for: the unit it binds, its relation,
/// its ID and its level list.
struct BlobMarker<U> {
    unit: U,
    relation: LockRelation,
    id: String,
    levels: Levels,
}

/// A Rust unit of a blob: an item, or a module (the file's, `None`, or an
/// inline `mod`'s by its item).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Item(usize),
    Module(Option<usize>),
}

/// Which node a marker's ID names: its bytes, or its resolution in the
/// working-tree corpus (as accept's).
struct NodeNames<'c, 'a> {
    corpus: &'c mut Corpus<'a>,
    project: &'c ProjectRoot,
}

impl NodeNames<'_, '_> {
    /// The marker ID `id` names the node `node` (its canonical cited ID).
    fn names(&mut self, id: &str, node: &str) -> bool {
        if id == node {
            return true;
        }
        let judged = self.corpus.judge(id);
        judged
            .nodes
            .iter()
            .any(|&at| node_id(&*self.corpus, self.project, at).as_deref() == Some(node))
    }
}

/// The markers of `markers` giving `key` at `unit`: bound to it, of the
/// key's relation, naming its node.
fn giving<'m, U: PartialEq>(
    markers: &'m [BlobMarker<U>],
    unit: &U,
    key: &EntryKey,
    names: &mut NodeNames<'_, '_>,
) -> Vec<&'m BlobMarker<U>> {
    markers
        .iter()
        .filter(|marker| {
            marker.unit == *unit
                && marker.relation == key.relation
                && names.names(&marker.id, &key.node)
        })
        .collect()
}

/// Of the candidates, the one located: one alone; else the only one a
/// marker giving the key binds. `None`: none or several (`ambiguous`).
fn located<U: PartialEq + Clone>(
    candidates: &[U],
    markers: &[BlobMarker<U>],
    key: &EntryKey,
    names: &mut NodeNames<'_, '_>,
) -> Option<U> {
    if let [only] = candidates {
        return Some(only.clone());
    }
    let mut marked = candidates
        .iter()
        .filter(|unit| !giving(markers, unit, key, names).is_empty());
    match (marked.next(), marked.next()) {
        (Some(one), None) => Some(one.clone()),
        _ => None,
    }
}

/// The levels an entry is written with: a format-2 entry's own; a format-1
/// entry's from the markers giving its key at the located unit, their
/// effective levels united (a malformed list: `bad_levels`; none: `sig,
/// body`).
fn entry_levels<U: PartialEq>(
    verified: &Verified,
    unit: &U,
    markers: &[BlobMarker<U>],
    key: &EntryKey,
    names: &mut NodeNames<'_, '_>,
) -> Result<BTreeSet<LockLevel>, DropReason> {
    if let EntryCode::Levels(code) = &verified.code {
        return Ok(code.levels.clone());
    }
    let giving = giving(markers, unit, key, names);
    if giving
        .iter()
        .any(|marker| matches!(marker.levels, Levels::Invalid(_)))
    {
        return Err(DropReason::BadLevels);
    }
    let levels: BTreeSet<LockLevel> = giving
        .iter()
        .flat_map(|marker| marker.levels.effective().unwrap_or_default())
        .copied()
        .map(lock_level)
        .collect();
    if levels.is_empty() {
        return Ok(DEFAULT_LEVELS.iter().copied().map(lock_level).collect());
    }
    Ok(levels)
}

/// `symbol` is `path`, or ends with it after a `::`.
fn ends_with_path(symbol: &str, path: &str) -> bool {
    symbol == path
        || symbol
            .strip_suffix(path)
            .is_some_and(|head| head.ends_with("::"))
}

/// What one entry of a Rust blob was located as, before hashing.
enum Found {
    Dropped(DropReason),
    At {
        unit: Unit,
        levels: BTreeSet<LockLevel>,
    },
}

/// The entries of one Rust blob, re-derived from it: parsed once (a panic
/// or no tree: exit 2), named in no package, each entry located, its
/// levels taken and its unit hashed, every item's levels in one call.
fn rederive_rust(
    blob: &Blob<'_>,
    parser: &mut RustParser,
    entries: &[(&EntryKey, &Verified)],
    names: &mut NodeNames<'_, '_>,
) -> Result<Vec<(EntryKey, Rederived)>, CliError> {
    let text = blob.text;
    let run = panic::catch_unwind(AssertUnwindSafe(|| {
        let tree = parser.parse(text)?;
        let binding = bind_tree(&tree, text);
        let analysis = &binding.analysis;
        let placed = placed_name_file(
            &NamedFile {
                path: Path::new(blob.file),
                package: None,
                analysis,
            },
            None,
        );
        let item_names = &placed.names.items;
        // Each item's in-file path: every segment but the package.
        let in_file: Vec<Option<String>> = placed
            .paths
            .iter()
            .zip(item_names)
            .map(|(path, name)| {
                let suffixes = path.suffixes(name.discriminator.as_deref());
                suffixes
                    .len()
                    .checked_sub(2)
                    .and_then(|at| suffixes.get(at).cloned())
            })
            .collect();
        let declarations: BTreeSet<(&str, &[String], usize)> = analysis
            .mod_declarations
            .iter()
            .map(|declared| {
                (
                    declared.name.as_str(),
                    declared.mod_path.as_slice(),
                    declared.line,
                )
            })
            .collect();
        let mut markers: Vec<BlobMarker<Unit>> = Vec::new();
        for comment in &binding.comments {
            let Some(comment_text) = text.get(comment.range.clone()) else {
                continue;
            };
            let unit = match Target::of_rust(
                comment.binding,
                analysis,
                item_names,
                Some(&placed.names.module),
            ) {
                Target::Item { index, .. } => Unit::Item(index),
                Target::Module { item, .. } => Unit::Module(item),
                Target::Ron { .. } | Target::Keyless { .. } => continue,
            };
            for marker in markers_in(comment_text) {
                markers.push(BlobMarker {
                    unit,
                    relation: relation_of(&marker),
                    id: marker.id,
                    levels: marker.levels,
                });
            }
        }

        // Located, and the levels.
        let found: Vec<Found> = entries
            .iter()
            .map(|(key, verified)| {
                let matches = |index: usize| {
                    in_file
                        .get(index)
                        .and_then(Option::as_deref)
                        .is_some_and(|path| ends_with_path(&key.symbol, path))
                };
                let candidates: Vec<Unit> = match key.kind {
                    LockKind::Item => analysis
                        .items
                        .iter()
                        .enumerate()
                        .filter(|(index, item)| {
                            key.item_kind.as_deref() == Some(item.kind.as_str()) && matches(*index)
                        })
                        .map(|(index, _)| Unit::Item(index))
                        .collect(),
                    LockKind::Module => std::iter::once(Unit::Module(None))
                        .chain(
                            analysis
                                .items
                                .iter()
                                .enumerate()
                                .filter(|(index, item)| {
                                    item.kind == MOD_KIND
                                        && !declarations.contains(&(
                                            item.name.as_deref().unwrap_or_default(),
                                            item.mod_path.as_slice(),
                                            item.line,
                                        ))
                                        && matches(*index)
                                })
                                .map(|(index, _)| Unit::Module(Some(index))),
                        )
                        .collect(),
                    LockKind::Ron => Vec::new(),
                };
                if candidates.is_empty() {
                    return Found::Dropped(DropReason::SymbolGone);
                }
                let Some(unit) = located(&candidates, &markers, key, names) else {
                    return Found::Dropped(DropReason::Ambiguous);
                };
                match entry_levels(verified, &unit, &markers, key, names) {
                    Ok(levels) => Found::At { unit, levels },
                    Err(reason) => Found::Dropped(reason),
                }
            })
            .collect();

        // Hashed: the items and inline `mod`s in one call, the file's
        // module once; the cross-check's items in one call.
        let wanted: Vec<usize> = found
            .iter()
            .filter_map(|found| match found {
                Found::At {
                    unit: Unit::Item(index) | Unit::Module(Some(index)),
                    ..
                } => Some(*index),
                _ => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let levels_of: BTreeMap<usize, LevelState> = wanted
            .iter()
            .copied()
            .zip(item_levels(&tree, text, &wanted))
            .filter_map(|(index, levels)| levels.map(|levels| (index, levels)))
            .collect();
        let module = found
            .iter()
            .any(|found| {
                matches!(
                    found,
                    Found::At {
                        unit: Unit::Module(None),
                        ..
                    }
                )
            })
            .then(|| module_levels(&tree, text));
        let checked: Vec<usize> = if blob.cross_check {
            found
                .iter()
                .zip(entries)
                .filter_map(|(found, (key, _))| match found {
                    Found::At {
                        unit: Unit::Item(index),
                        ..
                    } if key.kind == LockKind::Item => Some(*index),
                    _ => None,
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        let whole: BTreeMap<usize, String> = checked
            .iter()
            .copied()
            .zip(hash_bound_items(&tree, text, &checked))
            .filter_map(|(index, hashed)| {
                hashed
                    .and_then(|hashed| hashed.digest())
                    .map(|digest| (index, lock_digest(&digest)))
            })
            .collect();

        Some(
            found
                .into_iter()
                .zip(entries)
                .map(|(found, (key, verified))| {
                    let rederived = match found {
                        Found::Dropped(reason) => Err(reason),
                        Found::At { unit, levels } => {
                            let hashes = match unit {
                                Unit::Item(index) | Unit::Module(Some(index)) => {
                                    levels_of.get(&index)
                                }
                                Unit::Module(None) => module.as_ref(),
                            };
                            hashed_code(levels, hashes).and_then(|code| {
                                cross_checked(blob.cross_check, unit, verified, &whole)
                                    .map(|()| code)
                            })
                        }
                    };
                    ((*key).clone(), rederived)
                })
                .collect::<Vec<_>>(),
        )
    }));
    match run {
        Ok(Some(rederived)) => Ok(rederived),
        Ok(None) => Err(CliError::spec(format!(
            "`{}` at blob {} cannot be parsed (the parser gave no tree); nothing written",
            blob.file, blob.oid
        ))),
        Err(_) => Err(CliError::spec(format!(
            "the Rust parser failed on `{}` at blob {}; nothing written",
            blob.file, blob.oid
        ))),
    }
}

/// A located unit's levels and digests, or `cannot_verify` with its first
/// category.
fn hashed_code(
    levels: BTreeSet<LockLevel>,
    hashes: Option<&LevelState>,
) -> Result<LevelCode, DropReason> {
    match hashes {
        Some(LevelState::Hashed(hashes)) => Ok(level_code(levels, &Hashed::Levels(hashes))),
        Some(LevelState::CannotVerify { categories }) => Err(DropReason::CannotVerify(
            categories
                .first()
                .map(|category| category.as_str().to_owned()),
        )),
        None => Err(DropReason::CannotVerify(None)),
    }
}

/// The cross-check of a format-1 `item` entry at this build's whole-item
/// recipe: its `ast_hash` made again from the located item, else
/// `hash_mismatch`. Any other entry passes.
fn cross_checked(
    cross_check: bool,
    unit: Unit,
    verified: &Verified,
    whole: &BTreeMap<usize, String>,
) -> Result<(), DropReason> {
    let (true, Unit::Item(index), EntryCode::Whole(Some(ast_hash))) =
        (cross_check, unit, &verified.code)
    else {
        return Ok(());
    };
    match whole.get(&index) {
        Some(digest) if digest == ast_hash => Ok(()),
        _ => Err(DropReason::HashMismatch),
    }
}

/// The entries of one RON blob, re-derived from it: its comments anchored
/// once and its values hashed once (a panic: exit 2), each entry located at
/// its field path, its levels taken, its value's hash paired with its
/// comment by range.
fn rederive_ron(
    blob: &Blob<'_>,
    entries: &[(&EntryKey, &Verified)],
    names: &mut NodeNames<'_, '_>,
) -> Result<Vec<(EntryKey, Rederived)>, CliError> {
    let text = blob.text;
    let read = panic::catch_unwind(|| (comment_anchors(text), value_hashes(text)));
    let Ok((anchors, values)) = read else {
        return Err(CliError::spec(format!(
            "the RON reader failed on `{}` at blob {}; nothing written",
            blob.file, blob.oid
        )));
    };
    let mut markers: Vec<BlobMarker<String>> = Vec::new();
    for anchor in &anchors {
        let Some(comment_text) = text.get(anchor.comment.clone()) else {
            continue;
        };
        let Target::Ron { path, .. } = Target::of_ron(&anchor.anchor) else {
            continue;
        };
        for marker in markers_in(comment_text) {
            markers.push(BlobMarker {
                unit: path.clone(),
                relation: relation_of(&marker),
                id: marker.id,
                levels: marker.levels,
            });
        }
    }
    let mut rederived = Vec::with_capacity(entries.len());
    for (key, verified) in entries {
        let at_path: Vec<&specengine_code::CommentAnchor> = anchors
            .iter()
            .filter(|anchor| match &anchor.anchor {
                Anchor::Path { path, .. } | Anchor::Ambiguous { path, .. } => *path == key.symbol,
                Anchor::Unanchored | Anchor::CannotVerify => false,
            })
            .collect();
        let result = if at_path.is_empty() {
            Err(DropReason::SymbolGone)
        } else if at_path
            .iter()
            .any(|anchor| matches!(anchor.anchor, Anchor::Ambiguous { .. }))
        {
            Err(DropReason::Ambiguous)
        } else {
            entry_levels(verified, &key.symbol, &markers, key, names).and_then(|levels| {
                match &values {
                    RonValues::CannotVerify(category) => {
                        Err(DropReason::CannotVerify(Some((*category).to_owned())))
                    }
                    RonValues::Hashed(hashed) => at_path
                        .first()
                        .and_then(|anchor| {
                            hashed
                                .iter()
                                .find(|(range, _)| *range == anchor.comment)
                                .map(|(_, digest)| *digest)
                        })
                        .map(|digest| level_code(levels, &Hashed::Ron(digest)))
                        .ok_or(DropReason::CannotVerify(None)),
                }
            })
        };
        rederived.push(((*key).clone(), result));
    }
    Ok(rederived)
}

/// A plan's row as shown.
fn shown(key: &EntryKey, result: &Result<BTreeSet<LockLevel>, DropReason>) -> RebaseRow {
    let (change, levels, reason) = match result {
        Ok(levels) => (
            REBASED,
            Some(levels.iter().map(|level| level.as_str()).collect()),
            None,
        ),
        Err(reason) => (DROPPED, None, Some(reason.text())),
    };
    RebaseRow {
        change,
        node: key.node.clone(),
        relation: key.relation.as_str(),
        file: key.file.clone(),
        kind: key.kind.as_str(),
        symbol: key.symbol.clone(),
        item_kind: key.item_kind.clone(),
        levels,
        reason,
    }
}

/// `<change> | <node> | <relation> | <file> | <kind> <symbol>[
/// <item_kind>][ | <reason>]`, one line each.
fn rows_text(rows: &[RebaseRow]) -> String {
    let mut out = String::new();
    for row in rows {
        let mut binding = format!("{} {}", row.kind, one_line(&row.symbol));
        if let Some(item_kind) = &row.item_kind {
            binding.push(' ');
            binding.push_str(&one_line(item_kind));
        }
        out.push_str(&format!(
            "{} | {} | {} | {} | {binding}",
            row.change,
            one_line(&row.node),
            row.relation,
            one_line(&row.file)
        ));
        if let Some(reason) = &row.reason {
            out.push_str(" | ");
            out.push_str(&one_line(reason));
        }
        out.push('\n');
    }
    out
}

/// The outcome as stdout text: the rows not shown yet, then `committed
/// <commit>` (a verified commit) or `nothing to change`. A refusal (stderr)
/// ends nothing here, even one naming a commit made.
pub(crate) fn render_text(outcome: &RebaseOutcome) -> String {
    let mut out = outcome
        .report
        .as_deref()
        .map(escape_controls)
        .unwrap_or_default();
    if outcome.refusal.is_some() {
        return out;
    }
    if let Some(commit) = &outcome.commit {
        out.push_str(&format!("committed {commit}\n"));
    } else if outcome.unchanged {
        out.push_str("nothing to change\n");
    }
    out
}
