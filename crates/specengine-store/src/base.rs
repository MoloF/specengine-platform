//! The base of `spec check --staged` (docs/features/spec-cli-introduced.md)
//! and `--changed` (docs/features/spec-cli-changed.md): `HEAD`'s tree
//! under the root, walked and checked under the checked tree's scheme,
//! `[paths]` and check tables, so a config change never turns an old
//! violation into an introduced one; `HEAD`'s config gives only its mode,
//! `HEAD`'s baseline the new-debt rule. Core's
//! [`specengine_core::check::judge`] compares.
//!
//! Git, through the check's one [`Staged`] (its environment, its guard, its
//! one `cat-file --batch` session): `rev-parse --verify -q HEAD`, when born
//! `ls-tree -r -z <oid>`; then `HEAD`'s config and baseline (after the
//! checked ones), and after the index's blobs those of `HEAD`'s listed
//! paths whose (path, OID) the index's listing lacks. Each OID is read
//! once ([`Staged::blob`]): bytes are shared by OID across paths, a parse
//! only at the same path, where the checked [`CheckFile`] is reused.
//!
//! `--changed` has no index ([`Staged::head_only`]): every listed blob is
//! read, and every listed object judged before anything is shared or
//! parsed ([`HeadWalk::findings_by_bytes`]); a `HEAD` file reuses the
//! checked [`CheckFile`] only at the same path, read without error, with
//! bytes equal to `HEAD`'s blob.
//!
//! `HEAD`'s files are named in notes only as [`CONFIG_FILE`](crate::CONFIG_FILE),
//! [`BASELINE_FILE`](crate::BASELINE_FILE) or the flag's value as typed.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use specengine_core::WalkScope;
use specengine_core::check::{
    self, Baseline, Cause, CheckConfig, CheckFile, CheckInput, Finding, Mode,
};
use specengine_model::IdScheme;

use crate::check::{CheckSetup, parse_file, split_listing};
use crate::git::{Blob, Entry, EntryKind, GitFailure, Staged};
use crate::source::{GitIndex, IndexWalk, Listing, walk_index};

/// The cause, at its path, of a document `HEAD` lists whose blob is not in
/// the object database: the base would be partial (the staged side's
/// counterpart is [`MISSING_BLOB`](crate::git::MISSING_BLOB)). No fetch is
/// tried.
const MISSING_HEAD_BLOB: &str = "HEAD's blob is missing from the git object database";

/// The cause, at its path, of a document `HEAD` lists as a regular file
/// whose OID names a tree, a commit or a tag: the base would be partial
/// (the staged side's counterpart is [`NOT_A_BLOB`](crate::git::NOT_A_BLOB)).
const HEAD_NOT_A_BLOB: &str = "HEAD's object is not a blob";

/// The cause, at its path, of a document `HEAD` lists whose object the
/// session does not hold: the base would be partial. Not reached while
/// [`HeadWalk::read`] reads every listed OID; it fails closed should that
/// ever change.
const HEAD_BLOB_NOT_READ: &str = "HEAD's blob was not read";

#[cfg(test)]
thread_local! {
    /// Test seam (AC-14): files the base parsed itself, on this thread;
    /// a file reused from the checked run is not counted.
    pub(crate) static BASE_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Where the checked config or baseline lies, so `HEAD`'s counterpart can
/// be found, and how notes name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Placed {
    /// [`CONFIG_FILE`](crate::CONFIG_FILE), [`BASELINE_FILE`](crate::BASELINE_FILE), or the flag's value as typed.
    name: String,
    /// The flag that gave it (`--config`, `--baseline`); `None` for the
    /// root's own file.
    flag: Option<&'static str>,
    /// Root-relative, `/`-separated: where `HEAD`'s counterpart is looked
    /// up; `None` for a given file outside the root (or of unknown
    /// location): `HEAD`'s counterpart is unknown.
    path: Option<Vec<u8>>,
}

impl Placed {
    /// The root's own file `name`.
    pub(crate) fn root_file(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            flag: None,
            path: Some(name.as_bytes().to_vec()),
        }
    }

    /// A file given by `flag`, named `name`, at `path` on disk (`None`:
    /// unknown); `root` canonical. Below the root when its canonical path
    /// is.
    pub(crate) fn given(root: &Path, flag: &'static str, name: &str, path: Option<&Path>) -> Self {
        let path = path
            .and_then(|path| std::fs::canonicalize(path).ok())
            .and_then(|path| path.strip_prefix(root).ok().and_then(relative_bytes));
        Self {
            name: name.to_owned(),
            flag: Some(flag),
            path,
        }
    }

    /// How a note names the file outside the root: the flag and its value.
    fn outside(&self) -> String {
        match self.flag {
            Some(flag) => format!("{flag} {}", self.name),
            None => self.name.clone(),
        }
    }
}

/// `rel`'s components joined by `/`; `None` for an empty path or a
/// component that is not a plain name.
fn relative_bytes(rel: &Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    for component in rel.components() {
        let Component::Normal(name) = component else {
            return None;
        };
        if !bytes.is_empty() {
            bytes.push(b'/');
        }
        bytes.extend_from_slice(&os_bytes(name)?);
    }
    (!bytes.is_empty()).then_some(bytes)
}

