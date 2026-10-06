//! `spec approve PR [--note T] [--option N | --answer T] [--canon REF]` of
//! a question or a discrepancy (task spec `decision-apply`, "Rules and edge
//! cases"): the owner's choice made an accepted decision record in the
//! project's own shape, one new file and one `spec: apply PR-…` commit in
//! the recorded worktree, nothing else.
//!
//! - **First** (no event): `--option` off a discrepancy, `--answer` off a
//!   question, an open discrepancy without `--option`, no ID target and no
//!   `--canon`, a `--canon` that is no `canon:` value, a blank `--answer`,
//!   a choice with no line that is not blank (no title) exit 2; an
//!   `--option` out of range, an `--answer` over 2 048 bytes, a `--canon`
//!   over 512, a character a record never carries in any of its free-text
//!   sources exit 1. An `approved` one (an apply stopped after step 7)
//!   takes only `--note` and writes its stored `record_text`.
//! - Then its own commit completes it ([`crate::apply::complete`]; an open
//!   one holding a record, as recorded: `--option` or `--answer` naming
//!   another choice, or `--canon` another canon, exits 2); another commit
//!   with its `Proposal:` trailer refuses a new apply.
//! - **Steps**, each refusal logged as an update's: 2 the place; 3 the
//!   recorded root's `[decision_records]` and its template (no symlink on
//!   its path, regular, tracked, clean, UTF-8, at most 64 KiB, no
//!   character a record never carries, its slots known and placed,
//!   `{{canon}}` in its front-matter); 4 the index refreshed, the ID
//!   previewed (the one the proposal kept, else the queue's next), the
//!   path `<dir>/<ID>.md` in the walk, nothing there nor in git's index
//!   (what a killed run left, the held record intent-to-add: the two ways
//!   out named), no symlink or non-directory on its way; the committer
//!   identity; 5 the render (one pass; the run's date; free text only
//!   where the template puts it); 6 the record's structure (a defect named
//!   by the free-text value or `--canon` that brings it, else the
//!   template's), the tree checked as is and with the record; the owner's
//!   consent on the record shown, escaped; 7 the ID issued in the queue's
//!   transaction; 8 the file created, never replacing one; 9 its
//!   intent-to-add entry and `git commit --only`, a failure undone (the
//!   index entry, the file and the directories this run made); 10 the
//!   commit verified: one parent, the old `HEAD`, exactly `A <path>`, its
//!   blob the record, its trailer.
//!
//! The record's shape comes only from the project's table and template
//! (the domain-free core); the engine names no prefix, directory or
//! heading of its own.

use std::fs;
use std::io::Read as _;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use specengine_core::DOCUMENT_EXTENSION;
use specengine_core::intake::ANSWER_MAX;
use specengine_core::proposal::{CommitFacts, PROPOSAL_TRAILER, TEXT_MAX_BYTES, commit_message};
use specengine_core::record::{
    ACCEPTED, CANON_MAX_BYTES, Choice, RecordDefect, Slot, SlotValues, TEMPLATE_MAX_BYTES,
    Template, canon_text, highest_record_number, is_escaped, reads_back_canon, record_defect,
    record_title, refused_char,
};
use specengine_model::{ParsedFile, grammar};
use specengine_store::{
    CreateFileError, Decision, IndexWriter as _, Intake, NamedBytes, Proposal, ProposalFinding,
    ProposalKind, ProposalQueue as _, ProposalStatus, QueueError, RecordApproval, RecordSeries,
    SpecIndex as _, WorkingTree, WorktreeGit, create_file, default_baseline, introduced_findings,
    load_check,
};

use crate::apply::{
    ApproveFlags, ApproveRequest, Consent, Made, complete, failed, made_anyway, noted,
    recorded_already, verify,
};
use crate::location::open_index;
use crate::preflight::{
    Placed, StepFailure, adds_record, completes_when, completing, place_step, place_unchanged,
    recorded_project, trailer_lookup,
};
use crate::project::{CONFIG_FILE, ProjectRoot};
use crate::proposals::{
    ProposalOutcome, QueueCommand, QueueContext, finding_line, queue_cannot, queue_refusal, top_of,
    with_diff,
};
use crate::propose::is_path_target;
use crate::refresh::refresh;
use crate::{CliError, Env, Exit, Message, escape_controls, one_line};

const COMMAND: QueueCommand = QueueCommand::Approve;

