//! Helpers of the docs/features/spec-lock.md tests ("Acceptance criteria",
//! TE-1..TE-10 and the named RD tests): a scratch git repository made at
//! test time from `fixtures/code-a` (its `expected.json` left out; the
//! fixture is only read), every git process in the sandbox of
//! `common::git` (fixed author, committer and dates, no global config, git
//! auto-maintenance off), a scratch `HOME` for the data directory.
//!
//! `spec lock accept` and `spec lock regen` run two ways: through the
//! library (`lock_accept`, `lock_regen`) at an injected clock with a
//! consent closure that records what the owner was shown, so the lock's
//! bytes are reproducible; and the `spec` binary on a pseudo-terminal
//! (`script`, stdin and the controlling terminal a pty, stdout and stderr
//! into files of their own) where the terminal, the prompt or the printed
//! text is the subject, or with stdin null where the refusal of a
//! non-terminal is.
//!
//! Only the lock test binaries include this module (`mod lock_common;`).
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use specengine_cli::{
    CliError, Env, Exit, Globals, LockAcceptRequest, LockOutcome, LockPrompt, LockRegenRequest,
    RegenOutcome, lock_accept, lock_regen,
};
use specengine_code::{
    LevelState, RonValues, RustParser, analyze_file, analyze_tree, item_levels, module_levels,
    value_hashes,
};
use specengine_store::GitEnv;

use crate::common::git::Sandbox;
use crate::common::{NORM_SUFFIX, RUN_TIMEOUT, Run, SPEC, Scratch, snapshot, spec_with};

/// The clock of every library accept unless a test names another: its
/// date is not the day the suite runs on, so a lock written with the
/// process clock instead shows.
pub const CLOCK: &str = "2026-10-08T09:30:00Z";

/// [`CLOCK`]'s UTC date, `verified_at`.
pub const DAY: &str = "2026-10-08";

/// A later clock (regen's "the later `verified_at`").
pub const LATER: &str = "2026-10-09T23:59:59Z";

/// The lock's first line ("The file").
pub const HEADER: &str = "# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit";

/// This build's `norm_recipe`.
pub const NORM_RECIPE: &str = "specengine-norm/v1";

/// This build's `code_recipe` (docs/features/lock-levels.md "Format 2":
/// `<LEVEL_RECIPE> tree-sitter-rust/<version> abi/<n>`).
pub const CODE_RECIPE: &str = "specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15";

/// The `code_recipe` every format-1 lock of this grammar holds (v2's
/// `RECIPE`).
pub const CODE_RECIPE_FORMAT_1: &str = "specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15";

/// The lines after the header (lock-levels.md "Format 2").
pub const PREAMBLE: &str = "format = 2\nnorm_recipe = \"specengine-norm/v1\"\ncode_recipe = \"specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15\"\n";

/// A format-1 lock's lines after the header (`docs/canon/spec-lock.md`
/// "The file" before 5b-1a): the evidence of the readers that still read
/// it (Q10).
pub const PREAMBLE_FORMAT_1: &str = "format = 1\nnorm_recipe = \"specengine-norm/v1\"\ncode_recipe = \"specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15\"\n";

/// The format-1 golden (`fixtures/spec-lock/code-a.format-1.lock`): the
/// bytes of `code-a.lock` before 5b-1a, verbatim (lock-levels.md
/// "Fixtures and the golden").
pub const GOLDEN_FORMAT_1: &str = "code-a.format-1.lock";

/// The text every writer and code judge refuses a format-1 lock with
/// (lock-levels.md "Readers", Q27 (3)), after `spec.lock:<line>: `.
pub fn format_1_text(its_code_recipe: &str) -> String {
    format!(
        "format 1 (\"{its_code_recipe}\"), this build's is 2 (\"{CODE_RECIPE}\"): run spec lock rebase on a terminal"
    )
}

/// Q27 (4): a format-2 lock of another `code_recipe`.
pub fn code_recipe_text(its: &str) -> String {
    format!(
        "`code_recipe` is \"{its}\", this build's is \"{CODE_RECIPE}\": run spec lock rebase on a terminal"
    )
}

/// Q27 (2): another `norm_recipe`, today's text, no step.
pub fn norm_recipe_text(its: &str) -> String {
    format!(
        "`norm_recipe` is \"{its}\", this build's is \"{NORM_RECIPE}\": the lock was written by another build (a grammar or recipe change re-hashes every entry)"
    )
}

/// Q27 (1): a `format` above 2.
pub fn newer_format_text(format: i64) -> String {
    format!("format {format} is newer than this build's 2")
}

/// Q27 (1): a `format` below 1.
pub fn older_format_text(format: i64) -> String {
    format!("format {format} is not this build's: it reads formats 1 and 2")
}

/// Regen's one text (Q28) for a format-1 stage and for stages of two
/// `code_recipe`s.
pub const REGEN_TEXT: &str = "spec.lock: a stage is format 1, or the stages differ in `code_recipe`: run git merge --abort, spec lock rebase on the lagging branch, and merge again";

