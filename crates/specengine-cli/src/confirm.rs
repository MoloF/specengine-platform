//! A hub's confirm (task spec `hub-confirm`, canon `hub-team`): a login confirms
//! one staged choice, a commit in the hub's own checkout, nothing fetched,
//! merged or pushed. One proposal per call; no call takes a set or a choice.
//!
//! - [`confirm_question`] (`GET .../proposals/:id/confirm`) returns the text
//!   `spec approve PR` or `spec reject PR` would ask, every format character
//!   in it escaped ([`shown`]). It rehearses that run, answering no, with
//!   [`failed`](crate::apply::failed) quiet ([`HubRun`]), so a read writes no
//!   event.
//! - [`confirm_staged`] (`POST` on the same path) runs the same call again,
//!   and its only consent answers `shown(asked) == question`, byte for byte:
//!   it takes none from its caller, so nothing but the text the page showed
//!   says yes. A rebase, a new raise, a replaced stage or another record ID
//!   makes another text, and a refusal. The text is public, so it is no
//!   token: it says what is confirmed, and a script holding a person's
//!   session passes.
//!
//! Both judge the row first, nothing written on a refusal:
//!
//! - **A stage, never a choice**: the choice stays what [`StageBody`] stored,
//!   taken as `spec approve PR` with no flags takes it, and the stage picks the
//!   call, since a plain approve on a row with a reject staged applies.
//! - **A terminal's**: an `approved` row, a drift item, a task-bound proposal,
//!   an editorial stage (only a terminal writes `spec.lock`), an approve
//!   staged with no `span_hash` on a kind whose stage records one, its
//!   preview having failed: with none, a change to the target's span after
//!   the read would leave the question as it was.
//! - **The cover** ([`outside`]), [`TeamEntry::covers`] of every path named,
//!   each holder resolved now ([`named_paths`]).
//!
//! The run then proves it reads that row ([`HubRun::judged`]). The login and its
//! `email` go in as `GIT_COMMITTER_*` and `GIT_AUTHOR_*` ([`identified`]), read
//! too, since a record's text names its decider: every step of an apply reads
//! its identity from git, as on a terminal, and none changes here.
//!
//! [`StageBody`]: crate::StageBody

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use specengine_core::card::is_format_character;
use specengine_core::check::{CheckInput, Resolver};
use specengine_core::lock::LOCK_FILE;
use specengine_core::patch::{HolderError, one_holder};
use specengine_core::{TeamEntry, TeamRole};
use specengine_model::grammar::{self, Canon};
use specengine_model::{AMENDS, IdScript};
use specengine_store::{
    GitEnv, KindFamily, Proposal, ProposalQueue as _, ProposalStatus, Stage, same_repository,
};

use crate::amend::amended_ids;
use crate::apply::{
    ApproveFlags, ApproveRequest, Consent, HubRun, RejectRequest, run_approve, run_reject,
};
use crate::corpus::indexed;
use crate::preflight::{recorded_project, recorded_root};
use crate::project::ProjectRoot;
use crate::proposals::{
    ProposalDocument, ProposalOutcome, QueueCommand, QueueContext, checked_now, elsewhere,
    escaped_error, no_proposal, open_context, queue_cannot, repository_gone, with_diff, written_id,
};
use crate::propose::resolved_node;
use crate::stage::records_span;
use crate::{CliError, Env, Exit, Globals, Message};

/// [`confirm_question`]'s and [`confirm_staged`]'s options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmRequest {
    /// `PR` as given.
    pub id: String,
    /// The login's `[team]` entry, as the daemon read it inside the
    /// project's turn: who decides, and what they cover.
    pub entry: TeamEntry,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: String,
    /// The caller's environment, the identity added here; git runs without
    /// its local `GIT_*` variables.
    pub git: GitEnv,
}

/// The body of `POST .../proposals/:id/confirm`: what the read gave. Any other
/// key, a `choice` first of all, is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmBody {
    pub question: String,
    pub updated_at: String,
}

impl ConfirmBody {
    /// `bytes` decoded; else exit 2 naming the defect, nothing read or changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        serde_json::from_slice(bytes).map_err(|error| {
            escaped_error(CliError::spec(format!(
                "the confirm is not `{{\"question\",\"updated_at\"}}` as JSON: {error}; nothing \
                 changed"
            )))
        })
    }
}