#[cfg(unix)]
fn os_bytes(name: &std::ffi::OsStr) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    Some(name.as_bytes().to_vec())
}

#[cfg(not(unix))]
fn os_bytes(name: &std::ffi::OsStr) -> Option<Vec<u8>> {
    name.to_str().map(|name| name.as_bytes().to_vec())
}

/// `HEAD`'s side of a staged check, before its documents are read.
#[derive(Debug)]
pub(crate) struct Head {
    /// `HEAD`'s tree under the root, byte-sorted; empty when unborn.
    entries: Vec<Entry>,
    /// `HEAD`'s `[check] mode`: `None` when absent or not known.
    pub(crate) mode: Option<Mode>,
    /// `HEAD`'s baseline at the checked baseline's path, empty when absent
    /// there; `None` when not known (the new-debt rule is lifted).
    pub(crate) baseline: Option<Baseline>,
    /// Why `HEAD`'s mode is not known.
    config_note: Option<String>,
    /// Why `HEAD`'s baseline is not known.
    baseline_note: Option<String>,
}

impl Head {
    /// Probes `HEAD`, lists its tree under the root when born, then reads
    /// its config and its baseline at the checked ones' places. A git
    /// failure is the check's; anything wrong with `HEAD`'s files is a
    /// note. `checked` names the mode a note says the check runs in.
    pub(crate) fn read(
        staged: &mut Staged,
        config: &Placed,
        baseline: &Placed,
        checked: Mode,
    ) -> Result<Self, GitFailure> {
        let entries = match staged.head()? {
            Some(oid) => staged.head_entries(&oid)?,
            None => Vec::new(),
        };
        let mut head = Self {
            entries,
            mode: None,
            baseline: None,
            config_note: None,
            baseline_note: None,
        };
        match head.read_file(staged, config)? {
            Counterpart::Absent => {}
            Counterpart::Unknown(why) => {
                head.config_note = Some(format!(
                    "{why}: HEAD's mode is unknown, so the check runs in `{checked}`"
                ));
            }
            Counterpart::Text(text) => match CheckConfig::from_toml(text) {
                Ok(tables) => head.mode = Some(tables.mode),
                Err(error) => {
                    // Invalid TOML and invalid check tables alike.
                    head.config_note = Some(format!(
                        "HEAD's {} is not a valid config{}: HEAD's mode is unknown, so the check runs in `{checked}`",
                        config.name,
                        at_line(error.line)
                    ));
                }
            },
        }
        match head.read_file(staged, baseline)? {
            Counterpart::Absent => head.baseline = Some(Baseline::empty()),
            Counterpart::Unknown(why) => {
                head.baseline_note = Some(format!(
                    "{why}: new baseline entries are not judged against HEAD"
                ));
            }
            Counterpart::Text(text) => match Baseline::from_toml(text) {
                Ok(read) => head.baseline = Some(read),
                Err(error) => {
                    head.baseline_note = Some(format!(
                        "HEAD's {} is not a valid baseline{}: new baseline entries are not judged against HEAD",
                        baseline.name,
                        at_line(error.line)
                    ));
                }
            },
        }
        Ok(head)
    }

    /// `HEAD`'s counterpart of `placed`, its text borrowed from the
    /// session's object.
    fn read_file<'s>(
        &self,
        staged: &'s mut Staged,
        placed: &Placed,
    ) -> Result<Counterpart<'s>, GitFailure> {
        let Some(path) = &placed.path else {
            return Ok(Counterpart::Unknown(format!(
                "{} is outside the root",
                placed.outside()
            )));
        };
        let Some(entry) = entry_at(&self.entries, path) else {
            return Ok(Counterpart::Absent);
        };
        let unreadable = |why: &str| {
            Counterpart::Unknown(format!("HEAD's {} cannot be read ({why})", placed.name))
        };
        let why = match entry.kind {
            EntryKind::Regular => match staged.blob(&entry.oid)? {
                Blob::Bytes(bytes) => match std::str::from_utf8(bytes) {
                    Ok(text) => return Ok(Counterpart::Text(text)),
                    Err(_) => "not UTF-8",
                },
                Blob::Missing => "its blob is missing from the git object database",
                Blob::Foreign => "its object is not a blob",
            },
            EntryKind::Symlink => "a symbolic link",
            EntryKind::Gitlink => "a submodule",
            EntryKind::Other => "not a regular file",
        };
        Ok(unreadable(why))
    }

    /// `HEAD`'s listing by the checked `[paths]` (the index's scope), and
    /// each listed path's OID.
    pub(crate) fn walk(&self, index: &IndexWalk) -> HeadWalk {
        self.walk_by(&index.scope)
    }

    /// `HEAD`'s listing by `scope`, the checked `[paths]`, and each listed
    /// path's OID.
    pub(crate) fn walk_by(&self, scope: &WalkScope) -> HeadWalk {
        let (listing, listed) = walk_index(&self.entries, scope);
        HeadWalk { listing, listed }
    }

    /// The notes, in order: `HEAD`'s config unknown, its stricter mode
    /// applied (`checked` the checked config's mode), its baseline unknown.
    pub(crate) fn notes(&self, config: &Placed, checked: Mode) -> Vec<String> {
        let mut notes = Vec::new();
        notes.extend(self.config_note.clone());
        if let Some(mode) = self.mode
            && mode > checked
        {
            notes.push(format!(
                "HEAD's {} sets mode `{mode}`, stricter than `{checked}`: the check runs in `{mode}`",
                config.name
            ));
        }
        notes.extend(self.baseline_note.clone());
        notes
    }
}

