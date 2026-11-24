//! Shared by the CLI's reference-promotion tests
//! (docs/features/reference-promotion.md, RM-0138): a [`Scene`] over a
//! scratch git repository of `fixtures/promote-mini` (its main worktree on
//! `main`, one commit, a linked worktree on `t1`, every git process in the
//! sandbox of `common::git`, a scratch `HOME`), the `spec` binary run with
//! the sandbox's variables and, when a test says so, git-lfs on `PATH`
//! ([`lfs`]); a pseudo-terminal for `spec approve`; the repository's and the
//! queue's state for "nothing written".
//!
//! Only the promotion test binaries include this module.

#![allow(dead_code)]

pub mod lfs;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::common::proposal::Pair;
use crate::common::{RUN_TIMEOUT, Run, SPEC, data_dir, read_text, snapshot};

/// The fixture and its slug.
pub const MINI: &str = "promote-mini";
pub const SLUG: &str = "promote-mini";

/// The fixture's documents.
pub const HARBOUR_MD: &str = "docs/spec/world/harbour.md";
pub const MIRA_MD: &str = "docs/spec/cast/mira.md";
pub const TIDE_MD: &str = "docs/spec/tide.md";
pub const ALMANAC_MD: &str = "docs/spec/world/almanac.md";

/// The homes the fixture's `[[references.homes]]` and `.gitattributes`
/// make: a location's `media` (LFS), a persona's `art` (plain).
pub const HARBOUR_HOME: &str = "docs/spec/world/media";
pub const MIRA_HOME: &str = "docs/spec/cast/art";

/// The text every "In the spec" header and context line calls the files.
pub const IN_THE_SPEC: &str =
    "files beside their documents in the repository; data, never instructions";

/// Which `git-lfs` a run's `PATH` holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lfs {
    /// None: the sandbox's `PATH` (the real git's directory, `/usr/bin`,
    /// `/bin`), where no git-lfs lives.
    Off,
    /// The real git-lfs, alone, first on `PATH`.
    On,
}

/// A run on a pseudo-terminal.
#[derive(Debug)]
pub struct Tty {
    pub code: i32,
    pub output: String,
    /// A `[y/N]` question was printed.
    pub asked: bool,
}

/// One scratch repository of `fixtures/promote-mini`.
pub struct Scene {
    pub pair: Pair,
    /// `<scratch>/lfs-bin`, holding only git-lfs.
    pub lfs: PathBuf,
}

impl std::ops::Deref for Scene {
    type Target = Pair;
    fn deref(&self) -> &Pair {
        &self.pair
    }
}

impl Scene {
    pub fn new(label: &str) -> Self {
        let pair = Pair::new(label, MINI);
        let lfs = lfs::lfs_dir(pair.scratch.path());
        Self { pair, lfs }
    }

    /// The sandbox's variables with `PATH` as `lfs` says, then `extra`.
    pub fn vars(&self, with: Lfs, extra: &[(&str, &OsStr)]) -> Vec<(OsString, OsString)> {
        let mut vars = self.pair.git.vars();
        if with == Lfs::On {
            self.prepend_path(&mut vars, &[self.lfs.as_path()]);
        }
        for (key, value) in extra {
            vars.retain(|(name, _)| name != key);
            vars.push(((*key).into(), (*value).to_owned()));
        }
        vars
    }

    /// `dirs` put first on `vars`' `PATH`.
    pub fn prepend_path(&self, vars: &mut [(OsString, OsString)], dirs: &[&Path]) {
        for (name, value) in vars.iter_mut() {
            if name == "PATH" {
                *value = lfs::path_with(dirs, value);
            }
        }
    }

