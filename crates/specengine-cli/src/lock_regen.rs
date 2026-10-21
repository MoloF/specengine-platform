//! `spec lock regen` (canon `spec-lock`, "`spec lock regen`"): a
//! merge that conflicts in `spec.lock` resolved on the owner's terminal (canon
//! `architecture-code`, "`spec.lock` writers"). Only while `spec.lock` is
//! unmerged (`ls-files -u`): each stage present read without the build's
//! recipes (a format neither 1, 2 nor 3, a key or a form refused: exit 2),
//! all of one `norm_recipe` (else exit 2), then all format 2 or 3 of one
//! `code_recipe` (else exit 2, [`UNREBASED`]: the lagging branch is
//! rebased before the merge, task spec `lock-levels` Q28), merged per entry
//! key, and per node for the `[[tested]]` blocks, by core's
//! [`merge_stages`] (side-symmetric; no code or spec file read, no hash
//! computed: every entry kept is one an owner accepted, every block one a
//! run wrote, so drift or a stale block may follow, never a false `ok`;
//! the result format 3 iff a block remains). After the one `[y/N]` and the
//! stages and the file read again (a change: exit 1), the worktree file
//! alone is written; the owner adds it and commits the merge.

use serde::Serialize;
use specengine_core::lock::{LOCK_FILE, Lock, RegenRefusal, merge_stages, parse_lock, render_lock};
use specengine_store::{CreateFileError, GitEnv, WorktreeGit, create_file, replace_file};

use crate::lock::{LockConsent, LockPrompt, read_lock_bytes};
use crate::project::discover_with_text;
use crate::proposals::require_root_config;
use crate::refs::code_config;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// What the owner does after a regen.
const NEXT: &str = "next: git add spec.lock, then commit the merge";

/// Regen's refusal of a format-1 stage, or of stages of two
/// `code_recipe`s (Q28): a merge cannot carry either over, and `spec lock
/// rebase` reads no unmerged lock.
const UNREBASED: &str = "spec.lock: a stage is format 1, or the stages differ in \
                                    `code_recipe`: run git merge --abort, spec lock rebase on \
                                    the lagging branch, and merge again";

/// `spec lock regen` options.
#[derive(Debug, Clone)]
pub struct LockRegenRequest {
    pub git: GitEnv,
}

/// One entry key and how it was chosen: `<choice> | <node> | <relation> |
/// <file> | <kind> <symbol>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegenRow {
    /// `both`, `ours`, `theirs`, `deleted`, `later`, `bytes`.
    pub choice: &'static str,
    pub node: String,
    pub relation: &'static str,
    pub file: String,
    pub kind: &'static str,
    pub symbol: String,
    pub item_kind: Option<String>,
}

/// One `[[tested]]` block's node and how it was chosen: `<choice> | <node>
/// | tested`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegenTestedRow {
    /// `both`, `ours`, `theirs`, `deleted`, `later`, `bytes`.
    pub choice: &'static str,
    pub node: String,
}

/// What `spec lock regen` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegenOutcome {
    /// Every key of the stages, in key order.
    pub rows: Vec<RegenRow>,
    /// Every `[[tested]]` block's node of the stages and its choice, by
    /// node bytes: after the entries' rows, in the text and the JSON.
    pub tested: Vec<RegenTestedRow>,
    /// The merged lock's entries, a block counted as one.
    pub entries: usize,
    /// Keys and blocks chosen by `verified_at` or bytes.
    pub by_date_or_bytes: usize,
    /// The worktree file was written.
    pub written: bool,
    /// Why nothing was written (exit 1).
    pub refusal: Option<String>,
    #[serde(skip)]
    pub messages: Vec<Message>,
    /// The rows' text still to print; `None` once the prompt showed it.
    #[serde(skip)]
    pub report: Option<String>,
}

impl RegenOutcome {
    /// 1 when refused, else 0.
    pub fn exit(&self) -> Exit {
        if self.refusal.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    fn refused(mut self, reason: &str) -> Self {
        self.refusal = Some(one_line(reason));
        self
    }
}

/// The stage's name.
fn stage_name(stage: u8) -> &'static str {
    match stage {
        1 => "base",
        2 => "ours",
        _ => "theirs",
    }
}

