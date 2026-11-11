//! docs/features/proposal-rehearsal.md (RM-0067), the walk `spec approve
//! --open`: the library `approve_open` with a scripted terminal (whose `ask`
//! may change the queue while the question is shown), the binary on a piped
//! stdin, and the binary on a pseudo-terminal (`script`, as `round.rs` and
//! `lock_common` drive it) for the real `[y/N]`: stdout and stderr go to
//! files of their own and each `[y/N]` on stderr is answered by the next
//! key, since macOS `script` sends ^D at the end of its input before `spec`
//! reads.
//!
//! - AC-05: three open creates give three questions in order and three
//!   commits, each with its `Proposal:`; `n` on the second leaves it open,
//!   the third is asked, the outcome holds it in `left`, exit 1 (library and
//!   terminal, text and JSON). M: one answer reused; stopping at the decline.
//! - AC-06: a piped stdin carrying `y`: exit 2, no event, no commit, before
//!   anything is read (no data directory made; an unreadable queue or no
//!   project at all gives the same line); `--open` beside a `PR` or a flag:
//!   exit 2, a usage error, on a pipe and on a terminal. M: the terminal
//!   check after the list read.
//! - AC-07: a stale item, a dirty place and a conflict the walk itself makes
//!   between two good ones: the refusal printed, the other two applied,
//!   exit 1; questions, a discrepancy and drift items not asked, each named
//!   in its note in `spec round new`'s words. M: stopping at a refusal;
//!   walking `Decides`.
//! - "Rules": the rows listed once, at the start; an `approved` row not
//!   walked; another repository's not walked; every applying kind walked,
//!   each where it was raised; a staged item shows its stage; an exit 2 of
//!   the identity, the queue or the project stops the walk and each earlier
//!   commit stands; the order avoids a dangling reference.
//! - The walk rule as the owner's decision changed it after the spec's first
//!   text (`own_place_failure`): one proposal's own place failing (another
//!   branch, a detached `HEAD`, an operation in progress, the worktree gone)
//!   leaves that item open with `` `PR` left open: <reason> ``, its row as it
//!   was but for one `proposal.apply_failed`; the walk moves on, exit 1. M:
//!   every exit 2 taken for a place's.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod verify_common;

use std::ffi::OsString;
use std::fs;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::decision::{THREE, discrepancy, hit_names};
use common::git::Sandbox;
use common::proposal::{DECIDER, LATER, NOW, Pair, cannot, edit};
use common::{RUN_TIMEOUT, Run, SPEC, data_dir, read, replace, write};
use drift_common::{cap_changed, drifted, proposal, raised};
use lock_common::LockRepo;
use serde_json::{Value, json};
use specengine_cli::{
    ApproveOpenRequest, CliError, Exit, GitEnv, Globals, OpenWalk, Outcome, QuestionRequest,
    RoundTerminal, StageBody, StageRequest, approve_open, propose_question, render_json,
    render_text, stage,
};

const RATIONALE: &str = "Walk it.";
const R13: &str = "docs/records/R/R-13.md";
const R14: &str = "docs/records/R/R-14.md";
const R15: &str = "docs/records/R/R-15.md";
const SPRINT: &str = "docs/spec/movement/sprint.md";
const TARGET: &str = "EDGE-SPRINT-EMPTY";
/// When a stage is made.
const STAGED: &str = "2026-10-05T21:30:00Z";
/// code-a's slug.
const CODE_A: &str = "code-a";
/// The line of a walk with no open proposal ("Data").
const EMPTY_WALK: &str = "open walk: 0 applied, 0 left open\n";

/// A spec-a record `id`, `extra` closing its front-matter.
fn record(id: &str, extra: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n\
         {extra}---\n\n# Record {id}\n\nA body.\n"
    )
}

/// An agent's `spec propose create` of a new file from `cwd`, stored: its ID.
fn create(pair: &Pair, cwd: &Path, path: &str, text: &str) -> String {
    let run = pair.spec_piped(
        cwd,
        &[
            "--json",
            "propose",
            "create",
            path,
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--author-role",
            "spec-writer",
        ],
        text.as_bytes(),
    );
    run.code(0);
    run.json()["id"].as_str().expect("an ID").to_owned()
}

/// Three open creates raised in the linked worktree, oldest first.
fn three_creates(pair: &Pair) -> Vec<String> {
    [("R-13", R13), ("R-14", R14), ("R-15", R15)]
        .into_iter()
        .map(|(id, path)| create(pair, &pair.linked, path, &record(id, "")))
        .collect()
}

/// `git rev-list --reverse <base>..t1`.
fn commits_since(pair: &Pair, base: &str) -> Vec<String> {
    pair.git_text(
        &pair.main,
        &["rev-list", "--reverse", &format!("{base}..t1")],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// `commit` is `id`'s own: its applied commit, with its `Proposal:` trailer.
fn assert_own_commit(pair: &Pair, id: &str, commit: &str) {
    let stored = pair.proposal(id);
    assert_eq!(stored.status.as_str(), "applied", "{id}");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit), "{id}");
    let message = pair.git_text(&pair.main, &["log", "-1", "--format=%B", commit]);
    assert!(
        message
            .lines()
            .any(|line| line == format!("Proposal: {id}")),
        "{id}: {message}"
    );
}

fn status(pair: &Pair, id: &str) -> String {
    pair.proposal(id).status.as_str().to_owned()
}

/// The ID of each question, in order: `apply PR-… ? [y/N]`.
fn asked_in(questions: &[String]) -> Vec<String> {
    questions
        .iter()
        .map(|question| {
            let at = question
                .rfind("apply PR-")
                .unwrap_or_else(|| panic!("not an applying question: {question:?}"));
            question[at + "apply ".len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect::<String>()
        })
        .collect()
}

/// [`asked_in`] over a terminal's stderr, where a question stays unterminated
/// (`…? [y/N] `) and the answer is echoed elsewhere.
fn asked(stderr: &str) -> Vec<String> {
    let mut pieces: Vec<String> = stderr.split("[y/N]").map(str::to_owned).collect();
    pieces.pop();
    asked_in(&pieces)
}

// ---------------------------------------------------------- the library

/// The owner's terminal as a script: `keys[n]` answers question `n` (past
/// them, the end of input); `before` runs while the question is shown.
struct Script<'a> {
    keys: Vec<Option<bool>>,
    questions: Vec<String>,
    told: Vec<String>,
    reports: Vec<String>,
    before: Option<&'a mut dyn FnMut(usize)>,
}

impl<'a> Script<'a> {
    fn new(keys: &[Option<bool>]) -> Self {
        Self {
            keys: keys.to_vec(),
            questions: Vec::new(),
            told: Vec::new(),
            reports: Vec::new(),
            before: None,
        }
    }

    fn doing(keys: &[Option<bool>], before: &'a mut dyn FnMut(usize)) -> Self {
        Self {
            before: Some(before),
            ..Self::new(keys)
        }
    }
}

impl RoundTerminal for Script<'_> {
    fn report(&mut self, text: &str) {
        self.reports.push(text.to_owned());
    }

    fn ask(&mut self, question: &str) -> Option<bool> {
        let index = self.questions.len();
        self.questions.push(question.to_owned());
        if let Some(before) = &mut self.before {
            before(index);
        }
        self.keys.get(index).copied().flatten()
    }

    fn tell(&mut self, line: &str) {
        self.told.push(line.to_owned());
    }
}

