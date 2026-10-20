//! `spec verify [--changed | --staged] [--json]`
//! (canon `spec-verify`, "Rows", "Verdict and exit"; canon
//! `spec-verify-output`, "Hints"): drift between `spec.lock`
//! and the tree, one row per lock key, each with a state and a class (Fix,
//! Check, Cannot), Pre-existing with `--changed` when `HEAD` gives the
//! same; the verdict and exit under `[verify] mode` (canon
//! `architecture-code`, "Drift: `spec verify`"). Read-only: only the data
//! directory is written (the index refresh); never `spec.lock`, the
//! worktree, a git object, a queue row or an event.
//!
//! In order: discovery (its failure: exit 2, stdout empty); then every
//! failure is a run cause of the printed report, exit 2 in every mode, its
//! `cannot:` lines in this order: (1) a config error, `[code]`'s or
//! `[verify]`'s; (2) no `[code]`; (3) a walk gap; (4) no worktree, no
//! `git`, a git failure; (5) `spec.lock` unmerged; (6) `spec.lock`
//! unreadable, not UTF-8, invalid, format 1 or of other recipes (core's
//! text, naming `spec lock rebase` but for another `norm_recipe`: the step
//! is the cause's text, no hint); (7) a partial base, `HEAD`'s lock
//! invalid, format 1 or of other recipes; (8) `HOME` or the data
//! directory. Every check
//! that can run does (the walk whenever `[code]` reads), so the first
//! `cannot:` line is the first cause in that order; the base is read only
//! when no other cause is present. A run with a cause judges no row.
//!
//! The rows come from [`crate::verify_rows`]'s judge over the one walk;
//! `--changed` reads `HEAD` through the same git session, the stricter of
//! the current and `HEAD`'s modes applying. Plain, `enforce-introduced`
//! has no base and is judged as `enforce`, with a note. The mode shown,
//! text and JSON, is always the one the verdict used.
//!
//! `--staged` (task spec `rev-rule`; `--changed` excluded, a usage error)
//! judges what a commit records, through the one-commit seam
//! ([`crate::verify_rows::read_commit`], `judge_commit`) fed the git
//! index's listing (`GIT_INDEX_FILE` kept, read as `spec check --staged`
//! reads it): the index's config whole unless `--config` (disk), its
//! documents, code files, manifests and lock from its blobs; no disk walk,
//! no `hash-object`, no data-directory refresh. Its base is `HEAD`, read
//! over the index's side (a shared path and blob neither read nor parsed
//! again), Pre-existing as `--changed` marks it. An unmerged path is (5)
//! each; an index blob that cannot be read or parsed (3).

use std::collections::BTreeSet;
use std::fs;

use serde::Serialize;
use specengine_core::check::Mode;
use specengine_core::drift::raises;
use specengine_core::lock::{LOCK_FILE, LockLevel, key_problem};
use specengine_core::verify::{Finding, RowBinding, State, Verdict, verdict, verify_from_toml};
use specengine_core::{CodeConfig, ProjectConfig};
use specengine_store::{CommitFile, CommitListing, GitEnv, IndexRefusal, VerifyGit};

use crate::code_walk::read_tree;
use crate::corpus::indexed;
use crate::lock::{build_recipes, read_lock};
use crate::project::{CONFIG_FILE, ProjectRoot, discover_with_text, locate};
use crate::verify_rows::{
    CAUSE_CONFIG, CAUSE_GIT, CAUSE_LOCK, CAUSE_NO_CODE, CAUSE_WALK, CommitConfig, HeadMode,
    IndexShare, Judged, Row, RunCause, Walked, judge_commit, judge_walk, read_commit, read_head,
};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// The run causes of "Verdict and exit", by place (the others are the
/// judge's, `verify_rows`).
const CAUSE_UNMERGED: u8 = 5;
const CAUSE_DATA: u8 = 8;

/// What `spec verify` judges, and whether against `HEAD`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerifiedTree {
    /// Plain: the working tree, no base.
    #[default]
    WorkingTree,
    /// `--changed`: the working tree, against `HEAD`.
    Changed,
    /// `--staged`: what the git index records, against `HEAD`.
    Staged,
}

/// `spec verify` options.
#[derive(Debug, Clone)]
pub struct VerifyRequest {
    /// Plain, `--changed` or `--staged`.
    pub tree: VerifiedTree,
    /// Git's environment (`--staged`: its `GIT_INDEX_FILE` names the index).
    pub git: GitEnv,
}

/// One marker of a row: its line and `@N` (`None`: unversioned or
/// malformed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyMarker {
    pub line: usize,
    pub rev: Option<u32>,
}

/// A row's lock entry, as shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyLock {
    pub rev: u32,
    pub commit: String,
    pub verified_at: String,
}