/// A lock text's four refusal variants of TE-1, each with the message
/// `parse_lock(text, Some(build))` must give: Q27's first failing check
/// as `LockError` renders it (`spec.lock:<line>: …`, the canonical
/// layout: `format` on line 2, `norm_recipe` 3, `code_recipe` 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// The format-1 golden's bytes.
    Format1,
    /// A format-2 lock, its `code_recipe` another grammar's.
    OtherCode,
    /// A format-2 lock, its `norm_recipe` another.
    OtherNorm,
    /// A format-2 lock claiming `format = 3`.
    Format3,
}

impl Variant {
    pub const ALL: [Self; 4] = [
        Self::Format1,
        Self::OtherCode,
        Self::OtherNorm,
        Self::Format3,
    ];

    /// Another grammar's `code_recipe` (OtherCode).
    pub const OTHER_CODE: &'static str = "specengine-hash/v3 tree-sitter-rust/0.24.1 abi/15";

    /// Another `norm_recipe` (OtherNorm).
    pub const OTHER_NORM: &'static str = "specengine-norm/v0";

    pub fn label(self) -> &'static str {
        match self {
            Self::Format1 => "format 1",
            Self::OtherCode => "another code_recipe",
            Self::OtherNorm => "another norm_recipe",
            Self::Format3 => "format 3",
        }
    }

    /// The variant's bytes made from `format_2` (a format-2 lock in the
    /// canonical layout) or, for [`Variant::Format1`], the format-1 golden.
    pub fn text(self, format_2: &str) -> String {
        assert!(
            format_2.starts_with(&format!("{HEADER}\n{PREAMBLE}")),
            "a format-2 lock of this build:\n{format_2}"
        );
        match self {
            Self::Format1 => golden_format_1_text(),
            Self::OtherCode => format_2.replacen(CODE_RECIPE, Self::OTHER_CODE, 1),
            Self::OtherNorm => format_2.replacen(NORM_RECIPE, Self::OTHER_NORM, 1),
            Self::Format3 => format_2.replacen("\nformat = 2\n", "\nformat = 3\n", 1),
        }
    }

    /// The whole message: `spec.lock:<line>: <text>`.
    pub fn message(self) -> String {
        match self {
            Self::Format1 => format!("spec.lock:2: {}", format_1_text(CODE_RECIPE_FORMAT_1)),
            Self::OtherCode => format!("spec.lock:4: {}", code_recipe_text(Self::OTHER_CODE)),
            Self::OtherNorm => format!("spec.lock:3: {}", norm_recipe_text(Self::OTHER_NORM)),
            Self::Format3 => format!("spec.lock:2: {}", newer_format_text(3)),
        }
    }

    /// The message's text after `spec.lock:<line>: `.
    pub fn body(self) -> String {
        let message = self.message();
        let (_, body) = message
            .strip_prefix("spec.lock:")
            .and_then(|rest| rest.split_once(": "))
            .expect("spec.lock:<line>: ");
        body.to_owned()
    }
}

/// The format-1 golden's text.
pub fn golden_format_1_text() -> String {
    fs::read_to_string(crate::common::fixture("spec-lock").join(GOLDEN_FORMAT_1))
        .expect("fixtures/spec-lock/code-a.format-1.lock")
}

/// The committer of every scratch commit, as `Decided-by:` names it.
pub const DECIDER: &str = "Scratch Committer <committer@example.invalid>";

/// The fixture every scenario starts from.
pub const FIXTURE: &str = "code-a";

/// The keys of a format-2 `[[binding]]` block, in order (lock-levels.md
/// "Format 2", WA-4): `item_kind` an `item`'s only; `skeleton_hash`,
/// `quote_hash`, `body_bytes` an `item`'s or a `module`'s only.
pub const ENTRY_KEYS: [&str; 19] = [
    "node",
    "rev",
    "spec_hash",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "levels",
    "sig_hash",
    "body_hash",
    "deps_hash",
    "skeleton_hash",
    "quote_hash",
    "body_bytes",
    "file_blob",
    "commit",
    "verified_by",
    "verified_at",
];

/// The keys of a format-1 `[[binding]]` block, in order (`spec-lock.md`
/// "Entry" before 5b-1a): `item_kind`, `ast_hash` an `item`'s only.
pub const ENTRY_KEYS_FORMAT_1: [&str; 13] = [
    "node",
    "rev",
    "spec_hash",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "ast_hash",
    "file_blob",
    "commit",
    "verified_by",
    "verified_at",
];

/// The keys a format-2 block of `kind` holds, in order.
pub fn entry_keys(kind: &str) -> Vec<&'static str> {
    ENTRY_KEYS
        .iter()
        .copied()
        .filter(|key| match *key {
            "item_kind" => kind == "item",
            "skeleton_hash" | "quote_hash" | "body_bytes" => kind != "ron",
            _ => true,
        })
        .collect()
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique() -> usize {
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// What the owner was shown before the one question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompted {
    pub rows: String,
    pub question: String,
}