/// `spec approve` of a question or a discrepancy, `open` or `approved`
/// (`run_approve` has refused an applied or rejected one): see the module
/// documentation.
pub(crate) fn approve_record(
    env: &Env,
    request: &ApproveRequest,
    flags: &ApproveFlags,
    context: &mut QueueContext,
    proposal: &Proposal,
    consent: Consent<'_>,
) -> Result<ProposalOutcome, CliError> {
    let id = proposal.id.as_str();
    let Some(intake) = proposal.intake.as_ref() else {
        return Err(CliError::spec(format!(
            "`{id}` holds no fields of a {}",
            proposal.kind.as_str()
        )));
    };
    let mut messages = Vec::new();

    // First: the flags, the free text; nothing logged.
    let kept = match (proposal.status, &proposal.record) {
        (ProposalStatus::Approved, Some(record)) => {
            if flags.any() {
                return Err(CliError::spec(format!(
                    "`{id}` was approved with its record `{}` (an apply stopped after step 7): \
                     `spec approve {id}` writes that record and takes only `--note`; nothing \
                     changed",
                    record.id
                )));
            }
            Some(record.clone())
        }
        _ => None,
    };
    let (choice, sources) = match &kept {
        Some(record) => (record.choice.clone(), None),
        None => {
            let scheme = &context.project.config.scheme;
            let choice = match first_checks(proposal, intake, flags, scheme)? {
                Ok(choice) => choice,
                Err(reason) => return refused(context, request, proposal, &reason, messages),
            };
            let sources = Sources::new(proposal.kind, intake, &choice, request.note.as_deref());
            if sources.title().is_empty() {
                return Err(CliError::spec(format!(
                    "`{id}`'s record would have no title: {} holds no line that is not blank; \
                     nothing changed",
                    choice.described()
                )));
            }
            let canon = flags.canon.iter().map(|canon| ("--canon", canon.as_str()));
            let named = sources
                .fields
                .iter()
                .map(|field| (field.name.as_str(), field.text.as_str()))
                .chain(canon);
            for (name, text) in named {
                if let Some(c) = refused_char(text) {
                    let reason = format!(
                        "{name}: holds U+{:04X}: a decision record never carries it; nothing \
                         changed",
                        u32::from(c)
                    );
                    return refused(context, request, proposal, &reason, messages);
                }
            }
            (choice, Some(sources))
        }
    };

    // Its own commit completes it; another one with its trailer refuses a
    // new record. A lookup git cannot make is a note.
    let mut unknown = None;
    match trailer_lookup(&request.git, context, proposal) {
        Ok(found) => {
            if let Some(own) = completing(&found) {
                // An open one holding a record (reopened after its step 7)
                // completes as recorded: a choice or a canon given
                // otherwise is no choice made.
                if let Some(record) = &proposal.record {
                    let scheme = &context.project.config.scheme;
                    let mut given = Vec::new();
                    if (flags.option.is_some() || flags.answer.is_some()) && record.choice != choice
                    {
                        given.push(match (&record.choice, &choice) {
                            (Choice::Answer(_), Choice::Answer(_)) => "another answer".to_owned(),
                            _ => choice.described(),
                        });
                    }
                    if let Some(canon) = &flags.canon {
                        let parsed = parse_record(&record.path, &record.text, scheme);
                        if !parsed
                            .as_ref()
                            .is_some_and(|parsed| reads_back_canon(parsed, canon, scheme))
                        {
                            let stored = parsed
                                .as_ref()
                                .and_then(canon_text)
                                .map_or_else(|| "none".to_owned(), |text| format!("`{text}`"));
                            given.push(format!("the canon `{canon}` (the record's: {stored})"));
                        }
                    }
                    if !given.is_empty() {
                        // A discrepancy names its option again; a question
                        // takes no flag.
                        let again = match record.choice {
                            Choice::Option(option) => {
                                format!("spec approve {id} --option {option}")
                            }
                            Choice::WorkingAnswer | Choice::Answer(_) => {
                                format!("spec approve {id}")
                            }
                        };
                        return Err(CliError::spec(format!(
                            "`{id}` holds its record `{}` ({}), completed by its commit {} on \
                             `{}`: `{again}` completes it as recorded; the flags name {}; \
                             nothing changed",
                            record.id,
                            record.choice.described(),
                            own.commit,
                            proposal.place.branch,
                            given.join(" and ")
                        )));
                    }
                }
                let commit = own.commit.clone();
                return complete(env, request, context, proposal, &commit, consent, messages);
            }
            if let Some(first) = found.first() {
                let failure = StepFailure::refused(
                    4,
                    format!(
                        "the commit {} on `{}` carries `{PROPOSAL_TRAILER}: {id}` but {}: a new \
                         apply would record the decision again; {}; no new apply",
                        first.commit,
                        proposal.place.branch,
                        first.not_completing.as_deref().unwrap_or_default(),
                        completes_when(proposal)
                    ),
                );
                return failed(context, request, proposal, None, failure, None, messages);
            }
        }
        Err(error) => {
            let note = one_line(&format!(
                "cannot tell whether `{id}` has its commit on `{}`: {error}",
                proposal.place.branch
            ));
            messages.push(Message::Note(note.clone()));
            unknown = Some(note);
        }
    }
    // Until step 7 the run holds nothing: a refusal only logs.
    let before = |context: &mut QueueContext, mut failure: StepFailure, messages: Vec<Message>| {
        if failure.exit == Exit::CannotRun
            && let Some(unknown) = &unknown
        {
            failure.reason.push_str(&format!("; {unknown}"));
        }
        failed(context, request, proposal, None, failure, None, messages)
    };

    // Step 2: the place.
    let Placed { git, top, head } = match place_step(&request.git, proposal) {
        Ok(placed) => placed,
        Err(failure) => return before(context, failure, messages),
    };

    // Step 3: the config and the template.
    let place = &proposal.place;
    let root = if place.root_rel.is_empty() {
        top.clone()
    } else {
        top.join(&place.root_rel)
    };
    let recorded = match recorded_project(&root, &context.slug) {
        Ok(recorded) => recorded,
        Err(reason) => return before(context, StepFailure::cannot(3, reason), messages),
    };
    let tree = match WorkingTree::new(&recorded.root, &recorded.config.paths) {
        Ok(tree) => tree,
        Err(error) => {
            return before(context, StepFailure::cannot(3, error.to_string()), messages);
        }
    };
    let scheme = recorded.config.scheme.clone();
    let shape = match &kept {
        Some(_) => None,
        None => {
            let Some(table) = recorded.config.decision_records.clone() else {
                let reason = format!(
                    "{}/{CONFIG_FILE} has no `[decision_records]` (`prefix`, `dir`, \
                     `template`) for `{id}`'s record: add it, or `spec reject {id} --reason \
                     <answer>`; nothing changed",
                    recorded.root.display()
                );
                return before(context, StepFailure::cannot(3, reason), messages);
            };
            match read_template(&git, &recorded, proposal, &table.template) {
                Ok(template) => Some((table, template)),
                Err(failure) => return before(context, failure, messages),
            }
        }
    };

    // Step 4: the ID and the path.
    let mut index = match open_index(env, &recorded) {
        Ok(index) => index,
        Err(error) => return before(context, StepFailure::cannot(4, error.message), messages),
    };
    match refresh(&mut index.index, &recorded, false) {
        Ok((_, warnings)) => messages.extend(warnings),
        Err(error) => return before(context, StepFailure::cannot(4, error.message), messages),
    }
    let (record_id, path, series) = match (&kept, &shape) {
        (Some(record), _) => {
            let prefix = record.id.split('-').next().unwrap_or_default().to_owned();
            let series = RecordSeries {
                prefix,
                width: 0,
                corpus_max: 0,
            };
            (record.id.clone(), record.path.clone(), series)
        }
        (None, Some((table, _))) => {
            let Some(spec) = scheme.prefix(&table.prefix) else {
                let reason = format!(
                    "`[decision_records] prefix` `{}` is no `[ids]` prefix of {}",
                    table.prefix, recorded.config_label
                );
                return before(context, StepFailure::cannot(3, reason), messages);
            };
            let input = match index.index.indexed_input() {
                Ok(input) => input,
                Err(error) => {
                    return before(context, StepFailure::cannot(4, error.to_string()), messages);
                }
            };
            let parsed = input.files.iter().filter_map(|file| file.parsed.as_ref());
            let series = RecordSeries {
                prefix: table.prefix.clone(),
                width: spec.width.unwrap_or(1),
                corpus_max: highest_record_number(parsed, spec),
            };
            let preview = match &proposal.record {
                Some(record) => record.id.clone(),
                None => match context.queue.next_record(&series) {
                    Ok(preview) => preview,
                    Err(error) => {
                        let failure = StepFailure::cannot(4, error.to_string());
                        return before(context, failure, messages);
                    }
                },
            };
            let path = format!("{}/{preview}{DOCUMENT_EXTENSION}", table.dir);
            (preview, path, series)
        }
        (None, None) => return Err(CliError::spec("no record shape was read")),
    };
    if !recorded.config.paths.in_walk_scope(&path) {
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        let reason = format!(
            "`{path}` lies outside the walk of {} (`[decision_records] dir` = `{dir}`): a record \
             is a document of the corpus; name a directory under `[paths] roots`; nothing changed",
            recorded.root.display()
        );
        return before(context, StepFailure::cannot(4, reason), messages);
    }
    let top_path = top_of(proposal, &path);
    if let Err(reason) = path_free(&git, &recorded.root, &path, &top_path, "nothing changed") {
        let reason =
            left_behind(&git, &recorded.root, &path, &top_path, proposal).unwrap_or(reason);
        return before(context, StepFailure::refused(4, reason), messages);
    }
    // The owner's identity, before anything is rendered or asked.
    let decided_by = match git.committer_ident() {
        Ok(ident) => ident,
        Err(error) => {
            let failure = StepFailure::cannot(
                7,
                format!(
                    "no git identity in {}: {error}; set user.name and user.email",
                    place.worktree
                ),
            );
            return before(context, failure, messages);
        }
    };

    // Step 5: the render.
    let today = request.now.get(..10).unwrap_or_default();
    let (text, title, canon) = match (&kept, &shape, sources) {
        (Some(record), _, _) => (record.text.clone(), record.title.clone(), None),
        (None, Some((_, template)), Some(mut sources)) => {
            if let Some(c) = refused_char(&decided_by) {
                let reason = format!(
                    "decided_by: holds U+{:04X}: a decision record never carries it; nothing \
                     changed",
                    u32::from(c)
                );
                return before(context, StepFailure::refused(5, reason), messages);
            }
            sources.push("decided_by", &decided_by);
            let id_targets: Vec<String> = intake
                .target_ids
                .iter()
                .filter(|target| !is_path_target(target))
                .cloned()
                .collect();
            let canon = flags
                .canon
                .clone()
                .or_else(|| id_targets.first().cloned())
                .unwrap_or_default();
            let engine = Engine {
                id: &record_id,
                date: today,
                canon: &canon,
                targets: &id_targets,
                proposal: id,
            };
            let all = sources.fields.len();
            let text = template.render(&sources.values(&engine, all));
            if text.len() > TEXT_MAX_BYTES {
                let reason = format!(
                    "the record `{path}` renders to {} bytes; a record has at most \
                     {TEXT_MAX_BYTES}; nothing changed",
                    text.len()
                );
                return before(context, StepFailure::refused(5, reason), messages);
            }
            // Step 6: the structure, a defect named by what brings it.
            let defect = |kept_fields: usize| {
                let text = template.render(&sources.values(&engine, kept_fields));
                parse_record(&path, &text, &scheme)
                    .and_then(|parsed| record_defect(&parsed, &record_id, Some(&canon), &scheme))
            };
            // The template reads back a `canon:` it is given (the record's
            // own ID as a probe, every free-text value empty): a `canon:`
            // defect then is `--canon`'s, not the template's.
            let reads_canon = || {
                let probe = Engine {
                    canon: &record_id,
                    ..engine
                };
                let text = template.render(&sources.values(&probe, 0));
                parse_record(&path, &text, &scheme).is_some_and(|parsed| {
                    record_defect(&parsed, &record_id, Some(&record_id), &scheme).is_none()
                })
            };
            if let Some(found) = defect(all) {
                // With every free-text value empty: the template's defect,
                // or `--canon`'s; else the first value whose text, added to
                // those before it, brings one.
                let failure = match defect(0) {
                    Some(RecordDefect::Canon) if flags.canon.is_some() && reads_canon() => {
                        StepFailure::refused(
                            6,
                            format!("--canon: with it, {}; nothing changed", RecordDefect::Canon),
                        )
                    }
                    Some(empty) => StepFailure::cannot(
                        6,
                        format!(
                            "{}: the record it renders is no decision record: {empty}",
                            shape
                                .as_ref()
                                .map_or("", |(table, _)| table.template.as_str())
                        ),
                    ),
                    None => {
                        let culprit = (1..=all).find_map(|kept| defect(kept).map(|d| (kept, d)));
                        let (name, found) = match culprit {
                            Some((kept, found)) => (sources.fields[kept - 1].name.clone(), found),
                            None => ("the record".to_owned(), found),
                        };
                        StepFailure::refused(
                            6,
                            format!("{name}: with it, {found}; nothing changed"),
                        )
                    }
                };
                return before(context, failure, messages);
            }
            let title = sources.title();
            (text, title, Some(canon))
        }
        _ => return Err(CliError::spec("no record was rendered")),
    };
    let Some(parsed) = parse_record(&path, &text, &scheme) else {
        let failure = StepFailure::refused(6, format!("the spec parser failed on `{path}`"));
        return before(context, failure, messages);
    };
    if canon.is_none()
        && let Some(defect) = record_defect(&parsed, &record_id, None, &scheme)
    {
        let failure = StepFailure::refused(6, format!("`{path}`: {defect}; nothing changed"));
        return before(context, failure, messages);
    }
    let findings = introduced(
        &recorded,
        &tree,
        today,
        &path,
        text.as_bytes().to_vec(),
        &mut messages,
    );

    // Step 1: the owner's consent, on the record as it will be written.
    let mut question = format!("introduced: {}\n", findings.len());
    for finding in &findings {
        question.push_str(&finding_line(finding));
        question.push('\n');
    }
    question.push_str(&format!("record {record_id} at {path}:\n"));
    for line in text.split_terminator('\n') {
        if !line.is_empty() {
            question.push_str("  ");
            question.push_str(line);
        }
        question.push('\n');
    }
    question.push_str(&format!(
        "apply {id} as {record_id} ({}) on {} in {}? [y/N]",
        choice.described(),
        place.branch,
        place.worktree
    ));
    if !consent(&noted(unknown.as_deref(), &escape_controls(&question))) {
        let reason = format!("`{id}` not applied: the answer was not `y`; nothing changed");
        return refused(context, request, proposal, &reason, messages);
    }

    // Step 7: the ID issued, the record set, as a compare-and-set.
    let decision = Decision {
        decided_by: decided_by.clone(),
        note: request.note.clone(),
    };
    let approval = RecordApproval {
        series,
        preview: record_id.clone(),
        path: path.clone(),
        title: title.clone(),
        text: text.clone(),
        choice,
    };
    let approved = match context.queue.approve_record_from(
        id,
        &proposal.seen(),
        &approval,
        &decision,
        &request.now,
    ) {
        Ok(approved) => approved,
        Err(QueueError::Issued { next }) => {
            let failure = StepFailure::refused(
                7,
                format!(
                    "`{record_id}` was issued meanwhile (next `{next}`): run `spec approve {id}` \
                     again; nothing written"
                ),
            );
            return failed(context, request, proposal, None, failure, None, messages);
        }
        Err(
            error @ (QueueError::Changed { .. }
            | QueueError::Status { .. }
            | QueueError::Invalid(_)),
        ) => {
            let failure = StepFailure::refused(7, format!("{error}; nothing written"));
            return failed(context, request, proposal, None, failure, None, messages);
        }
        Err(error) => return Err(queue_cannot(error)),
    };
    let held = approved.seen();
    let after = |context: &mut QueueContext, failure: StepFailure, messages: Vec<Message>| {
        failed(
            context,
            request,
            proposal,
            Some(&held),
            failure,
            None,
            messages,
        )
    };

    // Step 8: the place and the path as checked, then the new file.
    if let Err(failure) = place_unchanged(&git, &head, place) {
        return after(context, failure, messages);
    }
    if let Err(reason) = path_free(&git, &recorded.root, &path, &top_path, "nothing written") {
        return after(context, StepFailure::refused(8, reason), messages);
    }
    let created = match create_file(&recorded.root, &path, text.as_bytes()) {
        Ok(created) => created,
        Err(CreateFileError::Exists) => {
            return after(
                context,
                StepFailure::refused(8, exists(&path, "nothing written")),
                messages,
            );
        }
        Err(error) => {
            let reason = format!("cannot create `{path}`: {error}; nothing written");
            return after(context, StepFailure::refused(8, reason), messages);
        }
    };

    // Step 9: its index entry, then the commit of that one path.
    let message = commit_message(&CommitFacts {
        id,
        rationale: &format!("{record_id}: {title}"),
        decided_by: &decided_by,
        author: &proposal.author,
        base_commit: &place.base_commit,
    });
    let committed = git
        .intent_to_add(&top_path)
        .and_then(|()| git.commit_only(&context.data_dir, &message, &top_path));
    if let Err(error) = committed {
        let branch = &place.branch;
        match made_anyway(&git, &head, proposal) {
            Made::Commit { tip, on_branch } => {
                let moved = if on_branch {
                    format!("`{branch}` moved to {tip}")
                } else {
                    format!("HEAD moved to {tip}")
                };
                messages.push(Message::Warning(format!(
                    "the commit reported a failure, yet {moved}: {error}"
                )));
            }
            Made::Nothing { foreign } => {
                if let Err(cause) = git.remove_cached(&top_path) {
                    messages.push(Message::Warning(format!(
                        "cannot drop the index entry of `{path}` in {}: {cause}; `git rm \
                         --cached -- {top_path}` drops it",
                        place.worktree
                    )));
                }
                for left in created.remove() {
                    messages.push(Message::Warning(left));
                }
                let foreign = foreign.map_or_else(String::new, |tip| {
                    format!(
                        "; `{branch}` moved to {tip}, which has no `{PROPOSAL_TRAILER}: {id}` \
                         trailer (not this apply's commit)"
                    )
                });
                let failure = StepFailure::refused(
                    9,
                    format!("the commit of `{path}` failed, the record removed: {error}{foreign}"),
                );
                return after(context, failure, messages);
            }
        }
    }

    // Step 10: this run's commit adds the record and nothing else.
    let adds_the_record = |commit: &str| -> Result<(), String> {
        adds_record(&git, &head, commit, &top_path, &text).map_err(|why| format!("it {why}"))
    };
    let commit = match verify(&git, &head, &approved, &adds_the_record) {
        Ok(commit) => commit,
        Err(reason) => return after(context, StepFailure::refused(10, reason), messages),
    };
    let applied = match context
        .queue
        .applied_with(id, &commit, &decision, &request.now)
    {
        Ok(applied) => applied,
        Err(error) => match recorded_already(context, &error, &commit, &mut messages) {
            Some(stored) => stored,
            None => {
                let reason = queue_refusal(error)?;
                return refused(context, request, &approved, &reason, messages);
            }
        },
    };
    if let Err(error) = index
        .index
        .update_paths(&tree, &recorded.config.scheme, &[path.as_str()])
    {
        messages.push(Message::Warning(format!(
            "the index was not updated after the commit: {error}; the next command updates it"
        )));
    }
    let canon = canon_text(&parsed).unwrap_or_else(|| "no section".to_owned());
    match &proposal.linked {
        Some(linked) => {
            if let Ok(Some(update)) = context.queue.get(linked)
                && update.status != ProposalStatus::Applied
            {
                messages.push(Message::Note(format!(
                    "`{linked}` (linked) is {}: `spec approve {linked}` changes {canon}",
                    update.status
                )));
            }
        }
        None => messages.push(Message::Note(format!(
            "{record_id} names {canon} in `canon:`, its text unchanged: `spec propose update` \
             changes it"
        ))),
    }
    let document = with_diff(&applied, &request.git, &context.data_dir);
    Ok(ProposalOutcome::done(COMMAND, document, messages))
}