/// A row's levels as judged (canon `spec-verify`, "States"): `lock` the
/// entry's, `markers` its markers' effective levels united, `changed` the
/// compared levels whose digest changed (`null` when no digest was
/// compared), each in canonical order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyLevels {
    pub lock: Vec<&'static str>,
    pub markers: Vec<&'static str>,
    pub changed: Option<Vec<&'static str>>,
}

/// One row: a lock key (keyless: `kind`, `symbol`, `item_kind` null), its
/// state, class and facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyRow {
    pub node: String,
    pub relation: &'static str,
    pub file: String,
    pub kind: Option<&'static str>,
    pub symbol: Option<String>,
    pub item_kind: Option<String>,
    #[serde(serialize_with = "state_name")]
    pub state: State,
    pub category: Option<String>,
    pub detail: Option<String>,
    /// `fix`, `check`, `cannot`; `None` for `ok`.
    pub class: Option<&'static str>,
    pub pre_existing: bool,
    pub markers: Vec<VerifyMarker>,
    /// The node's revision; `None` unknown or no node.
    pub node_rev: Option<u32>,
    pub lock: Option<VerifyLock>,
    /// `blob`, `ast`; `None` without an entry, decided earlier, or nothing
    /// to compare (both sides `[path]`).
    pub compared: Option<&'static str>,
    /// `None` without an entry or a bound marker, or with a marker's list
    /// malformed.
    pub levels: Option<VerifyLevels>,
    /// A keyless binding's word (`unanchored`, `cannot_verify`) and
    /// category, as the text prints them.
    #[serde(skip)]
    pub keyless: Option<(String, String)>,
    /// The row names one node.
    #[serde(skip)]
    pub resolved: bool,
    /// `key_problem`'s words, for the hint.
    #[serde(skip)]
    pub key_problem: Option<String>,
}

/// What `spec verify` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyOutcome {
    /// The mode judged; `None` when `[verify]` cannot be read.
    pub mode: Option<Mode>,
    /// `None` plain; `HEAD`, `unborn`.
    pub base: Option<&'static str>,
    pub verdict: Verdict,
    /// Every row, `ok` too, in key order.
    pub rows: Vec<VerifyRow>,
    /// Markers skipped: `queue` and `other_project` IDs.
    pub skipped: usize,
    /// The run causes, in order.
    pub cannot: Vec<String>,
    pub messages: Vec<Message>,
}

impl VerifyOutcome {
    /// 0 clean or observed, 1 blocked, 2 cannot verify.
    pub fn exit(&self) -> Exit {
        match self.verdict {
            Verdict::Clean | Verdict::Observed => Exit::Answered,
            Verdict::Blocked => Exit::NotFound,
            Verdict::CannotVerify => Exit::CannotRun,
        }
    }

    /// The `note:` lines' texts, in order.
    fn notes(&self) -> Vec<String> {
        self.messages
            .iter()
            .filter_map(|message| match message {
                Message::Note(text) => Some(one_line(text)),
                Message::Warning(_) => None,
            })
            .collect()
    }
}