/// What `GET .../proposals/:id/confirm` answers: the question as [`shown`],
/// and the state it was asked on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfirmRead {
    pub id: String,
    pub question: String,
    pub updated_at: String,
}

/// Why a confirm changed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmCause {
    /// No such proposal in the project's queue.
    Unknown,
    /// The row or the run refuses it (the refused document's last note):
    /// nothing staged, a terminal's, a moved question, a step refused.
    Refused,
    /// The entry does not cover what the act names: the daemon's 403.
    Outside,
}

/// What [`confirm_question`] or [`confirm_staged`] answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmOutcome {
    /// The review document (refused: its last note the reason).
    pub proposal: ProposalOutcome,
    /// [`confirm_question`]'s answer; `None` for a refusal and a confirm.
    pub read: Option<ConfirmRead>,
    /// `None`: asked, or confirmed.
    pub cause: Option<ConfirmCause>,
}

impl ConfirmOutcome {
    /// 0 asked or confirmed; 1 anything else.
    pub fn exit(&self) -> Exit {
        match self.cause {
            None => Exit::Answered,
            Some(ConfirmCause::Unknown | ConfirmCause::Refused | ConfirmCause::Outside) => {
                Exit::NotFound
            }
        }
    }

    /// A refusal judged before the library is called, `reason` its line: a
    /// login the daemon's read inside the project's turn finds outside
    /// `[team]`, or of a role that confirms nothing (a 403 on a hub).
    pub fn outside(reason: &str) -> Self {
        Self::refused(
            ConfirmCause::Outside,
            ProposalDocument::default(),
            reason,
            Vec::new(),
        )
    }

    fn refused(
        cause: ConfirmCause,
        document: ProposalDocument,
        reason: &str,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            proposal: ProposalOutcome::refused(QueueCommand::Review, document, reason, messages),
            read: None,
            cause: Some(cause),
        }
    }

    /// What a run of the approve or the reject gave.
    fn ran(proposal: ProposalOutcome) -> Self {
        let cause = proposal.refusal.is_some().then_some(ConfirmCause::Refused);
        Self {
            proposal,
            read: None,
            cause,
        }
    }
}

/// `{id, question, updated_at}`, else the review document.
impl Serialize for ConfirmOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.read {
            Some(read) => read.serialize(serializer),
            None => self.proposal.serialize(serializer),
        }
    }
}

/// The question a confirm of `request.id` would ask `request.entry` now (see
/// the module documentation), or why none. Nothing is written but the data
/// directory's index, as any read writes it. Exit 2 ([`CliError`]) only when
/// it cannot run here.
pub fn confirm_question(
    env: &Env,
    globals: &Globals,
    request: &ConfirmRequest,
) -> Result<ConfirmOutcome, CliError> {
    run_question(env, globals, request).map_err(escaped_error)
}

/// Confirms the choice staged on `request.id` (see the module documentation):
/// an approve applies, one commit in the recorded worktree; a reject rejects,
/// the queue alone. `sent` is what the read gave: another question or another
/// `updated_at` changes nothing. Exit 2 ([`CliError`]) only when it cannot run
/// here.
pub fn confirm_staged(
    env: &Env,
    globals: &Globals,
    request: &ConfirmRequest,
    sent: &ConfirmBody,
) -> Result<ConfirmOutcome, CliError> {
    run_confirm(env, globals, request, sent).map_err(escaped_error)
}

fn run_question(
    env: &Env,
    globals: &Globals,
    request: &ConfirmRequest,
) -> Result<ConfirmOutcome, CliError> {
    let git = identified(&request.git, &request.entry);
    let judged = match judged(env, globals, request, &git, None)? {
        Ok(judged) => judged,
        Err(refused) => return Ok(refused),
    };
    let mut question = None;
    let mut heard = |asked: &str| {
        question = Some(shown(asked));
        false
    };
    let run = staged_run(env, globals, request, &git, &judged, true, &mut heard)?;
    let Some(question) = question else {
        return Ok(ConfirmOutcome::ran(run));
    };
    Ok(ConfirmOutcome {
        proposal: ProposalOutcome::done(
            QueueCommand::Review,
            ProposalDocument::default(),
            Vec::new(),
        ),
        read: Some(ConfirmRead {
            id: judged.proposal.id,
            question,
            updated_at: judged.proposal.updated_at,
        }),
        cause: None,
    })
}