/// ` (line N)` when the line is known.
fn at_line(line: Option<usize>) -> String {
    line.map(|line| format!(" (line {line})"))
        .unwrap_or_default()
}

/// `HEAD`'s counterpart of the checked config or baseline.
enum Counterpart<'s> {
    /// No entry at that path (no note).
    Absent,
    /// Not known: why, for the note.
    Unknown(String),
    /// Its text.
    Text(&'s str),
}

/// The entry at exactly `path` of the byte-sorted `entries`.
fn entry_at<'e>(entries: &'e [Entry], path: &[u8]) -> Option<&'e Entry> {
    entries
        .binary_search_by(|entry| entry.path.as_slice().cmp(path))
        .ok()
        .map(|at| &entries[at])
}

/// `HEAD`'s listing under the checked `[paths]`.
#[derive(Debug)]
pub(crate) struct HeadWalk {
    listing: Listing,
    /// Every listed path and its blob's OID.
    listed: BTreeMap<String, String>,
}

impl HeadWalk {
    /// Reads every listed path's blob, in path order. An OID already read
    /// is not requested again. `--staged` (`read_base`) calls it after the
    /// index's blobs, so every (path, OID) the index's listing shares is a
    /// no-op and only those it lacks reach git. `--changed` (`read_head`)
    /// reads no index: every listed OID reaches git once, unless it is the
    /// OID of `HEAD`'s config or baseline, read before. Afterwards the session
    /// holds every listed OID, whichever of them [`HeadWalk::findings`] or
    /// [`HeadWalk::findings_by_bytes`] parses: no split to keep in step.
    pub(crate) fn read(&self, staged: &mut Staged) -> Result<(), GitFailure> {
        for oid in self.listed.values() {
            staged.blob(oid)?;
        }
        Ok(())
    }

    /// The base's findings: `HEAD`'s files checked under the checked
    /// tree's rules with an empty baseline and the same date, its causes
    /// dropped. A path whose OID the index lists there reuses `checked`'s
    /// file (no read, no parse); every other file is parsed from the bytes
    /// the session read, moved out of `index` at the OID's last use.
    ///
    /// `Err`, before any file is parsed: the base would be partial, so
    /// nothing is judged by it: a cause at each such file's path,
    /// [`MISSING_HEAD_BLOB`] for a blob not in the object database,
    /// [`HEAD_NOT_A_BLOB`] for an OID naming another object type (one
    /// shared with the index at the same path is the checked run's cause
    /// instead), [`HEAD_BLOB_NOT_READ`] for an OID the session lacks.
    pub(crate) fn findings(
        self,
        index: &mut GitIndex,
        checked: CheckInput,
        setup: &CheckSetup,
        today: &str,
    ) -> Result<Vec<Finding>, Vec<Cause>> {
        // Both in path order: a merge, no lookup table.
        let mut shared = checked.files;
        shared.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        let mut shared = shared.into_iter().peekable();
        let (_, problems) = split_listing(self.listing);
        let mut input = CheckInput {
            files: Vec::with_capacity(self.listed.len()),
            problems,
        };
        let mut fresh = Vec::new();
        for (path, oid) in self.listed {
            while shared.next_if(|file| file.path < path).is_some() {}
            if index.oid_of(&path) == Some(oid.as_str())
                && let Some(file) = shared.next_if(|file| file.path == path)
            {
                input.files.push(file);
            } else {
                fresh.push((path, oid));
            }
        }
        // By OID, so an OID's last use is known: its bytes are moved there,
        // copied before (the engine orders files by path itself).
        fresh.sort_unstable_by(|a, b| a.1.cmp(&b.1));
        // Every file's object is judged before any file is parsed: a
        // partial base parses nothing.
        let partial: Vec<Cause> = fresh
            .iter()
            .filter_map(|(path, oid)| {
                partial_cause(index.object(oid)).map(|message| Cause {
                    path: path.clone(),
                    message: message.to_owned(),
                })
            })
            .collect();
        if !partial.is_empty() {
            return Err(partial);
        }
        let take = |oid: &str, last: bool| {
            if last {
                index.take_object(oid)
            } else {
                index.object(oid).cloned()
            }
        };
        run_base(input, fresh, take, setup, today)
    }

