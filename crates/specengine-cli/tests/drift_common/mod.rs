//! Helpers of the docs/features/drift-queue.md tests ("Acceptance
//! criteria", TE-1..TE-11 and the named RD tests): scratch repositories of
//! real fixtures made at test time through `lock_common` and
//! `verify_common` (every git process sandboxed, fixed dates, a scratch
//! `HOME`):
//!
//! - `fixtures/code-a` with its golden lock
//!   (`fixtures/spec-lock/code-a.lock`) committed on top, then an edit
//!   committed: the drift of `HEAD`;
//! - `fixtures/verify-ecs` (an ECS-style game crate whose every binding is
//!   sound) with its four nodes accepted, then an edit committed.
//!
//! `spec propose drift` runs through the library at an injected clock
//! (`propose_drift`), its text and JSON as `spec` prints them, or as the
//! binary (stdin null) where an exit code or the process is the subject;
//! `spec approve`, `spec reject` and `spec review` through the library with
//! the owner's consent recorded, or the binary on a pseudo-terminal
//! (`lock_common::LockRepo::spec_pty`). The expected texts are built here
//! from the spec's "Data", never from the command's own output.
//!
//! Only the drift test binaries include this module (`mod drift_common;`,
//! after `mod common;`, `mod lock_common;` and `mod verify_common;`).

#![allow(dead_code)]

use std::fs;
use std::path::Path;

use serde_json::Value;
use specengine_cli::{
    ApproveFlags, ApproveRequest, CliError, DriftOutcome, DriftRequest, Exit, Globals, Outcome,
    ProposalOutcome, RejectRequest, ReviewRequest, approve, approve_editorial, propose_drift,
    reject, render_json, render_text, review,
};
use specengine_store::{Proposal, ProposalFilter, ProposalQueue as _, SqliteQueue};

use crate::common::data_dir;
use crate::lock_common::LockRepo;
use crate::verify_common::{ECS, ECS_REFS, golden};

/// The clock of every raise unless a test names another.
pub const RAISED: &str = "2026-10-14T09:00:00Z";

/// The clock of every decision.
pub const DECIDED: &str = "2026-10-14T10:00:00Z";

/// The day [`DECIDED`] falls on: a lock entry's `verified_at`.
pub const DECIDED_DAY: &str = "2026-10-14";

/// code-a's paths.
pub const COUNTER: &str = "crates/tally/src/counter.rs";
pub const BOUNDS: &str = "crates/tally/tests/bounds.rs";
pub const LIMITS: &str = "data/limits.ron";
pub const COUNTER_MD: &str = "docs/spec/counter.md";

/// verify-ecs's path of the systems.
pub const STAMINA: &str = "crates/game/src/stamina.rs";

/// The raising agent.
pub const ROLE: &str = "tester";

/// One library raise.
#[derive(Debug)]
pub struct Raised {
    pub outcome: Result<DriftOutcome, CliError>,
}