/// A refusal with no event: the stored proposal's document, `reason` last.
fn refused(
    context: &QueueContext,
    request: &ApproveRequest,
    proposal: &Proposal,
    reason: &str,
    messages: Vec<Message>,
) -> Result<ProposalOutcome, CliError> {
    let document = with_diff(proposal, &request.git, &context.data_dir);
    Ok(ProposalOutcome::refused(
        COMMAND, document, reason, messages,
    ))
}

/// The flags of an open question or discrepancy checked (see the module
/// documentation): its [`Choice`], a refusal (`Ok(Err)`, exit 1), or exit
/// 2.
fn first_checks(
    proposal: &Proposal,
    intake: &Intake,
    flags: &ApproveFlags,
    scheme: &specengine_model::IdScheme,
) -> Result<Result<Choice, String>, CliError> {
    let id = &proposal.id;
    let question = proposal.kind == ProposalKind::Question;
    let count = intake.options.len();
    let range = format!("0-{}", count.saturating_sub(1));
    if question && flags.option.is_some() {
        return Err(CliError::spec(format!(
            "`--option` names a discrepancy's option, and `{id}` is a question: its working \
             answer, or `--answer T`, answers it; nothing changed"
        )));
    }
    if !question && flags.answer.is_some() {
        return Err(CliError::spec(format!(
            "`--answer` answers a question, and `{id}` is a discrepancy: name the owner's choice \
             with `--option N` ({range}); nothing changed"
        )));
    }
    if !question && flags.option.is_none() {
        return Err(CliError::spec(format!(
            "`{id}` is a discrepancy: name the owner's choice with `--option N` ({range}); \
             nothing changed"
        )));
    }
    let names_an_id = intake
        .target_ids
        .iter()
        .any(|target| !is_path_target(target));
    if !names_an_id && flags.canon.is_none() {
        return Err(CliError::spec(format!(
            "`{id}` names no ID: name the section its record governs with `--canon REF`; nothing \
             changed"
        )));
    }
    if let Some(canon) = &flags.canon
        && grammar::parse_canon(canon, 0, scheme).is_none()
    {
        return Err(CliError::spec(format!(
            "`--canon {canon}` is no `ID`, `ID#SECTION` or `path#anchor`; nothing changed"
        )));
    }
    if let Some(answer) = &flags.answer
        && answer.trim().is_empty()
    {
        return Err(CliError::spec(
            "`--answer` is blank: give the owner's answer, or leave it out to take the working \
             answer; nothing changed",
        ));
    }
    if let Some(option) = flags.option
        && usize::try_from(option).map_or(true, |option| option >= count)
    {
        return Ok(Err(format!(
            "--option {option}: `{id}` has options {range}; nothing changed"
        )));
    }
    if let Some(answer) = &flags.answer
        && answer.len() > ANSWER_MAX
    {
        return Ok(Err(format!(
            "--answer: {} bytes; at most {ANSWER_MAX}; nothing changed",
            answer.len()
        )));
    }
    if let Some(canon) = &flags.canon
        && canon.len() > CANON_MAX_BYTES
    {
        return Ok(Err(format!(
            "--canon: {} bytes; at most {CANON_MAX_BYTES}; nothing changed",
            canon.len()
        )));
    }
    Ok(Ok(match (flags.option, &flags.answer) {
        (Some(option), _) => Choice::Option(option),
        (None, Some(answer)) => Choice::Answer(answer.clone()),
        (None, None) => Choice::WorkingAnswer,
    }))
}