    /// The base's findings of `spec check --changed`
    /// (docs/features/spec-cli-changed.md): [`HeadWalk::findings`] with the
    /// working tree as the checked side, `objects` every object the
    /// session read.
    ///
    /// Every listed object is judged first, before any file is shared or
    /// parsed (sharing needs `HEAD`'s bytes): `Err` with a cause at each
    /// path whose object is not a blob read ([`MISSING_HEAD_BLOB`],
    /// [`HEAD_NOT_A_BLOB`], [`HEAD_BLOB_NOT_READ`]), none exempt. Then a
    /// path reuses `checked`'s file only when that file lies at the same
    /// path, was read without error and holds exactly `HEAD`'s blob's
    /// bytes; never across paths. Every other file is parsed from `HEAD`'s
    /// bytes.
    pub(crate) fn findings_by_bytes(
        self,
        mut objects: BTreeMap<String, Blob>,
        checked: CheckInput,
        setup: &CheckSetup,
        today: &str,
    ) -> Result<Vec<Finding>, Vec<Cause>> {
        let partial: Vec<Cause> = self
            .listed
            .iter()
            .filter_map(|(path, oid)| {
                partial_cause(objects.get(oid)).map(|message| Cause {
                    path: path.clone(),
                    message: message.to_owned(),
                })
            })
            .collect();
        if !partial.is_empty() {
            return Err(partial);
        }
        // Both in path order: a merge, no lookup table.
        let mut shared = checked.files;
        shared.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        let mut shared = shared.into_iter().peekable();
        let (_, problems) = split_listing(self.listing);
        let mut input = CheckInput {
            files: Vec::with_capacity(self.listed.len()),
            problems,
        };
        let mut fresh = Vec::new();
        for (path, oid) in self.listed {
            while shared.next_if(|file| file.path < path).is_some() {}
            let same_bytes = |file: &CheckFile| {
                file.path == path
                    && file.read_error.is_none()
                    && matches!(objects.get(&oid), Some(Blob::Bytes(bytes)) if *bytes == file.bytes)
            };
            if let Some(file) = shared.next_if(same_bytes) {
                input.files.push(file);
            } else {
                fresh.push((path, oid));
            }
        }
        // By OID, so an OID's last use is known (see `run_base`).
        fresh.sort_unstable_by(|a, b| a.1.cmp(&b.1));
        let take = |oid: &str, last: bool| {
            if last {
                objects.remove(oid)
            } else {
                objects.get(oid).cloned()
            }
        };
        run_base(input, fresh, take, setup, today)
    }
}

/// The base's run: `input` holds the files reused from the checked run;
/// each of `fresh` (path, OID), sorted by OID, is parsed from the object
/// `take` gives for its OID, told whether this is the OID's last use (its
/// bytes may then be moved out; before, they are copied). Then
/// `check::run` with an empty baseline, `today`, its causes dropped.
fn run_base(
    mut input: CheckInput,
    fresh: Vec<(String, String)>,
    mut take: impl FnMut(&str, bool) -> Option<Blob>,
    setup: &CheckSetup,
    today: &str,
) -> Result<Vec<Finding>, Vec<Cause>> {
    let mut fresh = fresh.into_iter().peekable();
    let scheme = &setup.project.scheme;
    while let Some((path, oid)) = fresh.next() {
        let last = fresh.peek().is_none_or(|(_, next)| *next != oid);
        match take(&oid, last) {
            Some(Blob::Bytes(bytes)) => input.files.push(parse_head_file(path, bytes, scheme)),
            // Not reached: every object was seen before, and an OID is
            // moved out only at its last use. Fail closed regardless.
            other => {
                let message = partial_cause(other.as_ref()).unwrap_or(HEAD_BLOB_NOT_READ);
                return Err(vec![Cause {
                    path,
                    message: message.to_owned(),
                }]);
            }
        }
    }
    Ok(check::run(
        &input,
        scheme,
        &setup.project.paths,
        &setup.config,
        &Baseline::empty(),
        today,
    )
    .findings)
}

/// Why a `HEAD` file whose OID the session read as `blob` would make the
/// base partial: `None` when its bytes were read.
fn partial_cause(blob: Option<&Blob>) -> Option<&'static str> {
    match blob {
        Some(Blob::Bytes(_)) => None,
        Some(Blob::Missing) => Some(MISSING_HEAD_BLOB),
        Some(Blob::Foreign) => Some(HEAD_NOT_A_BLOB),
        None => Some(HEAD_BLOB_NOT_READ),
    }
}

/// One of `HEAD`'s files parsed by the base itself.
fn parse_head_file(path: String, bytes: Vec<u8>, scheme: &IdScheme) -> CheckFile {
    #[cfg(test)]
    BASE_PARSES.with(|parses| parses.set(parses.get() + 1));
    parse_file(path, bytes, scheme)
}

/// AC-14 of docs/features/spec-cli-introduced.md: the base parses only
/// `HEAD`'s files the index does not hold at the same path with the same
/// blob. N unchanged files, one changed, one renamed (`git mv`, the same
/// blob at a new path) → exactly 2 parses (the changed file's `HEAD`
/// version and the renamed file's old path); nothing changed → none. A
/// real scratch repository, every git process isolated (no global or
/// system config, a scratch `HOME`, a ceiling at the scratch).
#[cfg(test)]
mod base_parses {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use specengine_core::check::Verdict;

    use super::BASE_PARSES;
    use crate::{GitEnv, check_staged_with_notes};

