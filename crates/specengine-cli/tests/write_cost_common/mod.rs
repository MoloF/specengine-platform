//! docs/features/write-cost.md (RM-0125) WC9, the half the CLI, the daemon
//! and the MCP server's `write_cost_golden.rs` share (included by path from
//! `specengine-http` and `specengine-mcp`): the fixture corpora, a scratch
//! git repository of one of them, the node every case writes to, the texts
//! written, and the golden files `fixtures/write-cost-golden/<corpus>.json`.
//!
//! A golden file holds one section per surface (`cli`, `http`, `mcp`), each
//! a map from a case to its output. A test compares its own section with
//! the current build's answers, byte for byte, and never writes; with
//! [`CAPTURE_VAR`] set to `1` it writes its section instead, and refuses
//! unless this checkout is [`CAPTURED_AT`]'s commit with `crates/*/src`
//! untouched (the spec's "Capture").
//!
//! Normalized, as "Goldens" allows: the scratch root becomes `<root>`, a
//! commit made during the case `<sha>`. Beyond those two: a daemon and an
//! MCP server read the wall clock (0.47.0's public API injects none there),
//! so their answers' timestamps of the run become `<now>` and its date
//! `<today>` ([`WallClock`]); the CLI's answers are on an injected clock and
//! keep every byte. A date the corpus's own files hold is never masked
//! ([`WallClock::with_dates_of`]): a fixture's `2026-11-15` stays as written,
//! on that day as on any other.
//!
//! Every case writes to one node of the corpus ([`Probe`]): the first node
//! with an ID in `spec tree`'s pre-order, Tier 3 aside. Its new texts add
//! links to files and an anchor that do not exist, in reverse name order,
//! so a case's findings are more than one and their order is the check's
//! (WC9's mutation, findings sorted by another key, turns them red).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use specengine_cli::{
    CliError, Env, GitEnv, Globals, Outcome, ShowRequest, TreeRequest, render_json, render_text,
    show, tree,
};

/// Where and on what the goldens were captured: tag `v0.47.0`'s commit, and
/// `git diff --stat HEAD -- crates/*/src` then (empty).
pub const CAPTURED_AT: (&str, &str) = ("d38e70d4518d11ebf5e466fe8d2a7ba94ea6a4e6", "");

/// `1`: write this surface's section of each golden instead of comparing.
pub const CAPTURE_VAR: &str = "SPECENGINE_WRITE_COST_CAPTURE";

/// Every fixture corpus at 0.47.0: `fixtures/*/specengine.toml` and
/// `fixtures/*/*/specengine.toml`, in path order.
pub const CORPORA: [&str; 31] = [
    "anchor-states",
    "apply-outputs/forge",
    "apply-outputs/press",
    "auto-mini",
    "card-press",
    "code-a",
    "code-bevy",
    "code-index",
    "corpus-mini",
    "flow-aside",
    "flow-gaps",
    "flow-hire",
    "flow-relay",
    "impact",
    "import-layout/four",
    "import-layout/one",
    "import-layout/three",
    "import-layout/two",
    "parent-ref",
    "pilot-w/one",
    "pilot-w/two",
    "plant-diary",
    "roadmap-a",
    "roadmap-b",
    "spec-a",
    "spec-b",
    "spec-c",
    "studio-lamp",
    "trace-almanac",
    "verify-ecs",
    "verify-tests",
];

/// The injected clock of every proposal and stage.
pub const NOW: &str = "2026-10-05T21:14:03Z";
/// The injected clock of every decision.
pub const LATER: &str = "2026-10-06T08:00:00Z";

/// The fixed author and committer of every scratch commit (as the CLI
/// tests' sandbox): the fixture's commit is the same on every run.
pub const IDENTITY: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "Scratch Author"),
    ("GIT_AUTHOR_EMAIL", "author@example.invalid"),
    ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00+0000"),
    ("GIT_COMMITTER_NAME", "Scratch Committer"),
    ("GIT_COMMITTER_EMAIL", "committer@example.invalid"),
    ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00+0000"),
];

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// `fixtures/write-cost-golden`.
pub fn golden_dir() -> PathBuf {
    repository_root().join("fixtures").join("write-cost-golden")
}

/// The golden of `corpus`: its path with `/` made `-`.
pub fn golden_file(corpus: &str) -> PathBuf {
    golden_dir().join(format!("{}.json", corpus.replace('/', "-")))
}