/// One free-text value a record is made of: its name in a refusal, its
/// text.
struct Field {
    name: String,
    text: String,
}

/// The free-text sources of a record, in input order, and where each slot
/// takes them from (indices into `fields`).
struct Sources {
    fields: Vec<Field>,
    summary: usize,
    /// `file`, `qpath`, `lines`, `observed`, `documented`.
    evidence: Vec<[usize; 5]>,
    /// `label`, `effect`, `price`.
    options: Vec<[usize; 3]>,
    /// The chosen option.
    chosen: Option<usize>,
    /// A question's answer: its working answer, or `--answer`.
    answer: Option<usize>,
    /// What another answer costs (`--answer`).
    cost: Option<usize>,
    note: Option<usize>,
    decided_by: Option<usize>,
}

/// The engine's slot values.
struct Engine<'a> {
    id: &'a str,
    date: &'a str,
    canon: &'a str,
    targets: &'a [String],
    proposal: &'a str,
}

impl Sources {
    fn new(kind: ProposalKind, intake: &Intake, choice: &Choice, note: Option<&str>) -> Self {
        let mut sources = Self {
            fields: Vec::new(),
            summary: 0,
            evidence: Vec::new(),
            options: Vec::new(),
            chosen: None,
            answer: None,
            cost: None,
            note: None,
            decided_by: None,
        };
        sources.summary = sources.push("summary", &intake.summary);
        if kind == ProposalKind::Discrepancy {
            for (index, item) in intake.evidence.iter().enumerate() {
                let name = |field: &str| format!("evidence[{index}].{field}");
                let file = sources.push(&name("file"), &item.file);
                let qpath = sources.push(&name("qpath"), item.qpath.as_deref().unwrap_or_default());
                let lines = sources.push(&name("lines"), item.lines.as_deref().unwrap_or_default());
                let observed = sources.push(&name("observed"), &item.observed);
                let documented = sources.push(&name("documented"), &item.documented);
                sources
                    .evidence
                    .push([file, qpath, lines, observed, documented]);
            }
            for (index, option) in intake.options.iter().enumerate() {
                let name = |field: &str| format!("options[{index}].{field}");
                let label = sources.push(&name("label"), &option.label);
                let effect = sources.push(&name("effect"), &option.effect);
                let price = sources.push(&name("price"), &option.price);
                sources.options.push([label, effect, price]);
            }
        }
        match choice {
            Choice::Option(index) => sources.chosen = usize::try_from(*index).ok(),
            Choice::WorkingAnswer => {
                let answer = intake.working_answer.as_deref().unwrap_or_default();
                sources.answer = Some(sources.push("working_answer", answer));
            }
            Choice::Answer(answer) => {
                sources.answer = Some(sources.push("--answer", answer));
                let price = intake.price_of_other.as_deref().unwrap_or_default();
                sources.cost = Some(sources.push("price_of_other", price));
            }
        }
        if let Some(note) = note {
            sources.note = Some(sources.push("--note", note));
        }
        sources
    }

