//! Shared helpers of the `spec` CLI tests (docs/features/spec-cli.md):
//! scratch copies of `fixtures/spec-a` and `fixtures/spec-b` under
//! `std::env::temp_dir()` (the fixtures are only read), one spawned `spec`
//! process per call with a cleared environment and its own `HOME` inside the
//! scratch (never the real one, never `XDG_DATA_HOME`), a watchdog on every
//! run, and whole-tree snapshots for the read-only checks.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![allow(dead_code)]

pub mod bundle;
pub mod check;
pub mod git;
pub mod graph;
pub mod proposal;
pub mod staged;

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

/// The binary under test; nothing else is ever spawned.
pub const SPEC: &str = env!("CARGO_BIN_EXE_spec");

/// A `spec` run longer than this is killed and the test fails.
pub const RUN_TIMEOUT: Duration = Duration::from_secs(120);

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

pub fn fixture(name: &str) -> PathBuf {
    repository_root().join("fixtures").join(name)
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique() -> usize {
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// A scratch directory under the system temp dir, canonical, removed on
/// drop (permissions restored first).
pub struct Scratch {
    root: PathBuf,
}

impl Scratch {
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-cli-{name}-{}-{}",
            std::process::id(),
            unique()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self {
            root: fs::canonicalize(&path).expect("canonical scratch directory"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn join(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// A new, empty `HOME` of its own at `<scratch>/homes/<name>`.
    pub fn home(&self, name: &str) -> PathBuf {
        let home = self.root.join("homes").join(name);
        fs::create_dir_all(&home).expect("home directory");
        home
    }

    /// A copy of `fixtures/<name>` at `<scratch>/<dir>`, canonical.
    pub fn copy(&self, name: &str, dir: &str) -> PathBuf {
        let root = self.root.join(dir);
        copy_dir(&fixture(name), &root);
        fs::canonicalize(&root).expect("copy exists")
    }

    /// An empty directory `<scratch>/<dir>`, canonical.
    pub fn dir(&self, dir: &str) -> PathBuf {
        let path = self.root.join(dir);
        fs::create_dir_all(&path).expect("directory");
        fs::canonicalize(&path).expect("canonical directory")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        restore_permissions(&self.root);
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn restore_permissions(dir: &Path) {
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            restore_permissions(&path);
        } else {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
        }
    }
}

/// Copies every directory and regular file under `from` to `to`, in
/// directory-listing order.
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("copy target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("readable fixture directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let kind = entry.file_type().expect("file type");
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target);
        } else if kind.is_file() {
            copy_file(entry.path(), &target);
        }
    }
}

/// Copies the file `from` to `to`: its bytes and its permission bits (a
/// copied hook stays executable). Not `fs::copy`: on macOS it clones first
/// (`fclonefileat`), which a sandboxed run refuses with `EPERM` without
/// falling back, and it carries extended attributes along.
pub fn copy_file(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    let (from, to) = (from.as_ref(), to.as_ref());
    let bytes = fs::read(from).unwrap_or_else(|error| panic!("{}: {error}", from.display()));
    fs::write(to, bytes).unwrap_or_else(|error| panic!("{}: {error}", to.display()));
    let permissions = fs::metadata(from)
        .unwrap_or_else(|error| panic!("{}: {error}", from.display()))
        .permissions();
    fs::set_permissions(to, permissions)
        .unwrap_or_else(|error| panic!("{}: {error}", to.display()));
}

/// Writes `bytes` to `root/relative`, creating its directories.
pub fn write(root: &Path, relative: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("parent directory");
    fs::write(&path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

pub fn read(root: &Path, relative: &str) -> Vec<u8> {
    let path = root.join(relative);
    fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

pub fn read_text(root: &Path, relative: &str) -> String {
    String::from_utf8(read(root, relative)).expect("UTF-8 file")
}

/// Replaces the one occurrence of `from` in `root/relative` by `to`.
pub fn replace(root: &Path, relative: &str, from: &str, to: &str) {
    let text = read_text(root, relative);
    assert_eq!(
        text.matches(from).count(),
        1,
        "{relative}: {from:?} must occur exactly once"
    );
    write(root, relative, text.replacen(from, to, 1));
}

/// The directory the host rule puts the index databases in, under `home`.
pub fn data_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("specengine")
    } else {
        home.join(".local").join("share").join("specengine")
    }
}

/// One finished `spec` process.
#[derive(Debug, Clone)]
pub struct Run {
    pub args: Vec<String>,
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    /// Asserts the exit code, showing the whole run when it differs.
    pub fn code(&self, code: i32) -> &Self {
        assert_eq!(self.code, code, "spec {:?}:\n{}", self.args, self.show());
        self
    }

    pub fn show(&self) -> String {
        format!(
            "exit {}\n--- stdout\n{}--- stderr\n{}",
            self.code, self.stdout, self.stderr
        )
    }

    pub fn stderr_lines(&self) -> Vec<&str> {
        self.stderr.lines().collect()
    }

    /// stdout as exactly one JSON document followed by one line end.
    pub fn json(&self) -> Value {
        one_json_document(&self.stdout)
            .unwrap_or_else(|problem| panic!("spec {:?}: {problem}\n{}", self.args, self.show()))
    }
}

/// `text` as one JSON document and a line end; `Err` says why not.
pub fn one_json_document(text: &str) -> Result<Value, String> {
    let Some(body) = text.strip_suffix('\n') else {
        return Err("stdout does not end with a line end".to_owned());
    };
    if body.contains('\n') {
        return Err("stdout holds more than one line".to_owned());
    }
    let mut documents = serde_json::Deserializer::from_str(body).into_iter::<Value>();
    let first = documents
        .next()
        .ok_or("stdout holds no JSON document")?
        .map_err(|error| format!("stdout is no JSON document: {error}"))?;
    if documents.next().is_some() {
        return Err("stdout holds more than one JSON document".to_owned());
    }
    Ok(first)
}

/// Runs `spec args` in `cwd` with a cleared environment plus `env` only.
pub fn spec_with(cwd: &Path, args: &[&str], env: &[(&str, &OsStr)]) -> Run {
    let mut command = Command::new(SPEC);
    command
        .env_clear()
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("spawn spec");
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
    let stdout = out.join().expect("stdout reader");
    let stderr = err.join().expect("stderr reader");
    Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().expect("spec exited, not killed by a signal"),
        stdout: String::from_utf8(stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(stderr).expect("stderr is UTF-8"),
    }
}

/// Runs `spec args` in `cwd` with `HOME=home` and nothing else set.
pub fn spec(home: &Path, cwd: &Path, args: &[&str]) -> Run {
    spec_with(cwd, args, &[("HOME", home.as_os_str())])
}

/// `spec index` in `root`, which must answer.
pub fn index(home: &Path, root: &Path) -> Run {
    let run = spec(home, root, &["index"]);
    run.code(0);
    run
}

/// Every entry under `dir`, root-relative with `/`: directories as `None`,
/// files as their bytes, symlinks as `link -> target`.
pub fn snapshot(dir: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("under root")
                .to_string_lossy()
                .into_owned();
            let kind = entry.file_type().expect("file type");
            if kind.is_symlink() {
                let target = fs::read_link(&path).expect("link");
                out.insert(
                    relative,
                    Some(format!("link -> {}", target.display()).into_bytes()),
                );
            } else if kind.is_dir() {
                out.insert(relative, None);
                walk(root, &path, out);
            } else {
                out.insert(relative, Some(fs::read(&path).unwrap_or_default()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The paths of [`snapshot`].
pub fn paths_under(dir: &Path) -> Vec<String> {
    snapshot(dir).into_keys().collect()
}

/// Every `.md` file under `root`, root-relative, sorted.
pub fn md_files(root: &Path) -> Vec<String> {
    snapshot(root)
        .into_iter()
        .filter(|(path, bytes)| bytes.is_some() && path.ends_with(".md"))
        .map(|(path, _)| path)
        .collect()
}

/// Lines `first..=last` (1-based) of `bytes`, each with its line end.
pub fn lines(bytes: &[u8], first: usize, last: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut line = 1;
    for &byte in bytes {
        if (first..=last).contains(&line) {
            out.push(byte);
        }
        if byte == b'\n' {
            line += 1;
        }
    }
    out
}

/// No ANSI escape, no timing in `text`.
pub fn assert_plain(text: &str, context: &str) {
    assert!(
        !text.contains('\u{1b}'),
        "{context}: an ANSI escape in {text:?}"
    );
    for timing in [" ms", "ms)", "elapsed", "took ", "seconds"] {
        assert!(
            !text.contains(timing),
            "{context}: a timing ({timing:?}) in {text:?}"
        );
    }
}

/// The fixtures both genre tests run on, with their slugs.
pub const FIXTURES: [(&str, &str); 2] = [("spec-a", "lantern-keep"), ("spec-b", "zerkalo")];

/// A header line of `spec show`: the fields split on ` | `.
pub fn header_fields(line: &str) -> Vec<&str> {
    line.split(" | ").collect()
}

/// `HOME` as an `OsString`, for [`spec_with`].
pub fn os(text: &str) -> OsString {
    OsString::from(text)
}

/// The ` | span b3:<64 hex>` suffix every `spec show` text header ends with
/// (docs/features/proposal-apply.md, "Data", `spec show`).
pub const SPAN_SUFFIX: &str = " | span b3:";

/// `header` (one `spec show` header line) without its span suffix, which
/// must be there, last, with 64 lower-case hex digits: the header a bundle
/// target carries (docs/features/spec-cli-bundle.md, as amended by
/// proposal-apply Q1).
pub fn header_without_span(header: &str) -> &str {
    let (before, hex) = header
        .rsplit_once(SPAN_SUFFIX)
        .unwrap_or_else(|| panic!("no `{SPAN_SUFFIX}` in the header {header:?}"));
    assert!(
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "the span hash of {header:?} is not 64 lower-case hex digits"
    );
    before
}

/// `stdout` of `spec show` with its first line (the header) stripped of
/// the span suffix ([`header_without_span`]).
pub fn show_without_span(stdout: &str) -> String {
    let (header, rest) = stdout
        .split_once('\n')
        .unwrap_or_else(|| panic!("no header line in {stdout:?}"));
    format!("{}\n{rest}", header_without_span(header))
}