/// The corpora on disk now, as [`CORPORA`] names them.
pub fn corpora_on_disk() -> Vec<String> {
    let fixtures = repository_root().join("fixtures");
    let mut found = Vec::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir(&fixtures)
        .expect("fixtures")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let name = dir
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        if dir.join("specengine.toml").is_file() {
            found.push(name.clone());
        }
        let mut inner: Vec<PathBuf> = fs::read_dir(&dir)
            .expect("a fixture directory")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && path.join("specengine.toml").is_file())
            .collect();
        inner.sort();
        for path in inner {
            let child = path
                .file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            found.push(format!("{name}/{child}"));
        }
    }
    found.sort();
    found
}

/// Whether this run captures.
pub fn capturing() -> bool {
    std::env::var_os(CAPTURE_VAR).is_some_and(|value| value == "1")
}

// ------------------------------------------------------------ scratch

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A scratch directory under the system temp dir, canonical, removed on drop.
pub struct Scratch {
    root: PathBuf,
}

impl Scratch {
    pub fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-wc-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self {
            root: fs::canonicalize(&path).expect("canonical scratch"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    /// An empty directory `<scratch>/<name>`, canonical.
    pub fn dir(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::create_dir_all(&path).expect("directory");
        fs::canonicalize(&path).expect("canonical directory")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Copies every directory and regular file under `from` to `to`.
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("copy target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .unwrap_or_else(|error| panic!("{}: {error}", from.display()))
        .flatten()
        .collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let kind = entry.file_type().expect("file type");
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target);
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}

/// The first executable `git` on the test process's `PATH`, else
/// `/usr/bin/git`.
pub fn real_git() -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("git");
            let executable = fs::metadata(&candidate)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0);
            if executable && dir.is_absolute() {
                return candidate;
            }
        }
    }
    PathBuf::from("/usr/bin/git")
}

/// The real git's directory, then `/usr/bin:/bin`.
pub fn plain_path() -> OsString {
    let git = real_git();
    let mut dirs = vec![git.parent().expect("git's directory").to_path_buf()];
    for extra in ["/usr/bin", "/bin"] {
        let extra = PathBuf::from(extra);
        if !dirs.contains(&extra) {
            dirs.push(extra);
        }
    }
    std::env::join_paths(dirs).expect("PATH")
}

/// A scratch git repository of one corpus: its copy committed once on
/// `main` under [`IDENTITY`], a `HOME` for the data directory, and the
/// variables of every git process (no global or system config, a scratch
/// git `HOME`, no automatic maintenance).
pub struct Repo {
    pub scratch: Scratch,
    pub root: PathBuf,
    pub home: PathBuf,
    pub vars: Vec<(OsString, OsString)>,
}