    /// Adds a field; its index.
    fn push(&mut self, name: &str, text: &str) -> usize {
        if name == "decided_by" {
            self.decided_by = Some(self.fields.len());
        }
        self.fields.push(Field {
            name: name.to_owned(),
            text: text.to_owned(),
        });
        self.fields.len() - 1
    }

    /// The slot values with the first `kept` fields as written and the
    /// others empty (a defect's source: [`defect_source`]).
    fn values(&self, engine: &Engine<'_>, kept: usize) -> SlotValues {
        let text = |index: usize| {
            if index < kept {
                self.fields[index].text.as_str()
            } else {
                ""
            }
        };
        let evidence = self
            .evidence
            .iter()
            .map(|&[file, qpath, lines, observed, documented]| {
                let mut place = text(file).to_owned();
                if !text(lines).is_empty() {
                    place.push(':');
                    place.push_str(text(lines));
                }
                if !text(qpath).is_empty() {
                    place.push(' ');
                    place.push_str(text(qpath));
                }
                format!("- {place} | {} | {}", text(observed), text(documented))
            })
            .collect();
        let options = self
            .options
            .iter()
            .map(|&[label, effect, price]| {
                format!("- {} | {} | {}", text(label), text(effect), text(price))
            })
            .collect();
        let (choice, title, effect, cost) = match (self.chosen, self.answer) {
            (Some(chosen), _) => match self.options.get(chosen) {
                Some(&[label, effect, price]) => (
                    text(label),
                    record_title(text(label)),
                    text(effect),
                    text(price),
                ),
                None => ("", String::new(), "", ""),
            },
            (None, Some(answer)) => (
                text(answer),
                record_title(first_text_line(text(answer))),
                "",
                self.cost.map_or("", text),
            ),
            (None, None) => ("", String::new(), "", ""),
        };
        SlotValues {
            id: engine.id.to_owned(),
            date: engine.date.to_owned(),
            status: ACCEPTED.to_owned(),
            canon: engine.canon.to_owned(),
            targets: engine.targets.to_vec(),
            proposal: engine.proposal.to_owned(),
            title,
            choice: choice.to_owned(),
            effect: effect.to_owned(),
            cost: cost.to_owned(),
            summary: text(self.summary).to_owned(),
            options,
            evidence,
            note: self.note.map_or("", text).to_owned(),
            decided_by: self.decided_by.map_or("", text).to_owned(),
        }
    }