/// One library accept: its result and every prompt shown.
#[derive(Debug)]
pub struct Attempt {
    pub outcome: Result<LockOutcome, CliError>,
    pub prompts: Vec<Prompted>,
}

impl Attempt {
    /// Exit 0 with a commit, after exactly one question.
    pub fn committed(&self, context: &str) -> &LockOutcome {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
        assert_eq!(outcome.exit(), Exit::Answered, "{context}: {outcome:?}");
        assert!(
            outcome.commit.is_some(),
            "{context}: no commit: {outcome:?}"
        );
        assert_eq!(self.prompts.len(), 1, "{context}: {:?}", self.prompts);
        outcome
    }

    /// Exit 0, nothing to change, no question.
    pub fn unchanged(&self, context: &str) -> &LockOutcome {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
        assert_eq!(outcome.exit(), Exit::Answered, "{context}: {outcome:?}");
        assert!(
            outcome.unchanged,
            "{context}: not `nothing to change`: {outcome:?}"
        );
        assert!(outcome.commit.is_none(), "{context}: {outcome:?}");
        assert!(
            self.prompts.is_empty(),
            "{context}: asked {:?}",
            self.prompts
        );
        outcome
    }

    /// Exit 1: the refusal.
    pub fn refused(&self, context: &str) -> String {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
        assert_eq!(outcome.exit(), Exit::NotFound, "{context}: {outcome:?}");
        assert!(outcome.commit.is_none(), "{context}: {outcome:?}");
        outcome.refusal.clone().expect("a refusal")
    }

    /// Exit 2: the message.
    pub fn cannot(&self, context: &str) -> String {
        match &self.outcome {
            Ok(outcome) => panic!("{context}: expected exit 2, got {outcome:?}"),
            Err(error) => {
                assert_eq!(error.exit, Exit::CannotRun, "{context}: {error}");
                assert_eq!(error.exit.code(), 2, "{context}");
                error.message.clone()
            }
        }
    }

    /// The rows the owner saw (the one prompt's), else the rows of the
    /// outcome still to print.
    pub fn rows(&self) -> String {
        if let Some(prompt) = self.prompts.first() {
            return prompt.rows.clone();
        }
        self.outcome
            .as_ref()
            .ok()
            .and_then(|outcome| outcome.report.clone())
            .unwrap_or_default()
    }

    /// The one question.
    pub fn question(&self) -> &str {
        &self.prompts.first().expect("a question was asked").question
    }
}

/// One library regen.
#[derive(Debug)]
pub struct RegenAttempt {
    pub outcome: Result<RegenOutcome, CliError>,
    pub prompts: Vec<Prompted>,
}

impl RegenAttempt {
    /// Exit 0, written, after one question.
    pub fn written(&self, context: &str) -> &RegenOutcome {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
        assert_eq!(outcome.exit(), Exit::Answered, "{context}: {outcome:?}");
        assert!(outcome.written, "{context}: {outcome:?}");
        assert_eq!(self.prompts.len(), 1, "{context}: {:?}", self.prompts);
        outcome
    }

    pub fn refused(&self, context: &str) -> String {
        let outcome = self
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{context}: exit {:?}: {error}", error.exit));
        assert_eq!(outcome.exit(), Exit::NotFound, "{context}: {outcome:?}");
        assert!(!outcome.written, "{context}: {outcome:?}");
        outcome.refusal.clone().expect("a refusal")
    }

    pub fn cannot(&self, context: &str) -> String {
        match &self.outcome {
            Ok(outcome) => panic!("{context}: expected exit 2, got {outcome:?}"),
            Err(error) => {
                assert_eq!(error.exit, Exit::CannotRun, "{context}: {error}");
                error.message.clone()
            }
        }
    }

    pub fn rows(&self) -> String {
        if let Some(prompt) = self.prompts.first() {
            return prompt.rows.clone();
        }
        self.outcome
            .as_ref()
            .ok()
            .and_then(|outcome| outcome.report.clone())
            .unwrap_or_default()
    }

    pub fn question(&self) -> &str {
        &self.prompts.first().expect("a question was asked").question
    }
}

/// A pseudo-terminal run of `spec`: stdin a terminal; stdout and stderr
/// each read from a file of its own.
#[derive(Debug, Clone)]
pub struct PtyRun {
    pub args: Vec<String>,
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    /// A question (`…? [y/N]`) was seen on stderr (and answered: the
    /// given answer, `n` where none was given).
    pub asked: bool,
}

impl PtyRun {
    pub fn code(&self, code: i32) -> &Self {
        assert_eq!(self.code, code, "spec {:?}:\n{}", self.args, self.show());
        self
    }

    pub fn show(&self) -> String {
        format!(
            "exit {} (asked: {})\n--- stdout\n{}--- stderr\n{}",
            self.code, self.asked, self.stdout, self.stderr
        )
    }
}