/// `spec verify`: only discovery fails the call; every later failure is a
/// run cause of the outcome (exit 2).
pub fn verify(
    env: &Env,
    globals: &Globals,
    request: &VerifyRequest,
) -> Result<VerifyOutcome, CliError> {
    if request.tree == VerifiedTree::Staged {
        return verify_staged(env, globals, request);
    }
    let changed = request.tree == VerifiedTree::Changed;
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let mut messages = Vec::new();
    let mut causes: Vec<RunCause> = Vec::new();
    let label = &project.config_label;

    // (1), (2): the config's `[code]` and `[verify]`.
    let code = match CodeConfig::from_toml(&text) {
        Ok(Some(code)) => Some(code),
        Ok(None) => {
            causes.push(RunCause::new(
                CAUSE_NO_CODE,
                format!(
                    "{label} has no [code] table: the code layer is off (add [code] with roots)"
                ),
            ));
            None
        }
        Err(error) => {
            causes.push(RunCause::new(CAUSE_CONFIG, error.at(label)));
            None
        }
    };
    let configured = match verify_from_toml(&text) {
        Ok(config) => Some(config.mode),
        Err(error) => {
            causes.push(RunCause::new(CAUSE_CONFIG, error.at(label)));
            None
        }
    };

    // (4), (5): git, and the lock's unmerged stages.
    let mut git = match VerifyGit::open(&project.root, &request.git) {
        Ok(git) => Some(git),
        Err(cause) => {
            causes.push(RunCause::new(CAUSE_GIT, cause.message));
            None
        }
    };
    if let Some(open) = &mut git {
        match open.is_unmerged(LOCK_FILE) {
            Ok(false) => {}
            Ok(true) => causes.push(RunCause::new(
                CAUSE_UNMERGED,
                format!("{LOCK_FILE} is unmerged: run spec lock regen"),
            )),
            Err(cause) => causes.push(RunCause::new(CAUSE_GIT, cause.message)),
        }
    }

    // (6): the lock, against this build's recipes.
    let lock = match read_lock(&project.root, &build_recipes()) {
        Ok(read) => Some(read.lock),
        Err(error) => {
            causes.push(RunCause::new(CAUSE_LOCK, unprefixed(&error)));
            None
        }
    };

    // (8): the spec corpus, the index refreshed, every file read again.
    let input = match indexed(env, &project, &mut messages, true) {
        Ok(input) => Some(input),
        Err(error) => {
            causes.push(RunCause::new(CAUSE_DATA, unprefixed(&error)));
            None
        }
    };

    // (3): the one walk, whenever `[code]` reads (it only reads), so its
    // gap is listed in its place beside any other cause.
    let tree = match &code {
        Some(code) => match read_tree(&project.root, code, &mut messages) {
            Ok(tree) => Some(tree),
            Err(error) => {
                causes.push(RunCause::new(CAUSE_WALK, unprefixed(&error)));
                None
            }
        },
        None => None,
    };

    let mut base = changed.then_some("HEAD");
    let mut head_mode = HeadMode::Absent;
    let mut judged = None;
    match (causes.is_empty(), code, tree, lock, input, git) {
        (true, Some(code), Some(tree), Some(lock), Some(input), Some(mut open)) => {
            let walked = Walked {
                project: &project,
                code: &code,
                input: &input,
                tree: &tree,
                lock: &lock,
            };
            match run(env, globals, &walked, &mut open, changed) {
                Ok((rows, at_head, unborn, lock_note)) => {
                    if unborn {
                        base = Some("unborn");
                    }
                    head_mode = at_head;
                    judged = Some(rows);
                    messages.extend(lock_note.map(Message::Note));
                }
                Err(cause) => causes.push(cause),
            }
            if let Err(cause) = open.finish() {
                causes.push(RunCause::new(CAUSE_GIT, cause.message));
            }
        }
        // The session's end is no cause of its own: one is listed already.
        (.., Some(open)) => {
            let _ = open.finish();
        }
        _ => {}
    }
    Ok(concluded(Gathered {
        configured,
        has_base: changed,
        base,
        head_mode,
        judged,
        causes,
        messages,
        label,
    }))
}

/// What a run gathered before its verdict.
struct Gathered<'a> {
    /// `[verify] mode`; `None` when it cannot be read.
    configured: Option<Mode>,
    /// `--changed` or `--staged`.
    has_base: bool,
    base: Option<&'static str>,
    head_mode: HeadMode,
    judged: Option<Judged>,
    causes: Vec<RunCause>,
    messages: Vec<Message>,
    /// The config as named.
    label: &'a str,
}

/// The outcome: the mode the verdict uses, shown as such whether rows were
/// judged or a run cause stopped them; the rows only when no cause.
fn concluded(gathered: Gathered<'_>) -> VerifyOutcome {
    let Gathered {
        configured,
        has_base,
        base,
        head_mode,
        judged,
        mut causes,
        mut messages,
        label,
    } = gathered;
    let mode = configured
        .map(|configured| judged_mode(configured, has_base, &head_mode, label, &mut messages));

    causes.sort_by_key(|cause| cause.order);
    let (rows, skipped): (Vec<VerifyRow>, usize) = match judged {
        Some(judged) if causes.is_empty() => {
            (judged.rows.iter().map(shown).collect(), judged.skipped)
        }
        _ => (Vec::new(), 0),
    };
    let findings = rows.iter().map(|row| Finding {
        state: row.state,
        pre_existing: row.pre_existing,
    });
    let verdict = verdict(mode.unwrap_or(Mode::Observe), !causes.is_empty(), findings);
    VerifyOutcome {
        mode,
        base,
        verdict,
        rows,
        skipped,
        cannot: causes
            .into_iter()
            .map(|cause| one_line(&cause.message))
            .collect(),
        messages,
    }
}

