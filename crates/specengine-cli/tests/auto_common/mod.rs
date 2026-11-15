//! Helpers of the auto-mode CLI tests (docs/features/auto-rule.md,
//! auto-land.md, auto-runner.md, auto-feed.md "Acceptance criteria"): a
//! [`Pair`] of `fixtures/auto-mini` (its main worktree on `main`, a linked
//! one on `t1` made at the fixture's commit, every git process in the
//! sandbox of `common::git`, a scratch `HOME`), the rule variants of
//! `rules/` appended to its config and committed, the `spec` binary with
//! the sandbox's variables (stdin null, piped, or a pseudo-terminal), and
//! the queue's events read raw with the system's `sqlite3` (the key order
//! of a payload is a subject).
//!
//! Only the auto-mode test binaries include this module (`mod
//! auto_common;`), so the other CLI test binaries are not rebuilt.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;
use specengine_core::auto_toml::AutoRule;
use specengine_store::{AutoEvent, AutoStarted, SqliteQueue};

use crate::common::proposal::Pair;
use crate::common::{RUN_TIMEOUT, Run, SPEC, data_dir, fixture, read_text, write};

/// The fixture's slug.
pub const SLUG: &str = "auto-mini";

/// The sandbox's committer, as `by` and `Decided-by` read it.
pub const COMMITTER: &str = "Scratch Committer <committer@example.invalid>";

/// `rules/<name>` of the fixture, as written.
pub fn rule_text(name: &str) -> String {
    fs::read_to_string(fixture("auto-mini").join("rules").join(name))
        .unwrap_or_else(|e| panic!("rules/{name}: {e}"))
}

/// `config` with `table` appended after one blank line.
pub fn with_table(config: &str, table: &str) -> String {
    format!("{}\n\n{table}", config.trim_end())
}

/// The fixture's `expected.json`.
pub fn expected() -> Value {
    serde_json::from_str(
        &fs::read_to_string(fixture("auto-mini").join("expected.json")).expect("expected.json"),
    )
    .expect("expected.json is JSON")
}

/// One stored event, raw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEvent {
    pub seq: i64,
    pub kind: String,
    pub payload: String,
    pub at: String,
}

impl RawEvent {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.payload).unwrap_or_else(|e| panic!("{self:?}: {e}"))
    }
}

/// A `spec` run on a pseudo-terminal.
#[derive(Debug, Clone)]
pub struct PtyRun {
    pub args: Vec<String>,
    pub code: i32,
    /// stdout and stderr as the terminal showed them, CRLF made LF.
    pub output: String,
    /// A `[y/N]` prompt was seen.
    pub asked: bool,
}

impl PtyRun {
    pub fn code(&self, code: i32) -> &Self {
        assert_eq!(
            self.code, code,
            "spec {:?} on a terminal:\n{}",
            self.args, self.output
        );
        self
    }
}

/// The auto-mini repository of one test.
pub struct Auto {
    pub pair: Pair,
}

impl std::ops::Deref for Auto {
    type Target = Pair;
    fn deref(&self) -> &Pair {
        &self.pair
    }
}

impl Auto {
    /// The fixture as committed, no `[auto]` table.
    pub fn new(label: &str) -> Self {
        Self {
            pair: Pair::new(label, "auto-mini"),
        }
    }

    /// The fixture with `rules/<rule>` committed on `main`.
    pub fn with_rule(label: &str, rule: &str) -> Self {
        let auto = Self::new(label);
        auto.commit_rule(rule);
        auto
    }

    /// The main worktree's `specengine.toml`.
    pub fn config(&self) -> String {
        read_text(&self.main, "specengine.toml")
    }

    /// Writes `text` as the main worktree's `specengine.toml` (uncommitted).
    pub fn write_config(&self, text: &str) {
        write(&self.main, "specengine.toml", text);
    }

    /// `rules/<name>` appended to the main worktree's config, committed.
    pub fn commit_rule(&self, name: &str) {
        self.commit_table(&rule_text(name), &format!("auto: {name}"));
    }

    /// `table` appended to the main worktree's config, committed.
    pub fn commit_table(&self, table: &str, message: &str) {
        self.write_config(&with_table(&self.config(), table));
        self.commit_all(&self.main, message);
    }

    /// The config's `[auto]` table removed (up to the next table or the
    /// end), committed.
    pub fn commit_without_auto(&self, message: &str) {
        self.write_config(&without_table(&self.config(), "[auto]"));
        self.commit_all(&self.main, message);
    }