/// What a refused or failed run must leave as it was (TE-3): `HEAD`, the
/// branch, `git status --porcelain=v1 -z --untracked-files=all`, the index
/// and every file outside `.git`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoState {
    pub head: Option<String>,
    pub refs: String,
    pub status: Vec<u8>,
    pub index: String,
    pub files: BTreeMap<String, Option<Vec<u8>>>,
}

/// A scratch repository of `fixtures/code-a`, committed once on `main`.
pub struct LockRepo {
    pub scratch: Scratch,
    pub git: Sandbox,
    /// The worktree's top, canonical; the project root unless a test says.
    pub root: PathBuf,
    /// The data directory's `HOME`.
    pub home: PathBuf,
}

impl LockRepo {
    /// `fixtures/code-a` committed as `the fixture` on `main`.
    pub fn code_a(label: &str) -> Self {
        Self::of(label, FIXTURE, "repo", &[])
    }

    /// `fixtures/<fixture>` at `<scratch>/<dir>`, `git init` with
    /// `init_extra` (`--object-format=sha256`), committed once.
    pub fn of(label: &str, fixture: &str, dir: &str, init_extra: &[&str]) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy(fixture, dir);
        let _ = fs::remove_file(root.join("expected.json"));
        git.init_with(&root, init_extra);
        git.add_all(&root);
        git.commit(&root, "the fixture");
        let home = scratch.home("h");
        Self {
            scratch,
            git,
            root,
            home,
        }
    }

    /// A repository with the copy of `fixtures/code-a` not committed and
    /// not initialised: the test makes it.
    pub fn uncommitted(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy(FIXTURE, "repo");
        let _ = fs::remove_file(root.join("expected.json"));
        let home = scratch.home("h");
        Self {
            scratch,
            git,
            root,
            home,
        }
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

    // ------------------------------------------------------------ accept

    /// Library `lock_accept` from `cwd` with `git`, at `now`, `consent`
    /// answering the one question.
    pub fn accept_with(
        &self,
        cwd: &Path,
        reference: &str,
        editorial: bool,
        now: &str,
        git: GitEnv,
        consent: &mut dyn FnMut(&LockPrompt<'_>) -> bool,
    ) -> Result<LockOutcome, CliError> {
        lock_accept(
            &self.env(cwd),
            &Globals::default(),
            &LockAcceptRequest {
                reference: reference.to_owned(),
                editorial,
                now: now.to_owned(),
                git,
                // A plain accept: no queue item, no recorded branch
                // (docs/features/drift-queue.md "Accept's seams").
                proposal: None,
                branch: None,
            },
            consent,
            &[],
        )
    }

    /// [`LockRepo::accept_with`], the answer `answer`, every prompt
    /// recorded.
    pub fn attempt_at(
        &self,
        cwd: &Path,
        reference: &str,
        editorial: bool,
        now: &str,
        answer: bool,
    ) -> Attempt {
        let mut prompts = Vec::new();
        let mut consent = |prompt: &LockPrompt<'_>| {
            prompts.push(Prompted {
                rows: prompt.rows.to_owned(),
                question: prompt.question.to_owned(),
            });
            answer
        };
        let outcome = self.accept_with(
            cwd,
            reference,
            editorial,
            now,
            self.git_env(cwd),
            &mut consent,
        );
        Attempt { outcome, prompts }
    }

    /// An accept from the root at [`CLOCK`].
    pub fn accept(&self, reference: &str, answer: bool) -> Attempt {
        self.attempt_at(&self.root, reference, false, CLOCK, answer)
    }

    /// An `--editorial` accept from the root at [`CLOCK`].
    pub fn editorial(&self, reference: &str, answer: bool) -> Attempt {
        self.attempt_at(&self.root, reference, true, CLOCK, answer)
    }

    /// An accept that must commit: the outcome.
    pub fn accept_ok(&self, reference: &str) -> LockOutcome {
        let attempt = self.accept(reference, true);
        attempt.committed(&format!("accept {reference}")).clone()
    }

    /// An accept at `now` that must commit.
    pub fn accept_ok_at(&self, reference: &str, now: &str) -> LockOutcome {
        let attempt = self.attempt_at(&self.root, reference, false, now, true);
        attempt.committed(&format!("accept {reference}")).clone()
    }

    // ------------------------------------------------------------- regen

    pub fn regen_with(
        &self,
        cwd: &Path,
        consent: &mut dyn FnMut(&LockPrompt<'_>) -> bool,
    ) -> Result<RegenOutcome, CliError> {
        lock_regen(
            &self.env(cwd),
            &Globals::default(),
            &LockRegenRequest {
                git: self.git_env(cwd),
            },
            consent,
        )
    }

    pub fn regen(&self, answer: bool) -> RegenAttempt {
        let mut prompts = Vec::new();
        let mut consent = |prompt: &LockPrompt<'_>| {
            prompts.push(Prompted {
                rows: prompt.rows.to_owned(),
                question: prompt.question.to_owned(),
            });
            answer
        };
        let outcome = self.regen_with(&self.root, &mut consent);
        RegenAttempt { outcome, prompts }
    }

    // --------------------------------------------------------------- git

    /// `git args` at the root; it must succeed.
    pub fn git(&self, args: &[&str]) -> Vec<u8> {
        self.git.git(&self.root, args)
    }

    pub fn git_text(&self, args: &[&str]) -> String {
        self.git.git_text(&self.root, args)
    }

    /// `git add -A` and a commit without hooks.
    pub fn commit_all(&self, message: &str) {
        self.git.add_all(&self.root);
        self.git.commit(&self.root, message);
    }

    pub fn head(&self) -> String {
        self.git.head(&self.root).expect("HEAD is born")
    }

    /// `git rev-parse <commit>:<path>`.
    pub fn blob_at(&self, commit: &str, path: &str) -> String {
        self.git_text(&["rev-parse", &format!("{commit}:{path}")])
    }

    /// The committed bytes of `path` at `commit`.
    pub fn show_blob(&self, commit: &str, path: &str) -> Vec<u8> {
        self.git(&["cat-file", "blob", &format!("{commit}:{path}")])
    }

    /// The last commit's message, as git stores it.
    pub fn message(&self, commit: &str) -> String {
        String::from_utf8(self.git(&["log", "-1", "--format=%B", commit])).expect("UTF-8 message")
    }

    /// The last commit's parents.
    pub fn parents(&self, commit: &str) -> Vec<String> {
        self.git_text(&["log", "-1", "--format=%P", commit])
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }

    /// The paths `commit` changes against its first parent.
    pub fn changed(&self, commit: &str) -> Vec<String> {
        self.git_text(&[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "--root",
            commit,
        ])
        .lines()
        .map(str::to_owned)
        .collect()
    }

    /// `git status --porcelain=v1 -z --untracked-files=all`.
    pub fn porcelain(&self) -> Vec<u8> {
        self.git(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])
    }

    pub fn state(&self) -> RepoState {
        RepoState {
            head: self.git.head(&self.root),
            refs: self.git_text(&["for-each-ref", "--format=%(refname) %(objectname)"]),
            status: self.porcelain(),
            index: self.git_text(&["ls-files", "-s"]),
            files: snapshot(&self.root)
                .into_iter()
                .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
                .collect(),
        }
    }

    /// A hook at `.git/hooks/<name>` (a shell script, executable).
    pub fn hook(&self, name: &str, body: &str) {
        let dir = self.root.join(".git").join("hooks");
        fs::create_dir_all(&dir).expect("hooks directory");
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("hook");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable hook");
    }

    // ------------------------------------------------------------- files

    pub fn lock(&self) -> Option<Vec<u8>> {
        fs::read(self.root.join("spec.lock")).ok()
    }

    pub fn lock_text(&self) -> String {
        String::from_utf8(self.lock().expect("spec.lock exists")).expect("UTF-8 lock")
    }

    pub fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory");
        }
        fs::write(path, bytes).expect("write");
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).expect("read")
    }

    /// `from` replaced by `to` in `relative`, exactly once.
    pub fn replace(&self, relative: &str, from: &str, to: &str) {
        let text = self.read(relative);
        assert_eq!(
            text.matches(from).count(),
            1,
            "{relative}: {from:?} not found exactly once"
        );
        self.write(relative, text.replacen(from, to, 1));
    }

    /// `text` appended to `relative`.
    pub fn append(&self, relative: &str, text: &str) {
        let mut whole = self.read(relative);
        whole.push_str(text);
        self.write(relative, whole);
    }

    // ------------------------------------------------------------ binary

    /// Every variable a `spec` process of the test gets: the sandbox's,
    /// this `HOME`.
    fn spec_vars(&self) -> Vec<(OsString, OsString)> {
        let mut vars: Vec<(OsString, OsString)> = self
            .git
            .vars()
            .into_iter()
            .filter(|(name, _)| name != "HOME")
            .collect();
        vars.push(("HOME".into(), self.home.clone().into_os_string()));
        vars
    }

    /// `spec args` in `cwd` with stdin null (never a terminal).
    pub fn spec_piped(&self, cwd: &Path, args: &[&str]) -> Run {
        let vars = self.spec_vars();
        let env: Vec<(&str, &OsStr)> = vars
            .iter()
            .map(|(name, value)| (name.to_str().expect("UTF-8 name"), value.as_os_str()))
            .collect();
        spec_with(cwd, args, &env)
    }

    /// `spec args` in `cwd` on a pseudo-terminal (`script`): stdout and
    /// stderr go to files of their own; at the first `[y/N]` on stderr the
    /// terminal answers `answer` (`None`: the run must not ask, and `n` is
    /// typed so it cannot hang). A watchdog kills a run past
    /// [`RUN_TIMEOUT`].
    pub fn spec_pty(&self, cwd: &Path, args: &[&str], answer: Option<&str>) -> PtyRun {
        let n = unique();
        let out = self.scratch.join(&format!("pty-{n}.out"));
        let err = self.scratch.join(&format!("pty-{n}.err"));
        let script = ["/usr/bin/script", "/bin/script"]
            .into_iter()
            .find(|path| Path::new(path).exists())
            .expect("a `script` program gives `spec` a terminal");
        let inner: Vec<&str> = [
            "/bin/sh",
            "-c",
            "exec \"$0\" \"$@\" >\"$LOCK_PTY_OUT\" 2>\"$LOCK_PTY_ERR\"",
            SPEC,
        ]
        .into_iter()
        .chain(args.iter().copied())
        .collect();
        let mut command = Command::new(script);
        if cfg!(target_os = "macos") {
            command.arg("-q").arg("/dev/null").args(&inner);
        } else {
            let quoted: Vec<String> = inner
                .iter()
                .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
                .collect();
            command.arg("-qec").arg(quoted.join(" ")).arg("/dev/null");
        }
        command
            .env_clear()
            .envs(self.spec_vars())
            .env("LOCK_PTY_OUT", &out)
            .env("LOCK_PTY_ERR", &err)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("spawn script");
        let mut stdin = child.stdin.take().expect("stdin");
        let mut echo = child.stdout.take().expect("stdout");
        let drain = std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = echo.read_to_end(&mut sink);
        });
        let mut asked = false;
        let started = Instant::now();
        let status = loop {
            // The question ends stderr, unterminated: `…? [y/N] ` (a help
            // text naming `[y/N]` is no question).
            if !asked
                && fs::read(&err)
                    .map(|bytes| {
                        String::from_utf8_lossy(&bytes)
                            .trim_end()
                            .ends_with("? [y/N]")
                    })
                    .unwrap_or(false)
            {
                asked = true;
                let typed = answer.unwrap_or("n");
                let _ = stdin.write_all(format!("{typed}\n").as_bytes());
                let _ = stdin.flush();
            }
            if let Some(status) = child.try_wait().expect("wait on script") {
                break status;
            }
            if started.elapsed() > RUN_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                panic!("spec {args:?} on a terminal ran longer than {RUN_TIMEOUT:?}; killed");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        drop(stdin);
        drain.join().expect("the echo reader");
        let stdout = fs::read(&out).unwrap_or_default();
        let stderr = fs::read(&err).unwrap_or_default();
        let _ = fs::remove_file(&out);
        let _ = fs::remove_file(&err);
        let stderr = String::from_utf8(stderr).expect("UTF-8 stderr");
        PtyRun {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            code: status
                .code()
                .expect("script exited, not killed by a signal"),
            stdout: String::from_utf8(stdout).expect("UTF-8 stdout"),
            asked: asked || stderr.contains("? [y/N]"),
            stderr,
        }
    }

    /// The node's `norm_hash` as `spec show` prints it (` | norm b3:…`),
    /// over the worktree.
    pub fn norm_hash(&self, reference: &str) -> String {
        let run = self.spec_piped(&self.root, &["show", reference]);
        run.code(0);
        let header = run.stdout.lines().next().expect("a header");
        let (_, rest) = header
            .split_once(NORM_SUFFIX)
            .unwrap_or_else(|| panic!("no norm hash in {header:?}"));
        format!("b3:{}", &rest[..64])
    }
}