/// `spec verify --staged`: discovery on disk (`--config` read there, as
/// plain's), then every failure a run cause: (4) the index cannot be
/// listed, (5) each unmerged path; (1), (2) the index's config (or
/// `--config`'s); the index's side through the one-commit seam, its
/// causes (3), (4), (6); `HEAD`'s side over it (7).
fn verify_staged(
    env: &Env,
    globals: &Globals,
    request: &VerifyRequest,
) -> Result<VerifyOutcome, CliError> {
    let located = locate(env, globals)?;
    let given = match &globals.config {
        Some(_) => Some(discover_with_text(env, globals)?.1),
        None => None,
    };
    let label = located.config_label.as_str();
    let mut messages = Vec::new();
    let mut causes: Vec<RunCause> = Vec::new();

    // (4), (5): the index git names.
    let mut opened = match VerifyGit::staged(&located.root, &request.git) {
        Ok(opened) => Some(opened),
        Err(IndexRefusal::Git(cause)) => {
            causes.push(RunCause::new(CAUSE_GIT, cause.message));
            None
        }
        Err(IndexRefusal::Unmerged(paths)) => {
            causes.extend(paths.iter().map(|path| {
                RunCause::new(
                    CAUSE_UNMERGED,
                    if path == LOCK_FILE {
                        format!("{LOCK_FILE} is unmerged: run spec lock regen")
                    } else {
                        format!(
                            "`{path}` is unmerged in the git index: the conflict is not resolved"
                        )
                    },
                )
            }));
            None
        }
    };

    // (1), (2): the index's config, whole, unless `--config`.
    let staged_text = match (&given, &mut opened) {
        (None, Some((git, listing))) => match staged_config(git, listing, label) {
            Ok(text) => Some(text),
            Err(cause) => {
                causes.push(cause);
                None
            }
        },
        _ => None,
    };
    let mut project = None;
    let mut code = None;
    let mut configured = None;
    if let Some(text) = given.as_deref().or(staged_text.as_deref()) {
        match ProjectConfig::from_toml(text) {
            Ok(config) => {
                project = Some(ProjectRoot {
                    root: located.root.clone(),
                    config_label: label.to_owned(),
                    config,
                });
            }
            Err(error) => causes.push(RunCause::new(CAUSE_CONFIG, error.at(label))),
        }
        match CodeConfig::from_toml(text) {
            Ok(Some(read)) => code = Some(read),
            Ok(None) => causes.push(RunCause::new(
                CAUSE_NO_CODE,
                format!(
                    "{label} has no [code] table: the code layer is off (add [code] with roots)"
                ),
            )),
            Err(error) => causes.push(RunCause::new(CAUSE_CONFIG, error.at(label))),
        }
        match verify_from_toml(text) {
            Ok(config) => configured = Some(config.mode),
            Err(error) => causes.push(RunCause::new(CAUSE_CONFIG, error.at(label))),
        }
    }

    let mut base = Some("HEAD");
    let mut head_mode = HeadMode::Absent;
    let mut judged = None;
    match (causes.is_empty(), project, code, opened) {
        (true, Some(project), Some(code), Some((mut git, listing))) => {
            let config = staged_text.as_deref().map(str::as_bytes);
            match judge_index(env, globals, &project, code, &mut git, listing, config) {
                Ok((rows, at_head, unborn, lock_note)) => {
                    if unborn {
                        base = Some("unborn");
                    }
                    head_mode = at_head;
                    judged = Some(rows);
                    messages.extend(lock_note.map(Message::Note));
                }
                Err(cause) => causes.push(cause),
            }
            if let Err(cause) = git.finish() {
                causes.push(RunCause::new(CAUSE_GIT, cause.message));
            }
        }
        // The session's end is no cause of its own: one is listed already.
        (.., Some((git, _))) => {
            let _ = git.finish();
        }
        _ => {}
    }
    Ok(concluded(Gathered {
        configured,
        has_base: true,
        base,
        head_mode,
        judged,
        causes,
        messages,
        label,
    }))
}

/// The index's root `specengine.toml`, its text. Causes: git's (4); not
/// staged, not a regular file or its blob unreadable, not UTF-8 (1).
fn staged_config(
    git: &mut VerifyGit,
    listing: &CommitListing,
    label: &str,
) -> Result<String, RunCause> {
    let mut files = git
        .read_files(listing, [CONFIG_FILE])
        .map_err(|cause| RunCause::new(CAUSE_GIT, cause.message))?;
    match files.remove(CONFIG_FILE) {
        None => Err(RunCause::new(
            CAUSE_CONFIG,
            format!("{label}: not staged in the git index"),
        )),
        Some(CommitFile::Unreadable(why)) => Err(RunCause::new(
            CAUSE_CONFIG,
            format!("{label} in the index: {why}"),
        )),
        Some(CommitFile::Bytes(bytes)) => String::from_utf8(bytes)
            .map_err(|_| RunCause::new(CAUSE_CONFIG, format!("{label}: the file is not UTF-8"))),
    }
}

/// The index's side read through the one-commit seam, `HEAD`'s read over
/// it, and its rows judged against `HEAD`'s; `config` the index's config
/// bytes when it is the config judged. `HEAD`'s mode, whether it is
/// unborn, and the note when `HEAD`'s lock gave way to the index's.
fn judge_index(
    env: &Env,
    globals: &Globals,
    project: &ProjectRoot,
    code: CodeConfig,
    git: &mut VerifyGit,
    listing: CommitListing,
    config: Option<&[u8]>,
) -> Result<(Judged, HeadMode, bool, Option<String>), RunCause> {
    let side = read_commit(project, git, CommitConfig::of_index(listing, code.clone()))?;
    let config_path = config_path(env, globals, project);
    let share = IndexShare {
        side: &side,
        config,
    };
    let head = read_head(
        &side.walked(project, &code),
        git,
        config_path.as_deref(),
        Some(&share),
    )?;
    let judged = judge_commit(&side, project, Some(&head))?;
    let unborn = head.is_unborn();
    Ok((judged, head.mode, unborn, head.lock_note))
}