    /// The record's title: the chosen label, or the answer's first line
    /// that is not blank, normalised; empty when there is none.
    fn title(&self) -> String {
        match (self.chosen, self.answer) {
            (Some(chosen), _) => self
                .options
                .get(chosen)
                .map(|&[label, _, _]| record_title(&self.fields[label].text))
                .unwrap_or_default(),
            (None, Some(answer)) => record_title(first_text_line(&self.fields[answer].text)),
            (None, None) => String::new(),
        }
    }
}

/// The first line of `text` that is not blank (whitespace only); empty
/// when there is none.
fn first_text_line(text: &str) -> &str {
    text.split('\n')
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
}

/// `text` parsed as the record at `path`; `None` when the parser panics.
fn parse_record(path: &str, text: &str, scheme: &specengine_model::IdScheme) -> Option<ParsedFile> {
    panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(path, text.as_bytes(), scheme)
    }))
    .ok()
}

/// The findings the record introduces in the recorded root's check, the
/// tree checked as is and with it, never refusing.
fn introduced(
    recorded: &ProjectRoot,
    tree: &WorkingTree,
    today: &str,
    path: &str,
    bytes: Vec<u8>,
    messages: &mut Vec<Message>,
) -> Vec<ProposalFinding> {
    let config = NamedBytes::read(CONFIG_FILE, &recorded.root.join(CONFIG_FILE));
    let baseline = default_baseline(&recorded.root);
    match load_check(&config, baseline.as_ref()) {
        Ok(setup) => introduced_findings(tree, &setup, today, path, bytes),
        Err(report) => {
            messages.push(Message::Warning(format!(
                "the record was not checked (no introduced findings shown): {}",
                report.lines(false).join("; ")
            )));
            Vec::new()
        }
    }
}

