//! `spec health`'s row 6, `drift` (task spec `health-drift`): plain `spec
//! verify`'s judgement, run in-process over the code index the call
//! refreshed for rows 4-5 (one handle, [`CodeShare`]), its rows but `ok`;
//! never its mode, verdict, hints or exit. No walk of its own.
//!
//! - **Reads**: plain's [`plain_reads`] (run causes (1) `[code]` and
//!   `[verify]`, (2), (4) git, (5) the lock unmerged, (6) the lock), then
//!   the handle's (1) unknown `[code] frameworks` name, (3) gap and (8)
//!   data directory as rows 4-5 met them; with no cause, the whole-key
//!   judge ([`judge_every_key`]: every lock entry's file and every stored
//!   marker's file, read again and checked against the stored BLAKE3, plain's
//!   judge over the uncut lock), every `[[tested]]` block judged by
//!   verify's one function over that tree ([`tested_entries`]), the git
//!   session ended, and the outcome concluded as plain concludes it
//!   ([`concluded`], no mode: Health has none).
//! - **Document**: `state` (`measured`, `off` without `[code]`, nothing
//!   opened; `cannot` on any run cause), `causes` plain's `cannot:` lines
//!   (`off`: rows 4-5's sentence), `counts` plain's JSON `counts`, `rows`
//!   plain's rows whose class is not `null`, verbatim, in key order,
//!   `tested` the blocks not `verified`, by node bytes (`null` when they
//!   cannot be judged: a note says why), `notes` the row's own. Not
//!   measured: `counts`, `rows` and `tested` `null`, never 0.
//! - **Text**: each row's block as plain prints it, then each block's line,
//!   prefixed `drift | `; the summary `drift: <f> fix, <c> check, <k>
//!   cannot, <o> ok[ (<m> moved)]; <s> skipped`, else `drift: <state>:
//!   <cause>` per cause. Stderr: `note: drift: <cause>` per cause of a
//!   `cannot` row, `note: drift: <note>` per note.
//! - Nothing is written beyond rows 4-5's data directory; never the exit.

use serde::Serialize;
use specengine_model::TestedRow;
use specengine_store::GitEnv;

use crate::bindings::{KeyScope, judge_every_key};
use crate::health_code::{CodeShare, RowState, SharedIndex};
use crate::project::ProjectRoot;
use crate::tested::{is_verified, tested_entries, tested_line, tested_note};
use crate::verify::{
    CountsJson, Gathered, PlainReads, VerifyRow, concluded, counts_json, moved_ok, moved_suffix,
    plain_reads, row_block,
};
use crate::verify_rows::{CAUSE_GIT, HeadMode, RunCause};
use crate::{Message, escape_controls, one_line};

/// Row 6's name, in its lines and notes.
const DRIFT: &str = "drift";

/// Row 6 as one call measured it (the JSON's `drift`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthDrift {
    /// `measured`, `off` or `cannot`.
    pub state: RowState,
    /// Plain verify's `cannot:` lines, in its order (`off`: rows 4-5's
    /// sentence); none when measured.
    pub causes: Vec<String>,
    /// Plain verify's JSON `counts`; `None` unless measured.
    pub(crate) counts: Option<CountsJson>,
    /// Plain verify's rows whose class is not `null`, in key order; `None`
    /// unless measured.
    pub rows: Option<Vec<VerifyRow>>,
    /// The `[[tested]]` blocks not `verified`, by node bytes; `None` unless
    /// measured, and when they cannot be judged (a note says why).
    pub tested: Option<Vec<TestedRow>>,
    /// The row's own notes (`tested: <cause>`).
    pub notes: Vec<String>,
    /// The `ok` rows whose binding moved (the summary's).
    #[serde(skip)]
    pub(crate) moved: usize,
}

impl HealthDrift {
    fn not_measured(state: RowState, causes: Vec<String>) -> Self {
        Self {
            state,
            causes,
            counts: None,
            rows: None,
            tested: None,
            notes: Vec::new(),
            moved: 0,
        }
    }

    /// The stderr notes: `drift: <cause>` per cause of a `cannot` row, then
    /// `drift: <note>` per note.
    pub(crate) fn messages(&self) -> Vec<Message> {
        let causes = self
            .causes
            .iter()
            .filter(|_| self.state == RowState::Cannot);
        causes
            .chain(&self.notes)
            .map(|text| Message::Note(format!("{DRIFT}: {text}")))
            .collect()
    }
}