/// A state as its name.
fn state_name<S: serde::Serializer>(state: &State, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(state.as_str())
}

/// The base when asked, and the judge over the one walk; `HEAD`'s mode,
/// whether it is unborn, and the note when `HEAD`'s lock gave way to the
/// disk's.
fn run(
    env: &Env,
    globals: &Globals,
    walked: &Walked<'_>,
    git: &mut VerifyGit,
    changed: bool,
) -> Result<(crate::verify_rows::Judged, HeadMode, bool, Option<String>), RunCause> {
    let head = if changed {
        let config = config_path(env, globals, walked.project);
        Some(read_head(walked, git, config.as_deref(), None)?)
    } else {
        None
    };
    let judged = judge_walk(walked, git, head.as_ref())?;
    let (head_mode, unborn, lock_note) = match head {
        Some(head) => {
            let unborn = head.is_unborn();
            (head.mode, unborn, head.lock_note)
        }
        None => (HeadMode::Absent, false, None),
    };
    Ok((judged, head_mode, unborn, lock_note))
}

/// The config's root-relative path, for `HEAD`'s counterpart: the root's
/// own, or `--config`'s when it lies under the root.
fn config_path(
    env: &Env,
    globals: &Globals,
    project: &crate::project::ProjectRoot,
) -> Option<String> {
    let Some(given) = &globals.config else {
        return Some(CONFIG_FILE.to_owned());
    };
    let full = fs::canonicalize(env.cwd.join(given)).ok()?;
    let relative = full.strip_prefix(&project.root).ok()?;
    let parts: Option<Vec<&str>> = relative
        .components()
        .map(|component| match component {
            std::path::Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The mode judged: with a base (`--changed`, `--staged`) the stricter of
/// the configured one and `HEAD`'s (a note when `HEAD`'s is stricter or
/// unknown); plain, `enforce-introduced` judged as `enforce`, with a note.
fn judged_mode(
    configured: Mode,
    has_base: bool,
    head: &HeadMode,
    label: &str,
    messages: &mut Vec<Message>,
) -> Mode {
    if !has_base {
        if configured == Mode::EnforceIntroduced {
            messages.push(Message::Note(
                "mode `enforce-introduced` has no base without --changed: judged as `enforce`"
                    .to_owned(),
            ));
            return Mode::Enforce;
        }
        return configured;
    }
    match head {
        HeadMode::Absent => configured,
        HeadMode::Known(at_head) if *at_head > configured => {
            messages.push(Message::Note(format!(
                "HEAD's {label} sets mode `{at_head}`, stricter than `{configured}`: verify runs \
                 in `{at_head}`"
            )));
            *at_head
        }
        HeadMode::Known(_) => configured,
        HeadMode::Unknown(why) => {
            messages.push(Message::Note(format!(
                "{why}: HEAD's mode is unknown, so verify runs in `{configured}`"
            )));
            configured
        }
    }
}

/// A CLI error's message without its `spec: ` prefix.
fn unprefixed(error: &CliError) -> String {
    error
        .message
        .strip_prefix("spec: ")
        .unwrap_or(&error.message)
        .to_owned()
}

/// A judged row as shown (a drift item stores it too, task spec
/// `drift-queue`, "The item").
pub(crate) fn shown(row: &Row) -> VerifyRow {
    let (kind, symbol, item_kind, keyless) = match &row.key.binding {
        RowBinding::Keyed {
            kind,
            symbol,
            item_kind,
        } => (
            Some(kind.as_str()),
            Some(symbol.clone()),
            item_kind.clone(),
            None,
        ),
        RowBinding::Keyless { state, category } => {
            (None, None, None, Some((state.clone(), category.clone())))
        }
    };
    let judgement = &row.judgement;
    VerifyRow {
        node: row.key.node.clone(),
        relation: row.key.relation.as_str(),
        file: row.key.file.clone(),
        kind,
        symbol,
        item_kind,
        state: judgement.state,
        category: judgement.category.clone(),
        detail: judgement.detail.clone(),
        class: judgement.state.class().map(|class| class.as_str()),
        pre_existing: row.pre_existing,
        markers: row
            .markers
            .iter()
            .map(|marker| VerifyMarker {
                line: marker.line,
                rev: match marker.rev {
                    specengine_core::verify::MarkerRev::Written(n) => Some(n),
                    _ => None,
                },
            })
            .collect(),
        node_rev: row.node_rev,
        lock: row.entry.as_ref().map(|entry| VerifyLock {
            rev: entry.rev,
            commit: entry.commit.clone(),
            verified_at: entry.verified_at.clone(),
        }),
        compared: judgement.compared.map(|compared| compared.as_str()),
        levels: row.levels.as_ref().map(|levels| VerifyLevels {
            lock: level_names(&levels.lock),
            markers: level_names(&levels.markers),
            changed: levels.changed.as_ref().map(level_names),
        }),
        keyless,
        resolved: row.resolved,
        key_problem: row
            .key
            .entry_key()
            .as_ref()
            .and_then(key_problem)
            .map(|problem| problem.to_string()),
    }
}

/// Levels' names, in their order.
fn level_names<'a>(levels: impl IntoIterator<Item = &'a LockLevel>) -> Vec<&'static str> {
    levels.into_iter().map(|level| level.as_str()).collect()
}

// ---------------------------------------------------------------- output

/// The counts of the summary and the JSON.
struct Counts {
    rows: usize,
    ok: usize,
    fix: usize,
    check: usize,
    cannot: usize,
    pre_existing: usize,
    by_blob: usize,
    by_ast: usize,
}

fn counts(outcome: &VerifyOutcome) -> Counts {
    let class = |name: &str| {
        outcome
            .rows
            .iter()
            .filter(|row| row.class == Some(name))
            .count()
    };
    let compared = |name: &str| {
        outcome
            .rows
            .iter()
            .filter(|row| row.compared == Some(name))
            .count()
    };
    Counts {
        rows: outcome.rows.len(),
        ok: outcome
            .rows
            .iter()
            .filter(|row| row.class.is_none())
            .count(),
        fix: class("fix"),
        check: class("check"),
        cannot: class("cannot"),
        pre_existing: outcome.rows.iter().filter(|row| row.pre_existing).count(),
        by_blob: compared("blob"),
        by_ast: compared("ast"),
    }
}

/// A row's line ([`FindingLine`]); its binding a keyless row's words, else
/// [`keyed_binding`], else empty.
fn row_line(row: &VerifyRow) -> String {
    let binding = match &row.keyless {
        Some((word, category)) => format!("{word} {category}"),
        None => keyed_binding(row.kind, row.symbol.as_deref(), row.item_kind.as_deref())
            .unwrap_or_default(),
    };
    FindingLine {
        pre_existing: row.pre_existing,
        class: row.class.unwrap_or("ok"),
        state: row.state.as_str(),
        category: row.category.as_deref(),
        detail: row.detail.as_deref(),
        node: &row.node,
        relation: row.relation,
        file: &row.file,
        binding: &binding,
    }
    .render()
}

/// One row as `spec verify` prints it (a drift item's review prints its
/// stored rows so): `<class> | <state>[ <category>][ <detail>] | <node> |
/// <relation> | <file> | <binding>`, `pre-existing ` before the class;
/// each part on one line.
pub(crate) struct FindingLine<'a> {
    pub pre_existing: bool,
    pub class: &'a str,
    pub state: &'a str,
    pub category: Option<&'a str>,
    pub detail: Option<&'a str>,
    pub node: &'a str,
    pub relation: &'a str,
    pub file: &'a str,
    /// `<kind> <symbol>[ <item_kind>]` ([`keyed_binding`]), or what the
    /// caller shows without one.
    pub binding: &'a str,
}

