//! docs/features/doc-library.md (RM-0127) "Acceptance criteria", the scene
//! every library test reads: a git copy of `fixtures/spec-a` (the fixture
//! itself is only read) with the spec's changes:
//!
//! - DEC-0007 gains `canon: MEC-STAMINA`;
//! - `docs/records/DEC/DEC-0031.md` is added: `class: decision`, `status:
//!   rejected`, `date: 2026-09-10`, `scope: [sprint]`, `canon: MEC-SPRINT`,
//!   and an H1;
//! - `docs/spec/movement/stamina.md` gains `tags: [stamina, tuning]` (after
//!   `class: canon`, its line 5);
//! - `docs/generated/list.md` is added: `class: generated`, `generator: x`,
//!   `source: y` (no H1, no ID, no link: it moves no other row's uses);
//! - `specengine.toml` gains `[[module]] name = "movement", root =
//!   "DOM-MOVEMENT"` and `[[module]] name = "records", kinds = ["question",
//!   "requirement"]`, and `[paths] roots` naming the default walk and
//!   `docs/generated`, which the default walk leaves out, so the
//!   generated document is a row.
//!
//! C1 holds all of it, by `Ann Owner` as author and committer at
//! [`C1_AT`]; C2 ([`Scene::c2`]) appends a line to DEC-0023's body alone 60 s
//! later, C3 ([`Scene::c3`]) changes stamina.md's body 60 s after that.
//! Every commit sets `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE`.
//!
//! `specengine-http`'s `tests/library.rs` and `tests/daemon_keys.rs` include
//! this file by path beside `common/git.rs` as their `git` module, so it
//! names no `specengine-cli` or `specengine-core` item.
//!
//! Every name here is invented.

#![allow(dead_code)]

use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::git::Sandbox;

/// spec-a's `[project] slug`.
pub const SLUG: &str = "lantern-keep";
pub const STAMINA: &str = "docs/spec/movement/stamina.md";
pub const SPRINT: &str = "docs/spec/movement/sprint.md";
pub const MOVEMENT: &str = "docs/spec/movement/README.md";
pub const DEC_0007: &str = "docs/records/DEC/DEC-0007.md";
pub const DEC_0023: &str = "docs/records/DEC/DEC-0023.md";
pub const DEC_0031: &str = "docs/records/DEC/DEC-0031.md";
pub const LIST: &str = "docs/generated/list.md";

/// The written line of stamina's tags, and its line number in the file.
pub const TAGS_LINE: &str = "tags: [stamina, tuning]\n";
pub const TAGS_LINE_NO: u64 = 5;

/// C1, C2 and C3's committer (and author) times, 60 s apart.
pub const C1_AT: &str = "2026-01-02T10:00:00Z";
pub const C2_AT: &str = "2026-01-02T10:01:00Z";
pub const C3_AT: &str = "2026-01-02T10:02:00Z";

/// The line C2 appends to DEC-0023's body.
pub const C2_LINE: &str = "The rest delay was measured again on the reference machine.\n";
/// The edit of stamina.md's body that is first left uncommitted, then is C3.
pub const C3_FROM: &str = "Stamina limits sprinting.";
pub const C3_TO: &str = "Stamina limits sprinting and climbing.";

/// The scene's one person.
pub const ANN_NAME: &str = "Ann Owner";
pub const ANN_EMAIL: &str = "ann@example.org";

/// `Name <email>`, as `last_change.by` serves it raw.
pub fn ann() -> String {
    format!("{ANN_NAME} <{ANN_EMAIL}>")
}

/// The `[[module]]` tables the scene appends to spec-a's config, and a
/// `[paths]` whose `roots` are the default walk (spec-a has no
/// `docs/archive`) plus `docs/generated`, which no default walk reads
/// (`paths_toml.rs`): the generated document is a row and the other twelve
/// are unchanged.
pub const MODULES: &str = "\n[paths]\nroots = [\"docs/spec\", \"docs/records\", \
                           \"docs/features\", \"docs/generated\"]\n\n[[module]]\nname = \
                           \"movement\"\nroot = \"DOM-MOVEMENT\"\n\n[[module]]\nname = \
                           \"records\"\nkinds = [\"question\", \"requirement\"]\n";

/// DEC-0031, as the scene adds it.
pub const DEC_0031_TEXT: &str = "---\nid: DEC-0031\nclass: decision\nstatus: rejected\ndate: \
                                 2026-09-10\nscope: [sprint]\ncanon: MEC-SPRINT\n---\n\n# Sprint \
                                 drains the lantern\n\nRejected: the lantern stays out of the \
                                 stamina economy.\n";