fn run_confirm(
    env: &Env,
    globals: &Globals,
    request: &ConfirmRequest,
    sent: &ConfirmBody,
) -> Result<ConfirmOutcome, CliError> {
    let git = identified(&request.git, &request.entry);
    let judged = match judged(env, globals, request, &git, Some(&sent.updated_at))? {
        Ok(judged) => judged,
        Err(refused) => return Ok(refused),
    };
    let mut moved = false;
    let mut same = |asked: &str| {
        moved = shown(asked) != sent.question;
        !moved
    };
    let run = staged_run(env, globals, request, &git, &judged, false, &mut same)?;
    if !moved {
        return Ok(ConfirmOutcome::ran(run));
    }
    // The run's own words are a terminal's decline; no one declined.
    let ProposalOutcome {
        mut document,
        messages,
        ..
    } = run;
    document.notes.pop();
    Ok(ConfirmOutcome::refused(
        ConfirmCause::Refused,
        document,
        &format!(
            "`{}` moved since its question was read; read it again; nothing changed",
            judged.proposal.id
        ),
        messages,
    ))
}

/// `asked` as a hub shows and compares it: every format character (Unicode
/// Cf, core [`is_format_character`]: U+00AD, U+200B, U+2060, U+FEFF and the
/// rest) written `\u{XX}`, lower-case hex, as
/// [`escape_controls`](crate::escape_controls) writes a control, which the
/// asked text already went through. The terminal's question keeps them; the
/// web UI's `shownConfirmText` writes them alike. Many to one, as on a
/// terminal: a literal `\u{200b}` and the character read alike.
fn shown(asked: &str) -> String {
    let mut out = String::with_capacity(asked.len());
    for c in asked.chars() {
        if is_format_character(c) {
            out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

/// `git` with the login and its `email` as committer and author.
fn identified(git: &GitEnv, entry: &TeamEntry) -> GitEnv {
    git.clone()
        .with_var("GIT_COMMITTER_NAME", &entry.login)
        .with_var("GIT_COMMITTER_EMAIL", &entry.email)
        .with_var("GIT_AUTHOR_NAME", &entry.login)
        .with_var("GIT_AUTHOR_EMAIL", &entry.email)
}

/// The row a confirm may run on, and which call its stage picks.
struct Judged {
    proposal: Proposal,
    /// A reject is staged; else an approve.
    rejects: bool,
}

/// `request.id`'s row judged (see the module documentation): the first refusal
/// the answer, the terminal's and the stage's before the cover, which needs the
/// index; last the `updated_at` the page read, when one was sent.
fn judged(
    env: &Env,
    globals: &Globals,
    request: &ConfirmRequest,
    git: &GitEnv,
    read_at: Option<&str>,
) -> Result<Result<Judged, ConfirmOutcome>, CliError> {
    checked_now(&request.now)?;
    let unknown = |reason: &str| {
        ConfirmOutcome::refused(
            ConfirmCause::Unknown,
            ProposalDocument::default(),
            reason,
            Vec::new(),
        )
    };
    let written = match written_id(&request.id) {
        Ok(written) => written,
        Err(error) => {
            let line = error
                .message
                .strip_prefix("spec: ")
                .unwrap_or(&error.message);
            return Ok(Err(unknown(line)));
        }
    };
    let context = open_context(env, globals, git)?;
    let Some(id) = written else {
        return Ok(Err(unknown(&no_proposal(&request.id))));
    };
    let proposal = match context.queue.get(&id) {
        Ok(Some(proposal)) => proposal,
        Ok(None) => {
            return Ok(Err(unknown(&format!(
                "no proposal `{id}` in this project's queue"
            ))));
        }
        Err(error) => return Err(queue_cannot(error)),
    };
    let document = || with_diff(&proposal, git, &context.data_dir);
    let refused = |reason: &str| {
        ConfirmOutcome::refused(ConfirmCause::Refused, document(), reason, Vec::new())
    };
    if let Some(reason) = not_a_hubs(&context, &proposal) {
        return Ok(Err(refused(&reason)));
    }
    let rejects = match proposal.staged.as_ref().map(|staged| &staged.stage) {
        None => {
            return Ok(Err(refused(&format!(
                "`{id}` has nothing staged: a confirm takes a staged choice; nothing changed"
            ))));
        }
        Some(Stage::Approve {
            editorial: true, ..
        }) => {
            let why = format!("has an editorial approve staged, which writes {LOCK_FILE}");
            return Ok(Err(refused(&terminal_only(&proposal, &why))));
        }
        Some(Stage::Approve {
            span_hash: None, ..
        }) if records_span(&proposal) => {
            let why = "has an approve staged with no span_hash, its preview having failed: a \
                       change to its target after the read would leave the question as it was";
            return Ok(Err(refused(&terminal_only(&proposal, why))));
        }
        Some(Stage::Approve { .. }) => false,
        Some(Stage::Reject { .. }) => true,
    };
    let mut messages = Vec::new();
    if let Some(reason) = outside(env, &context, &proposal, &request.entry, &mut messages) {
        let document = document();
        return Ok(Err(ConfirmOutcome::refused(
            ConfirmCause::Outside,
            document,
            &reason,
            messages,
        )));
    }
    if let Some(read_at) = read_at.filter(|read_at| *read_at != proposal.updated_at) {
        return Ok(Err(refused(&format!(
            "`{id}` changed since it was read: updated at {}, the question was read on the one \
             of {read_at}; read it again; nothing changed",
            proposal.updated_at
        ))));
    }
    Ok(Ok(Judged { proposal, rejects }))
}

/// Why `proposal` is no confirm's whatever is staged: another repository, a
/// decided row, a row a terminal alone decides.
fn not_a_hubs(context: &QueueContext, proposal: &Proposal) -> Option<String> {
    let id = &proposal.id;
    if !same_repository(&proposal.place.git_common_dir, &context.common_dir) {
        return Some(elsewhere(context, proposal, repository_gone(proposal)));
    }
    match proposal.status {
        ProposalStatus::Open => {}
        ProposalStatus::Applied => {
            let commit = proposal.applied_commit.as_deref().unwrap_or("unknown");
            return Some(format!(
                "`{id}` is already applied: commit {commit}; nothing changed"
            ));
        }
        ProposalStatus::Rejected => return Some(format!("`{id}` is rejected; nothing changed")),
        ProposalStatus::Approved => {
            return Some(terminal_only(proposal, "is approved, its apply stopped"));
        }
    }
    if proposal.kind.family() == KindFamily::Locks {
        let why = format!(
            "is a {} item, which writes {LOCK_FILE}",
            proposal.kind.as_str()
        );
        return Some(terminal_only(proposal, &why));
    }
    if let Some(task) = &proposal.task_id {
        return Some(terminal_only(proposal, &format!("is bound to {task}")));
    }
    None
}

/// The refusal of a row only a terminal decides, naming the command.
fn terminal_only(proposal: &Proposal, why: &str) -> String {
    let id = &proposal.id;
    let command = match proposal.staged.as_ref().map(|staged| &staged.stage) {
        Some(Stage::Reject { .. }) => "reject",
        Some(Stage::Approve { .. }) | None => "approve",
    };
    format!("`{id}` {why}: decide it on a terminal, `spec {command} {id}`; nothing changed")
}

/// Why `entry` may not confirm `proposal`: a `member`; else the first path
/// named, by text, outside its zones ([`named_paths`]); a holder resolving to
/// nothing or no path to judge is `owner` only. An `owner` covers all of it,
/// a gone target too.
fn outside(
    env: &Env,
    context: &QueueContext,
    proposal: &Proposal,
    entry: &TeamEntry,
    messages: &mut Vec<Message>,
) -> Option<String> {
    let login = &entry.login;
    if !entry.role.confirms() {
        return Some(format!(
            "refused: {login} is a `member`: this act needs a `lead`"
        ));
    }
    if entry.role == TeamRole::Owner {
        return None;
    }
    let zones = entry.zones_text();
    let paths = match named_paths(env, context, proposal, messages) {
        Ok(paths) => paths,
        Err(why) => {
            return Some(format!(
                "refused: {login} leads {zones}; {why}: this act needs an `owner`"
            ));
        }
    };
    paths
        .into_iter()
        .find(|path| !entry.covers(path))
        .map(|path| format!("refused: {login} leads {zones}; {path} lies outside them"))
}

/// Every path a confirm of `proposal` names: `target_path` for a kind that
/// applies, and for an `amendment` the holder of every node its `amends`
/// links name; every target's holder for a kind that decides (its
/// `target_path` is the first target's alone); and a staged `canon` REF's
/// holder. Each holder is resolved now, in the recorded root's refreshed
/// index, read only when one is needed; else why one has none.
fn named_paths(
    env: &Env,
    context: &QueueContext,
    proposal: &Proposal,
    messages: &mut Vec<Message>,
) -> Result<BTreeSet<String>, String> {
    let decides = proposal.kind.family() == KindFamily::Decides;
    let amends = proposal.kind.amends();
    let canon = staged_canon(proposal);
    let mut paths = BTreeSet::new();
    if !decides {
        paths.insert(proposal.target_path.clone());
    }
    if !decides && !amends && canon.is_none() {
        return Ok(paths);
    }
    let now = Now::read(env, context, proposal, messages)?;
    if decides {
        let targets = proposal.target_ids();
        if targets.is_empty() {
            return Err(format!("`{}` names no target", proposal.id));
        }
        for target in &targets {
            paths.insert(now.holder(target)?);
        }
    }
    if amends {
        let amended = amended_ids(proposal, &now.recorded.config.scheme);
        if amended.is_empty() {
            return Err(format!("`{}` names no node it amends", proposal.id));
        }
        for id in &amended {
            let holder = now
                .holder(id)
                .map_err(|reason| format!("{AMENDS}: {reason}"))?;
            paths.insert(holder);
        }
    }
    if let Some(canon) = canon {
        paths.insert(now.canon_holder(canon)?);
    }
    Ok(paths)
}

/// The `canon` REF an approve staged on `proposal` carries.
fn staged_canon(proposal: &Proposal) -> Option<&str> {
    match proposal.staged.as_ref().map(|staged| &staged.stage) {
        Some(Stage::Approve {
            canon: Some(canon), ..
        }) => Some(canon.as_str()),
        _ => None,
    }
}

/// The recorded root and its index refreshed now: where a holder is
/// resolved.
struct Now {
    recorded: ProjectRoot,
    input: CheckInput,
}

impl Now {
    /// `proposal`'s recorded root, its index refreshed; else why not.
    fn read(
        env: &Env,
        context: &QueueContext,
        proposal: &Proposal,
        messages: &mut Vec<Message>,
    ) -> Result<Self, String> {
        let root = recorded_root(proposal);
        let recorded = recorded_project(&root, &context.slug)?;
        let input = indexed(env, &recorded, messages, false).map_err(|error| error.message)?;
        Ok(Self { recorded, input })
    }

    /// The holder of `written`, resolved as a proposal's target is.
    fn holder(&self, written: &str) -> Result<String, String> {
        match resolved_node(&self.recorded, &self.input, written) {
            Ok(Ok((_, path))) => Ok(path),
            Ok(Err(reason)) => Err(reason),
            Err(error) => Err(error.message),
        }
    }

    /// The holder of a staged `canon` REF (`ID`, `ID#SECTION`, `path#anchor`):
    /// the one file holding the ID, its section's too; a path's document,
    /// when the walk indexed it.
    fn canon_holder(&self, canon: &str) -> Result<String, String> {
        let config = &self.recorded.config;
        let staged = format!("the staged `canon` `{canon}`");
        match grammar::parse_canon(canon, 0, &config.scheme) {
            None => Err(format!(
                "{staged} is no `ID`, `ID#SECTION` or `path#anchor`"
            )),
            Some(Canon::Reference(found)) => {
                if !found.homoglyphs.is_empty() || found.reference.script == IdScript::Mixed {
                    return Err(format!("{staged} is no Latin ID"));
                }
                let resolver = Resolver::new(&self.input, &config.scheme, &config.paths);
                match one_holder(&resolver, &found.reference, canon) {
                    Ok(file) => Ok(resolver.paths()[file].to_owned()),
                    Err(HolderError::Dangling(reason)) => Err(format!("{staged} {reason}")),
                    Err(HolderError::Several(paths)) => Err(format!(
                        "{staged} is held by {} files: {}",
                        paths.len(),
                        paths.join(", ")
                    )),
                    Err(HolderError::Project) => Err(format!("{staged} names another project")),
                }
            }
            Some(Canon::Path(target)) => {
                let path = target.path;
                if self.input.files.iter().any(|file| file.path == path) {
                    Ok(path)
                } else {
                    Err(format!(
                        "{staged} names {path}, which is no indexed document"
                    ))
                }
            }
        }
    }
}

/// One run of what the stage picks, as `spec approve PR` with no flags or
/// `spec reject PR` with no `--reason` runs, `consent` its one question.
fn staged_run(
    env: &Env,
    globals: &Globals,
    request: &ConfirmRequest,
    git: &GitEnv,
    judged: &Judged,
    rehearsal: bool,
    consent: Consent<'_>,
) -> Result<ProposalOutcome, CliError> {
    let seen = judged.proposal.seen();
    let hub = Some(HubRun {
        judged: &seen,
        rehearsal,
    });
    let id = judged.proposal.id.clone();
    let (now, git) = (request.now.clone(), git.clone());
    if judged.rejects {
        let request = RejectRequest {
            id,
            reason: None,
            now,
            git,
        };
        run_reject(env, globals, &request, None, hub, consent)
    } else {
        let request = ApproveRequest {
            id,
            note: None,
            now,
            git,
        };
        let flags = ApproveFlags::default();
        run_approve(env, globals, &request, &flags, false, hub, consent)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use specengine_store::GitEnv;

    use super::*;
    use crate::{QuestionRequest, StageBody, StageRequest, propose_question, stage};

    /// A scratch directory, gone on drop.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("a directory");
        for entry in fs::read_dir(from).expect("the fixture").flatten() {
            let target = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).expect("a file");
            }
        }
    }

    /// The isolated variables of every git process here: the configs off, a
    /// fixed identity.
    fn git_vars(home: &Path) -> Vec<(OsString, OsString)> {
        let path = std::env::var_os("PATH").expect("PATH");
        let pairs = [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_AUTHOR_NAME", "Owner"),
            ("GIT_AUTHOR_EMAIL", "owner@example.invalid"),
            ("GIT_COMMITTER_NAME", "Owner"),
            ("GIT_COMMITTER_EMAIL", "owner@example.invalid"),
        ];
        let mut vars: Vec<(OsString, OsString)> = pairs
            .iter()
            .map(|(name, value)| ((*name).into(), (*value).into()))
            .collect();
        vars.push(("HOME".into(), home.as_os_str().to_owned()));
        vars.push(("PATH".into(), path));
        vars
    }

    fn git(root: &Path, home: &Path, args: &[&str]) {
        let status = Command::new("git")
            .env_clear()
            .envs(git_vars(home))
            .current_dir(root)
            .args(args)
            .stdout(std::process::Stdio::null())
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }

    /// The ID of the fixture's document named `file`, read from its
    /// front-matter: `tests/genre.rs` keeps every project word out of the CLI
    /// sources, so none is spelled here.
    fn id_of(root: &Path, file: &str) -> String {
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).expect("a directory").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.file_name().is_some_and(|name| name == file) {
                    let text = fs::read_to_string(&path).expect("a document");
                    let line = text.lines().find(|line| line.starts_with("id: "));
                    return line.expect("an `id:`")["id: ".len()..].to_owned();
                }
            }
        }
        panic!("no {file} in the fixture");
    }

    /// The print shop's fixture committed in a scratch repository, one
    /// question open on its back sheet, a reject staged.
    struct Staged {
        scratch: Scratch,
        env: Env,
        globals: Globals,
        git: GitEnv,
        id: String,
    }

    const NOW: &str = "2026-11-02T09:00:00Z";
    const LATER: &str = "2026-11-02T09:00:01Z";

    fn staged(name: &str) -> Staged {
        let dir = std::env::temp_dir().join(format!(
            "specengine-cli-confirm-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        let scratch = Scratch(fs::canonicalize(&dir).expect("canonical"));
        let (root, home) = (scratch.0.join("repo"), scratch.0.join("home"));
        fs::create_dir_all(&home).expect("a home");
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        copy_dir(&fixtures.join("card-press"), &root);
        git(&root, &home, &["init", "-q", "--initial-branch=main"]);
        git(&root, &home, &["add", "-A"]);
        git(&root, &home, &["commit", "-q", "-m", "the fixture"]);
        let env = Env {
            cwd: root.clone(),
            home: Some(home.as_os_str().to_owned()),
            xdg_data_home: None,
        };
        let globals = Globals {
            root: Some(root.clone()),
            config: None,
        };
        let git_env = GitEnv::new(root.clone(), git_vars(&home));
        let outcome = propose_question(
            &env,
            &globals,
            &QuestionRequest {
                node_ids: vec![id_of(&root, "sht-back.md")],
                text: "Does the tide table keep its rule?".to_owned(),
                working_answer: "Keep it.".to_owned(),
                price_of_other: "A new table.".to_owned(),
                severity: None,
                distinct_from: Vec::new(),
                author_role: None,
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: git_env.clone(),
            },
        )
        .expect("a question");
        let id = outcome.document.id.clone().expect("its ID");
        let staged = stage(
            &env,
            &globals,
            &StageRequest {
                id: id.clone(),
                body: StageBody::Reject {
                    reason: "Asked before.".to_owned(),
                    updated_at: NOW.to_owned(),
                },
                now: LATER.to_owned(),
                git: git_env.clone(),
                actor: None,
            },
        )
        .expect("a stage");
        assert_eq!(staged.cause, None, "{staged:?}");
        Staged {
            scratch,
            env,
            globals,
            git: git_env,
            id,
        }
    }

    impl Staged {
        fn row(&self) -> Proposal {
            open_context(&self.env, &self.globals, &self.git)
                .expect("the queue")
                .queue
                .get(&self.id)
                .expect("a read")
                .expect("the row")
        }

        /// The row as it read before anything happened to it: another
        /// `updated_at`, what a judge saw if a stage landed since.
        fn another(&self) -> Proposal {
            Proposal {
                updated_at: NOW.to_owned(),
                ..self.row()
            }
        }

        fn rejected(
            &self,
            judged: &Proposal,
            consent: Consent<'_>,
        ) -> Result<ProposalOutcome, CliError> {
            let seen = judged.seen();
            run_reject(
                &self.env,
                &self.globals,
                &RejectRequest {
                    id: self.id.clone(),
                    reason: None,
                    now: "2026-11-02T09:00:02Z".to_owned(),
                    git: self.git.clone(),
                },
                None,
                Some(HubRun {
                    judged: &seen,
                    rehearsal: false,
                }),
                consent,
            )
        }

        fn approved(
            &self,
            judged: &Proposal,
            consent: Consent<'_>,
        ) -> Result<ProposalOutcome, CliError> {
            let seen = judged.seen();
            run_approve(
                &self.env,
                &self.globals,
                &ApproveRequest {
                    id: self.id.clone(),
                    note: None,
                    now: "2026-11-02T09:00:02Z".to_owned(),
                    git: self.git.clone(),
                },
                &ApproveFlags::default(),
                false,
                Some(HubRun {
                    judged: &seen,
                    rehearsal: false,
                }),
                consent,
            )
        }
    }

    const MOVED: &str = "changed since it was judged";

    /// Task spec `hub-confirm`, `HubRun.judged`: the row a run reads must
    /// be the row judged, or a stage landing between the judge and the run is
    /// asked, and answered, unjudged. Its own run cannot be raced from outside
    /// the library, so the judged row is stated: another `updated_at` refuses
    /// the reject and the approve before any question, the row as it was. The
    /// row itself runs. M: `HubRun::moved` answering nothing.
    #[test]
    fn a_run_refuses_a_row_other_than_the_one_judged() {
        let staged = staged("judged");
        let row = staged.row();
        let judged = staged.another();
        assert_ne!(judged.seen(), row.seen(), "another state");
        let mut asked = 0;
        for outcome in [
            staged.rejected(&judged, &mut |question: &str| {
                asked += usize::from(!question.is_empty());
                true
            }),
            staged.approved(&judged, &mut |question: &str| {
                asked += usize::from(!question.is_empty());
                true
            }),
        ] {
            let outcome = outcome.expect("a refusal is a document");
            let refusal = outcome.refusal.expect("a refusal");
            assert!(refusal.contains(MOVED), "{refusal}");
        }
        assert_eq!(asked, 0, "no question on a row nobody judged");
        assert_eq!(staged.row(), row, "the row as it was");

        let outcome = staged
            .rejected(&row, &mut |question: &str| {
                asked += usize::from(!question.is_empty());
                true
            })
            .expect("a run");
        assert_eq!(outcome.refusal, None, "{outcome:?}");
        assert_eq!(asked, 1, "the row judged is asked");
        assert_eq!(staged.row().status, ProposalStatus::Rejected);
        assert!(staged.scratch.0.exists());
    }
}
