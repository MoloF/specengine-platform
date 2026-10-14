//! Helpers of the docs/features/rev-discipline.md tests ("Acceptance
//! criteria", TE-1..TE-3 and the named RD tests): the queue's commands
//! through the library over scratch repositories of real fixtures
//! (`common::proposal::Pair` over `fixtures/spec-a`, `lock_common::LockRepo`
//! over `fixtures/code-a` with its golden lock or `fixtures/verify-ecs`
//! with its four nodes accepted), the owner's consent recorded, never a
//! mock of the system.
//!
//! The expected texts are built here from the spec's "Data" ("Raising at
//! apply", "Editorial refresh", "Review document, stage, queue"), never
//! from the command's own output.
//!
//! Only the rev test binaries include this module (`mod rev_common;`,
//! after `mod common;` and, where used, `mod lock_common;`).

#![allow(dead_code)]

use std::path::Path;

use serde_json::Value;
use specengine_cli::{
    ApproveFlags, ApproveRequest, CliError, CreateRequest, Env, Exit, Globals, Outcome,
    ProposalOutcome, ProposeRequest, ProposedText, RejectRequest, ReviewRequest, approve,
    approve_editorial, propose, propose_create, reject, render_json, render_text, review,
};
use specengine_store::GitEnv;

/// The keys of one `revisions` entry, in order ("Review document, stage,
/// queue": "5 keys", `refused` last, Q15). The one place a test pins them:
/// the entry's shape grows here.
pub const REVISION_ENTRY_KEYS: [&str; 5] = ["id", "rev", "to", "lock_entries", "refused"];

/// The clock of every proposal these tests raise.
pub const NOW: &str = "2026-10-05T21:14:03Z";

/// The clock of every decision.
pub const LATER: &str = "2026-10-06T08:00:00Z";

/// Where a library call runs: the caller's directory and git environment.
pub struct At<'a> {
    pub env: Env,
    pub git: GitEnv,
    pub cwd: &'a Path,
}

impl<'a> At<'a> {
    pub fn new(cwd: &'a Path, home: &Path, git: GitEnv) -> Self {
        Self {
            env: Env {
                cwd: cwd.to_path_buf(),
                home: Some(home.as_os_str().to_owned()),
                xdg_data_home: None,
            },
            git,
            cwd,
        }
    }
}

/// A library approve: its outcome and every question asked.
pub struct Asked {
    pub outcome: Result<ProposalOutcome, CliError>,
    pub questions: Vec<String>,
}

impl Asked {
    /// The outcome of an apply that must have answered.
    pub fn applied(&self, context: &str) -> &ProposalOutcome {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{context}: {outcome:?}");
        outcome
    }

    /// The reason of a refusal (exit 1).
    pub fn refused(&self, context: &str) -> String {
        match &self.outcome {
            Ok(outcome) => {
                assert_eq!(outcome.exit(), Exit::NotFound, "{context}: {outcome:?}");
                outcome.refusal.clone().expect("a refusal")
            }
            Err(error) => panic!("{context}: exit {:?}: {error}", error.exit),
        }
    }

    /// The message of a run that cannot run (exit 2).
    pub fn cannot(&self, context: &str) -> String {
        match &self.outcome {
            Ok(outcome) => panic!("{context}: expected exit 2, got {outcome:?}"),
            Err(error) => {
                assert_eq!(error.exit, Exit::CannotRun, "{context}: {error}");
                error.message.clone()
            }
        }
    }

    /// The one question asked.
    pub fn question(&self) -> &str {
        assert_eq!(self.questions.len(), 1, "{:?}", self.questions);
        &self.questions[0]
    }
}

/// `spec propose update` of `text` against `base` on `target`, an agent's.
pub fn propose_text(
    at: &At<'_>,
    target: &str,
    base: &str,
    text: &str,
) -> Result<ProposalOutcome, CliError> {
    propose(
        &at.env,
        &Globals::default(),
        &ProposeRequest {
            target: target.to_owned(),
            base: base.to_owned(),
            text: ProposedText::Given(text.as_bytes().to_vec()),
            rationale: format!("Update {target}."),
            author_role: Some("spec-writer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: NOW.to_owned(),
            git: at.git.clone(),
        },
    )
}

/// [`propose_text`] that must be stored: its ID.
pub fn proposed(at: &At<'_>, target: &str, base: &str, text: &str) -> String {
    let outcome = propose_text(at, target, base, text)
        .unwrap_or_else(|error| panic!("propose {target}: {error}"));
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "propose {target}: {outcome:?}"
    );
    outcome.document.id.clone().expect("an ID")
}

/// `spec propose create TARGET` of `text` (`base` for the section form).
pub fn propose_create_text(
    at: &At<'_>,
    target: &str,
    base: Option<&str>,
    text: &str,
) -> Result<ProposalOutcome, CliError> {
    propose_create(
        &at.env,
        &Globals::default(),
        &CreateRequest {
            target: target.to_owned(),
            base: base.map(str::to_owned),
            text: ProposedText::Given(text.as_bytes().to_vec()),
            rationale: format!("Create in {target}."),
            author_role: Some("spec-writer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: NOW.to_owned(),
            git: at.git.clone(),
        },
    )
}

/// `spec approve PR` (`editorial`: `spec approve PR --editorial`), the
/// owner answering `answer`.
pub fn approve_at(at: &At<'_>, id: &str, editorial: bool, answer: bool) -> Asked {
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        answer
    };
    let request = ApproveRequest {
        id: id.to_owned(),
        note: None,
        now: LATER.to_owned(),
        git: at.git.clone(),
    };
    let outcome = if editorial {
        approve_editorial(
            &at.env,
            &Globals::default(),
            &request,
            &ApproveFlags::default(),
            &mut consent,
        )
    } else {
        approve(&at.env, &Globals::default(), &request, &mut consent)
    };
    Asked { outcome, questions }
}