/// `` `<path>` exists `` refusal.
fn exists(path: &str, tail: &str) -> String {
    format!("`{path}` exists: a decision record never replaces a file; {tail}")
}

/// Step 4 (and 8): nothing at `path` under `root` (a dangling symlink
/// too) nor in git's index (`top_path`), and every existing component of
/// its way a directory, no symlink; else why, ending in `tail`.
fn path_free(
    git: &WorktreeGit,
    root: &Path,
    path: &str,
    top_path: &str,
    tail: &str,
) -> Result<(), String> {
    let components: Vec<&str> = path.split('/').collect();
    let mut at = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        at.push(component);
        let shown = components[..=index].join("/");
        let last = index + 1 == components.len();
        match fs::symlink_metadata(&at) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(format!("cannot read `{shown}`: {error}; {tail}")),
            Ok(_) if last => return Err(exists(path, tail)),
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "`{shown}` is a symlink: a decision record is never written through one; \
                     {tail}"
                ));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(format!(
                    "`{shown}` is not a directory, so `{path}` cannot be created; {tail}"
                ));
            }
            Ok(_) => {}
        }
    }
    match git.is_tracked(top_path) {
        Ok(true) => Err(exists(path, tail)),
        Ok(false) => Ok(()),
        Err(error) => Err(format!(
            "cannot tell whether git's index holds `{path}`: {error}; {tail}"
        )),
    }
}

