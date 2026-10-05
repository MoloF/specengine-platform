//! Helpers of the docs/features/proposal-apply.md tests ("Roles",
//! `test-engineer`): a scratch git repository holding a copy of
//! `fixtures/spec-a` or `fixtures/spec-b` (its main worktree), one commit,
//! and a linked worktree on branch `t1`; every git process isolated by
//! [`super::git::Sandbox`] (no `GIT_*` of the test process, no global or
//! system config, a scratch `HOME`, a fixed identity); a scratch `HOME` for
//! the data directory. The queue's commands run through the library
//! (`specengine_cli::{propose, inbox, review, approve, reject}`) with the
//! sandbox's variables as the caller's git environment, an injected clock
//! and, for `approve` and `reject`, a consent callback that records the
//! question; `spec` itself runs only where the binary is the subject (the
//! terminal check, `show --json`).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use specengine_cli::{
    ApproveRequest, CliError, Env, Exit, Globals, InboxOutcome, InboxRequest, Outcome,
    ProposalOutcome, ProposeRequest, ProposedText, RejectRequest, ReviewRequest, ShowRequest,
    ShownNode, approve, inbox, propose, reject, render_json, render_text, review, show,
};
use specengine_store::{Event, GitEnv, Proposal, ProposalFilter, ProposalQueue as _, SqliteQueue};

use super::git::Sandbox;
use super::{RUN_TIMEOUT, Run, SPEC, Scratch, data_dir, snapshot};

/// The injected clock of every queue call (the validation's today is its
/// date).
pub const NOW: &str = "2026-10-05T21:14:03Z";
/// A later clock, for the decisions.
pub const LATER: &str = "2026-10-06T08:00:00Z";

/// The sandbox's committer, as `Decided-by:` and `decided_by` carry it.
pub const DECIDER: &str = "Scratch Committer <committer@example.invalid>";

/// The fixtures with their slugs and a section each test edits: one that
/// holds three lines of prose with ASCII anchors.
pub struct FixtureCase {
    pub fixture: &'static str,
    pub slug: &'static str,
    /// The section edited.
    pub target: &'static str,
    /// Its file, root-relative.
    pub path: &'static str,
    /// Two edits on different lines of its span, and one overlapping the
    /// first: `(from, to)`, each `from` once in the span.
    pub first: (&'static str, &'static str),
    pub second: (&'static str, &'static str),
    pub overlapping: (&'static str, &'static str),
}

/// spec-a (a game) and spec-b (a command-line tool, Russian prose).
pub const CASES: [FixtureCase; 2] = [
    FixtureCase {
        fixture: "spec-a",
        slug: "lantern-keep",
        target: "EDGE-SPRINT-EMPTY",
        path: "docs/spec/movement/sprint.md",
        first: ("the sprint ends;", "the sprint ends at once;"),
        second: ("is not a reference", "is never a reference"),
        overlapping: ("the sprint ends;", "the sprint stops;"),
    },
    FixtureCase {
        fixture: "spec-b",
        slug: "zerkalo",
        target: "CMD-SYNC",
        path: "docs/spec/cli.md",
        first: ("`sync`", "`sync --all`"),
        second: ("dry-run/CRIT-01", "dry-run/CRIT-01 (dry run)"),
        overlapping: ("`sync`", "`sync --each`"),
    },
];

/// `text` with the one occurrence of `from` replaced by `to`.
pub fn edit(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "{from:?} must occur once in {text:?}"
    );
    text.replacen(from, to, 1)
}

/// The repository of one test: its main worktree (a fixture copy, one
/// commit on `main`) and a linked worktree on `t1`.
pub struct Pair {
    pub scratch: Scratch,
    pub git: Sandbox,
    /// The main worktree's top, canonical.
    pub main: PathBuf,
    /// The linked worktree on `t1`, canonical.
    pub linked: PathBuf,
    /// The data directory's `HOME`.
    pub home: PathBuf,
    pub slug: String,
}