impl Repo {
    pub fn new(label: &str, corpus: &str) -> Self {
        let scratch = Scratch::new(label);
        let git_home = scratch.dir("git-home");
        let mut vars: Vec<(OsString, OsString)> = vec![
            ("HOME".into(), git_home.into_os_string()),
            ("PATH".into(), plain_path()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
            (
                "GIT_CEILING_DIRECTORIES".into(),
                scratch.path().as_os_str().into(),
            ),
        ];
        for (name, value) in IDENTITY {
            vars.push((name.into(), value.into()));
        }
        let root = scratch.path().join("served");
        copy_dir(&repository_root().join("fixtures").join(corpus), &root);
        let root = fs::canonicalize(&root).expect("canonical root");
        let home = scratch.dir("home");
        let repo = Self {
            scratch,
            root,
            home,
            vars,
        };
        repo.git(&["init", "-q", "--template=", "--initial-branch=main"]);
        for (key, value) in [("maintenance.auto", "false"), ("gc.auto", "0")] {
            repo.git(&["config", "--local", key, value]);
        }
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "-q", "-m", "the fixture"]);
        repo
    }

    /// The library's environment at the root, this `HOME`.
    pub fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    /// The caller's git environment: the sandbox's variables.
    pub fn git_env(&self) -> GitEnv {
        GitEnv::new(self.root.as_path(), self.vars.clone())
    }

    /// `git args` at the root, which must succeed: its stdout.
    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new(real_git())
            .env_clear()
            .envs(self.vars.iter().map(|(k, v)| (k, v)))
            .current_dir(&self.root)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("UTF-8 git output")
    }

    pub fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// Everything under the root committed as `message`.
    pub fn commit_all(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    /// The commits after `base` up to `HEAD`, newest first.
    pub fn commits_since(&self, base: &str) -> Vec<String> {
        self.git(&["rev-list", &format!("{base}..HEAD")])
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// `git show` of `commit` without its dates: the hash, the parents, the
    /// author and committer, the message and the patch with its stat, each
    /// `index` line left out (a blob's hash follows its bytes, which may hold
    /// the run's date: the bytes are kept).
    pub fn shown(&self, commit: &str) -> String {
        self.git(&[
            "show",
            "--no-color",
            "--patch-with-stat",
            "--format=%H%n%P%n%an <%ae>%n%cn <%ce>%n%n%B",
            commit,
        ])
        .split_inclusive('\n')
        .filter(|line| !line.starts_with("index "))
        .collect()
    }
}

// --------------------------------------------------------------- probe

/// The node every case of a corpus writes to, as `spec show` gives it, and
/// an accepted node an interpretation names.
#[derive(Debug, Clone)]
pub struct Probe {
    pub target: String,
    pub path: String,
    pub base: String,
    pub text: String,
    /// The first node with an ID whose document is an `accepted` decision
    /// (`class: decision`), else the target (an interpretation of it is
    /// then refused).
    pub decision: String,
    /// The front matter of the target's file, between its fences.
    pub front: String,
}

/// The probe of the corpus at `env`'s root: the first node with an ID in
/// `spec tree`'s pre-order outside Tier 3; when the tree lists none, the
/// first ID written in a `.md` file (path order: an `id:` line, then each
/// `{#ID}`) that `spec show` finds outside Tier 3. `Err` the tree's own
/// refusal (a corpus the queue cannot open); `Ok(None)` no such node.
pub fn probe(env: &Env, git: &GitEnv) -> Result<Option<Probe>, String> {
    let listed = tree(
        env,
        &Globals::default(),
        &TreeRequest {
            root: None,
            depth: None,
            kinds: Vec::new(),
            archive: false,
        },
    )
    .map_err(|error| error.message)?;
    let shown = |reference: &str| {
        show(
            env,
            &Globals::default(),
            &ShowRequest {
                reference: reference.to_owned(),
                links: false,
                archive: false,
                bindings: false,
                history: false,
                git: git.clone(),
            },
        )
        .ok()
        .and_then(|outcome| outcome.nodes.into_iter().next())
        .filter(|node| !node.archived)
    };
    let from_tree = listed
        .nodes
        .iter()
        .find(|node| node.id.is_some() && !node.archived)
        .and_then(|node| node.id.clone());
    let found = match from_tree {
        Some(target) => shown(&target).map(|node| (target, node)),
        None => written_ids(&env.cwd)
            .into_iter()
            .find_map(|target| shown(&target).map(|node| (target, node))),
    };
    let Some((target, node)) = found else {
        return Ok(None);
    };
    let decision = accepted_decision(&env.cwd).unwrap_or_else(|| target.clone());
    let file = fs::read_to_string(env.cwd.join(&node.path)).unwrap_or_default();
    Ok(Some(Probe {
        target,
        path: node.path,
        base: node.span_hash,
        text: node.text,
        decision,
        front: front_matter(&file),
    }))
}

/// Every ID written in the `.md` files under `root`, path order: a file's
/// `id:` line, then each heading's `{#ID}`.
pub fn written_ids(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    markdown_files(root, &mut files);
    files.sort();
    let mut ids = Vec::new();
    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let front = front_matter(&text);
        ids.extend(
            front
                .lines()
                .filter_map(|line| line.strip_prefix("id: "))
                .map(|id| id.trim().to_owned()),
        );
        for line in text.lines().filter(|line| line.starts_with('#')) {
            if let Some((_, rest)) = line.split_once("{#") {
                let id: String = rest
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != '}')
                    .collect();
                if !id.is_empty() {
                    ids.push(id);
                }
            }
        }
    }
    ids
}