/// Step 4's `exists` naming the two ways out when `path` is what a run
/// killed between its steps 8 and 9 left: a regular file whose bytes are
/// the proposal's held `record_text`, its index entry intent-to-add.
/// `None` otherwise (the plain refusal stands).
fn left_behind(
    git: &WorktreeGit,
    root: &Path,
    path: &str,
    top_path: &str,
    proposal: &Proposal,
) -> Option<String> {
    let record = proposal
        .record
        .as_ref()
        .filter(|record| record.path == path)?;
    let at = root.join(path);
    let regular = fs::symlink_metadata(&at).is_ok_and(|meta| meta.file_type().is_file());
    if !regular {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(&at)
        .ok()?
        .take(record.text.len() as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes != record.text.as_bytes() || !git.is_intent_to_add(top_path).ok()? {
        return None;
    }
    let (id, record_id) = (&proposal.id, &record.id);
    Some(exists(
        path,
        &format!(
            "it holds `{record_id}`'s record as an interrupted apply left it, with an \
             intent-to-add entry in git's index; two ways out in {}: commit it by hand (`git \
             commit --only --trailer '{PROPOSAL_TRAILER}: {id}' -m '{record_id}' -- \
             {top_path}`), then `spec approve {id}` completes it; or `git rm --cached -- \
             {top_path}`, delete the file, then `spec approve {id}` writes it again; nothing \
             changed",
            proposal.place.worktree
        ),
    ))
}

/// Step 3's template: `template` (root-relative) under the recorded root,
/// no symlink on its way, a regular file, tracked and clean in the
/// worktree, UTF-8, at most [`TEMPLATE_MAX_BYTES`], no character a record
/// never carries ([`is_escaped`]), its slots known and placed, `{{canon}}`
/// in its front-matter; else exit 2 `<template>:<line>: <problem>`.
fn read_template(
    git: &WorktreeGit,
    recorded: &ProjectRoot,
    proposal: &Proposal,
    template: &str,
) -> Result<Template, StepFailure> {
    let fail = |line: usize, problem: String| {
        StepFailure::cannot(3, format!("{template}:{line}: {problem}"))
    };
    let worktree = &proposal.place.worktree;
    let components: Vec<&str> = template.split('/').collect();
    let mut at = recorded.root.clone();
    for (index, component) in components.iter().enumerate() {
        at.push(component);
        let shown = components[..=index].join("/");
        let meta = fs::symlink_metadata(&at)
            .map_err(|error| fail(1, format!("cannot read it: {error}")))?;
        if meta.file_type().is_symlink() {
            return Err(fail(
                1,
                format!("`{shown}` is a symlink: a template is read through none"),
            ));
        }
        let last = index + 1 == components.len();
        if !last && !meta.is_dir() {
            return Err(fail(1, format!("`{shown}` is not a directory")));
        }
        if last && !meta.is_file() {
            return Err(fail(1, "not a regular file".to_owned()));
        }
    }
    let top = top_of(proposal, template);
    match git.is_tracked(&top) {
        Ok(true) => {}
        Ok(false) => {
            return Err(fail(
                1,
                format!("not tracked by git in {worktree}: commit it first"),
            ));
        }
        Err(error) => {
            return Err(fail(
                1,
                format!("cannot tell whether git tracks it: {error}"),
            ));
        }
    }
    match git.is_dirty(&top) {
        Ok(false) => {}
        Ok(true) => {
            return Err(fail(
                1,
                format!(
                    "has uncommitted changes in {worktree} (staged or not): commit, stash or \
                     restore them first"
                ),
            ));
        }
        Err(error) => return Err(fail(1, format!("cannot tell whether it is clean: {error}"))),
    }
    let mut file =
        fs::File::open(&at).map_err(|error| fail(1, format!("cannot read it: {error}")))?;
    let regular = file.metadata().is_ok_and(|meta| meta.is_file());
    if !regular {
        return Err(fail(1, "not a regular file".to_owned()));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(TEMPLATE_MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| fail(1, format!("cannot read it: {error}")))?;
    if bytes.len() > TEMPLATE_MAX_BYTES {
        return Err(fail(
            1,
            format!("over {TEMPLATE_MAX_BYTES} bytes; a template has at most {TEMPLATE_MAX_BYTES}"),
        ));
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            let valid = error.utf8_error().valid_up_to();
            let line = error.as_bytes()[..valid]
                .iter()
                .filter(|&&byte| byte == b'\n')
                .count()
                + 1;
            return Err(fail(line, "not UTF-8".to_owned()));
        }
    };
    if let Some((at, c)) = text.char_indices().find(|&(_, c)| is_escaped(c)) {
        let line = text.as_bytes()[..at]
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count()
            + 1;
        return Err(fail(
            line,
            format!(
                "holds U+{:04X}: a decision record never carries it",
                u32::from(c)
            ),
        ));
    }
    let parsed = Template::parse(&text).map_err(|error| fail(error.line, error.message))?;
    if !parsed.holds_in_front_matter(Slot::Canon) {
        return Err(fail(
            1,
            format!(
                "its front-matter holds no `{{{{{}}}}}`: a record names the section it governs \
                 as `canon: {{{{{0}}}}}`",
                Slot::Canon.name()
            ),
        ));
    }
    Ok(parsed)
}