/// The generated document, as the scene adds it.
pub const LIST_TEXT: &str =
    "---\nclass: generated\ngenerator: x\nsource: y\n---\n\nA list made by a script.\n";

/// git's date of a UTC stamp `YYYY-MM-DDTHH:MM:SSZ`.
pub fn git_date(utc: &str) -> String {
    format!("{}+0000", utc.trim_end_matches('Z'))
}

/// `fixtures/<name>` of this repository.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

/// Every file under `dir`, root-relative with `/`, and its bytes, sorted by
/// path.
fn files_under(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).expect("a readable fixture").flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(root, &path, out);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .to_str()
                .expect("a UTF-8 path")
                .replace('\\', "/");
            out.push((relative, fs::read(&path).expect("a readable file")));
        }
    }
}

/// `text` with its one `from` replaced by `to`.
fn once(text: &str, from: &str, to: &str, what: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "{what}: {from:?}");
    text.replacen(from, to, 1)
}

/// The scene's files and bytes, sorted by path: spec-a with the spec's
/// changes.
pub fn scene_files() -> Vec<(String, Vec<u8>)> {
    let source = fixture("spec-a");
    let mut files = Vec::new();
    files_under(&source, &source, &mut files);
    for (path, bytes) in &mut files {
        let text = || String::from_utf8(bytes.clone()).expect("a UTF-8 fixture file");
        let changed = match path.as_str() {
            DEC_0007 => Some(once(
                &text(),
                "date: 2026-08-01\n",
                "date: 2026-08-01\ncanon: MEC-STAMINA\n",
                path,
            )),
            STAMINA => Some(once(
                &text(),
                "class: canon\n",
                &format!("class: canon\n{TAGS_LINE}"),
                path,
            )),
            "specengine.toml" => Some(format!("{}{MODULES}", text())),
            _ => None,
        };
        if let Some(changed) = changed {
            *bytes = changed.into_bytes();
        }
    }
    files.push((DEC_0031.to_owned(), DEC_0031_TEXT.as_bytes().to_vec()));
    files.push((LIST.to_owned(), LIST_TEXT.as_bytes().to_vec()));
    files.sort();
    let stamina = files
        .iter()
        .find(|(path, _)| path == STAMINA)
        .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
        .expect("stamina.md");
    assert_eq!(
        stamina
            .lines()
            .nth(usize::try_from(TAGS_LINE_NO).unwrap() - 1),
        Some(TAGS_LINE.trim_end()),
        "premise: the tags are on line {TAGS_LINE_NO}"
    );
    files
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// The scene's repository and its `HOME`, removed on drop.
pub struct Scene {
    dir: PathBuf,
    pub git: Sandbox,
    pub root: PathBuf,
    pub home: PathBuf,
    /// C1's OID.
    pub c1: String,
    messages: Cell<usize>,
}

impl Drop for Scene {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

impl Scene {
    /// The scene at C1, its files written in path order.
    pub fn new(label: &str) -> Self {
        Self::build(label, false, |_, _, _| {})
    }

    /// The scene at C1: its files written in path order, or the reverse,
    /// `after(root, home, n)` called after the `n`th file is written (before
    /// `git init`).
    pub fn build(label: &str, reversed: bool, mut after: impl FnMut(&Path, &Path, usize)) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-dl-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        let dir = fs::canonicalize(&dir).expect("canonical");
        let git = Sandbox::new(&dir);
        let root = dir.join("repo");
        fs::create_dir_all(&root).expect("the root");
        let home = dir.join("h");
        fs::create_dir_all(&home).expect("a home");
        let mut files = scene_files();
        if reversed {
            files.reverse();
        }
        for (n, (path, bytes)) in files.iter().enumerate() {
            let target = root.join(path);
            fs::create_dir_all(target.parent().expect("a parent")).expect("a directory");
            fs::write(&target, bytes).expect("a file");
            after(&root, &home, n + 1);
        }
        git.init(&root);
        let mut scene = Self {
            dir,
            git,
            root,
            home,
            c1: String::new(),
            messages: Cell::new(0),
        };
        scene.c1 = scene.commit(C1_AT, "The scene\n");
        scene
    }

    /// The scratch directory every repository of the test lives in.
    pub fn scratch(&self) -> &Path {
        &self.dir
    }

    /// A fresh directory in the scratch.
    pub fn dir(&self, name: &str) -> PathBuf {
        let path = self.dir.join(name);
        fs::create_dir_all(&path).expect("a directory");
        path
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn put(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).expect("a readable file")
    }

    /// `relative` with its one `from` replaced by `to`.
    pub fn edit(&self, relative: &str, from: &str, to: &str) {
        let text = self.read(relative);
        self.put(relative, &once(&text, from, to, relative));
    }

    /// `text` appended to `relative`.
    pub fn append(&self, relative: &str, text: &str) {
        let mut whole = self.read(relative);
        whole.push_str(text);
        self.put(relative, &whole);
    }

    /// `git add -A`, then a commit of `message` by Ann (author and
    /// committer) at `at` (both dates): its OID.
    pub fn commit(&self, at: &str, message: &str) -> String {
        self.git.add_all(&self.root);
        let n = self.messages.get();
        self.messages.set(n + 1);
        let file = self.dir.join(format!("message-{n}.txt"));
        fs::write(&file, message).expect("a message file");
        let date = OsString::from(git_date(at));
        let pairs: [(&str, &OsStr); 6] = [
            ("GIT_AUTHOR_NAME", OsStr::new(ANN_NAME)),
            ("GIT_AUTHOR_EMAIL", OsStr::new(ANN_EMAIL)),
            ("GIT_AUTHOR_DATE", date.as_os_str()),
            ("GIT_COMMITTER_NAME", OsStr::new(ANN_NAME)),
            ("GIT_COMMITTER_EMAIL", OsStr::new(ANN_EMAIL)),
            ("GIT_COMMITTER_DATE", date.as_os_str()),
        ];
        self.git.git_env(
            &self.root,
            &[
                "commit",
                "-q",
                "--no-verify",
                "--allow-empty",
                "--cleanup=verbatim",
                "-F",
                file.to_str().expect("a UTF-8 scratch path"),
            ],
            &pairs,
        );
        self.head()
    }

    /// `HEAD`'s OID.
    pub fn head(&self) -> String {
        self.git.git_text(&self.root, &["rev-parse", "HEAD"])
    }

    /// C2: a line appended to DEC-0023's body alone, committed at [`C2_AT`].
    pub fn c2(&self) -> String {
        self.append(DEC_0023, C2_LINE);
        self.commit(C2_AT, "Measure the rest delay again\n")
    }

    /// The uncommitted edit of stamina.md's body ([`C3_FROM`] -> [`C3_TO`]).
    pub fn stamina_edit(&self) {
        self.edit(STAMINA, C3_FROM, C3_TO);
    }

    /// C3: stamina.md's body changed (the edit, made here when not yet
    /// made), committed at [`C3_AT`].
    pub fn c3(&self) -> String {
        if !self.read(STAMINA).contains(C3_TO) {
            self.stamina_edit();
        }
        self.commit(C3_AT, "Stamina limits climbing too\n")
    }

    /// `git checkout -q --detach <oid>`.
    pub fn checkout(&self, oid: &str) {
        self.git
            .git(&self.root, &["checkout", "-q", "--detach", oid]);
    }

    /// The root's files, `.git` left out, copied to `to`.
    pub fn copy_without_git(&self, to: &Path) {
        fn walk(from: &Path, to: &Path) {
            fs::create_dir_all(to).expect("a directory");
            for entry in fs::read_dir(from).expect("readable").flatten() {
                let path = entry.path();
                if entry.file_name() == ".git" {
                    continue;
                }
                let target = to.join(entry.file_name());
                if path.is_dir() {
                    walk(&path, &target);
                } else {
                    fs::write(&target, fs::read(&path).expect("a file")).expect("a copy");
                }
            }
        }
        walk(&self.root, to);
    }

    /// The variables a `spec` process runs with: the sandbox's, `HOME` the
    /// scene's (or `home`), then `extra` over them.
    pub fn vars_with(&self, home: &Path, extra: &[(&str, &OsStr)]) -> Vec<(String, OsString)> {
        let mut vars: Vec<(String, OsString)> = self
            .git
            .vars()
            .into_iter()
            .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
            .filter(|(name, _)| name != "HOME")
            .collect();
        vars.push(("HOME".to_owned(), home.as_os_str().to_owned()));
        for (name, value) in extra {
            vars.retain(|(known, _)| known != name);
            vars.push(((*name).to_owned(), value.to_os_string()));
        }
        vars
    }

    pub fn vars(&self, extra: &[(&str, &OsStr)]) -> Vec<(String, OsString)> {
        self.vars_with(&self.home, extra)
    }
}