impl FindingLine<'_> {
    pub(crate) fn render(&self) -> String {
        let mut state = one_line(self.state);
        for part in [self.category, self.detail].into_iter().flatten() {
            state.push(' ');
            state.push_str(&one_line(part));
        }
        format!(
            "{}{} | {state} | {} | {} | {} | {}",
            if self.pre_existing {
                "pre-existing "
            } else {
                ""
            },
            one_line(self.class),
            one_line(self.node),
            one_line(self.relation),
            one_line(self.file),
            one_line(self.binding),
        )
    }
}

/// A keyed row's binding: `<kind> <symbol>[ <item_kind>]`; `None` without
/// a kind or a symbol.
pub(crate) fn keyed_binding(
    kind: Option<&str>,
    symbol: Option<&str>,
    item_kind: Option<&str>,
) -> Option<String> {
    let (kind, symbol) = (kind?, symbol?);
    let mut binding = format!("{kind} {}", one_line(symbol));
    if let Some(item_kind) = item_kind {
        binding.push(' ');
        binding.push_str(&one_line(item_kind));
    }
    Some(binding)
}

/// The next step for text changed at the locked revision (accept refuses
/// it): no proposal raises a revision, approval sets it.
const SAME_REVISION_STEP: &str = "the node's text changed at its locked revision: raise its \
     `rev` by hand (`spec bump` prints the line while the edit is uncommitted), commit, then \
     accept; or `--editorial` for an editorial change (`spec lock accept <ID> --editorial`); \
     accept refuses text changed at a revision";