/// The `[[binding]]` blocks of a lock's text, each as its `key = value`
/// lines (the header and top keys dropped).
pub fn blocks(text: &str) -> Vec<Vec<(String, String)>> {
    let mut out: Vec<Vec<(String, String)>> = Vec::new();
    for line in text.lines() {
        if line == "[[binding]]" {
            out.push(Vec::new());
            continue;
        }
        let Some(block) = out.last_mut() else {
            continue;
        };
        if let Some((key, value)) = line.split_once(" = ") {
            block.push((key.to_owned(), value.trim_matches('"').to_owned()));
        }
    }
    out
}

/// The value of `key` in `block`.
pub fn value<'a>(block: &'a [(String, String)], key: &str) -> Option<&'a str> {
    block
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

/// The blocks of `text` whose `node` is `node`, each rendered back as its
/// lines (the block's bytes, no `[[binding]]` line).
pub fn node_blocks(text: &str, node: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in text.split("\n[[binding]]\n").skip(1) {
        let first = chunk.lines().next().unwrap_or_default();
        if first == format!("node = \"{node}\"") {
            out.push(chunk.to_owned());
        }
    }
    out
}

/// The rows of a prompt or report, one per line.
pub fn row_lines(rows: &str) -> Vec<&str> {
    rows.lines().collect()
}