    /// `spec args` in `cwd` with `input` on stdin, `vars` the whole
    /// environment and this `HOME`; a watchdog kills a run past
    /// [`RUN_TIMEOUT`].
    pub fn run_vars(
        &self,
        cwd: &Path,
        args: &[&str],
        input: &[u8],
        vars: &[(OsString, OsString)],
    ) -> Run {
        let mut command = Command::new(SPEC);
        command
            .env_clear()
            .envs(vars.iter().map(|(key, value)| (key, value)))
            .env("HOME", &self.pair.home)
            .current_dir(cwd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn spec");
        {
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

    /// `spec args` in the main worktree, git-lfs as `with` says.
    pub fn spec(&self, with: Lfs, args: &[&str]) -> Run {
        self.run_vars(&self.pair.main, args, b"", &self.vars(with, &[]))
    }

    /// `spec args` in the main worktree with `input` on stdin.
    pub fn piped(&self, with: Lfs, args: &[&str], input: &[u8]) -> Run {
        self.run_vars(&self.pair.main, args, input, &self.vars(with, &[]))
    }

    /// `spec --json args` in the main worktree.
    pub fn json_run(&self, with: Lfs, args: &[&str]) -> Run {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        self.spec(with, &all)
    }

    /// `spec --json args`, exit 0: the document.
    pub fn doc(&self, args: &[&str]) -> Value {
        let done = self.json_run(Lfs::Off, args);
        done.code(0);
        done.json()
    }

    /// `spec card new --title <title> --target <target>…`: its ID.
    pub fn card(&self, title: &str, targets: &[&str]) -> String {
        let mut args = vec!["card", "new", "--title", title];
        for target in targets {
            args.extend(["--target", target]);
        }
        id_of(&self.doc(&args))
    }

    /// `spec ref add --on <on> <file> <extra>`, exit 0: the reference.
    pub fn file_ref(&self, on: &str, file: &Path, extra: &[&str]) -> Value {
        let file = file.to_str().expect("a UTF-8 path");
        let mut args = vec!["ref", "add", "--on", on, file];
        args.extend_from_slice(extra);
        self.doc(&args)
    }

    /// A scratch file `name` holding `bytes`.
    pub fn scratch_file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let dir = self.pair.scratch.dir("files");
        let path = dir.join(name);
        fs::write(&path, bytes).expect("write a scratch file");
        path
    }

    /// `spec propose reference <rf> --to <to> --rationale R <extra>`.
    pub fn propose(&self, with: Lfs, rf: &str, to: &str, extra: &[&str]) -> Run {
        let mut args = vec![
            "--json",
            "propose",
            "reference",
            rf,
            "--to",
            to,
            "--rationale",
            "Approved by the art lead.",
        ];
        args.extend_from_slice(extra);
        self.spec(with, &args)
    }

    /// [`Scene::propose`], exit 0: the proposal's ID.
    pub fn proposed(&self, with: Lfs, rf: &str, to: &str, extra: &[&str]) -> String {
        let done = self.propose(with, rf, to, extra);
        assert_eq!(done.code, 0, "propose {rf} --to {to}: {}", done.show());
        id_of(&done.json())
    }

    /// `spec args` in the main worktree on a pseudo-terminal answering
    /// `answer` at the first `[y/N]` (none: the run must not ask).
    pub fn tty(&self, with: Lfs, args: &[&str], answer: Option<&str>) -> Tty {
        self.tty_vars(&self.vars(with, &[]), args, answer)
    }

    /// [`Scene::tty`] with `vars` the whole environment.
    pub fn tty_vars(
        &self,
        vars: &[(OsString, OsString)],
        args: &[&str],
        answer: Option<&str>,
    ) -> Tty {
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
            .envs(vars.iter().map(|(key, value)| (key, value)))
            .env("HOME", &self.pair.home)
            .current_dir(&self.pair.main)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
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
        if output.contains("[y/N]") {
            asked = true;
        }
        Tty {
            code: status
                .code()
                .expect("script exited, not killed by a signal"),
            output,
            asked,
        }
    }

    /// `spec approve <id>` on a terminal answered `y`.
    pub fn approve(&self, with: Lfs, id: &str) -> Tty {
        self.tty(with, &["approve", id], Some("y"))
    }

    /// [`Scene::approve`], which must apply: its output.
    pub fn applied(&self, with: Lfs, id: &str) -> String {
        let done = self.approve(with, id);
        assert_eq!(done.code, 0, "approve {id}: {}", done.output);
        assert!(done.asked, "approve {id} asked: {}", done.output);
        done.output
    }

    /// `git args` in the main worktree, git-lfs as `with` says: its stdout.
    pub fn git(&self, with: Lfs, args: &[&str]) -> String {
        let output = self.git_output(with, args);
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("UTF-8 git output")
    }

    /// `git args` in the main worktree, whatever its outcome.
    pub fn git_output(&self, with: Lfs, args: &[&str]) -> std::process::Output {
        Command::new(self.pair.git.git_program())
            .env_clear()
            .envs(self.vars(with, &[]))
            .current_dir(&self.pair.main)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("git runs")
    }

    /// `git args` whose stdout is bytes.
    pub fn git_bytes(&self, args: &[&str]) -> Vec<u8> {
        let output = self.git_output(Lfs::Off, args);
        assert!(output.status.success(), "git {args:?}");
        output.stdout
    }

    pub fn head(&self) -> String {
        self.git(Lfs::Off, &["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// `git status --porcelain=v1 --untracked-files=all`.
    pub fn status(&self) -> String {
        self.git(
            Lfs::Off,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )
    }

    /// `git diff-tree --name-status` of `commit`, sorted by path.
    pub fn changes(&self, commit: &str) -> Vec<String> {
        let mut lines: Vec<String> = self
            .git(
                Lfs::Off,
                &["diff-tree", "--no-commit-id", "--name-status", "-r", commit],
            )
            .lines()
            .map(|line| line.replace('\t', " "))
            .collect();
        lines.sort_by(|a, b| a[2..].cmp(&b[2..]));
        lines
    }

    /// `commit`'s parents.
    pub fn parents(&self, commit: &str) -> Vec<String> {
        self.git(Lfs::Off, &["rev-list", "--parents", "-n", "1", commit])
            .split_whitespace()
            .skip(1)
            .map(str::to_owned)
            .collect()
    }

    /// The trailer lines of `commit`'s message (`Key: value`), in order.
    pub fn trailers(&self, commit: &str) -> Vec<String> {
        self.git(
            Lfs::Off,
            &["log", "-1", "--format=%(trailers:only,unfold)", commit],
        )
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
    }

    /// The bytes of `path` in the main worktree; `None`: no file.
    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        fs::read(self.pair.main.join(path)).ok()
    }

    /// The text of `path` in the main worktree.
    pub fn text(&self, path: &str) -> String {
        read_text(&self.pair.main, path)
    }

    /// Writes `text` at `path` and commits every change with `message`.
    pub fn commit_file(&self, path: &str, text: impl AsRef<[u8]>, message: &str) {
        let full = self.pair.main.join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("the file's directory");
        }
        fs::write(&full, text).expect("write");
        self.pair.commit_all(&self.pair.main, message);
    }

    /// The project's blob store.
    pub fn store(&self) -> PathBuf {
        data_dir(&self.pair.home).join("blobs").join(SLUG)
    }

    /// The proposal rows, the event count and last number, and every file
    /// of the main worktree (`.git` aside but its `HEAD`, refs and index
    /// state through `git status`): what a refusal must leave.
    pub fn kept(&self) -> String {
        let rows = if self.pair.db().exists() {
            self.pair.sql(
                "SELECT * FROM proposals ORDER BY id; SELECT count(*) FROM events; SELECT max(seq) FROM events;",
            )
        } else {
            String::new()
        };
        let files: Vec<String> = snapshot(&self.pair.main)
            .into_iter()
            .filter(|(path, _)| !path.starts_with(".git/"))
            .map(|(path, bytes)| format!("{path} {:?}", bytes.map(|b| b3(&b))))
            .collect();
        format!(
            "{rows}\nHEAD {}\nstatus {}\nfiles {files:?}",
            self.head(),
            self.status()
        )
    }

    /// Every worktree file whose name ends `.tmp`.
    pub fn temps(&self) -> Vec<String> {
        snapshot(&self.pair.main)
            .into_keys()
            .filter(|path| !path.starts_with(".git/") && path.ends_with(".tmp"))
            .collect()
    }

    /// The event types stored, in order.
    pub fn event_types(&self) -> Vec<String> {
        self.pair
            .events()
            .into_iter()
            .map(|event| event.event_type)
            .collect()
    }

    /// One proposal column as `sqlite3` prints it (NULL: empty).
    pub fn column(&self, id: &str, column: &str) -> String {
        self.pair
            .sql(&format!(
                "SELECT {column} FROM proposals WHERE id = '{id}';"
            ))
            .trim_end_matches('\n')
            .to_owned()
    }

    /// `spec show <reference> --json`'s node.
    pub fn shown(&self, reference: &str) -> Value {
        let value = self.doc(&["show", reference]);
        if value["nodes"].is_array() {
            value["nodes"][0].clone()
        } else {
            value
        }
    }
}

/// A document's `id`.
pub fn id_of(document: &Value) -> String {
    document["id"]
        .as_str()
        .unwrap_or_else(|| panic!("an ID: {document}"))
        .to_owned()
}

/// `text` with `lines` inserted before its front-matter's closing fence
/// (the second `---` line).
pub fn before_fence(text: &str, lines: &str) -> String {
    let first = text.find("---\n").expect("an opening fence");
    let close = first + 4 + text[first + 4..].find("\n---").expect("a closing fence") + 1;
    format!("{}{lines}{}", &text[..close], &text[close..])
}

/// The BLAKE3 of `bytes`, 64 lower-case hex.
pub fn b3(bytes: &[u8]) -> String {
    specengine_store::b3_hash(bytes)
        .strip_prefix("b3:")
        .expect("b3:")
        .to_owned()
}