/// `spec lock regen` (see the module documentation).
pub fn lock_regen(
    env: &Env,
    globals: &Globals,
    request: &LockRegenRequest,
    consent: LockConsent<'_>,
) -> Result<RegenOutcome, CliError> {
    let mut outcome = RegenOutcome {
        rows: Vec::new(),
        tested: Vec::new(),
        entries: 0,
        by_date_or_bytes: 0,
        written: false,
        refusal: None,
        messages: Vec::new(),
        report: None,
    };
    let (project, text) = discover_with_text(env, globals)?;
    require_root_config(env, globals, &project)?;
    project.slug()?;
    code_config(&project, &text)?;

    let at_root = WorktreeGit::new(&project.root, &request.git)
        .map_err(|error| CliError::spec(format!("cannot run git: {error}")))?;
    let top = at_root.top().map_err(|error| {
        CliError::spec(format!(
            "the project root {} lies in no git worktree ({error}): spec.lock is committed there",
            project.root.display()
        ))
    })?;
    let root_rel = project
        .root
        .strip_prefix(&top)
        .ok()
        .and_then(|inner| inner.to_str())
        .map(|inner| inner.replace('\\', "/"))
        .ok_or_else(|| {
            CliError::spec(format!(
                "the project root {} does not lie in the worktree {} git names for it",
                project.root.display(),
                top.display()
            ))
        })?;
    let worktree = top.display().to_string();
    let git = WorktreeGit::new(&top, &request.git)
        .map_err(|error| CliError::spec(format!("cannot run git: {error}")))?;
    let top_path = if root_rel.is_empty() {
        LOCK_FILE.to_owned()
    } else {
        format!("{root_rel}/{LOCK_FILE}")
    };

    let stages = git
        .unmerged_stages(&top_path)
        .map_err(|error| CliError::spec(format!("git: {error}")))?;
    if stages.is_empty() {
        return Ok(outcome.refused(&format!(
            "{LOCK_FILE} is not unmerged in {worktree}: no conflict to merge; nothing written"
        )));
    }
    let mut locks: [Option<Lock>; 3] = [None, None, None];
    for (stage, bytes) in &stages {
        let name = stage_name(*stage);
        let text = std::str::from_utf8(bytes).map_err(|_| {
            CliError::cannot(format!(
                "{LOCK_FILE} ({name}'s stage): not UTF-8: SpecEngine writes it"
            ))
        })?;
        let lock = parse_lock(text, None)
            .map_err(|error| CliError::cannot(format!("{error} ({name}'s stage)")))?;
        if let Some(slot) = locks.get_mut(usize::from(*stage) - 1) {
            *slot = Some(lock);
        }
    }
    let [base, ours, theirs] = &locks;
    let merged = match merge_stages(base.as_ref(), ours.as_ref(), theirs.as_ref()) {
        Ok(merged) => merged,
        Err(RegenRefusal::NoSides) => {
            return Err(CliError::spec(format!(
                "{LOCK_FILE} has neither our nor their stage in {worktree}: nothing to merge"
            )));
        }
        Err(RegenRefusal::RecipesDiffer { first, second }) => {
            return Err(CliError::spec(format!(
                "the stages of {LOCK_FILE} name different recipes (\"{}\", \"{}\" and \"{}\", \
                 \"{}\"): one side was written by another build; nothing written",
                first.norm, first.code, second.norm, second.code
            )));
        }
        Err(RegenRefusal::Unrebased) => return Err(CliError::cannot(UNREBASED)),
    };
    outcome.rows = merged
        .rows
        .iter()
        .map(|(key, choice)| RegenRow {
            choice: choice.as_str(),
            node: key.node.clone(),
            relation: key.relation.as_str(),
            file: key.file.clone(),
            kind: key.kind.as_str(),
            symbol: key.symbol.clone(),
            item_kind: key.item_kind.clone(),
        })
        .collect();
    outcome.tested = merged
        .tested
        .iter()
        .map(|(node, choice)| RegenTestedRow {
            choice: choice.as_str(),
            node: node.clone(),
        })
        .collect();
    outcome.entries = merged.lock.entries.len() + merged.lock.tested.len();
    outcome.by_date_or_bytes = merged.by_date_or_bytes();
    let rows = rows_text(&outcome.rows, &outcome.tested);
    let file_bytes = read_lock_bytes(&project.root)?;
    let question = format!(
        "merge {LOCK_FILE} ({} entries, {} by verified_at or bytes) in {worktree}? [y/N]",
        outcome.entries, outcome.by_date_or_bytes
    );
    let escaped_rows = escape_controls(&rows);
    let escaped_question = escape_controls(&question);
    if !consent(&LockPrompt {
        rows: &escaped_rows,
        question: &escaped_question,
    }) {
        return Ok(outcome.refused("not confirmed; nothing written"));
    }

    let again = git
        .unmerged_stages(&top_path)
        .map_err(|error| CliError::spec(format!("git: {error}")))?;
    if again != stages || read_lock_bytes(&project.root)? != file_bytes {
        return Ok(outcome.refused(&format!(
            "{LOCK_FILE} changed while being merged; nothing written"
        )));
    }
    let rendered = render_lock(&merged.lock);
    let path = project.root.join(LOCK_FILE);
    if file_bytes.is_some() {
        replace_file(&path, rendered.as_bytes()).map_err(|error| {
            CliError::spec(format!(
                "cannot write {LOCK_FILE}: {error}; nothing written"
            ))
        })?;
    } else {
        match create_file(&project.root, LOCK_FILE, rendered.as_bytes()) {
            Ok(_) => {}
            Err(CreateFileError::Exists) => {
                return Ok(outcome.refused(&format!(
                    "{LOCK_FILE} appeared while being merged; nothing written"
                )));
            }
            Err(error) => {
                return Err(CliError::spec(format!(
                    "cannot create {LOCK_FILE}: {error}; nothing written"
                )));
            }
        }
    }
    outcome.written = true;
    Ok(outcome)
}

/// `<choice> | <node> | <relation> | <file> | <kind> <symbol>`, one line
/// each, then `<choice> | <node> | tested` per block.
fn rows_text(rows: &[RegenRow], tested: &[RegenTestedRow]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(&format!(
            "{} | {} | {} | {} | {} {}\n",
            row.choice,
            one_line(&row.node),
            row.relation,
            one_line(&row.file),
            row.kind,
            one_line(&row.symbol)
        ));
    }
    for row in tested {
        out.push_str(&format!(
            "{} | {} | tested\n",
            row.choice,
            one_line(&row.node)
        ));
    }
    out
}

/// The outcome as stdout text: the rows not shown yet, then the next step
/// once written.
pub(crate) fn render_text(outcome: &RegenOutcome) -> String {
    let mut out = outcome
        .report
        .as_deref()
        .map(escape_controls)
        .unwrap_or_default();
    if outcome.written {
        out.push_str(NEXT);
        out.push('\n');
    }
    out
}