/// The question's fixed tail for a repository at `head` on `branch` in
/// `worktree`.
pub fn at(head: &str, branch: &str, worktree: &Path) -> String {
    format!(" at {head} on {branch} in {}? [y/N]", worktree.display())
}

/// The canonical format-2 lock of `entries` (each a block's `key = value`
/// lines, LF-ended, no `[[binding]]` line).
pub fn lock_of(entries: &[String]) -> String {
    let mut out = format!("{HEADER}\n{PREAMBLE}");
    for entry in entries {
        out.push_str("\n[[binding]]\n");
        out.push_str(entry);
    }
    out
}

/// The canonical format-1 lock of `entries` ([`block_format_1`]'s).
pub fn lock_of_format_1(entries: &[String]) -> String {
    let mut out = format!("{HEADER}\n{PREAMBLE_FORMAT_1}");
    for entry in entries {
        out.push_str("\n[[binding]]\n");
        out.push_str(entry);
    }
    out
}

/// The code fields of a format-2 block (lock-levels.md "Format 2"),
/// between `item_kind` and `file_blob`: `levels`, the three level
/// digests (every kind), and an `item`'s or a `module`'s fingerprints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    /// Canonical order, as written.
    pub levels: Vec<String>,
    pub sig: String,
    pub body: String,
    pub deps: String,
    /// `skeleton_hash`, `quote_hash`, `body_bytes`: `None` for a `ron`
    /// entry.
    pub fingerprints: Option<(String, String, u32)>,
}