    /// A `[team]` of `logins` (`(login, role)`, a lead's zone `docs`)
    /// appended and committed.
    pub fn commit_team(&self, logins: &[(&str, &str)]) {
        let mut table = String::new();
        for (login, role) in logins {
            table.push_str(&format!(
                "[team.{login}]\nrole  = \"{role}\"\nemail = \"{login}@example.org\"\n"
            ));
            if *role == "lead" {
                table.push_str("zones = [\"docs\"]\n");
            }
            table.push('\n');
        }
        self.commit_table(&table, "team");
    }

    /// The data directory.
    pub fn data(&self) -> PathBuf {
        data_dir(&self.home)
    }

    /// The stop file's path.
    pub fn stop_file(&self) -> PathBuf {
        self.data().join("auto-stop")
    }

    /// `spec args` in `cwd`, stdin null, the sandbox's variables, this
    /// `HOME` and `extra`.
    pub fn run(&self, cwd: &Path, args: &[&str], extra: &[(&str, &OsStr)]) -> Run {
        self.run_with(cwd, args, extra, None)
    }

    /// [`Self::run`] in the main worktree.
    pub fn spec(&self, args: &[&str]) -> Run {
        self.run(&self.main, args, &[])
    }

    /// `spec --json args` in the main worktree, which must exit `code`:
    /// its one JSON document.
    pub fn json(&self, args: &[&str], code: i32) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        let run = self.spec(&all);
        run.code(code);
        run.json()
    }

    /// `spec args` with `input` on a piped stdin.
    pub fn piped(&self, cwd: &Path, args: &[&str], input: &[u8]) -> Run {
        self.run_with(cwd, args, &[], Some(input))
    }

    fn run_with(
        &self,
        cwd: &Path,
        args: &[&str],
        extra: &[(&str, &OsStr)],
        input: Option<&[u8]>,
    ) -> Run {
        let mut command = Command::new(SPEC);
        command
            .env_clear()
            .envs(self.git.vars())
            .env("HOME", &self.home)
            .current_dir(cwd)
            .args(args)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in extra {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("spawn spec");
        if let Some(input) = input {
            let mut stdin = child.stdin.take().expect("stdin");
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

    /// `spec args` in `cwd` with stdin on a pseudo-terminal (BSD or
    /// util-linux `script`), the sandbox's variables, this `HOME` and
    /// `extra`; at the first `[y/N]` the terminal answers `answer` (none:
    /// nothing is typed). A watchdog kills a run past [`RUN_TIMEOUT`].
    pub fn pty(
        &self,
        cwd: &Path,
        args: &[&str],
        answer: Option<&str>,
        extra: &[(&str, &OsStr)],
    ) -> PtyRun {
        let script = ["/usr/bin/script", "/bin/script"]
            .into_iter()
            .find(|path| Path::new(path).exists())
            .expect("a `script` program gives `spec` a terminal");
        let mut command = Command::new(script);
        if cfg!(target_os = "macos") {
            command.arg("-q").arg("/dev/null").arg(SPEC).args(args);
        } else {
            let quoted: Vec<String> = std::iter::once(SPEC)
                .chain(args.iter().copied())
                .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
                .collect();
            command.arg("-qec").arg(quoted.join(" ")).arg("/dev/null");
        }
        command
            .env_clear()
            .envs(self.git.vars())
            .env("HOME", &self.home)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in extra {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("spawn script");
        let mut stdin = child.stdin.take().expect("stdin");
        let mut stdout = child.stdout.take().expect("stdout");
        let (sender, chunks) = mpsc::channel::<Vec<u8>>();
        let reader = std::thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match stdout.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if sender.send(buffer[..read].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let mut output = Vec::new();
        let mut asked = false;
        let started = Instant::now();
        let status = loop {
            while let Ok(chunk) = chunks.try_recv() {
                output.extend_from_slice(&chunk);
            }
            if !asked && String::from_utf8_lossy(&output).contains("[y/N]") {
                asked = true;
                if let Some(answer) = answer {
                    let _ = stdin.write_all(format!("{answer}\n").as_bytes());
                    let _ = stdin.flush();
                }
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
        reader.join().expect("the reader");
        while let Ok(chunk) = chunks.try_recv() {
            output.extend_from_slice(&chunk);
        }
        let output = String::from_utf8(output)
            .expect("UTF-8 terminal output")
            .replace("\r\n", "\n");
        if !asked && output.contains("[y/N]") {
            asked = true;
        }
        PtyRun {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            code: status
                .code()
                .expect("script exited, not killed by a signal"),
            output,
            asked,
        }
    }

    /// Every event of the queue, raw, by `seq` (none without a database).
    pub fn raw_events(&self) -> Vec<RawEvent> {
        if !self.db().exists() {
            return Vec::new();
        }
        self.sql(
            "SELECT seq || char(9) || type || char(9) || payload || char(9) || at \
             FROM events ORDER BY seq",
        )
        .lines()
        .map(|line| {
            let mut parts = line.splitn(4, '\t');
            RawEvent {
                seq: parts.next().unwrap().parse().expect("a seq"),
                kind: parts.next().unwrap().to_owned(),
                payload: parts.next().unwrap().to_owned(),
                at: parts.next().unwrap().to_owned(),
            }
        })
        .collect()
    }

    /// The `auto.*` events, raw.
    pub fn auto_events(&self) -> Vec<RawEvent> {
        self.raw_events()
            .into_iter()
            .filter(|event| event.kind.starts_with("auto."))
            .collect()
    }

    /// The events of `kind`, raw.
    pub fn events_of_kind(&self, kind: &str) -> Vec<RawEvent> {
        self.raw_events()
            .into_iter()
            .filter(|event| event.kind == kind)
            .collect()
    }

    /// The repository as `auto.started` names it: the main worktree's
    /// canonical git common dir.
    pub fn repository(&self) -> String {
        fs::canonicalize(self.main.join(".git"))
            .expect("the git dir")
            .to_str()
            .expect("UTF-8")
            .to_owned()
    }

    /// Every ref and its object, `git for-each-ref` in the main worktree.
    pub fn refs(&self) -> BTreeMap<String, String> {
        self.git_text(
            &self.main,
            &["for-each-ref", "--format=%(refname) %(objectname)"],
        )
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(name, oid)| (name.to_owned(), oid.to_owned()))
        .collect()
    }

    /// `git log -1 --format=%B <rev>` in `dir`.
    pub fn message(&self, dir: &Path, rev: &str) -> String {
        String::from_utf8(self.git.git(dir, &["log", "-1", "--format=%B", rev])).expect("UTF-8")
    }

    /// The span hash of `reference` as `spec show --json` prints it.
    pub fn span_hash(&self, cwd: &Path, reference: &str) -> String {
        let run = self.run(cwd, &["--json", "show", reference], &[]);
        run.code(0);
        let value = run.json();
        let node = if value["nodes"].is_array() {
            value["nodes"][0].clone()
        } else {
            value
        };
        node["span_hash"]
            .as_str()
            .unwrap_or_else(|| panic!("no span_hash for {reference}: {node}"))
            .to_owned()
    }

    /// The span text of `reference` (its heading to its end) in `cwd`'s
    /// file, as `spec show` prints it after its header line.
    pub fn span_text(&self, cwd: &Path, reference: &str) -> String {
        let run = self.run(cwd, &["show", reference], &[]);
        run.code(0);
        let (_, body) = run.stdout.split_once('\n').expect("a header line");
        body.to_owned()
    }

    /// `spec propose update <reference>` from `cwd`: `from` replaced by `to`
    /// in its span, raised with `extra` arguments (`--task T`, an author);
    /// its ID.
    pub fn propose_update(
        &self,
        cwd: &Path,
        reference: &str,
        from: &str,
        to: &str,
        extra: &[&str],
    ) -> String {
        let base = self.span_hash(cwd, reference);
        let text = self.span_text(cwd, reference);
        assert_eq!(text.matches(from).count(), 1, "{from:?} in {text:?}");
        let new = text.replacen(from, to, 1);
        let mut args = vec![
            "--json",
            "propose",
            "update",
            reference,
            "--base",
            &base,
            "--text-file",
            "-",
            "--rationale",
            "The test's edit.",
        ];
        args.extend_from_slice(extra);
        let run = self.piped(cwd, &args, new.as_bytes());
        run.code(0);
        run.json()["id"]
            .as_str()
            .unwrap_or_else(|| panic!("no id: {}", run.show()))
            .to_owned()
    }

    /// `spec propose create <path>` of a new file from `cwd`; its ID.
    pub fn propose_new_file(&self, cwd: &Path, path: &str, text: &str, extra: &[&str]) -> String {
        let mut args = vec![
            "--json",
            "propose",
            "create",
            path,
            "--text-file",
            "-",
            "--rationale",
            "A new file.",
        ];
        args.extend_from_slice(extra);
        let run = self.piped(cwd, &args, text.as_bytes());
        run.code(0);
        run.json()["id"]
            .as_str()
            .unwrap_or_else(|| panic!("no id: {}", run.show()))
            .to_owned()
    }
}

impl Auto {
    /// The rule at the main worktree's `HEAD`, read by the core.
    pub fn head_rule(&self) -> AutoRule {
        let config = self.git_text(&self.main, &["show", "HEAD:specengine.toml"]);
        AutoRule::from_toml(config.as_bytes(), "specengine.toml")
            .rule
            .expect("a rule at HEAD")
    }

    /// The queue, opened through the store.
    pub fn store(&self) -> SqliteQueue {
        SqliteQueue::open(self.db(), SLUG).expect("the queue opens")
    }

    /// A pack of `tasks` and `lines` recorded through the store (no
    /// terminal): `auto.started` with `HEAD`'s rule frozen, the branch at
    /// `HEAD`, `by` the sandbox's committer, at `now`. Its `seq`.
    pub fn record_pack(
        &self,
        tasks: &[&str],
        lines: &[&str],
        until: Option<&str>,
        now: &str,
    ) -> i64 {
        self.record_pack_of(&self.head_rule(), tasks, lines, until, now)
    }

    /// [`Self::record_pack`] with `rule` frozen (a pack started under a
    /// rule `HEAD` no longer holds).
    pub fn record_pack_of(
        &self,
        rule: &AutoRule,
        tasks: &[&str],
        lines: &[&str],
        until: Option<&str>,
        now: &str,
    ) -> i64 {
        if !self.db().exists() {
            self.spec(&["index"]).code(0);
        }
        let head = self.rev(&self.main, "HEAD");
        let started = AutoStarted {
            repository: self.repository(),
            tasks: tasks.iter().map(|t| (*t).to_owned()).collect(),
            lines: lines.iter().map(|t| (*t).to_owned()).collect(),
            until: until.map(str::to_owned),
            branch: rule.branch.clone(),
            branch_at: head,
            rule: rule.hash.clone(),
            frozen: rule.canonical_form(),
            by: COMMITTER.to_owned(),
        };
        self.store()
            .record_auto(&AutoEvent::Started(started), now)
            .expect("the pack is recorded")
    }

    /// `spec task new --nodes …` in `cwd` with `extra` variables: its ID.
    pub fn task_new(
        &self,
        cwd: &Path,
        nodes: &[&str],
        title: &str,
        extra: &[(&str, &OsStr)],
    ) -> Run {
        let mut args = vec!["--json", "task", "new", "--title", title, "--nodes"];
        args.extend_from_slice(nodes);
        self.run(cwd, &args, extra)
    }

    /// A `draft` task on `nodes`, made through the binary: its ID.
    pub fn make_task(&self, nodes: &[&str], title: &str) -> String {
        let run = self.task_new(&self.main, nodes, title, &[]);
        run.code(0);
        run.json()["id"].as_str().expect("an ID").to_owned()
    }

    /// `spec task plan T` with a one-line plan and `affected` nodes: the
    /// task goes to `review`.
    pub fn plan_task(&self, id: &str, affected: &[&str]) {
        let mut args = vec!["task", "plan", id, "--plan-file", "-"];
        for node in affected {
            args.push("--affected");
            args.push(node);
        }
        let run = self.piped(&self.main, &args, b"1. Change it.\n");
        run.code(0);
    }
}

impl Auto {
    /// The rule's branch made at the main worktree's `HEAD`, as `spec auto
    /// start` makes it.
    pub fn make_branch(&self, branch: &str) {
        self.git.git(&self.main, &["branch", branch, "HEAD"]);
    }

    /// A pack of `tasks` recorded through the store, the branch `auto`
    /// made at `HEAD` (when absent), started a minute ago: its `seq`.
    pub fn pack(&self, tasks: &[&str], lines: &[&str]) -> i64 {
        if !self.refs().contains_key("refs/heads/auto") {
            self.make_branch("auto");
        }
        self.record_pack(tasks, lines, None, &utc(-60))
    }

    /// A planned (`review`) task on `nodes`, `affected` too: its ID.
    pub fn planned(&self, nodes: &[&str], title: &str, affected: &[&str]) -> String {
        let id = self.make_task(nodes, title);
        self.plan_task(&id, affected);
        id
    }

    /// The task's worktree, `<scratch>/wt/<T>` on `auto-<T>` from `auto`,
    /// made when absent, as the runner makes it.
    pub fn task_worktree(&self, task: &str) -> PathBuf {
        let dir = self.scratch.join("wt").join(task);
        if !dir.exists() {
            fs::create_dir_all(dir.parent().unwrap()).expect("wt/");
            self.git.git(
                &self.main,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    &format!("auto-{task}"),
                    dir.to_str().unwrap(),
                    "auto",
                ],
            );
        }
        fs::canonicalize(&dir).expect("the task's worktree")
    }

    /// `spec auto approve T --root <its worktree>`.
    pub fn auto_approve(&self, task: &str) -> Run {
        let wt = self.task_worktree(task);
        self.run(
            &wt,
            &["auto", "approve", task, "--root", wt.to_str().unwrap()],
            &[],
        )
    }

    /// The task claimed in its worktree.
    pub fn claim_in(&self, task: &str) -> PathBuf {
        let wt = self.task_worktree(task);
        self.run(
            &wt,
            &[
                "task",
                "claim",
                task,
                "--role",
                "developer",
                "--worktree",
                wt.to_str().unwrap(),
            ],
            &[],
        )
        .code(0);
        wt
    }

    /// The task's run reported `outcome` and the task completed, from its
    /// worktree.
    pub fn finish(&self, task: &str, outcome: &str, summary: &str) {
        let wt = self.task_worktree(task);
        self.run(
            &wt,
            &[
                "task",
                "report",
                task,
                "--outcome",
                outcome,
                "--summary",
                summary,
                "--changed",
                "src/lib.rs",
            ],
            &[],
        )
        .code(0);
        self.run(&wt, &["task", "complete", task], &[]).code(0);
    }

    /// A pack task taken from `review` to `done`: approved by rule,
    /// claimed, `edit` applied to its worktree (`(path, from, to)`,
    /// uncommitted), reported completed and completed. Its worktree.
    pub fn to_done(&self, task: &str, edits: &[(&str, &str, &str)]) -> PathBuf {
        self.auto_approve(task).code(0);
        let wt = self.claim_in(task);
        for (path, from, to) in edits {
            crate::common::replace(&wt, path, from, to);
        }
        self.finish(task, "completed", "The timer resets on close.");
        wt
    }

    /// `spec auto land T` from the main worktree.
    pub fn land(&self, task: &str) -> Run {
        self.spec(&["auto", "land", task])
    }

    /// `spec auto next args`: its one JSON document (exit 0).
    pub fn next(&self, args: &[&str]) -> Value {
        let mut all = vec!["auto", "next"];
        all.extend_from_slice(args);
        let run = self.spec(&all);
        run.code(0);
        run.json()
    }

    /// The task's state, `spec task show --json`'s `status`.
    pub fn status(&self, task: &str) -> String {
        let run = self.spec(&["--json", "task", "show", task]);
        run.code(0);
        run.json()["status"].as_str().expect("a status").to_owned()
    }
}

/// Now, UTC, `YYYY-MM-DDTHH:MM:SSZ`, `offset` seconds away.
pub fn utc(offset: i64) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after the epoch")
        .as_secs() as i64
        + offset;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// `config` without the table whose header line is `header` (up to the
/// next table header or the end).
pub fn without_table(config: &str, header: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in config.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == header;
        }
        if !inside {
            out.push_str(line);
        }
    }
    format!("{}\n", out.trim_end())
}

/// `text` with every 40-hex commit ID and every 12-hex prefix of one
/// replaced by `<c1>`, `<c2>`, … in order of first appearance, and `root`
/// by `<root>`: two runs on two scratch repositories compared.
pub fn normalized(text: &str, roots: &[&Path]) -> String {
    let mut text = text.to_owned();
    for root in roots {
        text = text.replace(root.to_str().expect("UTF-8"), "<root>");
    }
    let bytes = text.as_bytes();
    let mut ids: Vec<String> = Vec::new();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let run = bytes[i..]
            .iter()
            .take_while(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
            .count();
        let bounded = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        if bounded && (run == 40 || run == 12 || run == 64) {
            let id = &text[i..i + run];
            let key = &id[..12];
            let n = ids
                .iter()
                .position(|known| known == key)
                .unwrap_or_else(|| {
                    ids.push(key.to_owned());
                    ids.len() - 1
                });
            out.push_str(&format!("<c{}:{run}>", n + 1));
            i += run;
        } else if run > 0 {
            out.push_str(&text[i..i + run]);
            i += run;
        } else {
            let c = text[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}