impl Pair {
    pub fn new(label: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let main = scratch.copy(fixture, "main");
        git.init(&main);
        git.add_all(&main);
        git.commit(&main, "the fixture");
        let linked = scratch.join("t1");
        git.git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "t1",
                linked.to_str().expect("a UTF-8 scratch path"),
            ],
        );
        let linked = fs::canonicalize(linked).expect("the linked worktree");
        let home = scratch.home("h");
        let slug = super::FIXTURES
            .iter()
            .find(|(name, _)| *name == fixture)
            .map_or_else(
                || panic!("no slug for {fixture}"),
                |(_, slug)| (*slug).to_owned(),
            );
        Self {
            scratch,
            git,
            main,
            linked,
            home,
            slug,
        }
    }

    /// The case of `fixture` in [`CASES`] with its pair.
    pub fn of(label: &str, case: &FixtureCase) -> Self {
        Self::new(label, case.fixture)
    }

    pub fn env(&self, cwd: &Path) -> Env {
        Env {
            cwd: cwd.to_path_buf(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    /// The caller's git environment: the sandbox's variables.
    pub fn git_env(&self, cwd: &Path) -> GitEnv {
        GitEnv::new(cwd, self.git.vars())
    }

    /// The project's database.
    pub fn db(&self) -> PathBuf {
        data_dir(&self.home).join(format!("{}.db", self.slug))
    }

    /// `spec show ID` through the library in `cwd`: its one node.
    pub fn node(&self, cwd: &Path, reference: &str) -> ShownNode {
        let outcome = show(
            &self.env(cwd),
            &Globals::default(),
            &ShowRequest {
                reference: reference.to_owned(),
                links: false,
                archive: false,
            },
        )
        .unwrap_or_else(|error| panic!("show {reference}: {error}"));
        assert_eq!(outcome.nodes.len(), 1, "show {reference}: {outcome:?}");
        outcome.nodes.into_iter().next().expect("one node")
    }

    /// `(span_hash, text)` of `reference` as `spec show` gives them in `cwd`.
    pub fn span(&self, cwd: &Path, reference: &str) -> (String, String) {
        let node = self.node(cwd, reference);
        (node.span_hash, node.text)
    }

    pub fn propose_with(
        &self,
        cwd: &Path,
        request: ProposeRequest,
    ) -> Result<ProposalOutcome, CliError> {
        propose(&self.env(cwd), &Globals::default(), &request)
    }

    /// A `ProposeRequest` of `text` against `base`, an agent's.
    pub fn request(&self, cwd: &Path, target: &str, base: &str, text: &str) -> ProposeRequest {
        ProposeRequest {
            target: target.to_owned(),
            base: base.to_owned(),
            text: ProposedText::Given(text.as_bytes().to_vec()),
            rationale: format!("Update {target}."),
            author_role: Some("spec-writer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: NOW.to_owned(),
            git: self.git_env(cwd),
        }
    }

    pub fn propose(
        &self,
        cwd: &Path,
        target: &str,
        base: &str,
        text: &str,
    ) -> Result<ProposalOutcome, CliError> {
        self.propose_with(cwd, self.request(cwd, target, base, text))
    }

    /// Proposes the current span of `target` (read in `cwd`) with `from`
    /// replaced by `to`; it must be stored: its ID.
    pub fn propose_edit(&self, cwd: &Path, target: &str, from: &str, to: &str) -> String {
        let (hash, text) = self.span(cwd, target);
        let outcome = self
            .propose(cwd, target, &hash, &edit(&text, from, to))
            .unwrap_or_else(|error| panic!("propose {target}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "propose {target}: {outcome:?}"
        );
        outcome.document.id.clone().expect("an ID")
    }

    /// Library `approve` in `cwd`, consent `answer`: the outcome and the
    /// questions asked.
    pub fn approve_answer(
        &self,
        cwd: &Path,
        id: &str,
        answer: bool,
        git: GitEnv,
    ) -> (Result<ProposalOutcome, CliError>, Vec<String>) {
        let mut questions = Vec::new();
        let mut consent = |question: &str| {
            questions.push(question.to_owned());
            answer
        };
        let outcome = approve(
            &self.env(cwd),
            &Globals::default(),
            &ApproveRequest {
                id: id.to_owned(),
                note: None,
                now: LATER.to_owned(),
                git,
            },
            &mut consent,
        );
        (outcome, questions)
    }

    /// Library approve: consent yes.
    pub fn approve(&self, cwd: &Path, id: &str) -> Result<ProposalOutcome, CliError> {
        self.approve_answer(cwd, id, true, self.git_env(cwd)).0
    }

    /// Library approve that must apply: the outcome.
    pub fn approve_ok(&self, cwd: &Path, id: &str) -> ProposalOutcome {
        let outcome = self
            .approve(cwd, id)
            .unwrap_or_else(|error| panic!("approve {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
        outcome
    }

    pub fn reject_answer(
        &self,
        cwd: &Path,
        id: &str,
        reason: &str,
        answer: bool,
    ) -> (Result<ProposalOutcome, CliError>, Vec<String>) {
        let mut questions = Vec::new();
        let mut consent = |question: &str| {
            questions.push(question.to_owned());
            answer
        };
        let outcome = reject(
            &self.env(cwd),
            &Globals::default(),
            &RejectRequest {
                id: id.to_owned(),
                reason: reason.to_owned(),
                now: LATER.to_owned(),
                git: self.git_env(cwd),
            },
            &mut consent,
        );
        (outcome, questions)
    }

    pub fn review(&self, cwd: &Path, id: &str) -> Result<ProposalOutcome, CliError> {
        review(
            &self.env(cwd),
            &Globals::default(),
            &ReviewRequest {
                id: id.to_owned(),
                git: self.git_env(cwd),
            },
        )
    }

    pub fn review_ok(&self, cwd: &Path, id: &str) -> ProposalOutcome {
        let outcome = self
            .review(cwd, id)
            .unwrap_or_else(|error| panic!("review {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "review {id}: {outcome:?}");
        outcome
    }

    pub fn inbox(&self, cwd: &Path, all: bool) -> Result<InboxOutcome, CliError> {
        inbox(
            &self.env(cwd),
            &Globals::default(),
            &InboxRequest {
                all,
                git: self.git_env(cwd),
            },
        )
    }

    /// The queue, opened directly.
    pub fn queue(&self) -> SqliteQueue {
        SqliteQueue::open(self.db(), &self.slug).expect("the queue opens")
    }

    /// Every stored proposal, by ID.
    pub fn proposals(&self) -> Vec<Proposal> {
        if !self.db().exists() {
            return Vec::new();
        }
        self.queue().list(&ProposalFilter::default()).expect("list")
    }

    pub fn proposal(&self, id: &str) -> Proposal {
        self.queue()
            .get(id)
            .expect("get")
            .unwrap_or_else(|| panic!("{id} is stored"))
    }

    pub fn events(&self) -> Vec<Event> {
        if !self.db().exists() {
            return Vec::new();
        }
        self.queue().events().expect("events")
    }

    /// `(type, step)` of every event of `id`, in `seq` order.
    pub fn events_of(&self, id: &str) -> Vec<(String, Option<u64>)> {
        self.events()
            .iter()
            .filter(|event| event.payload["id"] == id)
            .map(|event| (event.event_type.clone(), event.payload["step"].as_u64()))
            .collect()
    }

    pub fn git_text(&self, dir: &Path, args: &[&str]) -> String {
        self.git.git_text(dir, args)
    }

    /// `git rev-parse <rev>` in `dir`.
    pub fn rev(&self, dir: &Path, rev: &str) -> String {
        self.git_text(dir, &["rev-parse", "--verify", "-q", rev])
    }

    /// Commits `message` in `dir` after `git add -A` (no hooks).
    pub fn commit_all(&self, dir: &Path, message: &str) {
        self.git.add_all(dir);
        self.git.commit(dir, message);
    }

    /// Everything a refusal must leave as it was: both worktrees' files
    /// (`.git` aside), every ref, both `git status` outputs and staged
    /// entries, the proposals as stored.
    pub fn state(&self) -> State {
        let files = |dir: &Path| {
            snapshot(dir)
                .into_iter()
                .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
                .collect::<BTreeMap<_, _>>()
        };
        let status = |dir: &Path| {
            String::from_utf8_lossy(&self.git.git(
                dir,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            ))
            .into_owned()
        };
        let staged = |dir: &Path| self.git_text(dir, &["ls-files", "-s"]);
        State {
            main_files: files(&self.main),
            linked_files: if self.linked.exists() {
                files(&self.linked)
            } else {
                BTreeMap::new()
            },
            refs: self.git_text(
                &self.main,
                &["for-each-ref", "--format=%(refname) %(objectname)"],
            ),
            main_head: self.rev(&self.main, "HEAD"),
            main_status: status(&self.main),
            linked_status: if self.linked.exists() {
                status(&self.linked)
            } else {
                String::new()
            },
            main_staged: staged(&self.main),
            linked_staged: if self.linked.exists() {
                staged(&self.linked)
            } else {
                String::new()
            },
            proposals: self
                .proposals()
                .iter()
                .map(|proposal| {
                    (
                        proposal.id.clone(),
                        proposal.status.as_str().to_owned(),
                        proposal.decided_by.clone(),
                        proposal.applied_commit.clone(),
                    )
                })
                .collect(),
        }
    }

    /// `spec args` in `cwd` with the sandbox's variables and this `HOME`,
    /// `input` on a piped stdin (never a terminal), with a watchdog.
    pub fn spec_piped(&self, cwd: &Path, args: &[&str], input: &[u8]) -> Run {
        let mut command = Command::new(SPEC);
        command
            .env_clear()
            .envs(self.git.vars())
            .env("HOME", &self.home)
            .current_dir(cwd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn spec");
        {
            let mut stdin = child.stdin.take().expect("stdin");
            // The child may exit before reading: a broken pipe is fine.
            let _ = stdin.write_all(input);
        }
        let mut stdout = child.stdout.take().expect("stdout");
        let mut stderr = child.stderr.take().expect("stderr");
        let out = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            bytes
        });
        let err = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes);
            bytes
        });
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().expect("wait on spec") {
                break status;
            }
            if started.elapsed() > RUN_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                panic!("spec {args:?} ran longer than {RUN_TIMEOUT:?}; killed");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        Run {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            code: status.code().expect("spec exited, not killed by a signal"),
            stdout: String::from_utf8(out.join().expect("stdout")).expect("UTF-8 stdout"),
            stderr: String::from_utf8(err.join().expect("stderr")).expect("UTF-8 stderr"),
        }
    }

    /// `sqlite3 <db> <sql>` (the system's client: the CLI crate has no SQL
    /// dependency of its own); it must succeed.
    pub fn sql(&self, sql: &str) -> String {
        let output = Command::new(sqlite3())
            .arg(self.db())
            .arg(sql)
            .stdin(Stdio::null())
            .output()
            .expect("sqlite3 runs");
        assert!(
            output.status.success(),
            "sqlite3 {sql}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

/// The system's `sqlite3`.
pub fn sqlite3() -> &'static OsStr {
    for candidate in ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"] {
        if Path::new(candidate).exists() {
            return OsStr::new(candidate);
        }
    }
    panic!("no sqlite3 client: AC-16 needs one to set a proposal back to approved")
}

/// See [`Pair::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub main_files: BTreeMap<String, Option<Vec<u8>>>,
    pub linked_files: BTreeMap<String, Option<Vec<u8>>>,
    pub refs: String,
    pub main_head: String,
    pub main_status: String,
    pub linked_status: String,
    pub main_staged: String,
    pub linked_staged: String,
    pub proposals: Vec<(String, String, Option<String>, Option<String>)>,
}

/// The outcome was refused (exit 1) by the proposal or its target: its
/// reason.
pub fn refused(outcome: &Result<ProposalOutcome, CliError>, context: &str) -> String {
    match outcome {
        Ok(outcome) => {
            assert_eq!(outcome.exit(), Exit::NotFound, "{context}: {outcome:?}");
            outcome.refusal.clone().expect("a refusal")
        }
        Err(error) => panic!("{context}: exit {:?}: {error}", error.exit),
    }
}

/// The command could not run (exit 2): its message.
pub fn cannot(outcome: &Result<impl std::fmt::Debug, CliError>, context: &str) -> String {
    match outcome {
        Ok(outcome) => panic!("{context}: expected exit 2, got {outcome:?}"),
        Err(error) => {
            assert_eq!(error.exit, Exit::CannotRun, "{context}: {error}");
            assert_eq!(error.exit.code(), 2, "{context}");
            error.message.clone()
        }
    }
}

/// The outcome's stdout text and JSON, as `spec` prints them.
pub fn printed(outcome: &ProposalOutcome) -> (String, String) {
    let outcome = Outcome::Proposal(Box::new(outcome.clone()));
    (render_text(&outcome), render_json(&outcome))
}

/// The inbox's stdout text and JSON.
pub fn printed_inbox(outcome: &InboxOutcome) -> (String, String) {
    let outcome = Outcome::Inbox(outcome.clone());
    (render_text(&outcome), render_json(&outcome))
}

/// The JSON document of a printed outcome.
pub fn json_of(text: &str) -> Value {
    super::one_json_document(text).unwrap_or_else(|problem| panic!("{problem}: {text}"))
}

/// The variables of a process: the sandbox's plus `extra`.
pub fn with_vars(git: &GitEnv, extra: &[(&str, &OsStr)]) -> GitEnv {
    let mut env = git.clone();
    for (name, value) in extra {
        env = env.with_var(OsString::from(name), value.to_os_string());
    }
    env
}