/// The `id:` of the first `.md` file under `root` (path order, `.git`
/// aside) whose front matter says `class: decision` and `status: accepted`.
pub fn accepted_decision(root: &Path) -> Option<String> {
    let mut files = Vec::new();
    markdown_files(root, &mut files);
    files.sort();
    files.iter().find_map(|path| {
        let front = front_matter(&fs::read_to_string(path).ok()?);
        let lines: Vec<&str> = front.lines().collect();
        if !lines.contains(&"class: decision") || !lines.contains(&"status: accepted") {
            return None;
        }
        lines
            .iter()
            .find_map(|line| line.strip_prefix("id: "))
            .map(|id| id.trim().to_owned())
    })
}

/// The first `.md` file under `root` in path order (`.git` aside),
/// root-relative.
pub fn first_markdown(root: &Path) -> Option<String> {
    let mut files = Vec::new();
    markdown_files(root, &mut files);
    files.sort();
    files.first().and_then(|path| {
        path.strip_prefix(root)
            .ok()
            .map(|relative| relative.to_string_lossy().into_owned())
    })
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() && entry.file_name() != ".git" {
            markdown_files(&path, out);
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

/// The lines between a file's opening `---` and its closing one; empty when
/// it has none.
pub fn front_matter(file: &str) -> String {
    let text = file.replace("\r\n", "\n");
    let Some(rest) = text.strip_prefix("---\n") else {
        return String::new();
    };
    match rest.find("\n---\n") {
        Some(end) => format!("{}\n", &rest[..end]),
        None => String::new(),
    }
}

/// `front` without the top-level keys `keys` (each with its indented or
/// listed continuation lines).
pub fn without_keys(front: &str, keys: &[&str]) -> String {
    let mut out = String::new();
    let mut dropping = false;
    for line in front.split_inclusive('\n') {
        let top = !line.starts_with([' ', '\t', '-']);
        if top {
            dropping = keys.iter().any(|key| line.starts_with(&format!("{key}:")));
        }
        if !dropping {
            out.push_str(line);
        }
    }
    out
}

/// The probe's lines naming what does not exist: two files and an anchor,
/// tagged `tag`, the later line's names sorting first.
pub fn dangling(tag: &str) -> String {
    format!(
        "See [the second probe](zz-{tag}-b.md) here.\n\nSee [the first probe](zz-{tag}-a.md) and \
         [its anchor](#zz-{tag}-anchor) here.\n"
    )
}

/// `text` with [`dangling`] appended as its own paragraph.
pub fn appended(text: &str, tag: &str) -> String {
    let mut out = text.to_owned();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&dangling(tag));
    out
}

/// A new file: `front` between fences, a title, [`dangling`].
pub fn new_file(front: &str, title: &str, tag: &str) -> String {
    let mut out = String::new();
    if !front.is_empty() {
        out.push_str("---\n");
        out.push_str(front);
        out.push_str("---\n\n");
    }
    out.push_str(&format!("# {title}\n\n"));
    out.push_str(&dangling(tag));
    out
}

/// `name` in the directory of `path` (root-relative, `/`-separated).
pub fn sibling(path: &str, name: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{name}"),
        None => name.to_owned(),
    }
}

// ------------------------------------------------------------- outputs

/// A library call's answer as `spec` prints it: its exit, stdout as text and
/// as `--json`, and its stderr lines; or the error's exit and line.
pub fn printed(result: Result<Outcome, CliError>) -> Value {
    match result {
        Ok(outcome) => json!({
            "exit": outcome.exit().code(),
            "text": render_text(&outcome),
            "json": render_json(&outcome),
            "stderr": outcome.stderr_lines(),
        }),
        Err(error) => json!({
            "exit": error.exit.code(),
            "error": error.message,
        }),
    }
}

/// `value` with every string's `from` made `to`, recursively.
pub fn replaced(value: Value, from: &str, to: &str) -> Value {
    if from.is_empty() {
        return value;
    }
    match value {
        Value::String(text) => Value::String(text.replace(from, to)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| replaced(item, from, to))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, item)| (key, replaced(item, from, to)))
                .collect(),
        ),
        other => other,
    }
}

/// `value` with the root (as given and canonical) made `<root>` and each of
/// `commits` (full, and its 7- to 12-digit abbreviations) `<sha>`.
pub fn normalized(value: Value, root: &Path, commits: &[String]) -> Value {
    let mut value = value;
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    for root in [canonical.as_path(), root] {
        value = replaced(value, &root.to_string_lossy(), "<root>");
    }
    for commit in commits {
        value = replaced(value, commit, "<sha>");
        for width in (7..=12).rev() {
            value = replaced(value, &commit[..width], "<sha>");
        }
    }
    value
}

