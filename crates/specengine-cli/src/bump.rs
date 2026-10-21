//! `spec bump [--staged] [--json]` (canon `node-revision`, "Raising";
//! task spec `rev-rule`, "spec bump"): the nodes with an ID at `HEAD`
//! whose meaning hash changed while their revision is not above `HEAD`'s,
//! by path and line: `required` when the rule `rev-not-raised` finds it
//! (`HEAD`'s `spec.lock` vouches for its old text and the checked lock does
//! not carry the new hash), else `suggested`; each with `HEAD`'s revision,
//! the one a raise writes, the lock entries vouching for it (its
//! `[[tested]]` block counted as one, core's rule) and the line
//! to write, as an apply writes it (neither where an apply refuses the
//! raise: an unreadable revision, at the largest revision, a document of a
//! closed class that admits no `rev`, a line that cannot be written).
//!
//! The checked tree is `spec check --changed`'s (the disk), or with
//! `--staged` `spec check --staged`'s (the git index), against `HEAD`: the
//! store's run of that check ([`specengine_store::bump_changed`],
//! [`specengine_store::bump_staged`]), the root's baseline as that check
//! reads it. Read-only: no file, index entry, git object, queue row or
//! event is written, and the data directory is not opened. Exit 0 when it
//! ran, whatever it lists; 2 when that check cannot run (its causes, the
//! rule's lock causes among them, as `cannot:` lines). No `--editorial`:
//! an editorial change goes through the queue.

use serde::Serialize;
use specengine_core::check::rev_rule::{BumpRow, Need};
use specengine_store::{GitEnv, GivenFile, NamedBytes, bump_changed, bump_staged, today_utc};

use crate::project::locate;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// The hint after `required` rows: the finding's ways out.
const REQUIRED_HINT: &str = "hint: required: spec.lock vouches for the old text: write the \
     line shown and commit (a raise), or send the change through the queue (spec propose update) \
     and have the owner approve it with --editorial";

/// The hint after `required` rows none of which has a line to write (each
/// shows `-`): the queue is the only way out.
const REQUIRED_EDITORIAL_HINT: &str = "hint: required: spec.lock vouches for the old text: send \
     the change through the queue (spec propose update) and have the owner approve it with \
     --editorial";

/// The hint after `suggested` rows.
const SUGGESTED_HINT: &str = "hint: suggested: raising it makes markers naming it lag";

/// `spec bump` options.
#[derive(Debug, Clone)]
pub struct BumpRequest {
    /// `--staged`: the git index, not the disk.
    pub staged: bool,
    /// Git's environment (`--staged`: its `GIT_INDEX_FILE` names the index).
    pub git: GitEnv,
}

/// What `spec bump` lists: `{rows}`, and `cannot` when the check could not
/// run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BumpOutcome {
    /// By path, line and ID; none when it could not run.
    pub rows: Vec<BumpRow>,
    /// The check's causes, `path: message`, in its order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cannot: Vec<String>,
    #[serde(skip)]
    pub messages: Vec<Message>,
}

impl BumpOutcome {
    /// 0 when it ran, 2 when the check could not.
    pub fn exit(&self) -> Exit {
        if self.cannot.is_empty() {
            Exit::Answered
        } else {
            Exit::CannotRun
        }
    }
}

/// `spec bump`: only discovery fails the call; the check's causes are in
/// the outcome (exit 2).
pub fn bump(env: &Env, globals: &Globals, request: &BumpRequest) -> Result<BumpOutcome, CliError> {
    let today = today_utc();
    let located = locate(env, globals)?;
    let config = globals.config.is_some().then(|| GivenFile {
        bytes: NamedBytes::read(located.config_label.clone(), &located.config_file),
        path: Some(located.config_file.clone()),
    });
    let run = if request.staged {
        bump_staged(&located.root, config, &request.git, &today)
    } else {
        bump_changed(&located.root, config, &request.git, &today)
    };
    let cannot = run
        .report
        .cannot_check
        .iter()
        .map(|cause| {
            let path = if cause.path.is_empty() {
                "."
            } else {
                cause.path.as_str()
            };
            format!("{path}: {}", cause.message)
        })
        .collect();
    Ok(BumpOutcome {
        rows: run.rows,
        cannot,
        messages: Vec::new(),
    })
}

/// `required | ID | path:line | rev 3 -> 4 | 2 lock entries | line` per
/// row, the `cannot:` lines, the hints; `nothing to raise` when it ran and
/// lists none.
pub(crate) fn render_text(outcome: &BumpOutcome) -> String {
    let mut out = String::new();
    for row in &outcome.rows {
        let rev = match (row.rev, row.to) {
            (Some(rev), Some(to)) => format!("rev {rev} -> {to}"),
            (Some(rev), None) => format!("rev {rev}"),
            (None, _) => "rev ?".to_owned(),
        };
        let entries = match row.lock_entries {
            1 => "1 lock entry".to_owned(),
            count => format!("{count} lock entries"),
        };
        out.push_str(&one_line(&format!(
            "{} | {} | {}:{} | {rev} | {entries} | {}",
            row.need.as_str(),
            row.id,
            row.path,
            row.line,
            row.write.as_deref().unwrap_or("-")
        )));
        out.push('\n');
    }
    for cause in &outcome.cannot {
        out.push_str(&format!("cannot: {}\n", one_line(cause)));
    }
    let mut required = outcome
        .rows
        .iter()
        .filter(|row| row.need == Need::Required)
        .peekable();
    if required.peek().is_some() {
        // A raise is a way out only where a line can be written.
        let hint = if required.all(|row| row.write.is_none()) {
            REQUIRED_EDITORIAL_HINT
        } else {
            REQUIRED_HINT
        };
        out.push_str(hint);
        out.push('\n');
    }
    if outcome.rows.iter().any(|row| row.need == Need::Suggested) {
        out.push_str(SUGGESTED_HINT);
        out.push('\n');
    }
    if outcome.rows.is_empty() && outcome.cannot.is_empty() {
        out.push_str("nothing to raise\n");
    }
    escape_controls(&out)
}