    const UNCHANGED: usize = 6;

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    /// A scratch directory, canonical, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-store-base-parses-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(path.join("home")).expect("scratch");
            Self(std::fs::canonicalize(&path).expect("canonical scratch"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn vars(scratch: &Path) -> Vec<(String, String)> {
        let home = scratch.join("home").display().to_string();
        vec![
            (
                "PATH".to_owned(),
                std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned()),
            ),
            ("HOME".to_owned(), home.clone()),
            ("XDG_CONFIG_HOME".to_owned(), home),
            ("GIT_CONFIG_GLOBAL".to_owned(), "/dev/null".to_owned()),
            ("GIT_CONFIG_NOSYSTEM".to_owned(), "1".to_owned()),
            (
                "GIT_CEILING_DIRECTORIES".to_owned(),
                scratch.display().to_string(),
            ),
            ("GIT_AUTHOR_NAME".to_owned(), "Scratch".to_owned()),
            (
                "GIT_AUTHOR_EMAIL".to_owned(),
                "a@example.invalid".to_owned(),
            ),
            (
                "GIT_AUTHOR_DATE".to_owned(),
                "2026-01-01T00:00:00+0000".to_owned(),
            ),
            ("GIT_COMMITTER_NAME".to_owned(), "Scratch".to_owned()),
            (
                "GIT_COMMITTER_EMAIL".to_owned(),
                "c@example.invalid".to_owned(),
            ),
            (
                "GIT_COMMITTER_DATE".to_owned(),
                "2026-01-01T00:00:00+0000".to_owned(),
            ),
        ]
    }

    fn git(scratch: &Path, dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(dir)
            .env_clear()
            .envs(vars(scratch))
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
        std::fs::write(path, text).expect("a file");
    }

    fn document(title: &str) -> String {
        format!(
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# {title}\n\nText of {title}.\n"
        )
    }

    /// The base's parses of one staged check of `repo`, and its report's
    /// verdict.
    fn parses(scratch: &Path, repo: &Path) -> (usize, Verdict, usize) {
        BASE_PARSES.with(|count| count.set(0));
        let env = GitEnv::new(repo, vars(scratch));
        let checked = check_staged_with_notes(repo, None, None, &env, "2026-10-01");
        assert!(
            checked.report.cannot_check.is_empty(),
            "{:?}",
            checked.report
        );
        (
            BASE_PARSES.with(std::cell::Cell::get),
            checked.report.verdict,
            checked.report.counts.documents,
        )
    }

    #[test]
    fn the_base_parses_only_what_the_index_does_not_share() {
        let scratch = Scratch::new();
        let repo = scratch.0.join("repo");
        git(
            &scratch.0,
            &scratch.0,
            &["init", "--template=", "-q", "-b", "main", "repo"],
        );
        write(
            &repo,
            "specengine.toml",
            "[paths]\nroots = [\"notes\"]\n\n[ids]\nR = { kind = \"requirement\", width = 2 }\n",
        );
        for number in 0..UNCHANGED {
            write(
                &repo,
                &format!("notes/same-{number}.md"),
                &document(&format!("Same {number}")),
            );
        }
        write(&repo, "notes/changed.md", &document("Before"));
        write(&repo, "notes/old.md", &document("Renamed"));
        git(&scratch.0, &repo, &["add", "-A"]);
        git(
            &scratch.0,
            &repo,
            &["commit", "-q", "--no-verify", "-m", "base"],
        );

        let (count, verdict, documents) = parses(&scratch.0, &repo);
        assert_eq!(count, 0, "nothing changed: every HEAD file is shared");
        assert_eq!(verdict, Verdict::Clean);
        assert_eq!(documents, UNCHANGED + 2);

        write(&repo, "notes/changed.md", &document("After"));
        git(&scratch.0, &repo, &["mv", "notes/old.md", "notes/new.md"]);
        git(&scratch.0, &repo, &["add", "-A"]);
        let (count, verdict, documents) = parses(&scratch.0, &repo);
        assert_eq!(count, 2, "the changed file's HEAD side and the old path");
        assert_eq!(verdict, Verdict::Clean);
        assert_eq!(documents, UNCHANGED + 2);
    }
}

/// A partial base (docs/features/spec-cli-introduced.md, AC-5 and AC-14,
/// implementation iteration 3): [`partial_cause`] per object state; a
/// `HEAD` OID the session does not hold is [`HEAD_BLOB_NOT_READ`] at its
/// path, never a skipped or unreadable file; and a partial base returns
/// before it parses anything. The private flow of `read_base` and
/// `check_staged_with_notes`, step by step, on real scratch repositories,
/// every git process isolated (no global or system config, a scratch
/// `HOME`, a ceiling at the scratch).
#[cfg(test)]
mod base_partial {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use specengine_core::check::{Cause, CheckInput, Finding};

    use super::{BASE_PARSES, Head, HeadWalk, Placed, partial_cause};
    use crate::check::{CheckSetup, NamedBytes, check_input, load_check};
    use crate::git::{Blob, Staged};
    use crate::source::{GitIndex, IndexWalk};
    use crate::{BASELINE_FILE, CONFIG_FILE, GitEnv, check_staged_with_notes};