/// The wall clock of a run that cannot be injected: its first and last
/// readings, `YYYY-MM-DDTHH:MM:SSZ`.
#[derive(Debug, Clone)]
pub struct WallClock {
    from: String,
    to: String,
    /// The dates the corpus's own files hold ([`WallClock::with_dates_of`]):
    /// never masked.
    fixture_dates: std::collections::BTreeSet<String>,
}

/// The wall clock now, `YYYY-MM-DDTHH:MM:SSZ`.
pub fn utc_now() -> String {
    specengine_cli::utc_now()
}

impl WallClock {
    pub fn start() -> Self {
        let now = utc_now();
        Self::between(&now, &now)
    }

    /// A run read from `from` to `to` (`YYYY-MM-DDTHH:MM:SSZ`): the
    /// harness's own tests, which cannot move the wall clock.
    pub fn between(from: &str, to: &str) -> Self {
        Self {
            from: from.to_owned(),
            to: to.to_owned(),
            fixture_dates: std::collections::BTreeSet::new(),
        }
    }

    /// With every `YYYY-MM-DD` in the files under `root` (`.git` aside)
    /// known as the corpus's own, never masked.
    ///
    /// The limit: on a day the corpus's files also hold, a date the clock
    /// gave that day stays as written too (the two are one string).
    pub fn with_dates_of(mut self, root: &Path) -> Self {
        fn walk(dir: &Path, out: &mut std::collections::BTreeSet<String>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_name() == ".git" {
                    continue;
                }
                if path.is_dir() {
                    walk(&path, out);
                } else if let Ok(bytes) = fs::read(&path) {
                    for window in bytes.windows(10) {
                        if date_bytes(window) {
                            out.insert(String::from_utf8_lossy(window).into_owned());
                        }
                    }
                }
            }
        }
        walk(root, &mut self.fixture_dates);
        self
    }

    /// Whether `date` is the run's day and no date of the corpus's own.
    fn masks(&self, date: &str) -> bool {
        (date == &self.from[..10] || date == &self.to[..10]) && !self.fixture_dates.contains(date)
    }

    pub fn stop(&mut self) {
        self.to = utc_now();
    }

    /// `value` with each timestamp read during the run made `<now>` and each
    /// of its dates `<today>`.
    pub fn normalized(&self, value: Value) -> Value {
        match value {
            Value::String(text) => Value::String(self.text(&text)),
            Value::Array(items) => Value::Array(
                items
                    .into_iter()
                    .map(|item| self.normalized(item))
                    .collect(),
            ),
            Value::Object(map) => Value::Object(
                map.into_iter()
                    .map(|(key, item)| (key, self.normalized(item)))
                    .collect(),
            ),
            other => other,
        }
    }

    fn text(&self, text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        while at < bytes.len() {
            if let Some(stamp) = text.get(at..at + 20)
                && is_stamp(stamp)
                && stamp >= self.from.as_str()
                && stamp <= self.to.as_str()
            {
                out.push_str("<now>");
                at += 20;
                continue;
            }
            if let Some(date) = text.get(at..at + 10)
                && is_date(date)
                && self.masks(date)
            {
                out.push_str("<today>");
                at += 10;
                continue;
            }
            let next = text[at..].chars().next().expect("a char");
            out.push(next);
            at += next.len_utf8();
        }
        out
    }
}

fn is_date(text: &str) -> bool {
    date_bytes(text.as_bytes())
}