impl Raised {
    /// The outcome of a raise that answered (exit 0).
    pub fn ok(&self, context: &str) -> &DriftOutcome {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
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

    /// The message of a raise that cannot run (exit 2).
    pub fn cannot(&self, context: &str) -> String {
        match &self.outcome {
            Ok(outcome) => panic!("{context}: expected exit 2, got {outcome:?}"),
            Err(error) => {
                assert_eq!(error.exit, Exit::CannotRun, "{context}: {error}");
                error.message.clone()
            }
        }
    }

    fn printed(&self) -> Outcome {
        Outcome::Drift(Box::new(self.outcome.as_ref().expect("an answer").clone()))
    }

    /// stdout as `spec propose drift` prints it.
    pub fn text(&self) -> String {
        render_text(&self.printed())
    }

    /// stdout as `spec propose drift --json` prints it.
    pub fn json_text(&self) -> String {
        render_json(&self.printed())
    }

    pub fn json(&self) -> Value {
        serde_json::from_str(&self.json_text()).expect("the drift document")
    }

    /// stderr's lines.
    pub fn stderr(&self) -> Vec<String> {
        self.printed().stderr_lines()
    }

    /// The created items: `(id, node)`, in order.
    pub fn created(&self) -> Vec<(String, String)> {
        let document = self.json();
        document["created"]
            .as_array()
            .expect("created")
            .iter()
            .map(|item| {
                (
                    item["id"].as_str().expect("id").to_owned(),
                    item["node"].as_str().expect("node").to_owned(),
                )
            })
            .collect()
    }

    /// The created nodes, in order.
    pub fn created_nodes(&self) -> Vec<String> {
        self.created().into_iter().map(|(_, node)| node).collect()
    }

    /// The hits: `(id, node, status, same)`, in order.
    pub fn hits(&self) -> Vec<(String, String, String, bool)> {
        let document = self.json();
        document["hits"]
            .as_array()
            .expect("hits")
            .iter()
            .map(|hit| {
                (
                    hit["id"].as_str().expect("id").to_owned(),
                    hit["node"].as_str().expect("node").to_owned(),
                    hit["status"].as_str().expect("status").to_owned(),
                    hit["same"].as_bool().expect("same"),
                )
            })
            .collect()
    }

    /// The document's notes.
    pub fn notes(&self) -> Vec<String> {
        self.json()["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .map(|note| note.as_str().expect("a note").to_owned())
            .collect()
    }
}

/// `spec propose drift REFS` from `cwd` at `now`, an agent's.
pub fn raise_at(repo: &LockRepo, cwd: &Path, references: &[&str], now: &str) -> Raised {
    raise_with_task(repo, cwd, references, now, None)
}

/// [`raise_at`] bound to `--task`.
pub fn raise_with_task(
    repo: &LockRepo,
    cwd: &Path,
    references: &[&str],
    now: &str,
    task: Option<&str>,
) -> Raised {
    let outcome = propose_drift(
        &repo.env(cwd),
        &Globals::default(),
        &DriftRequest {
            references: references.iter().map(|text| (*text).to_owned()).collect(),
            author_role: Some(ROLE.to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: now.to_owned(),
            git: repo.git_env(cwd),
        },
        task,
    );
    Raised { outcome }
}

/// A raise from the root at [`RAISED`].
pub fn raise(repo: &LockRepo, references: &[&str]) -> Raised {
    raise_at(repo, &repo.root, references, RAISED)
}

/// A raise from the root that must answer.
pub fn raised(repo: &LockRepo, references: &[&str]) -> Raised {
    let raised = raise(repo, references);
    raised.ok(&format!("propose drift {references:?}"));
    raised
}

// ------------------------------------------------------------- the queue

/// `repo`'s data directory (the queue and the index).
pub fn data_dir_of(repo: &LockRepo) -> std::path::PathBuf {
    data_dir(&repo.home)
}

/// The project's queue database in `repo`'s data directory.
pub fn db(repo: &LockRepo, slug: &str) -> std::path::PathBuf {
    data_dir(&repo.home).join(format!("{slug}.db"))
}

/// code-a's queue, opened read-write (`None` when never made).
pub fn queue_of(repo: &LockRepo, slug: &str) -> Option<SqliteQueue> {
    let path = db(repo, slug);
    path.exists()
        .then(|| SqliteQueue::open(&path, slug).expect("the queue opens"))
}

/// Every proposal of `slug`, by ID.
pub fn proposals(repo: &LockRepo, slug: &str) -> Vec<Proposal> {
    queue_of(repo, slug)
        .map(|queue| queue.list(&ProposalFilter::default()).expect("list"))
        .unwrap_or_default()
}

/// The proposal `id` of `slug`.
pub fn proposal(repo: &LockRepo, slug: &str, id: &str) -> Proposal {
    queue_of(repo, slug)
        .expect("a queue")
        .get(id)
        .expect("get")
        .unwrap_or_else(|| panic!("no {id}"))
}

/// The queue's `dump()` (`""` when never made): every row and event.
pub fn dump(repo: &LockRepo, slug: &str) -> String {
    queue_of(repo, slug)
        .map(|queue| queue.dump().expect("dump"))
        .unwrap_or_default()
}

/// `sql` on the queue database through the system's `sqlite3`.
pub fn sql(repo: &LockRepo, slug: &str, sql: &str) -> String {
    let output = std::process::Command::new("/usr/bin/sqlite3")
        .env_clear()
        .args(["-cmd", ".timeout 2000"])
        .arg(db(repo, slug))
        .arg(sql)
        .output()
        .expect("run sqlite3");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

// ----------------------------------------------------------- decisions

/// A library approve or reject: its outcome and every question asked.
pub struct Asked {
    pub outcome: Result<ProposalOutcome, CliError>,
    pub questions: Vec<String>,
}

impl Asked {
    /// The outcome of a run that answered (exit 0).
    pub fn done(&self, context: &str) -> &ProposalOutcome {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
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

    /// The messages (notes, warnings) of an answer, as stderr lines.
    pub fn stderr(&self) -> Vec<String> {
        match &self.outcome {
            Ok(outcome) => Outcome::Proposal(Box::new(outcome.clone())).stderr_lines(),
            Err(error) => vec![error.message.clone()],
        }
    }
}

/// `spec approve ID` (`editorial`: `--editorial`) from `cwd` at
/// [`DECIDED`], the owner answering `answer`.
pub fn approve_at(repo: &LockRepo, cwd: &Path, id: &str, editorial: bool, answer: bool) -> Asked {
    approve_with_flags(repo, cwd, id, editorial, &ApproveFlags::default(), answer)
}

/// [`approve_at`] with a decision's flags.
pub fn approve_with_flags(
    repo: &LockRepo,
    cwd: &Path,
    id: &str,
    editorial: bool,
    flags: &ApproveFlags,
    answer: bool,
) -> Asked {
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        answer
    };
    let request = ApproveRequest {
        id: id.to_owned(),
        note: None,
        now: DECIDED.to_owned(),
        git: repo.git_env(cwd),
    };
    let outcome = if editorial {
        approve_editorial(
            &repo.env(cwd),
            &Globals::default(),
            &request,
            flags,
            &mut consent,
        )
    } else if *flags == ApproveFlags::default() {
        approve(&repo.env(cwd), &Globals::default(), &request, &mut consent)
    } else {
        specengine_cli::approve_with(
            &repo.env(cwd),
            &Globals::default(),
            &request,
            flags,
            &mut consent,
        )
    };
    Asked { outcome, questions }
}

/// `spec approve ID` from the root, `y`.
pub fn approve_yes(repo: &LockRepo, id: &str) -> Asked {
    approve_at(repo, &repo.root, id, false, true)
}

/// `spec reject ID --reason R` from `cwd`, the owner answering yes.
pub fn reject_at(repo: &LockRepo, cwd: &Path, id: &str) -> Asked {
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = reject(
        &repo.env(cwd),
        &Globals::default(),
        &RejectRequest {
            id: id.to_owned(),
            reason: Some("Not drift.".to_owned()),
            now: DECIDED.to_owned(),
            git: repo.git_env(cwd),
        },
        &mut consent,
    );
    Asked { outcome, questions }
}

/// `spec review ID` from the root: its text, its JSON as printed, and
/// the JSON read.
pub fn review_of(repo: &LockRepo, id: &str) -> (String, String, Value) {
    let outcome = review(
        &repo.env(&repo.root),
        &Globals::default(),
        &ReviewRequest {
            id: id.to_owned(),
            git: repo.git_env(&repo.root),
        },
    )
    .unwrap_or_else(|error| panic!("review {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "review {id}: {outcome:?}");
    let printed = Outcome::Proposal(Box::new(outcome));
    let json = render_json(&printed);
    let value = serde_json::from_str(&json).expect("the review document");
    (render_text(&printed), json, value)
}

// ------------------------------------------------------------ repos

/// code-a with its golden lock committed, `edit` applied and committed as
/// `the edit` (no hook runs).
pub fn drifted(label: &str, edit: impl FnOnce(&LockRepo)) -> LockRepo {
    let repo = golden(label);
    edit(&repo);
    repo.commit_all("the edit");
    repo
}

/// verify-ecs with its four nodes accepted, `edit` applied and committed.
pub fn ecs_drifted(label: &str, edit: impl FnOnce(&LockRepo)) -> LockRepo {
    let repo = LockRepo::of(label, ECS, "repo", &[]);
    for reference in ECS_REFS {
        repo.accept_ok(reference);
    }
    edit(&repo);
    repo.commit_all("the edit");
    repo
}

/// `CAP` changed in code-a: `LAW-BOUND`'s `code_ahead code_changed`.
pub fn cap_changed(repo: &LockRepo) {
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
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

/// A file's bytes under `root`, `None` when absent.
pub fn bytes(root: &Path, relative: &str) -> Option<Vec<u8>> {
    fs::read(root.join(relative)).ok()
}