/// `spec reject PR --reason R`, the owner answering yes.
pub fn reject_at(at: &At<'_>, id: &str) -> Asked {
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = reject(
        &at.env,
        &Globals::default(),
        &RejectRequest {
            id: id.to_owned(),
            reason: Some("Not this one.".to_owned()),
            now: LATER.to_owned(),
            git: at.git.clone(),
        },
        &mut consent,
    );
    Asked { outcome, questions }
}

/// `spec review PR`: text and JSON.
pub fn review_at(at: &At<'_>, id: &str) -> (String, Value) {
    let outcome = review(
        &at.env,
        &Globals::default(),
        &ReviewRequest {
            id: id.to_owned(),
            git: at.git.clone(),
        },
    )
    .unwrap_or_else(|error| panic!("review {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "review {id}: {outcome:?}");
    let (text, json) = printed(&outcome);
    (
        text,
        serde_json::from_str(&json).unwrap_or_else(|error| panic!("{error}: {json}")),
    )
}

/// The outcome's stdout text and JSON, as `spec` prints them.
pub fn printed(outcome: &ProposalOutcome) -> (String, String) {
    let outcome = Outcome::Proposal(Box::new(outcome.clone()));
    (render_text(&outcome), render_json(&outcome))
}

/// The trailer block of a commit message: its last paragraph's lines.
pub fn trailers(message: &str) -> Vec<String> {
    message
        .trim_end()
        .rsplit("\n\n")
        .next()
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The trailers after `Base-commit:` (the ones this slice adds, and an
/// interpretation's `Interprets:`).
pub fn after_base(message: &str) -> Vec<String> {
    let all = trailers(message);
    let at = all
        .iter()
        .position(|line| line.starts_with("Base-commit: "))
        .unwrap_or_else(|| panic!("no Base-commit: in {message:?}"));
    all[at + 1..].to_vec()
}

/// One `revisions` entry as the spec writes it, for a node a meaning
/// approve raises to `to` (`refused` null).
pub fn revision_entry(id: &str, rev: u32, to: u32, lock_entries: Option<usize>) -> Value {
    entry_of(id, rev, Some(to), lock_entries, None)
}

/// One `revisions` entry whose raise step 6 refuses (Q15): `to` null,
/// `refused` the step-6 reason as a refusal prints it after `(step 6): `.
pub fn refused_entry(id: &str, rev: u32, lock_entries: Option<usize>, refused: &str) -> Value {
    entry_of(id, rev, None, lock_entries, Some(refused))
}

fn entry_of(
    id: &str,
    rev: u32,
    to: Option<u32>,
    lock_entries: Option<usize>,
    refused: Option<&str>,
) -> Value {
    let mut entry = serde_json::Map::new();
    let values = [
        Value::from(id),
        Value::from(rev),
        to.map_or(Value::Null, Value::from),
        lock_entries.map_or(Value::Null, Value::from),
        refused.map_or(Value::Null, Value::from),
    ];
    for (key, value) in REVISION_ENTRY_KEYS.iter().zip(values) {
        entry.insert((*key).to_owned(), value);
    }
    Value::Object(entry)
}

/// `spec review PR`: text, the JSON as printed (its key order kept) and
/// the JSON read.
pub fn review_printed(at: &At<'_>, id: &str) -> (String, String, Value) {
    let outcome = review(
        &at.env,
        &Globals::default(),
        &ReviewRequest {
            id: id.to_owned(),
            git: at.git.clone(),
        },
    )
    .unwrap_or_else(|error| panic!("review {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "review {id}: {outcome:?}");
    let (text, json) = printed(&outcome);
    let value = serde_json::from_str(&json).unwrap_or_else(|error| panic!("{error}: {json}"));
    (text, json, value)
}

/// The notes of a review document that start with `prefix`, in order.
pub fn notes_starting(document: &Value, prefix: &str) -> Vec<String> {
    document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(Value::as_str)
        .filter(|note| note.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

/// The reason of a step-6 refusal as printed after `(step 6): `.
pub fn after_step_6(reason: &str) -> &str {
    reason
        .split_once("(step 6): ")
        .unwrap_or_else(|| panic!("no `(step 6): ` in {reason:?}"))
        .1
}

/// Each `revisions` entry of a review document carries exactly
/// [`REVISION_ENTRY_KEYS`], in order (the rendered JSON's order).
pub fn assert_entry_keys(json_text: &str, document: &Value) {
    let Some(entries) = document["revisions"].as_array() else {
        return;
    };
    for entry in entries {
        let keys: Vec<&str> = entry
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        let mut want = REVISION_ENTRY_KEYS.to_vec();
        want.sort_unstable();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, want, "{entry}");
    }
    let at = json_text.find("\"revisions\":[{").map(|at| at + 14);
    if let Some(at) = at {
        let first = &json_text[at..];
        let positions: Vec<usize> = REVISION_ENTRY_KEYS
            .iter()
            .map(|key| first.find(&format!("\"{key}\":")).expect("the key"))
            .collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "entry keys in order: {first}"
        );
    }
}