impl Code {
    /// Made-up digests of the right form (regen's entries, which no hash
    /// is computed for): every digest `b3:` and `digit`, `levels` the
    /// default, fingerprints when `fingerprints`.
    pub fn fake(digit: char, fingerprints: bool) -> Self {
        Self {
            levels: vec!["sig".to_owned(), "body".to_owned()],
            sig: fake_b3(digit),
            body: fake_b3(digit),
            deps: fake_b3(digit),
            fingerprints: fingerprints.then(|| (fake_b3(digit), fake_b3(digit), 212)),
        }
    }

    /// The same digests under other `levels`.
    pub fn with_levels(mut self, levels: &[&str]) -> Self {
        self.levels = levels.iter().map(|level| (*level).to_owned()).collect();
        self
    }

    /// The block's lines, LF-ended.
    pub fn lines(&self) -> String {
        let mut out = format!(
            "{}\nsig_hash = \"{}\"\nbody_hash = \"{}\"\ndeps_hash = \"{}\"\n",
            levels_line(&self.levels),
            self.sig,
            self.body,
            self.deps
        );
        if let Some((skeleton, quote, body_bytes)) = &self.fingerprints {
            out.push_str(&format!(
                "skeleton_hash = \"{skeleton}\"\nquote_hash = \"{quote}\"\nbody_bytes = {body_bytes}\n"
            ));
        }
        out
    }

    /// `lock_code` (lock-levels.md "Drift evidence"): the `levels`,
    /// `sig_hash`, `body_hash`, `deps_hash` lines as the render writes
    /// them, LF-joined, no final LF; never a fingerprint.
    pub fn lock_code(&self) -> String {
        format!(
            "{}\nsig_hash = \"{}\"\nbody_hash = \"{}\"\ndeps_hash = \"{}\"",
            levels_line(&self.levels),
            self.sig,
            self.body,
            self.deps
        )
    }
}

/// `levels = ["sig", "body"]`: basic strings, `", "` between, no
/// trailing comma.
pub fn levels_line<S: AsRef<str>>(levels: &[S]) -> String {
    let items: Vec<String> = levels
        .iter()
        .map(|level| format!("\"{}\"", level.as_ref()))
        .collect();
    format!("levels = [{}]", items.join(", "))
}

/// `b3:` and a digest's hex.
pub fn b3(digest: &specengine_code::Digest) -> String {
    format!("b3:{}", digest.to_hex())
}

fn hashed(state: &LevelState, what: &str) -> (String, String, String, (String, String, u32)) {
    match state {
        LevelState::Hashed(hashes) => (
            b3(&hashes.sig),
            b3(&hashes.body),
            b3(&hashes.deps),
            (b3(&hashes.skeleton), b3(&hashes.quote), hashes.body_bytes),
        ),
        LevelState::CannotVerify { categories } => {
            panic!("{what} cannot be hashed: {categories:?}")
        }
    }
}

fn owned(levels: &[&str]) -> Vec<String> {
    levels.iter().map(|level| (*level).to_owned()).collect()
}

/// The code fields of the item of kind `item_kind` starting on 1-based
/// `line` of `source`, through `specengine-code`'s `item_levels` (an
/// index into `analyze_tree`'s items), under `levels`.
pub fn item_code(source: &[u8], line: usize, item_kind: &str, levels: &[&str]) -> Code {
    let source = std::str::from_utf8(source).expect("UTF-8 source");
    let mut parser = RustParser::new().expect("the Rust grammar");
    let tree = parser.parse(source).expect("a tree");
    let analysis = analyze_tree(&tree, source);
    let indexes: Vec<usize> = analysis
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.line == line && item.kind == item_kind)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        indexes.len(),
        1,
        "one {item_kind} on line {line}: {indexes:?}"
    );
    let states = item_levels(&tree, source, &indexes);
    let state = states[0]
        .as_ref()
        .unwrap_or_else(|| panic!("no levels for the {item_kind} on line {line}"));
    let (sig, body, deps, fingerprints) = hashed(state, item_kind);
    Code {
        levels: owned(levels),
        sig,
        body,
        deps,
        fingerprints: Some(fingerprints),
    }
}