/// Row 6 of `project`, whose config reads as `text` (row 3's bytes), over
/// what rows 4-5 left it: never an error, a run cause is its cause.
pub(crate) fn drift_row(
    project: &ProjectRoot,
    text: &str,
    git_env: &GitEnv,
    share: CodeShare,
) -> HealthDrift {
    let share = match share {
        // No `[code]`: nothing else is read, whatever plain would list.
        CodeShare::Off(cause) => return HealthDrift::not_measured(RowState::Off, vec![cause]),
        other => other,
    };
    let mut causes: Vec<RunCause> = Vec::new();
    let PlainReads {
        code,
        configured: _,
        git,
        lock,
    } = plain_reads(project, text, git_env, &mut causes);
    let shared = match share {
        CodeShare::Open(shared) => Some(shared),
        // Plain's read of `[code]` named it already when it does not read.
        CodeShare::Failed(cause) if code.is_some() => {
            causes.push(cause);
            None
        }
        CodeShare::Failed(_) | CodeShare::Unread | CodeShare::Off(_) => None,
    };

    let mut messages = Vec::new();
    let mut judged = None;
    let mut tested = None;
    match (causes.is_empty(), code, lock, git, shared) {
        (true, Some(code), Some(lock), Some(git), Some(shared)) => {
            let SharedIndex { mut index, input } = shared;
            let scope = KeyScope {
                project,
                code: &code,
                input: &input,
                lock: &lock,
            };
            // A second refresh's notes are dropped: rows 4-5 print the
            // first's.
            let mut refreshed = Vec::new();
            match judge_every_key(&scope, &mut index, &git, &mut refreshed, 0) {
                Ok(keys) => {
                    // Every block, over the tree holding every marker file;
                    // a failure to judge them one note, `tested` null.
                    match tested_entries(project, &input, &keys.tree, &lock, &git, &|_| true) {
                        Ok(entries) => tested = Some(entries),
                        Err(cause) => messages.push(tested_note(&cause)),
                    }
                    judged = Some(keys.judged);
                }
                Err(cause) => causes.push(cause),
            }
            if let Err(cause) = git.finish() {
                causes.push(RunCause::new(CAUSE_GIT, cause.message));
            }
        }
        // The session's end is no cause of its own: one is listed already.
        (_, _, _, Some(git), _) => {
            let _ = git.finish();
        }
        _ => {}
    }
    let was_judged = judged.is_some();
    let outcome = concluded(Gathered {
        configured: None,
        has_base: false,
        base: None,
        task: None,
        node: None,
        head_mode: HeadMode::Absent,
        judged,
        causes,
        messages,
        label: &project.config_label,
        tests: None,
        symbols: None,
        tested,
    });
    // Measured only when judged with no cause: never 0 for a run that did
    // not judge.
    if !was_judged || !outcome.cannot.is_empty() {
        return HealthDrift::not_measured(RowState::Cannot, outcome.cannot);
    }
    let counts = counts_json(&outcome);
    let moved = moved_ok(&outcome.rows);
    let notes = outcome.notes();
    HealthDrift {
        state: RowState::Measured,
        causes: Vec::new(),
        counts: Some(counts),
        rows: Some(
            outcome
                .rows
                .into_iter()
                .filter(|row| row.class.is_some())
                .collect(),
        ),
        tested: outcome.tested.map(|entries| {
            entries
                .into_iter()
                .filter(|entry| !is_verified(entry))
                .collect()
        }),
        notes,
        moved,
    }
}

/// Row 6's lines, after row 5's: each row's block, then each block's line,
/// prefixed `drift | `, through [`escape_controls`].
pub(crate) fn drift_lines(drift: &HealthDrift, out: &mut String) {
    for row in drift.rows.iter().flatten() {
        out.push_str(&escape_controls(&format!("{DRIFT} | {}", row_block(row))));
    }
    for entry in drift.tested.iter().flatten() {
        out.push_str(&escape_controls(&format!(
            "{DRIFT} | {}\n",
            tested_line(entry)
        )));
    }
}

/// Row 6's summary, after row 5's: its counts, else `drift: <state>:
/// <cause>` per cause.
pub(crate) fn drift_summary(drift: &HealthDrift, out: &mut String) {
    match &drift.counts {
        Some(counts) if drift.state == RowState::Measured => {
            out.push_str(&format!(
                "{DRIFT}: {} fix, {} check, {} cannot, {} ok{}; {} skipped\n",
                counts.fix,
                counts.check,
                counts.cannot,
                counts.ok,
                moved_suffix(drift.moved),
                counts.skipped
            ));
        }
        _ => {
            for cause in &drift.causes {
                out.push_str(&escape_controls(&format!(
                    "{DRIFT}: {}: {}\n",
                    drift.state.as_str(),
                    one_line(cause)
                )));
            }
        }
    }
}