    const TODAY: &str = "2026-10-01";
    const NOT_READ: &str = "HEAD's blob was not read";
    const MISSING: &str = "HEAD's blob is missing from the git object database";
    const FOREIGN: &str = "HEAD's object is not a blob";

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    /// A scratch directory, canonical, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-store-base-partial-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(path.join("home")).expect("scratch");
            Self(std::fs::canonicalize(&path).expect("canonical scratch"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn vars(scratch: &Path) -> Vec<(String, String)> {
        let home = scratch.join("home").display().to_string();
        let mut vars: Vec<(String, String)> = [
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_AUTHOR_NAME", "Scratch"),
            ("GIT_AUTHOR_EMAIL", "a@example.invalid"),
            ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00+0000"),
            ("GIT_COMMITTER_NAME", "Scratch"),
            ("GIT_COMMITTER_EMAIL", "c@example.invalid"),
            ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00+0000"),
        ]
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
        vars.push((
            "PATH".to_owned(),
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned()),
        ));
        vars.push(("HOME".to_owned(), home.clone()));
        vars.push(("XDG_CONFIG_HOME".to_owned(), home));
        vars.push((
            "GIT_CEILING_DIRECTORIES".to_owned(),
            scratch.display().to_string(),
        ));
        vars
    }

    /// Runs git in `dir`, isolated; its stdout, trimmed.
    fn git(scratch: &Path, dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(dir)
            .env_clear()
            .envs(vars(scratch))
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("UTF-8 output")
            .trim()
            .to_owned()
    }

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
        std::fs::write(path, text).expect("a file");
    }

    fn document(title: &str) -> String {
        format!(
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# {title}\n\nText of {title}.\n"
        )
    }

    fn config(roots: &str) -> String {
        format!(
            "[paths]\nroots = [\"{roots}\"]\n\n[ids]\nR = {{ kind = \"requirement\", width = 2 }}\n"
        )
    }

    /// A scratch repository with `config` committed and `documents`
    /// committed, then each rewritten and staged (every one changed).
    fn repository(scratch: &Path, config: &str, documents: &[&str]) -> PathBuf {
        let repo = scratch.join("repo");
        git(
            scratch,
            scratch,
            &["init", "--template=", "-q", "-b", "main", "repo"],
        );
        write(&repo, CONFIG_FILE, config);
        for path in documents {
            write(&repo, path, &document(&format!("Before {path}")));
        }
        git(scratch, &repo, &["add", "-A"]);
        git(
            scratch,
            &repo,
            &["commit", "-q", "--no-verify", "-m", "base"],
        );
        for path in documents {
            write(&repo, path, &document(&format!("After {path}")));
        }
        git(scratch, &repo, &["add", "-A"]);
        repo
    }

    /// The base about to judge: `HEAD`'s walk, the index (every object the
    /// session read), the checked run's input and the setup.
    struct Prepared {
        head_walk: HeadWalk,
        index: GitIndex,
        input: CheckInput,
        setup: CheckSetup,
    }

    /// `read_base` and `check_staged_with_notes` up to the base's
    /// findings, step by step; `HeadWalk::read` only when `read_head`.
    fn prepare(scratch: &Path, repo: &Path, config: &str, read_head: bool) -> Prepared {
        let env = GitEnv::new(repo, vars(scratch));
        let mut staged = Staged::read(repo, &env).expect("the index is listed");
        let named = NamedBytes {
            name: CONFIG_FILE.to_owned(),
            bytes: Ok(config.as_bytes().to_vec()),
        };
        let setup = load_check(&named, None).expect("the config loads");
        let head = Head::read(
            &mut staged,
            &Placed::root_file(CONFIG_FILE),
            &Placed::root_file(BASELINE_FILE),
            setup.config.mode,
        )
        .expect("HEAD is read");
        let walk = IndexWalk::new(&staged.entries, &setup.project.paths);
        walk.read(&mut staged).expect("the index's blobs are read");
        let head_walk = head.walk(&walk);
        if read_head {
            head_walk.read(&mut staged).expect("HEAD's blobs are read");
        }
        staged.finish().expect("the session ends");
        let index = GitIndex::from_walk(staged, walk);
        let input = check_input(&index, &setup.project.scheme);
        Prepared {
            head_walk,
            index,
            input,
            setup,
        }
    }

    /// The base's findings of `prepared`, and the files it parsed itself.
    fn judge(prepared: Prepared) -> (Result<Vec<Finding>, Vec<Cause>>, usize) {
        let Prepared {
            head_walk,
            mut index,
            input,
            setup,
        } = prepared;
        BASE_PARSES.with(|parses| parses.set(0));
        let result = head_walk.findings(&mut index, input, &setup, TODAY);
        (result, BASE_PARSES.with(std::cell::Cell::get))
    }

    fn cause(path: &str, message: &str) -> Cause {
        Cause {
            path: path.to_owned(),
            message: message.to_owned(),
        }
    }