/// The code fields of `source`'s file module, through `module_levels`.
pub fn module_code(source: &[u8], levels: &[&str]) -> Code {
    let source = std::str::from_utf8(source).expect("UTF-8 source");
    let mut parser = RustParser::new().expect("the Rust grammar");
    let tree = parser.parse(source).expect("a tree");
    let (sig, body, deps, fingerprints) = hashed(&module_levels(&tree, source), "the module");
    Code {
        levels: owned(levels),
        sig,
        body,
        deps,
        fingerprints: Some(fingerprints),
    }
}

/// The code fields of the RON value the comment starting on 1-based
/// `line` of `source` anchors to, through `ron::value_hashes` (keyed by
/// the comment's range): `sig` = `body` = the value hash, `deps` the empty
/// set's digest (Q24), no fingerprint.
pub fn ron_code(source: &[u8], line: usize, levels: &[&str]) -> Code {
    let source = std::str::from_utf8(source).expect("UTF-8 source");
    let RonValues::Hashed(values) = value_hashes(source) else {
        panic!("the RON file is rejected")
    };
    let found: Vec<_> = values
        .iter()
        .filter(|(range, _)| source[..range.start].matches('\n').count() + 1 == line)
        .collect();
    assert_eq!(found.len(), 1, "one anchored comment on line {line}");
    let value = b3(&found[0].1);
    Code {
        levels: owned(levels),
        sig: value.clone(),
        body: value,
        deps: b3(&specengine_code::empty_deps_digest()),
        fingerprints: None,
    }
}

/// The v2 `b3:` digest of the item of kind `item_kind` starting on
/// 1-based `line` of `source`, as `specengine-code`'s full analysis hashes
/// it (a format-1 entry's `ast_hash`).
pub fn item_hash(source: &[u8], line: usize, item_kind: &str) -> String {
    let source = std::str::from_utf8(source).expect("UTF-8 source");
    let mut parser = RustParser::new().expect("the Rust grammar");
    let analysis = analyze_file(&mut parser, source).expect("a tree");
    let items: Vec<_> = analysis
        .items
        .iter()
        .filter(|item| item.line == line && item.kind == item_kind)
        .collect();
    assert_eq!(items.len(), 1, "one {item_kind} on line {line}: {items:?}");
    let digest = items[0]
        .state
        .digest()
        .unwrap_or_else(|| panic!("the {item_kind} on line {line} has no digest"));
    format!("b3:{}", digest.to_hex())
}

/// One format-2 block's lines in the order of "Format 2"; `item_kind` an
/// `item`'s.
#[allow(clippy::too_many_arguments)]
pub fn block(
    node: &str,
    rev: u32,
    spec_hash: &str,
    relation: &str,
    file: &str,
    kind: &str,
    symbol: &str,
    item_kind: Option<&str>,
    code: &Code,
    file_blob: &str,
    commit: &str,
    verified_at: &str,
) -> String {
    let mut out = format!(
        "node = \"{node}\"\nrev = {rev}\nspec_hash = \"{spec_hash}\"\nrelation = \"{relation}\"\nfile = \"{file}\"\nkind = \"{kind}\"\nsymbol = \"{symbol}\"\n"
    );
    if let Some(item_kind) = item_kind {
        out.push_str(&format!("item_kind = \"{item_kind}\"\n"));
    }
    out.push_str(&code.lines());
    out.push_str(&format!(
        "file_blob = \"{file_blob}\"\ncommit = \"{commit}\"\nverified_by = \"owner\"\nverified_at = \"{verified_at}\"\n"
    ));
    out
}

/// One format-1 block's lines in the order of format 1's "Entry".
#[allow(clippy::too_many_arguments)]
pub fn block_format_1(
    node: &str,
    rev: u32,
    spec_hash: &str,
    relation: &str,
    file: &str,
    kind: &str,
    symbol: &str,
    item: Option<(&str, &str)>,
    file_blob: &str,
    commit: &str,
    verified_at: &str,
) -> String {
    let mut out = format!(
        "node = \"{node}\"\nrev = {rev}\nspec_hash = \"{spec_hash}\"\nrelation = \"{relation}\"\nfile = \"{file}\"\nkind = \"{kind}\"\nsymbol = \"{symbol}\"\n"
    );
    if let Some((item_kind, ast_hash)) = item {
        out.push_str(&format!(
            "item_kind = \"{item_kind}\"\nast_hash = \"{ast_hash}\"\n"
        ));
    }
    out.push_str(&format!(
        "file_blob = \"{file_blob}\"\ncommit = \"{commit}\"\nverified_by = \"owner\"\nverified_at = \"{verified_at}\"\n"
    ));
    out
}

/// A synthetic 64-hex digest: `b3:` and `digit` repeated (regen's entries,
/// which no hash is computed for).
pub fn fake_b3(digit: char) -> String {
    format!("b3:{}", digit.to_string().repeat(64))
}

/// A synthetic 40-hex OID.
pub fn fake_oid(digit: char) -> String {
    digit.to_string().repeat(40)
}