/// `YYYY-MM-DD`, ASCII.
fn date_bytes(bytes: &[u8]) -> bool {
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

fn is_stamp(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && date_bytes(&bytes[..10])
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && [11, 12, 14, 15, 17, 18]
            .iter()
            .all(|&index| bytes[index].is_ascii_digit())
}

// -------------------------------------------------------------- goldens

/// Compares `cases`, the `section` of `corpus`'s golden, with the file; or,
/// capturing, writes them as that section.
pub fn settle(corpus: &str, section: &str, cases: BTreeMap<String, Value>) {
    let path = golden_file(corpus);
    if capturing() {
        assert_capture_point();
        fs::create_dir_all(golden_dir()).expect("the golden directory");
        let _lock = Lock::take(&path);
        let mut golden: Value = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).expect("a golden is JSON"),
            Err(_) => json!({}),
        };
        golden["corpus"] = json!(corpus);
        golden["captured_at"] = json!({"commit": CAPTURED_AT.0, "src_diff_stat": CAPTURED_AT.1});
        golden[section] = json!(cases);
        let mut text = ascii_json(&serde_json::to_string_pretty(&golden).expect("JSON"));
        text.push('\n');
        fs::write(&path, text).expect("write a golden");
        return;
    }
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}; capture it at v0.47.0 ({CAPTURE_VAR}=1)",
            path.display()
        )
    });
    let golden: Value = serde_json::from_str(&text).expect("a golden is JSON");
    assert_eq!(
        golden["captured_at"]["commit"],
        json!(CAPTURED_AT.0),
        "{corpus}: captured elsewhere"
    );
    let stored = golden[section]
        .as_object()
        .unwrap_or_else(|| panic!("{corpus}: no `{section}` section in {}", path.display()));
    let stored_keys: Vec<&String> = stored.keys().collect();
    let keys: Vec<&String> = cases.keys().collect();
    assert_eq!(keys, stored_keys, "{corpus} {section}: the cases");
    for (case, value) in &cases {
        let want = &stored[case];
        if value != want {
            panic!(
                "{corpus} {section} {case}: differs from 0.47.0's golden\n{}",
                first_difference(want, value)
            );
        }
    }
}

/// `json` with every non-ASCII character written `\\uXXXX` (a UTF-16 pair
/// above the BMP): a golden holds no raw non-English letter (ADR-0024; eval's
/// `anonymity.rs`), and a compare parses the JSON, so the bytes compared are
/// the strings' own. Outside strings JSON is ASCII, so this touches strings
/// alone.
pub fn ascii_json(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    for c in json.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

/// The first line where `want` and `got`, pretty-printed and each string's
/// lines split out, differ.
fn first_difference(want: &Value, got: &Value) -> String {
    let lines = |value: &Value| -> Vec<String> {
        let mut out = Vec::new();
        flatten("", value, &mut out);
        out
    };
    let (want, got) = (lines(want), lines(got));
    for (index, (a, b)) in want.iter().zip(&got).enumerate() {
        if a != b {
            return format!("line {index}:\n  0.47.0: {a}\n  now:    {b}");
        }
    }
    format!(
        "0.47.0 has {} lines, now {} (the first {} equal)",
        want.len(),
        got.len(),
        want.len().min(got.len())
    )
}

fn flatten(at: &str, value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, item) in map {
                flatten(&format!("{at}.{key}"), item, out);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                flatten(&format!("{at}[{index}]"), item, out);
            }
        }
        Value::String(text) => {
            // A JSON document carried as text (`--json`, a reply's body) is
            // compared field by field, so a difference names its field.
            let trimmed = text.trim_start();
            if (trimmed.starts_with('{') || trimmed.starts_with('['))
                && let Ok(inner) = serde_json::from_str::<Value>(text)
            {
                flatten(&format!("{at}~"), &inner, out);
                return;
            }
            for line in text.split('\n') {
                out.push(format!("{at}: {line}"));
            }
        }
        other => out.push(format!("{at}: {other}")),
    }
}

/// A capture runs only at [`CAPTURED_AT`]: `HEAD` its commit and
/// `crates/*/src` untouched.
fn assert_capture_point() {
    let git = |args: &[&str]| {
        let output = Command::new(real_git())
            .current_dir(repository_root())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("spawn git");
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8(output.stdout).expect("UTF-8")
    };
    assert_eq!(
        git(&["rev-parse", "HEAD"]).trim(),
        CAPTURED_AT.0,
        "a capture runs at v0.47.0 only"
    );
    assert_eq!(
        git(&["diff", "--stat", "HEAD", "--", "crates/*/src"]).trim(),
        CAPTURED_AT.1,
        "a capture runs with crates/*/src untouched"
    );
}

/// A lock beside a golden, so two surfaces capturing at once never lose a
/// section.
struct Lock(PathBuf);

impl Lock {
    fn take(golden: &Path) -> Self {
        let path = golden.with_extension("lock");
        let started = Instant::now();
        loop {
            if fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .is_ok()
            {
                return Self(path);
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "{} held for a minute",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Seconds since the epoch, for a label.
pub fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}