    #[test]
    fn partial_cause_names_each_object_state() {
        assert_eq!(partial_cause(None), Some(NOT_READ));
        assert_eq!(partial_cause(Some(&Blob::Missing)), Some(MISSING));
        assert_eq!(partial_cause(Some(&Blob::Foreign)), Some(FOREIGN));
        assert_eq!(partial_cause(Some(&Blob::Bytes(Vec::new()))), None);
        assert_eq!(
            partial_cause(Some(&Blob::Bytes(document("Read").into_bytes()))),
            None
        );
    }

    /// A changed `doc.md` whose `HEAD` OID the session does not hold:
    /// `HeadWalk::read` skipped, or its object taken out after it. Either
    /// way `Err` at `doc.md`, nothing parsed; with the object held the same
    /// flow judges (control).
    #[test]
    fn a_head_oid_the_session_lacks_is_not_read_at_its_path() {
        let scratch = Scratch::new();
        let config = config("doc.md");
        let repo = repository(&scratch.0, &config, &["doc.md"]);
        let head_oid = git(&scratch.0, &repo, &["rev-parse", "HEAD:doc.md"]);
        let staged_oid = git(&scratch.0, &repo, &["rev-parse", ":doc.md"]);
        assert_ne!(head_oid, staged_oid, "doc.md is changed");

        let control = prepare(&scratch.0, &repo, &config, true);
        let (result, parses) = judge(control);
        assert!(result.is_ok(), "the base judges: {result:?}");
        assert_eq!(parses, 1, "HEAD's doc.md is parsed by the base");

        let skipped = prepare(&scratch.0, &repo, &config, false);
        assert_eq!(skipped.head_walk.listed.get("doc.md"), Some(&head_oid));
        assert_eq!(skipped.index.oid_of("doc.md"), Some(staged_oid.as_str()));
        assert!(skipped.index.object(&head_oid).is_none(), "never read");
        let (result, parses) = judge(skipped);
        assert_eq!(result.err(), Some(vec![cause("doc.md", NOT_READ)]));
        assert_eq!(parses, 0);

        let mut taken = prepare(&scratch.0, &repo, &config, true);
        assert!(
            matches!(taken.index.take_object(&head_oid), Some(Blob::Bytes(_))),
            "HeadWalk::read read HEAD's blob"
        );
        let (result, parses) = judge(taken);
        assert_eq!(result.err(), Some(vec![cause("doc.md", NOT_READ)]));
        assert_eq!(parses, 0);
    }

    /// Six changed documents; `HEAD`'s loose object of the one whose `HEAD`
    /// OID sorts last is deleted, so a loop by OID meets five readable
    /// files first. `Err` with the missing-blob cause at that path and no
    /// parse, through the private flow and through
    /// `check_staged_with_notes`.
    #[test]
    fn a_partial_base_parses_nothing() {
        let scratch = Scratch::new();
        let config = config("notes");
        let paths: Vec<String> = (0..6).map(|number| format!("notes/d{number}.md")).collect();
        let documents: Vec<&str> = paths.iter().map(String::as_str).collect();
        let repo = repository(&scratch.0, &config, &documents);
        let mut at_head: Vec<(String, String)> = documents
            .iter()
            .map(|path| {
                let oid = git(&scratch.0, &repo, &["rev-parse", &format!("HEAD:{path}")]);
                ((*path).to_owned(), oid)
            })
            .collect();
        at_head.sort_by(|a, b| a.1.cmp(&b.1));
        let (gone_path, gone_oid) = at_head.last().cloned().expect("documents");
        assert_eq!(
            at_head.iter().filter(|(_, oid)| *oid < gone_oid).count(),
            5,
            "five fresh files sort before the missing one: {at_head:?}"
        );
        let loose = repo
            .join(".git/objects")
            .join(&gone_oid[..2])
            .join(&gone_oid[2..]);
        assert!(loose.is_file(), "{} is loose", loose.display());
        std::fs::remove_file(&loose).expect("the loose object is deleted");

        let prepared = prepare(&scratch.0, &repo, &config, true);
        assert_eq!(prepared.index.object(&gone_oid), Some(&Blob::Missing));
        let (result, parses) = judge(prepared);
        assert_eq!(result.err(), Some(vec![cause(&gone_path, MISSING)]));
        assert_eq!(parses, 0, "a partial base parses nothing");

        BASE_PARSES.with(|parses| parses.set(0));
        let env = GitEnv::new(&repo, vars(&scratch.0));
        let checked = check_staged_with_notes(&repo, None, None, &env, TODAY);
        assert_eq!(
            checked.report.cannot_check,
            vec![cause(&gone_path, MISSING)],
            "{:?}",
            checked.report
        );
        assert!(checked.report.findings.is_empty(), "{:?}", checked.report);
        assert_eq!(BASE_PARSES.with(std::cell::Cell::get), 0);
    }
}

/// AC-05 of docs/features/spec-cli-changed.md: under `--changed` the base
/// reuses the checked parse of a `HEAD` file only when the disk file at
/// the same path holds exactly its bytes. N unchanged documents, one
/// changed, one moved (a plain rename on disk, the same bytes at a new
/// path), one deleted, one untracked → exactly 3 parses (the changed
/// file's `HEAD` version, the moved file's old path, the deleted file);
/// nothing changed → none. A real scratch repository, nothing staged
/// after the commit, every git process isolated (no global or system
/// config, a scratch `HOME`, a ceiling at the scratch).
#[cfg(test)]
mod base_parses_changed {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use specengine_core::check::Verdict;