/// The hint after the others when a row is drift a queue item takes (task
/// spec `drift-queue`, "report_drift and texts").
const DRIFT_HINT: &str = "hint: drift: commit first; spec propose drift queues HEAD's drift for \
     the owner";

/// One hint per state and category present (`unlocked key_problem` apart):
/// the next step, as accept takes it; then, once, the drift hint when a row
/// raises an item in a sweep (`drift::raises`, no node named).
fn hints(rows: &[VerifyRow]) -> Vec<String> {
    let mut hints = state_hints(rows);
    let drifting = rows
        .iter()
        .any(|row| raises(row.state, row.detail.as_deref(), false));
    if drifting {
        hints.push(DRIFT_HINT.to_owned());
    }
    hints
}

/// One hint per state and category present (`unlocked key_problem` apart):
/// the next step, as accept takes it.
fn state_hints(rows: &[VerifyRow]) -> Vec<String> {
    let mut present: BTreeSet<(State, Option<String>)> = BTreeSet::new();
    let mut key_problems: BTreeSet<String> = BTreeSet::new();
    // Conflict rows at the entry's revision, and at another.
    let mut conflicts = (false, false);
    for row in rows.iter().filter(|row| row.class.is_some()) {
        let state = row.state;
        if state == State::Conflict {
            let same = row
                .lock
                .as_ref()
                .is_some_and(|lock| row.node_rev == Some(lock.rev));
            if same {
                conflicts.0 = true;
            } else {
                conflicts.1 = true;
            }
        }
        let label = if row.detail.as_deref() == Some("key_problem") {
            key_problems.extend(row.key_problem.clone());
            Some("key_problem".to_owned())
        } else {
            row.category.clone()
        };
        present.insert((state, label));
    }
    present
        .into_iter()
        .map(|(state, label)| {
            let step = match (state, label.as_deref()) {
                (State::CannotVerify, Some("item_error")) => {
                    "fix the item's parse error, or move the marker to an item that parses; no \
                     lock vouches for it meanwhile (`spec refs <ID>`)"
                        .to_owned()
                }
                (State::CannotVerify, Some("ambiguous_symbol")) => {
                    "make the binding name one symbol (no `#[path]` target, a package for the \
                     file, no twin), or move the marker (`spec refs <ID>`)"
                        .to_owned()
                }
                (State::CannotVerify, Some("ambiguous_id")) => {
                    "the ID names several nodes: cite one (`slug/ID`, `ID#SECTION`) or make the \
                     IDs unique"
                        .to_owned()
                }
                (State::CannotVerify, Some("rev_unknown")) => {
                    "write the node's `rev` as a number from 1 (`spec show <ID>`)".to_owned()
                }
                (State::CannotVerify, Some("hash_null")) => {
                    "fix what nulls the node's meaning hash (`spec check` names it)".to_owned()
                }
                (State::CannotVerify, Some("unreadable")) => {
                    "make the file readable UTF-8 under [code], or restore the binding \
                     (`spec refs <ID>`)"
                        .to_owned()
                }
                (State::CannotVerify, Some("hidden_by_error")) => {
                    "fix the parse error before the symbol (`spec refs <ID>`)".to_owned()
                }
                (State::BrokenBinding, Some("node_gone")) => {
                    "commit first (the node is gone only when HEAD agrees), then `spec lock \
                     accept <old ID>` on a terminal drops its entries"
                        .to_owned()
                }
                (State::BrokenBinding | State::Unlocked | State::CodeAhead, _)
                    if label.as_deref() != Some("key_problem") =>
                {
                    "commit, then `spec lock accept <ID>` on a terminal".to_owned()
                }
                (State::Unlocked, _) => format!(
                    "spec.lock cannot hold the key ({}): rename what gives it; `spec lock \
                     accept` cannot lock it",
                    key_problems.iter().cloned().collect::<Vec<_>>().join("; ")
                ),
                (State::SpecAhead, Some("same_revision")) => SAME_REVISION_STEP.to_owned(),
                (State::Conflict, _) => {
                    // By R against the entry's revision: text changed at
                    // the locked revision, or a revision the marker never
                    // reached.
                    let mut steps = Vec::new();
                    if conflicts.0 {
                        steps.push(SAME_REVISION_STEP);
                    }
                    if conflicts.1 {
                        steps.push(
                            "at a revision the lock does not hold, the marker not raised: \
                             check the code against the node, set the marker to its \
                             revision, commit, then `spec lock accept <ID>` on a terminal",
                        );
                    }
                    steps.join("; ")
                }
                _ => "restore, raise or lower the marker (`spec refs <ID>`)".to_owned(),
            };
            let mut head = state.as_str().to_owned();
            if let Some(label) = label {
                head.push(' ');
                head.push_str(&label);
            }
            format!("hint: {head}: {step}")
        })
        .collect()
}