/// The library's walk from `cwd` at [`LATER`], `git` the caller's.
fn library_walk(
    pair: &Pair,
    cwd: &Path,
    git: GitEnv,
    script: &mut Script<'_>,
) -> Result<OpenWalk, CliError> {
    approve_open(
        &pair.env(cwd),
        &Globals::default(),
        &ApproveOpenRequest {
            now: LATER.to_owned(),
            git,
        },
        script,
    )
}

/// [`library_walk`] from the main worktree, which must run.
fn walked(pair: &Pair, script: &mut Script<'_>) -> OpenWalk {
    library_walk(pair, &pair.main, pair.git_env(&pair.main), script)
        .unwrap_or_else(|error| panic!("the walk: {error}"))
}

// --------------------------------------------------------- the terminal

/// `spec args` in `cwd` with `vars`, stdin a pseudo-terminal, stdout and
/// stderr into files under `scratch`: the `n`th `[y/N]` on stderr answered
/// by `key(n)`. A watchdog kills a run past [`RUN_TIMEOUT`].
fn on_terminal(
    vars: &[(OsString, OsString)],
    scratch: &Path,
    cwd: &Path,
    args: &[&str],
    key: &mut dyn FnMut(usize) -> String,
) -> Run {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let n = RUNS.fetch_add(1, Ordering::Relaxed);
    let out = scratch.join(format!("walk-{n}.out"));
    let err = scratch.join(format!("walk-{n}.err"));
    let script = ["/usr/bin/script", "/bin/script"]
        .into_iter()
        .find(|path| Path::new(path).exists())
        .expect("a `script` program gives `spec` a terminal");
    let inner: Vec<&str> = [
        "/bin/sh",
        "-c",
        "exec \"$0\" \"$@\" >\"$WALK_OUT\" 2>\"$WALK_ERR\"",
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
        .envs(vars.iter().map(|(name, value)| (name, value)))
        .env("WALK_OUT", &out)
        .env("WALK_ERR", &err)
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
    let mut answered = 0;
    let started = Instant::now();
    let status = loop {
        let shown = fs::read(&err)
            .map(|bytes| String::from_utf8_lossy(&bytes).matches("[y/N]").count())
            .unwrap_or(0);
        while answered < shown {
            let typed = key(answered);
            let _ = stdin.write_all(format!("{typed}\n").as_bytes());
            let _ = stdin.flush();
            answered += 1;
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
    Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status
            .code()
            .expect("script exited, not killed by a signal"),
        stdout: String::from_utf8(stdout).expect("UTF-8 stdout"),
        stderr: String::from_utf8(stderr).expect("UTF-8 stderr"),
    }
}

/// The sandbox's variables with this `HOME`.
fn vars(git: &Sandbox, home: &Path) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .collect();
    vars.push(("HOME".into(), home.as_os_str().to_owned()));
    vars
}

/// The keys in order; past them `n`, so a question too many fails an
/// assertion, not the watchdog.
fn typed<'a>(keys: &'a [&'a str]) -> impl FnMut(usize) -> String + 'a {
    move |n| keys.get(n).copied().unwrap_or("n").to_owned()
}

/// `spec [--json] approve --open` of `pair` from `cwd` on a terminal.
fn walk(pair: &Pair, cwd: &Path, json: bool, keys: &[&str]) -> Run {
    let args: &[&str] = if json {
        &["--json", "approve", "--open"]
    } else {
        &["approve", "--open"]
    };
    on_terminal(
        &vars(&pair.git, &pair.home),
        pair.scratch.path(),
        cwd,
        args,
        &mut typed(keys),
    )
}

// ------------------------------------------------------------------ AC-05

/// AC-05: three open creates: three questions in order, three commits, each
/// with its `Proposal:`; nothing left, exit 0. M: one question for the walk.
#[test]
fn ac05_three_creates_give_three_questions_and_three_commits() {
    let pair = Pair::new("pw-ac05", "spec-a");
    let ids = three_creates(&pair);
    let base = pair.rev(&pair.linked, "HEAD");
    let mut script = Script::new(&[Some(true), Some(true), Some(true)]);
    let outcome = walked(&pair, &mut script);

    let worktree = pair.linked.display();
    assert_eq!(
        script.questions,
        [
            format!(
                "apply {} to {R13} on t1 in {worktree} (new file)? [y/N]",
                ids[0]
            ),
            format!(
                "apply {} to {R14} on t1 in {worktree} (new file)? [y/N]",
                ids[1]
            ),
            format!(
                "apply {} to {R15} on t1 in {worktree} (new file)? [y/N]",
                ids[2]
            ),
        ]
    );
    assert_eq!(outcome.applied, ids);
    assert!(outcome.left.is_empty(), "{outcome:?}");
    assert!(outcome.notes.is_empty(), "{outcome:?}");
    assert_eq!(outcome.exit(), Exit::Answered);

    let commits = commits_since(&pair, &base);
    assert_eq!(commits.len(), 3, "{commits:?}");
    for (id, commit) in ids.iter().zip(&commits) {
        assert_own_commit(&pair, id, commit);
    }
    assert_eq!(
        pair.git_text(&pair.main, &["diff", "--name-only", &format!("{base}..t1")])
            .lines()
            .collect::<Vec<_>>(),
        [R13, R14, R15]
    );
}

/// AC-05: `n` on the second leaves it open, the third is asked, the outcome
/// holds it in `left`, exit 1. M: one answer reused; stopping at the decline.
#[test]
fn ac05_n_on_the_second_leaves_it_open_and_asks_the_third() {
    let pair = Pair::new("pw-ac05-n", "spec-a");
    let ids = three_creates(&pair);
    let base = pair.rev(&pair.linked, "HEAD");
    let mut script = Script::new(&[Some(true), Some(false), Some(true)]);
    let outcome = walked(&pair, &mut script);

    assert_eq!(asked_in(&script.questions), ids);
    assert_eq!(outcome.applied, [ids[0].clone(), ids[2].clone()]);
    assert_eq!(outcome.left, [ids[1].as_str()]);
    assert_eq!(outcome.exit(), Exit::NotFound);
    assert_eq!(outcome.exit().code(), 1);

    let commits = commits_since(&pair, &base);
    assert_eq!(commits.len(), 2, "{commits:?}");
    assert_own_commit(&pair, &ids[0], &commits[0]);
    assert_own_commit(&pair, &ids[2], &commits[1]);
    assert_eq!(status(&pair, &ids[1]), "open");
    assert!(!pair.linked.join(R14).exists(), "declined: no file");

    // "The walk's output", as `spec` prints the outcome.
    let printed = Outcome::OpenWalk(outcome);
    assert_eq!(render_text(&printed), "open walk: 2 applied, 1 left open\n");
    let document: Value = serde_json::from_str(&render_json(&printed)).expect("one document");
    assert_eq!(
        document,
        json!({"applied": [ids[0], ids[2]], "left": [ids[1]], "notes": []})
    );
}

/// AC-05 with the one answer the other way round: `n`, `y`, `n`. M: one
/// answer reused.
#[test]
fn ac05_each_item_takes_its_own_answer() {
    let pair = Pair::new("pw-ac05-own", "spec-a");
    let ids = three_creates(&pair);
    let mut script = Script::new(&[Some(false), Some(true), Some(false)]);
    let outcome = walked(&pair, &mut script);
    assert_eq!(asked_in(&script.questions), ids);
    assert_eq!(outcome.applied, [ids[1].as_str()]);
    assert_eq!(outcome.left, [ids[0].clone(), ids[2].clone()]);
}

/// AC-05 on the real `[y/N]`: keys `y`, `n`, `y`: three questions on stderr,
/// each item's own output on stdout in turn, then the line; the decline's
/// reason said; a second walk asks only what stayed open.
#[test]
fn ac05_on_a_terminal_the_keys_decide_each_item() {
    let pair = Pair::new("pw-ac05-terminal", "spec-a");
    let ids = three_creates(&pair);
    let run = walk(&pair, &pair.main, false, &["y", "n", "y"]);
    run.code(1);
    assert_eq!(asked(&run.stderr), ids, "{}", run.show());
    assert_eq!(status(&pair, &ids[1]), "open");
    let first = pair.proposal(&ids[0]).applied_commit.expect("a commit");
    let third = pair.proposal(&ids[2]).applied_commit.expect("a commit");
    let own = |id: &str, commit: &str| {
        let line = format!("applied {id} as {commit} on t1");
        run.stdout
            .find(&line)
            .unwrap_or_else(|| panic!("{line}: {}", run.show()))
    };
    assert!(
        own(&ids[0], &first) < own(&ids[2], &third),
        "{}",
        run.show()
    );
    assert!(
        run.stdout.ends_with("open walk: 2 applied, 1 left open\n"),
        "{}",
        run.show()
    );
    assert!(
        run.stderr
            .contains(&format!("`{}` not applied: the answer was not `y`", ids[1])),
        "{}",
        run.show()
    );

    let run = walk(&pair, &pair.main, false, &["y"]);
    run.code(0);
    assert_eq!(asked(&run.stderr), [ids[1].as_str()], "{}", run.show());
    assert!(
        run.stdout.ends_with("open walk: 1 applied, 0 left open\n"),
        "{}",
        run.show()
    );
    assert_eq!(status(&pair, &ids[1]), "applied");
}

/// "The walk's output", JSON: one document at the end, no item's output on
/// stdout; the questions still on stderr.
#[test]
fn ac05_on_a_terminal_the_json_is_one_document_at_the_end() {
    let pair = Pair::new("pw-ac05-json", "spec-a");
    let ids = three_creates(&pair);
    let run = walk(&pair, &pair.main, true, &["y", "n", "y"]);
    run.code(1);
    assert_eq!(asked(&run.stderr), ids, "{}", run.show());
    assert_eq!(
        run.json(),
        json!({"applied": [ids[0], ids[2]], "left": [ids[1]], "notes": []}),
        "{}",
        run.show()
    );

    let run = walk(&pair, &pair.main, true, &["n"]);
    run.code(1);
    assert_eq!(
        run.json(),
        json!({"applied": [], "left": [ids[1]], "notes": []}),
        "{}",
        run.show()
    );
}

/// "Data": no open proposal: the line, exit 0, text and JSON, nothing asked,
/// byte for byte on a repeat.
#[test]
fn no_open_proposal_is_a_walk_of_nothing() {
    let pair = Pair::new("pw-empty", "spec-a");
    let head = pair.rev(&pair.main, "HEAD");
    let first = walk(&pair, &pair.main, false, &[]);
    first.code(0);
    assert_eq!(first.stdout, EMPTY_WALK, "{}", first.show());
    assert_eq!(asked(&first.stderr), Vec::<String>::new());
    let second = walk(&pair, &pair.main, false, &[]);
    second.code(0);
    assert_eq!(second.stdout, first.stdout);
    assert_eq!(second.stderr, first.stderr);

    let run = walk(&pair, &pair.main, true, &[]);
    run.code(0);
    assert_eq!(
        run.json(),
        json!({"applied": [], "left": [], "notes": []}),
        "{}",
        run.show()
    );
    assert_eq!(pair.rev(&pair.main, "HEAD"), head);
}

// ------------------------------------------------------------------ AC-06

/// AC-06: a piped stdin carrying `y`: exit 2, one line, no prompt, no event,
/// no commit.
#[test]
fn ac06_a_piped_stdin_exits_2_and_changes_nothing() {
    let pair = Pair::new("pw-ac06", "spec-a");
    let ids = three_creates(&pair);
    let before = pair.state();
    let events = pair.events();
    for args in [
        vec!["approve", "--open"],
        vec!["--json", "approve", "--open"],
    ] {
        let run = pair.spec_piped(&pair.main, &args, b"y\nyes\ny\n");
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}: {}", run.show());
        assert!(
            run.stderr.starts_with("spec: ") && run.stderr.contains("terminal"),
            "{args:?}: {}",
            run.show()
        );
        assert!(!run.stderr.contains("[y/N]"), "{args:?}: {}", run.show());
        assert_eq!(run.stderr.lines().count(), 1, "{}", run.show());
        assert_eq!(pair.state(), before, "{args:?}");
        assert_eq!(pair.events(), events, "{args:?}");
    }
    for id in &ids {
        assert_eq!(status(&pair, id), "open");
    }
}

/// AC-06, "before anything is read": the terminal's line whatever a read
/// would have met: no queue yet (and none made), a queue that does not read,
/// no project at all. M: the terminal check after the list read.
#[test]
fn ac06_the_terminal_check_comes_before_the_list_read() {
    let pair = Pair::new("pw-ac06-order", "spec-a");
    let line = |run: &Run| {
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
        run.stderr.clone()
    };
    let args = ["approve", "--open"];

    let fresh = line(&pair.spec_piped(&pair.main, &args, b"y\n"));
    assert!(fresh.contains("terminal"), "{fresh}");
    assert!(!data_dir(&pair.home).exists(), "the data directory made");

    fs::create_dir_all(data_dir(&pair.home)).expect("the data directory");
    fs::write(pair.db(), b"not a database").expect("a bad queue");
    assert_eq!(line(&pair.spec_piped(&pair.main, &args, b"y\n")), fresh);
    assert_eq!(
        fs::read(pair.db()).expect("the bad queue"),
        b"not a database"
    );

    let nowhere = pair.scratch.dir("nowhere");
    assert_eq!(line(&pair.spec_piped(&nowhere, &args, b"y\n")), fresh);
}

/// AC-06, "Rules": `--open` beside a `PR` or any of `--note`, `--option`,
/// `--answer`, `--canon`, `--editorial`: a usage error, not the terminal's:
/// exit 2 naming `--open` on a pipe and on a terminal, nothing asked; no
/// `--yes`; neither a `PR` nor `--open`: a usage error too. M: `--open`
/// beside a flag walking and dropping it.
#[test]
fn ac06_open_beside_a_pr_or_a_decision_flag_is_a_usage_error() {
    let pair = Pair::new("pw-ac06-usage", "spec-a");
    let id = create(&pair, &pair.linked, R13, &record("R-13", ""));
    let before = pair.state();
    let events = pair.events();
    let settings = vars(&pair.git, &pair.home);
    for args in [
        vec!["approve", "--open", id.as_str()],
        vec!["approve", id.as_str(), "--open"],
        vec!["approve", "--open", "--note", "fine"],
        vec!["approve", "--open", "--option", "1"],
        vec!["approve", "--open", "--answer", "Yes."],
        vec!["approve", "--open", "--canon", "R-12"],
        vec!["approve", "--open", "--editorial"],
        vec!["approve", "--open", "--yes"],
        vec!["approve"],
    ] {
        let piped = pair.spec_piped(&pair.main, &args, b"y\n");
        let terminal = on_terminal(
            &settings,
            pair.scratch.path(),
            &pair.main,
            &args,
            &mut typed(&["y"]),
        );
        for run in [&piped, &terminal] {
            run.code(2);
            assert_eq!(run.stdout, "", "{args:?}: {}", run.show());
            assert!(!run.stderr.contains("[y/N]"), "{args:?}: {}", run.show());
            assert!(
                !run.stderr.contains("asks the owner for consent"),
                "{args:?}: the usage comes first: {}",
                run.show()
            );
        }
        let named = match args.as_slice() {
            ["approve"] => "PR",
            [.., "--yes"] => "--yes",
            _ => "--open",
        };
        assert!(
            terminal.stderr.contains(named),
            "{args:?}: {}",
            terminal.show()
        );
        assert_eq!(pair.state(), before, "{args:?}");
        assert_eq!(pair.events(), events, "{args:?}");
    }
}

// ------------------------------------------------------------------ AC-07

/// How the middle item of AC-07 goes bad.
#[derive(Clone, Copy, Debug)]
enum Spoiled {
    /// Its base moved under it, on the very line it edits.
    Conflict,
    /// Its file is modified and not committed.
    Dirty,
}

/// AC-07 and "Rules": a stale item and a dirty place between two good ones:
/// no question for it, its refusal printed, the other two applied, exit 1,
/// text and JSON. M: stopping at a refusal.
#[test]
fn ac07_a_refused_item_between_two_good_ones_is_printed_and_left() {
    for (spoiled, step, json) in [
        (Spoiled::Conflict, "(step 5)", false),
        (Spoiled::Conflict, "(step 5)", true),
        (Spoiled::Dirty, "(step 3)", false),
    ] {
        let context = format!("{spoiled:?}");
        let pair = Pair::new("pw-ac07", "spec-a");
        let first = create(&pair, &pair.linked, R13, &record("R-13", ""));
        let stale = pair.propose_edit(
            &pair.linked,
            TARGET,
            "the sprint ends;",
            "the sprint ends at once;",
        );
        let third = create(&pair, &pair.linked, R14, &record("R-14", ""));
        match spoiled {
            Spoiled::Conflict => {
                replace(
                    &pair.linked,
                    SPRINT,
                    "the sprint ends;",
                    "the sprint stops;",
                );
                pair.commit_all(&pair.linked, "An overlapping edit.");
            }
            Spoiled::Dirty => replace(
                &pair.linked,
                SPRINT,
                "## Tuning notes",
                "## Tuning notes (draft)",
            ),
        }

        let run = walk(&pair, &pair.main, json, &["y", "y"]);
        run.code(1);
        assert_eq!(
            asked(&run.stderr),
            [first.clone(), third.clone()],
            "{context}: {}",
            run.show()
        );
        let printed = format!("{}{}", run.stdout, run.stderr);
        assert!(
            printed.contains(&stale) && printed.contains(step),
            "{context}: its refusal: {}",
            run.show()
        );
        if json {
            assert_eq!(
                run.json(),
                json!({"applied": [first, third], "left": [stale], "notes": []}),
                "{context}: {}",
                run.show()
            );
        } else {
            assert!(
                run.stdout.ends_with("open walk: 2 applied, 1 left open\n"),
                "{context}: {}",
                run.show()
            );
        }
        assert_eq!(status(&pair, &first), "applied", "{context}");
        assert_eq!(status(&pair, &stale), "open", "{context}");
        assert_eq!(status(&pair, &third), "applied", "{context}");
    }
}

/// AC-07 through the library, the conflict the walk itself makes: two
/// updates of one line: the first applied, the second refused at step 5 with
/// no question, the next asked and applied. M: stopping at a refusal.
#[test]
fn ac07_a_conflict_the_walk_makes_is_a_refusal_and_it_moves_on() {
    let pair = Pair::new("pw-ac07-own", "spec-a");
    let first = pair.propose_edit(
        &pair.linked,
        TARGET,
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let second = pair.propose_edit(
        &pair.linked,
        TARGET,
        "the sprint ends;",
        "the sprint stops;",
    );
    let third = create(&pair, &pair.linked, R13, &record("R-13", ""));
    let mut script = Script::new(&[Some(true), Some(true)]);
    let outcome = walked(&pair, &mut script);
    assert_eq!(asked_in(&script.questions), [first.clone(), third.clone()]);
    assert_eq!(outcome.applied, [first, third]);
    assert_eq!(outcome.left, [second.as_str()]);
    assert_eq!(outcome.exit(), Exit::NotFound);
    assert_eq!(status(&pair, &second), "open");
    assert!(
        script.told.iter().any(|line| line.contains("(step 5)")),
        "{:?}",
        script.told
    );
}

/// A question of a `developer` on code-a's `PIECE-COUNTER`, stored: its ID.
fn question(repo: &LockRepo, text: &str) -> String {
    let mut request = QuestionRequest {
        node_ids: vec!["PIECE-COUNTER".to_owned()],
        text: text.to_owned(),
        working_answer: "Keep it.".to_owned(),
        price_of_other: "One more test.".to_owned(),
        severity: None,
        distinct_from: Vec::new(),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        login: None,
        now: NOW.to_owned(),
        git: repo.git_env(&repo.root),
        card: None,
    };
    let env = repo.env(&repo.root);
    let ask = |request: &QuestionRequest| {
        propose_question(&env, &Globals::default(), request)
            .unwrap_or_else(|error| panic!("ask: {error}"))
    };
    let first = ask(&request);
    let outcome = if first.document.created {
        first
    } else {
        request.distinct_from = hit_names(&first);
        ask(&request)
    };
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

/// An agent's create of a new premise in code-a: its ID.
fn premise(repo: &LockRepo, id: &str, path: &str) -> String {
    let file = repo.scratch.join(&format!("{id}.md"));
    fs::write(
        &file,
        format!(
            "---\nclass: canon\nid: {id}\nowner: owner\nreviewed: 2026-10-10\n---\n\n# Premise \
             {id}\n\nA body.\n"
        ),
    )
    .expect("the text");
    let run = repo.spec_piped(
        &repo.root,
        &[
            "--json",
            "propose",
            "create",
            path,
            "--text-file",
            file.to_str().expect("a UTF-8 path"),
            "--rationale",
            RATIONALE,
            "--author-role",
            "spec-writer",
        ],
    );
    run.code(0);
    run.json()["id"].as_str().expect("an ID").to_owned()
}

/// AC-07 on a real drift (fixtures/code-a with its golden lock): drift items
/// and two questions in the queue are not asked and each stays `open`, named
/// in its note in the spec's words, in the text and in the JSON; only the
/// creates are asked. M: walking `Decides`; walking a drift item.
#[test]
fn ac07_questions_and_drift_items_are_noted_not_asked() {
    let repo = drifted("pw-ac07-notes", cap_changed);
    let drift: Vec<String> = raised(&repo, &[])
        .created()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert!(!drift.is_empty(), "the edit drifted");
    let one = question(&repo, "Does the counter stop at the cap or wrap around?");
    let two = question(&repo, "Who resets a counter once a run has ended?");
    let good = premise(&repo, "PREM-900", "docs/spec/premise-900.md");
    let kept = premise(&repo, "PREM-901", "docs/spec/premise-901.md");
    let deciding =
        format!("2 open deciding item(s) not walked: {one}, {two}; spec round new lists them");
    let drifting = format!(
        "{} open drift item(s) not walked: {}; spec review <ID>, then spec approve or spec reject",
        drift.len(),
        drift.join(", ")
    );
    let settings = vars(&repo.git, &repo.home);
    let head = repo.head();

    let run = on_terminal(
        &settings,
        repo.scratch.path(),
        &repo.root,
        &["approve", "--open"],
        &mut typed(&["y", "n"]),
    );
    run.code(1);
    assert_eq!(
        asked(&run.stderr),
        [good.clone(), kept.clone()],
        "{}",
        run.show()
    );
    let printed = format!("{}{}", run.stdout, run.stderr);
    let walked_line = printed
        .find("open walk: 1 applied, 1 left open\n")
        .unwrap_or_else(|| panic!("the line: {}", run.show()));
    for note in [&deciding, &drifting] {
        let at = printed
            .find(note.as_str())
            .unwrap_or_else(|| panic!("{note}: {}", run.show()));
        assert!(at > walked_line, "after the line: {}", run.show());
    }
    assert_ne!(repo.head(), head, "the create's commit");

    let run = on_terminal(
        &settings,
        repo.scratch.path(),
        &repo.root,
        &["--json", "approve", "--open"],
        &mut typed(&["n"]),
    );
    run.code(1);
    assert_eq!(asked(&run.stderr), [kept.as_str()], "{}", run.show());
    assert_eq!(
        run.json(),
        json!({"applied": [], "left": [kept], "notes": [deciding, drifting]}),
        "{}",
        run.show()
    );

    for id in drift.iter().chain([&one, &two]) {
        assert_eq!(proposal(&repo, CODE_A, id).status.as_str(), "open", "{id}");
    }
}

/// A queue holding only what the walk does not take: a question and drift
/// items: nothing asked, nothing left, exit 0, each named in its note, text
/// and JSON; no commit. M: a noted item counted as left.
#[test]
fn a_queue_of_only_a_question_and_drift_items_exits_0_with_both_noted() {
    let repo = drifted("pw-unwalked", cap_changed);
    let drift: Vec<String> = raised(&repo, &[])
        .created()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert!(!drift.is_empty(), "the edit drifted");
    let one = question(&repo, "Does the counter stop at the cap or wrap around?");
    let deciding = format!("1 open deciding item(s) not walked: {one}; spec round new lists them");
    let drifting = format!(
        "{} open drift item(s) not walked: {}; spec review <ID>, then spec approve or spec reject",
        drift.len(),
        drift.join(", ")
    );
    let settings = vars(&repo.git, &repo.home);
    let head = repo.head();

    let run = on_terminal(
        &settings,
        repo.scratch.path(),
        &repo.root,
        &["approve", "--open"],
        &mut typed(&[]),
    );
    run.code(0);
    assert!(!run.stderr.contains("[y/N]"), "{}", run.show());
    assert!(run.stdout.starts_with(EMPTY_WALK), "{}", run.show());
    for note in [&deciding, &drifting] {
        assert!(run.stdout.contains(note.as_str()), "{}", run.show());
    }

    let run = on_terminal(
        &settings,
        repo.scratch.path(),
        &repo.root,
        &["--json", "approve", "--open"],
        &mut typed(&[]),
    );
    run.code(0);
    assert!(!run.stderr.contains("[y/N]"), "{}", run.show());
    assert_eq!(
        run.json(),
        json!({"applied": [], "left": [], "notes": [deciding, drifting]}),
        "{}",
        run.show()
    );

    assert_eq!(repo.head(), head, "no commit");
    for id in drift.iter().chain([&one]) {
        assert_eq!(proposal(&repo, CODE_A, id).status.as_str(), "open", "{id}");
    }
}

/// AC-07, all that decides: a question and a discrepancy on spec-a, one note,
/// and no drift note with no drift item. M: walking `Decides`.
#[test]
fn ac07_a_discrepancy_counts_as_deciding_and_no_drift_note_is_made_up() {
    let pair = Pair::new("pw-ac07-deciding", "spec-a");
    let asked_one = pair.raise_question(
        &pair.main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
        "It waits.",
        "A new rule case.",
    );
    let reported = pair.raise_discrepancy(
        &pair.main,
        discrepancy(
            &["RULE-STAM-REGEN"],
            "Regeneration starts while sprinting in the code",
            &THREE,
        ),
    );
    let declined = create(&pair, &pair.linked, R13, &record("R-13", ""));
    let mut script = Script::new(&[Some(false)]);
    let outcome = walked(&pair, &mut script);
    assert_eq!(asked_in(&script.questions), [declined.as_str()]);
    assert_eq!(outcome.left, [declined]);
    assert_eq!(
        outcome.notes,
        [format!(
            "2 open deciding item(s) not walked: {asked_one}, {reported}; spec round new lists \
             them"
        )]
    );
    assert_eq!(status(&pair, &asked_one), "open");
    assert_eq!(status(&pair, &reported), "open");
}

// ------------------------------------------------------------------ rules

/// "Rules": the walk lists `open` rows once, at its start: one raised while a
/// question is shown waits for the next walk. M: listing again per item.
#[test]
fn the_walk_lists_once_at_its_start() {
    let pair = Pair::new("pw-once", "spec-a");
    let first = create(&pair, &pair.linked, R13, &record("R-13", ""));
    let second = create(&pair, &pair.linked, R14, &record("R-14", ""));
    let mut raised_late = None;
    let mut before = |index: usize| {
        if index == 0 {
            raised_late = Some(create(&pair, &pair.linked, R15, &record("R-15", "")));
        }
    };
    let mut script = Script::doing(&[Some(true), Some(true)], &mut before);
    let outcome = walked(&pair, &mut script);
    let questions = asked_in(&script.questions);
    let late = raised_late.expect("raised during the walk");
    assert_eq!(questions, [first.clone(), second.clone()]);
    assert_eq!(outcome.applied, [first, second]);
    assert!(outcome.left.is_empty(), "{outcome:?}");
    assert_eq!(status(&pair, &late), "open");

    let mut script = Script::new(&[Some(true)]);
    let outcome = walked(&pair, &mut script);
    assert_eq!(outcome.applied, [late]);
}

/// "Rules": an `approved` row is not walked and counts as nothing left;
/// `spec approve <ID>` completes it, as today. M: listing `approved` too.
#[test]
fn an_approved_row_is_not_walked() {
    let pair = Pair::new("pw-approved", "spec-a");
    let held = create(&pair, &pair.linked, R13, &record("R-13", ""));
    let open = create(&pair, &pair.linked, R14, &record("R-14", ""));
    pair.sql(&format!(
        "update proposals set status = 'approved', decided_by = '{DECIDER}', \
         decided_at = '{NOW}', decision_note = 'by hand', updated_at = '{NOW}' where id = '{held}'"
    ));
    let mut script = Script::new(&[Some(true)]);
    let outcome = walked(&pair, &mut script);
    assert_eq!(asked_in(&script.questions), [open.as_str()]);
    assert_eq!(outcome.applied, [open]);
    assert!(outcome.left.is_empty(), "{outcome:?}");
    assert_eq!(outcome.exit(), Exit::Answered);
    assert_eq!(status(&pair, &held), "approved");

    pair.approve_ok(&pair.main, &held);
    assert_eq!(status(&pair, &held), "applied");
}

/// "This repository's": a proposal of another repository of the same slug
/// (one database) is not asked and not touched. M: listing the project's.
#[test]
fn another_repositorys_proposal_is_not_walked() {
    let pair = Pair::new("pw-other", "spec-a");
    let other = pair.scratch.copy("spec-a", "other");
    pair.git.init(&other);
    pair.git.add_all(&other);
    pair.git.commit(&other, "the fixture");
    let theirs = create(&pair, &other, R15, &record("R-15", ""));
    let ours = create(&pair, &pair.linked, R13, &record("R-13", ""));
    let mut script = Script::new(&[Some(true), Some(true)]);
    let outcome = walked(&pair, &mut script);
    assert_eq!(asked_in(&script.questions), [ours.as_str()]);
    assert_eq!(outcome.applied, [ours]);
    assert!(outcome.left.is_empty(), "{outcome:?}");
    assert_eq!(status(&pair, &theirs), "open");
    assert!(!other.join(R15).exists(), "their worktree untouched");
}

/// "Applying kinds": an update raised on `main`, a create, an interpretation
/// and an amendment raised on `t1`: all four asked by ID number, each applied
/// where it was raised; from either worktree. M: walking update and create
/// alone.
#[test]
fn every_applying_kind_is_walked_each_where_it_was_raised() {
    for from_linked in [false, true] {
        let pair = Pair::new("pw-kinds", "spec-a");
        let on_main = pair.propose_edit(
            &pair.main,
            TARGET,
            "the sprint ends;",
            "the sprint ends at once;",
        );
        let created = create(&pair, &pair.linked, R13, &record("R-13", ""));
        let (hash, span) = pair.span(&pair.linked, "RULE-STAM-REGEN");
        let interpreted = pair.spec_piped(
            &pair.linked,
            &[
                "--json",
                "propose",
                "interpretation",
                "RULE-STAM-REGEN",
                "--base",
                &hash,
                "--decision",
                "DEC-0023",
                "--how",
                "Rest is any speed under 40 % of walking.",
                "--why",
                "DEC-0023 says rest; the rule names no speed.",
                "--text-file",
                "-",
                "--rationale",
                RATIONALE,
                "--author-role",
                "spec-author",
            ],
            edit(
                &span,
                "Base rate 10 units/s,",
                "Base rate 10 units/s at rest (under 40 % of walking),",
            )
            .as_bytes(),
        );
        interpreted.code(0);
        let interpreted = interpreted.json()["id"].as_str().expect("an ID").to_owned();
        let amendment = "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
                         [RULE-SPRINT-COST]\n---\n\n# The cost after dusk\n\n## Dusk {#AC-01}\n\n\
                         The cost doubles after dusk.\n";
        let amended = pair.spec_piped(
            &pair.linked,
            &[
                "--json",
                "propose",
                "amendment",
                "docs/features/dusk-cost.md",
                "--text-file",
                "-",
                "--rationale",
                RATIONALE,
                "--author-role",
                "spec-author",
            ],
            amendment.as_bytes(),
        );
        amended.code(0);
        let amended = amended.json()["id"].as_str().expect("an ID").to_owned();

        let ids = vec![on_main.clone(), created, interpreted, amended];
        let head_main = pair.rev(&pair.main, "main");
        let head_t1 = pair.rev(&pair.main, "t1");
        let cwd = if from_linked {
            &pair.linked
        } else {
            &pair.main
        };
        let mut script = Script::new(&[Some(true); 4]);
        let outcome = library_walk(&pair, cwd, pair.git_env(cwd), &mut script)
            .unwrap_or_else(|error| panic!("the walk: {error}"));
        assert_eq!(asked_in(&script.questions), ids);
        assert_eq!(outcome.applied, ids);
        assert_eq!(outcome.exit(), Exit::Answered);
        let onto = |branch: &str, from: &str| {
            pair.git_text(
                &pair.main,
                &["rev-list", "--count", &format!("{from}..{branch}")],
            )
        };
        assert_eq!(onto("main", &head_main), "1", "the update, on main");
        assert_eq!(onto("t1", &head_t1), "3", "the other three, on t1");
        assert_eq!(
            pair.proposal(&on_main).place.branch,
            "main",
            "where it was raised"
        );
    }
}

/// "Rules", the walk's order: a create and its dependant applied oldest
/// first leave no `ref-dangling` in `spec check`.
#[test]
fn the_order_applies_a_holder_before_its_dependant() {
    let pair = Pair::new("pw-order", "spec-a");
    create(&pair, &pair.linked, R13, &record("R-13", ""));
    let follows = record("R-14", "links: {follows: [R-13]}\n");
    create(&pair, &pair.linked, R14, &follows);
    let mut script = Script::new(&[Some(true), Some(true)]);
    let outcome = walked(&pair, &mut script);
    assert_eq!(outcome.applied.len(), 2, "{outcome:?}");
    let run = pair.spec_piped(&pair.linked, &["check"], b"");
    let report = format!("{}{}", run.stdout, run.stderr);
    assert!(
        !report
            .lines()
            .any(|line| line.contains("ref-dangling") && line.contains("R-13")),
        "{}",
        run.show()
    );
}

/// "Rules": a staged item shows its stage as `spec approve` does
/// (`decision-staging.md` "Terminal"): the staged line above the question,
/// the question marked; `n` keeps the stage; `y` applies with it.
#[test]
fn a_staged_item_shows_its_stage() {
    let pair = Pair::new("pw-staged", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        TARGET,
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let updated_at = pair.proposal(&id).updated_at;
    let staged = stage(
        &pair.env(&pair.main),
        &Globals::default(),
        &StageRequest {
            id: id.clone(),
            body: StageBody::Approve {
                option: None,
                answer: None,
                canon: None,
                editorial: None,
                note: Some("n".to_owned()),
                updated_at,
                actor: None,
            },
            now: STAGED.to_owned(),
            git: pair.git_env(&pair.main),
            actor: None,
        },
    )
    .unwrap_or_else(|error| panic!("stage: {error}"));
    assert_eq!(staged.cause, None, "{staged:?}");

    let run = walk(&pair, &pair.main, false, &["n"]);
    run.code(1);
    assert!(
        run.stderr
            .contains(&format!("staged {STAGED}: spec approve {id} --note \"n\"")),
        "{}",
        run.show()
    );
    assert!(run.stderr.contains(", staged)? [y/N]"), "{}", run.show());
    assert!(pair.proposal(&id).staged.is_some(), "the stage kept");
    assert_eq!(status(&pair, &id), "open");

    let run = walk(&pair, &pair.main, false, &["y"]);
    run.code(0);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.decision_note.as_deref(), Some("n"));
}

/// A row with a reject staged between two good items: a walk of `y`s cannot
/// confirm a reject, so it is never asked; it stands in `left` with its note,
/// its row and its stage as they were; the two good items apply; exit 1. M:
/// asking it anyway.
#[test]
fn a_row_with_a_reject_staged_is_never_asked_and_stands_in_left() {
    let pair = Pair::new("pw-reject-staged", "spec-a");
    let first = create(&pair, &pair.linked, R13, &record("R-13", ""));
    let held = create(&pair, &pair.linked, R14, &record("R-14", ""));
    let third = create(&pair, &pair.linked, R15, &record("R-15", ""));
    let updated_at = pair.proposal(&held).updated_at;
    let staged = stage(
        &pair.env(&pair.main),
        &Globals::default(),
        &StageRequest {
            id: held.clone(),
            body: StageBody::Reject {
                reason: "Not this one.".to_owned(),
                updated_at,
                actor: None,
            },
            now: STAGED.to_owned(),
            git: pair.git_env(&pair.main),
            actor: None,
        },
    )
    .unwrap_or_else(|error| panic!("stage: {error}"));
    assert_eq!(staged.cause, None, "{staged:?}");
    let row = pair.proposal(&held);
    let base = pair.rev(&pair.linked, "HEAD");

    let mut script = Script::new(&[Some(true), Some(true), Some(true)]);
    let outcome = walked(&pair, &mut script);

    assert_eq!(
        asked_in(&script.questions),
        [first.as_str(), third.as_str()]
    );
    assert_eq!(outcome.applied, [first.as_str(), third.as_str()]);
    assert_eq!(outcome.left, [held.as_str()]);
    assert_eq!(outcome.exit(), Exit::NotFound);
    let notes: Vec<&String> = outcome
        .notes
        .iter()
        .filter(|note| note.contains(&held) && note.contains("has a reject staged"))
        .collect();
    assert_eq!(notes.len(), 1, "{outcome:?}");
    assert_eq!(
        pair.proposal(&held),
        row,
        "its row and its stage as they were"
    );
    assert!(!pair.linked.join(R14).exists(), "no file");
    let commits = commits_since(&pair, &base);
    assert_eq!(commits.len(), 2, "{commits:?}");
    assert_own_commit(&pair, &first, &commits[0]);
    assert_own_commit(&pair, &third, &commits[1]);
}

/// A reject staged while another item's question is shown: the row is read
/// at its turn, not from the walk's list, so it is not asked either. M: the
/// stage read from the list.
#[test]
fn a_reject_staged_during_the_walk_is_not_asked_either() {
    let pair = Pair::new("pw-reject-late", "spec-a");
    let first = create(&pair, &pair.linked, R13, &record("R-13", ""));
    let held = create(&pair, &pair.linked, R14, &record("R-14", ""));
    let mut before = |index: usize| {
        if index == 0 {
            let updated_at = pair.proposal(&held).updated_at;
            let staged = stage(
                &pair.env(&pair.main),
                &Globals::default(),
                &StageRequest {
                    id: held.clone(),
                    body: StageBody::Reject {
                        reason: "Not this one.".to_owned(),
                        updated_at,
                        actor: None,
                    },
                    now: STAGED.to_owned(),
                    git: pair.git_env(&pair.main),
                    actor: None,
                },
            )
            .unwrap_or_else(|error| panic!("stage: {error}"));
            assert_eq!(staged.cause, None, "{staged:?}");
        }
    };
    let mut script = Script::doing(&[Some(true), Some(true)], &mut before);
    let outcome = walked(&pair, &mut script);
    assert_eq!(asked_in(&script.questions), [first.as_str()]);
    assert_eq!(outcome.applied, [first.as_str()]);
    assert_eq!(outcome.left, [held.as_str()]);
    assert_eq!(outcome.exit(), Exit::NotFound);
    assert!(
        outcome
            .notes
            .iter()
            .any(|note| note.contains(&held) && note.contains("has a reject staged")),
        "{outcome:?}"
    );
    assert_eq!(status(&pair, &held), "open");
}

/// The hook that runs after a commit in `pair`'s repository.
fn post_commit(pair: &Pair, body: &str) {
    let hooks = pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).expect("hooks");
    let path = hooks.join("post-commit");
    fs::write(&path, body).expect("the hook");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// "Rules": approve fails with an exit 2, no identity: the walk stops there
/// with exit 2, each earlier commit stands, nothing later is asked. The
/// identity comes from the repository's config, and a `post-commit` hook
/// takes it away after the first commit. M: the walk moves on.
#[test]
fn no_identity_stops_the_walk_and_the_earlier_commit_stands() {
    let pair = Pair::new("pw-identity", "spec-a");
    let ids = three_creates(&pair);
    let base = pair.rev(&pair.linked, "HEAD");
    for (key, value) in [
        ("user.name", "Config Owner"),
        ("user.email", "owner@example.invalid"),
        ("user.useConfigOnly", "true"),
    ] {
        pair.git.git(&pair.main, &["config", key, value]);
    }
    let git_program = pair.git.git_program().display().to_string();
    post_commit(
        &pair,
        &format!(
            "#!/bin/sh\n'{git_program}' config --local --unset user.name\n\
             '{git_program}' config --local --unset user.email\nexit 0\n"
        ),
    );
    let git = pair
        .git_env(&pair.main)
        .without_var("GIT_COMMITTER_NAME")
        .without_var("GIT_COMMITTER_EMAIL")
        .without_var("GIT_AUTHOR_NAME")
        .without_var("GIT_AUTHOR_EMAIL")
        .with_var("EMAIL", "");
    let mut script = Script::new(&[Some(true), Some(true), Some(true)]);
    let outcome = library_walk(&pair, &pair.main, git, &mut script);
    let message = cannot(&outcome, "no identity at the second item");
    assert!(message.contains("(step 7)"), "{message}");
    assert_eq!(asked_in(&script.questions), [ids[0].as_str()]);
    let commits = commits_since(&pair, &base);
    assert_eq!(commits.len(), 1, "{commits:?}");
    assert_own_commit(&pair, &ids[0], &commits[0]);
    assert_eq!(status(&pair, &ids[1]), "open");
    assert_eq!(status(&pair, &ids[2]), "open");
}

/// "Rules": approve fails with an exit 2, a queue that cannot be read: the
/// table goes away while the second question is shown; step 7 cannot run, the
/// walk stops with exit 2, the third is not asked, the first commit stands.
/// M: the walk moves on.
#[test]
fn a_queue_that_cannot_be_read_stops_the_walk_and_the_earlier_commit_stands() {
    let pair = Pair::new("pw-queue", "spec-a");
    let ids = three_creates(&pair);
    let base = pair.rev(&pair.linked, "HEAD");
    let mut before = |index: usize| {
        if index == 1 {
            pair.sql("alter table proposals rename to proposals_gone");
        }
    };
    let mut script = Script::doing(&[Some(true), Some(true), Some(true)], &mut before);
    let outcome = library_walk(&pair, &pair.main, pair.git_env(&pair.main), &mut script);
    let questions = asked_in(&script.questions);
    cannot(&outcome, "no table at the second item");
    assert_eq!(questions, ids[..2]);
    pair.sql("alter table proposals_gone rename to proposals");
    let commits = commits_since(&pair, &base);
    assert_eq!(commits.len(), 1, "{commits:?}");
    assert_own_commit(&pair, &ids[0], &commits[0]);
    assert_eq!(status(&pair, &ids[1]), "open");
    assert_eq!(status(&pair, &ids[2]), "open");
}

/// `git rev-list --reverse <base>..main`.
fn main_commits_since(pair: &Pair, base: &str) -> Vec<String> {
    pair.git_text(
        &pair.main,
        &["rev-list", "--reverse", &format!("{base}..main")],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// How the linked worktree stops being the place an item was raised in.
#[derive(Clone, Copy, Debug)]
enum Place {
    AnotherBranch,
    Detached,
    MergeInProgress,
    Gone,
}

impl Place {
    /// Spoils the linked worktree: what the reason then says.
    fn spoil(self, pair: &Pair) -> &'static str {
        match self {
            Self::AnotherBranch => {
                pair.git.git(&pair.linked, &["checkout", "-q", "-b", "t2"]);
                "is on `t2`"
            }
            Self::Detached => {
                pair.git.git(&pair.linked, &["checkout", "-q", "--detach"]);
                "HEAD is detached"
            }
            Self::MergeInProgress => {
                let line = |text: &str| {
                    let mut bytes = read(&pair.linked, "docs/spec/game.md");
                    bytes.extend_from_slice(text.as_bytes());
                    write(&pair.linked, "docs/spec/game.md", bytes);
                };
                pair.git
                    .git(&pair.linked, &["checkout", "-q", "-b", "side"]);
                line("Side line.\n");
                pair.commit_all(&pair.linked, "side");
                pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);
                line("Our line.\n");
                pair.commit_all(&pair.linked, "ours");
                let merge =
                    pair.git
                        .git_output(&pair.linked, &["merge", "-q", "--no-edit", "side"], &[]);
                assert!(!merge.status.success(), "the merge stops on a conflict");
                "merge is in progress"
            }
            Self::Gone => {
                let linked = pair.linked.to_str().expect("a UTF-8 scratch path");
                pair.git
                    .git(&pair.main, &["worktree", "remove", "--force", linked]);
                assert!(!pair.linked.exists(), "the worktree removed");
                "no longer exists"
            }
        }
    }
}

/// One proposal's own place failing, between two good items: the good two
/// applied, it left open with its note, its row as it was but for one
/// `proposal.apply_failed`, exit 1. M: stopping at a place's failure.
#[test]
fn a_failure_of_one_items_place_leaves_it_open_and_the_walk_moves_on() {
    for place in [
        Place::AnotherBranch,
        Place::Detached,
        Place::MergeInProgress,
        Place::Gone,
    ] {
        let context = format!("{place:?}");
        let pair = Pair::new("pw-place", "spec-a");
        let first = create(&pair, &pair.main, R13, &record("R-13", ""));
        let spoiled = pair.propose_edit(
            &pair.linked,
            TARGET,
            "the sprint ends;",
            "the sprint ends at once;",
        );
        let third = create(&pair, &pair.main, R14, &record("R-14", ""));
        let base = pair.rev(&pair.main, "main");
        let reason = place.spoil(&pair);
        let row = pair.proposal(&spoiled);

        let mut script = Script::new(&[Some(true), Some(true)]);
        let outcome = walked(&pair, &mut script);

        assert_eq!(
            asked_in(&script.questions),
            [first.clone(), third.clone()],
            "{context}: no question for it"
        );
        assert_eq!(outcome.applied, [first.clone(), third.clone()], "{context}");
        assert_eq!(outcome.left, [spoiled.as_str()], "{context}");
        assert_eq!(outcome.exit(), Exit::NotFound, "{context}");
        let opening = format!("`{spoiled}` left open: ");
        let notes: Vec<&String> = outcome
            .notes
            .iter()
            .filter(|note| note.starts_with(&opening))
            .collect();
        assert_eq!(notes.len(), 1, "{context}: {outcome:?}");
        assert!(notes[0].contains(reason), "{context}: {outcome:?}");
        assert!(
            script.told.iter().any(|line| line.contains(reason)),
            "{context}: {:?}",
            script.told
        );

        assert_eq!(pair.proposal(&spoiled), row, "{context}: its row unchanged");
        assert_eq!(
            pair.events_of(&spoiled),
            [
                ("proposal.created".to_owned(), None),
                ("proposal.apply_failed".to_owned(), Some(2)),
            ],
            "{context}"
        );
        let commits = main_commits_since(&pair, &base);
        assert_eq!(commits.len(), 2, "{context}: {commits:?}");
        assert_own_commit(&pair, &first, &commits[0]);
        assert_own_commit(&pair, &third, &commits[1]);
    }
}

/// The same through the binary on a terminal: an item on another branch
/// between two good ones: exit 1, and the note in the JSON and in the text.
#[test]
fn an_item_on_another_branch_is_left_on_a_terminal_too() {
    for json in [false, true] {
        let pair = Pair::new("pw-place-terminal", "spec-a");
        let first = create(&pair, &pair.main, R13, &record("R-13", ""));
        let spoiled = pair.propose_edit(
            &pair.linked,
            TARGET,
            "the sprint ends;",
            "the sprint ends at once;",
        );
        let third = create(&pair, &pair.main, R14, &record("R-14", ""));
        Place::AnotherBranch.spoil(&pair);
        let note = format!(
            "`{spoiled}` left open: the worktree {} is on `t2`; the proposal applies on `t1`: \
             check it out there",
            pair.linked.display()
        );

        let run = walk(&pair, &pair.main, json, &["y", "y"]);
        run.code(1);
        assert_eq!(
            asked(&run.stderr),
            [first.clone(), third.clone()],
            "{}",
            run.show()
        );
        if json {
            assert_eq!(
                run.json(),
                json!({"applied": [first, third], "left": [spoiled], "notes": [note]}),
                "{}",
                run.show()
            );
        } else {
            assert!(
                run.stdout.contains("open walk: 2 applied, 1 left open\n"),
                "{}",
                run.show()
            );
            assert!(run.stdout.contains(&note), "{}", run.show());
        }
    }
}

/// "Rules": approve fails with an exit 2 of the project: `specengine.toml`
/// goes bad while the first question is shown: that item's run has read it;
/// the next cannot, in a place that is fine: the walk stops with exit 2, the
/// first commit stands. M: every exit 2 taken for a place's.
#[test]
fn a_project_that_cannot_be_read_stops_the_walk_and_the_earlier_commit_stands() {
    let pair = Pair::new("pw-project", "spec-a");
    let ids = three_creates(&pair);
    let base = pair.rev(&pair.linked, "HEAD");
    let mut before = |index: usize| {
        if index == 0 {
            write(&pair.main, "specengine.toml", "not [ toml\n");
        }
    };
    let mut script = Script::doing(&[Some(true), Some(true), Some(true)], &mut before);
    let outcome = library_walk(&pair, &pair.main, pair.git_env(&pair.main), &mut script);
    let questions = asked_in(&script.questions);
    let message = cannot(&outcome, "no project at the second item");
    assert!(message.contains(&ids[1]), "{message}");
    assert_eq!(questions, ids[..1]);
    let commits = commits_since(&pair, &base);
    assert_eq!(commits.len(), 1, "{commits:?}");
    assert_own_commit(&pair, &ids[0], &commits[0]);
    assert_eq!(status(&pair, &ids[1]), "open");
    assert_eq!(status(&pair, &ids[2]), "open");
}

/// No consent, no commit: the end of input at the second question applies
/// neither it nor anything after it, and both stay open. The spec does not
/// say whether the rest are asked, so this test does not either.
#[test]
fn the_end_of_input_applies_nothing_unanswered() {
    let pair = Pair::new("pw-end", "spec-a");
    let ids = three_creates(&pair);
    let mut script = Script::new(&[Some(true), None, None]);
    let outcome = walked(&pair, &mut script);
    assert_eq!(outcome.applied, [ids[0].as_str()]);
    assert_eq!(outcome.left, ids[1..]);
    assert_eq!(outcome.exit(), Exit::NotFound);
    assert_eq!(status(&pair, &ids[1]), "open");
    assert_eq!(status(&pair, &ids[2]), "open");
    assert!(!pair.linked.join(R14).exists() && !pair.linked.join(R15).exists());
}