    use super::BASE_PARSES;
    use crate::{GitEnv, check_changed_with_notes};

    const UNCHANGED: usize = 6;

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    /// A scratch directory, canonical, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-store-base-parses-changed-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(path.join("home")).expect("scratch");
            Self(std::fs::canonicalize(&path).expect("canonical scratch"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn vars(scratch: &Path) -> Vec<(String, String)> {
        let home = scratch.join("home").display().to_string();
        vec![
            (
                "PATH".to_owned(),
                std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned()),
            ),
            ("HOME".to_owned(), home.clone()),
            ("XDG_CONFIG_HOME".to_owned(), home),
            ("GIT_CONFIG_GLOBAL".to_owned(), "/dev/null".to_owned()),
            ("GIT_CONFIG_NOSYSTEM".to_owned(), "1".to_owned()),
            (
                "GIT_CEILING_DIRECTORIES".to_owned(),
                scratch.display().to_string(),
            ),
            ("GIT_AUTHOR_NAME".to_owned(), "Scratch".to_owned()),
            (
                "GIT_AUTHOR_EMAIL".to_owned(),
                "a@example.invalid".to_owned(),
            ),
            (
                "GIT_AUTHOR_DATE".to_owned(),
                "2026-01-01T00:00:00+0000".to_owned(),
            ),
            ("GIT_COMMITTER_NAME".to_owned(), "Scratch".to_owned()),
            (
                "GIT_COMMITTER_EMAIL".to_owned(),
                "c@example.invalid".to_owned(),
            ),
            (
                "GIT_COMMITTER_DATE".to_owned(),
                "2026-01-01T00:00:00+0000".to_owned(),
            ),
        ]
    }

    fn git(scratch: &Path, dir: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new("git")
            .current_dir(dir)
            .env_clear()
            .envs(vars(scratch))
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
        std::fs::write(path, text).expect("a file");
    }

    fn document(title: &str) -> String {
        format!(
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# {title}\n\nText of {title}.\n"
        )
    }

    /// The base's parses of one `--changed` check of `repo`, its verdict
    /// and its document count.
    fn parses(scratch: &Path, repo: &Path) -> (usize, Verdict, usize) {
        BASE_PARSES.with(|count| count.set(0));
        let env = GitEnv::new(repo, vars(scratch));
        let checked = check_changed_with_notes(repo, None, None, &env, "2026-10-01");
        assert!(
            checked.report.cannot_check.is_empty(),
            "{:?}",
            checked.report
        );
        assert!(checked.notes.is_empty(), "{:?}", checked.notes);
        (
            BASE_PARSES.with(std::cell::Cell::get),
            checked.report.verdict,
            checked.report.counts.documents,
        )
    }

    #[test]
    fn the_base_parses_only_what_the_disk_does_not_hold_at_its_path() {
        let scratch = Scratch::new();
        let repo = scratch.0.join("repo");
        git(
            &scratch.0,
            &scratch.0,
            &["init", "--template=", "-q", "-b", "main", "repo"],
        );
        write(
            &repo,
            "specengine.toml",
            "[paths]\nroots = [\"notes\"]\n\n[ids]\nR = { kind = \"requirement\", width = 2 }\n\n[check]\nmode = \"enforce-introduced\"\n",
        );
        for number in 0..UNCHANGED {
            write(
                &repo,
                &format!("notes/same-{number}.md"),
                &document(&format!("Same {number}")),
            );
        }
        write(&repo, "notes/changed.md", &document("Before"));
        write(&repo, "notes/old.md", &document("Moved"));
        write(&repo, "notes/gone.md", &document("Deleted"));
        git(&scratch.0, &repo, &["add", "-A"]);
        git(
            &scratch.0,
            &repo,
            &["commit", "-q", "--no-verify", "-m", "base"],
        );

        let (count, verdict, documents) = parses(&scratch.0, &repo);
        assert_eq!(count, 0, "nothing changed: every HEAD file is shared");
        assert_eq!(verdict, Verdict::Clean);
        assert_eq!(documents, UNCHANGED + 3);

        write(&repo, "notes/changed.md", &document("After"));
        std::fs::rename(repo.join("notes/old.md"), repo.join("notes/new.md")).expect("mv");
        std::fs::remove_file(repo.join("notes/gone.md")).expect("rm");
        write(&repo, "notes/untracked.md", &document("Untracked"));
        assert!(
            git(&scratch.0, &repo, &["diff", "--cached", "--name-only"]).is_empty(),
            "nothing staged"
        );
        let (count, verdict, documents) = parses(&scratch.0, &repo);
        assert_eq!(
            count, 3,
            "the changed file's HEAD side, the moved file's old path, the deleted file"
        );
        assert_eq!(verdict, Verdict::Clean);
        assert_eq!(documents, UNCHANGED + 3);
    }
}