/// The text report: the non-`ok` rows in key order, the `cannot:` lines,
/// the hints, the summary.
pub(crate) fn render_text(outcome: &VerifyOutcome) -> String {
    let mut out = String::new();
    for row in outcome.rows.iter().filter(|row| row.class.is_some()) {
        out.push_str(&row_line(row));
        out.push('\n');
    }
    for cause in &outcome.cannot {
        out.push_str(&format!("cannot: {}\n", one_line(cause)));
    }
    for hint in hints(&outcome.rows) {
        out.push_str(&hint);
        out.push('\n');
    }
    let counts = counts(outcome);
    let mode = outcome
        .mode
        .map(|mode| format!(" [{mode}]"))
        .unwrap_or_default();
    let pre_existing = if outcome.base.is_some() {
        format!(" ({} pre-existing)", counts.pre_existing)
    } else {
        String::new()
    };
    out.push_str(&format!(
        "verify: {}{mode}: {} fix, {} check{pre_existing}, {} cannot, {} ok; {} by blob, {} by \
         ast; {} skipped\n",
        outcome.verdict.as_str(),
        counts.fix,
        counts.check,
        counts.cannot,
        counts.ok,
        counts.by_blob,
        counts.by_ast,
        outcome.skipped
    ));
    escape_controls(&out)
}

/// A row's key in the JSON's `delta`.
#[derive(Serialize)]
struct KeyJson<'a> {
    node: &'a str,
    relation: &'a str,
    file: &'a str,
    kind: Option<&'a str>,
    symbol: Option<&'a str>,
    item_kind: Option<&'a str>,
}

impl<'a> KeyJson<'a> {
    fn of(row: &'a VerifyRow) -> Self {
        Self {
            node: &row.node,
            relation: row.relation,
            file: &row.file,
            kind: row.kind,
            symbol: row.symbol.as_deref(),
            item_kind: row.item_kind.as_deref(),
        }
    }
}

#[derive(Serialize)]
struct CountsJson {
    rows: usize,
    ok: usize,
    fix: usize,
    check: usize,
    cannot: usize,
    pre_existing: usize,
    skipped: usize,
}

#[derive(Serialize)]
struct CascadeJson {
    by_blob: usize,
    by_ast: usize,
}

#[derive(Serialize)]
struct BindingsJson<'a> {
    moved: Option<()>,
    ambiguous: Vec<KeyJson<'a>>,
    lost: Vec<KeyJson<'a>>,
    stale: Vec<KeyJson<'a>>,
}

#[derive(Serialize)]
struct DeltaJson<'a> {
    symbols: Option<()>,
    bindings: BindingsJson<'a>,
    affected_nodes: Vec<&'a str>,
    tests_reaching: Option<()>,
}

#[derive(Serialize)]
struct VerifyJson<'a> {
    mode: Option<&'static str>,
    base: Option<&'static str>,
    verdict: &'static str,
    counts: CountsJson,
    cascade: CascadeJson,
    rows: &'a [VerifyRow],
    delta: DeltaJson<'a>,
    cannot: &'a [String],
    notes: Vec<String>,
}

impl Serialize for VerifyOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let counts = counts(self);
        // Over the rows not Pre-existing.
        let introduced: Vec<&VerifyRow> =
            self.rows.iter().filter(|row| !row.pre_existing).collect();
        let keys = |wanted: &dyn Fn(&VerifyRow) -> bool| -> Vec<KeyJson<'_>> {
            introduced
                .iter()
                .filter(|row| wanted(row))
                .map(|row| KeyJson::of(row))
                .collect()
        };
        let affected: BTreeSet<&str> = introduced
            .iter()
            .filter(|row| row.class.is_some() && row.resolved)
            .map(|row| row.node.as_str())
            .collect();
        let document = VerifyJson {
            mode: self.mode.map(Mode::as_str),
            base: self.base,
            verdict: self.verdict.as_str(),
            counts: CountsJson {
                rows: counts.rows,
                ok: counts.ok,
                fix: counts.fix,
                check: counts.check,
                cannot: counts.cannot,
                pre_existing: counts.pre_existing,
                skipped: self.skipped,
            },
            cascade: CascadeJson {
                by_blob: counts.by_blob,
                by_ast: counts.by_ast,
            },
            rows: &self.rows,
            delta: DeltaJson {
                symbols: None,
                bindings: BindingsJson {
                    moved: None,
                    ambiguous: keys(&|row| {
                        row.state == State::CannotVerify
                            && row
                                .category
                                .as_deref()
                                .is_some_and(|category| category.starts_with("ambiguous_"))
                    }),
                    lost: keys(&|row| row.state == State::BrokenBinding),
                    stale: keys(&|row| {
                        row.state == State::CodeAhead || row.state == State::Conflict
                    }),
                },
                affected_nodes: affected.into_iter().collect(),
                tests_reaching: None,
            },
            cannot: &self.cannot,
            notes: self.notes(),
        };
        document.serialize(serializer)
    }
}
