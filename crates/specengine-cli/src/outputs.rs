//! The index's outputs in an apply's commit (task spec `apply-outputs`):
//! every apply (`spec approve`, `spec round answer`, a hub's confirm) also
//! commits the outputs of the `[[generators]]` entry with `index = true`
//! whose bytes its change alters, rendered by SpecEngine (core
//! `render_index_set`), never by a project's own generator command.
//!
//! - **The committed tree** ([`Outputs::render`], store
//!   [`committed_tree`]): `HEAD` at step 2's commit under the recorded root,
//!   its `specengine.toml` and its index entry, each path the apply writes
//!   at the bytes step 8 writes; never the disk walk.
//! - **Never an output** ([`never_output`]): the apply's target, a file
//!   named `spec.lock`, `specengine.toml` or `.spec-debt.toml`, a path
//!   outside the walk, a document `HEAD` holds that is not `class:
//!   generated` (its front-matter unread included), one `HEAD` lacks whose
//!   bytes on disk do not read as a generated document: one the config
//!   names narrows the commit to the target, so no spec file and no
//!   person's uncommitted file is ever written by way of the config.
//! - **Which**: the outputs whose render differs from `HEAD`'s blob, or
//!   that `HEAD` lacks; none when `HEAD`'s config has no index entry (the
//!   commit as before, byte for byte).
//! - **Judged** at step 8: inspected as `spec export index` inspects (each
//!   component below the root a real directory, the file absent or a
//!   regular file, its parent there, no two outputs one file), one `HEAD`
//!   lacks not ignored by git, its name on disk byte for byte the
//!   configured one, none skip-worktree; then written atomically after the
//!   target ([`Outputs::write`]), staged in a step of their own
//!   ([`Outputs::stage`]), put back with the target when the commit fails
//!   ([`Outputs::restore`]), their committed blobs compared at step 10
//!   ([`Outputs::compare`]).
//! - **An output never refuses**: a gap (`HEAD`'s config unknown or
//!   invalid, a path never an output, a walk gap, a blob the object
//!   database lacks, a judge, a write or the staging failing) narrows the
//!   commit to the target with one warning naming the cause and the
//!   entry's `command`; the apply goes on. Nothing is blocked by an output.
//!
//! Completion's side: [`parent_outputs`], the outputs of the index entry in
//! the parent's root config that an apply may write (none the parent holds
//! as a document that is not generated), and [`Beside`], what a judge
//! accepts beside its path.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use specengine_core::check::{
    CheckConfig, DocClass, Generator, front_matter_failure, render_index_set,
};
use specengine_core::lock::LOCK_FILE;
use specengine_core::{Paths, ProjectConfig, is_clean_relative};
use specengine_model::IdScheme;
use specengine_store::{
    BASELINE_FILE, ChangedPath, CommittedTree, CreatedFile, DiskName, GitEnv, GitError, HeadFile,
    NamedBytes, Proposal, WorktreeGit, check_input, committed_tree, create_file, disk_name,
    load_config, replace_file,
};

use crate::Message;
use crate::export::{identity, incomplete};
use crate::project::{CONFIG_FILE, ProjectRoot};
use crate::proposals::top_of;
use crate::revisions::{Restored, restore};

/// The outputs an apply's commit carries beside its target, or the gap
/// that narrows it to the target.
#[derive(Debug)]
pub(crate) struct Outputs {
    /// The changed outputs, in the render's order (the root, the shards in
    /// config order, the roadmap view).
    files: Vec<Output>,
    /// The warning of a gap: the commit carries the target alone.
    gap: Option<String>,
    /// How the warning names what the commit carries: `` `<target>` ``
    /// (`` `<target>` and `spec.lock` `` for an editorial apply).
    alone: String,
    /// The index entry's `command`, byte for byte; `None` without one.
    command: Option<String>,
}

/// One changed output.
#[derive(Debug)]
struct Output {
    /// Root-relative.
    path: String,
    /// From the worktree's top.
    top_path: String,
    /// `<root>/<path>`.
    file: PathBuf,
    /// The render.
    bytes: Vec<u8>,
    /// The file on disk before the apply; `None`: absent.
    prior: Option<Vec<u8>>,
    /// Git's index has an entry at it.
    tracked: bool,
    /// Created by this run (absent before), once written.
    created: Option<CreatedFile>,
    /// Written by this run.
    written: bool,
    /// Its intent-to-add entry made by this run.
    staged: bool,
}

impl Outputs {
    /// Step 8, before anything is written: the outputs of `HEAD`'s index
    /// entry rendered from the committed tree of `recorded` at `head` with
    /// `overlay` (root-relative paths and the bytes the apply writes there,
    /// its target first) laid over it, those that differ from `HEAD`
    /// judged. Git runs with `git`'s variables, the local `GIT_*` ones
    /// dropped.
    pub(crate) fn render(
        git: &WorktreeGit,
        recorded: &ProjectRoot,
        proposal: &Proposal,
        head: &str,
        overlay: &[(&str, &[u8])],
    ) -> Self {
        let paths: Vec<&str> = overlay.iter().map(|(path, _)| *path).collect();
        let mut outputs = Self {
            files: Vec::new(),
            gap: None,
            alone: format!("`{}`", names(&paths)),
            command: None,
        };
        let target = paths.first().copied().unwrap_or_default();
        let env = GitEnv::new(recorded.root.clone(), git.vars());
        let committed = match committed_tree(&recorded.root, &env, head, overlay) {
            CommittedTree::NoIndex => return outputs,
            // `HEAD`'s entry unknown: the disk config's names the command;
            // with none there either, the commit as before, no warning.
            CommittedTree::Unknown(why) => {
                outputs.command = disk_command(&recorded.root);
                if outputs.command.is_some() {
                    outputs.narrow(&why);
                }
                return outputs;
            }
            CommittedTree::Indexed(committed) => committed,
        };
        let Some(generator) = committed.check.index_generator().cloned() else {
            return outputs;
        };
        outputs.command = Some(generator.command.clone());
        let project = &committed.project;
        let Some(index_path) = project.paths.index.clone() else {
            outputs.narrow("`[paths] index` is not set");
            return outputs;
        };
        // Never an output, whatever the config names: by its path, as
        // `HEAD` holds it, as the disk holds one `HEAD` lacks.
        for path in output_paths(&index_path, &generator) {
            let why = never_output(path, target, &project.paths).or_else(|| {
                match committed.at_head(path) {
                    HeadFile::Bytes(bytes) => (document_class(path, bytes, &project.scheme)
                        != Class::Generated)
                        .then_some(SPEC_DOCUMENT),
                    HeadFile::Absent => disk_document(&recorded.root, path).and_then(|bytes| {
                        match document_class(path, &bytes, &project.scheme) {
                            Class::Generated => None,
                            Class::Other => Some(SPEC_DOCUMENT),
                            Class::Unread => Some(NOT_GENERATED_ON_DISK),
                        }
                    }),
                    HeadFile::Unreadable(_) => None,
                }
            });
            if let Some(why) = why {
                outputs.narrow(&format!("`{path}` can never be an output ({why})"));
                return outputs;
            }
        }
        // The walk's gap first, the documents whose blobs are unread set
        // aside; then the first of those.
        let mut input = check_input(&committed.tree, &project.scheme);
        input
            .files
            .retain(|file| !committed.unread.contains_key(&file.path));
        if let Some(why) = incomplete(&input, &project.paths) {
            outputs.narrow(&why);
            return outputs;
        }
        if let Some(why) = committed.unread.values().next() {
            outputs.narrow(why);
            return outputs;
        }
        let rendered = render_index_set(&input, &index_path, &generator);
        let mut files = Vec::new();
        for output in rendered {
            let bytes = output.bytes.into_bytes();
            let at_head = match committed.at_head(&output.path) {
                HeadFile::Bytes(at_head) if *at_head == bytes => continue,
                HeadFile::Bytes(_) => true,
                HeadFile::Absent => false,
                HeadFile::Unreadable(why) => {
                    outputs.narrow(why);
                    return outputs;
                }
            };
            match judged(git, &recorded.root, proposal, output.path, bytes, at_head) {
                Ok(file) => files.push(file),
                Err(why) => {
                    outputs.narrow(&why);
                    return outputs;
                }
            }
        }
        if let Err(why) = one_file_each(&files) {
            outputs.narrow(&why);
            return outputs;
        }
        outputs.files = files;
        outputs
    }

    /// The commit narrowed to the target: no output, the warning of `why`.
    fn narrow(&mut self, why: &str) {
        self.files.clear();
        let command = self.command.as_deref().unwrap_or_default();
        self.gap = Some(format!(
            "the commit carries {} alone: the index's outputs were not rendered: {why}; \
             `{command}` writes them",
            self.alone
        ));
    }

    /// The warning of a gap, when the commit carries the target alone.
    pub(crate) fn gap(&self) -> Option<&str> {
        self.gap.as_deref()
    }

    /// The outputs the commit carries, root-relative, in the render's order.
    pub(crate) fn paths(&self) -> Vec<&str> {
        self.files.iter().map(|file| file.path.as_str()).collect()
    }

    /// The outputs the commit carries, from the worktree's top.
    pub(crate) fn top_paths(&self) -> Vec<String> {
        self.files
            .iter()
            .map(|file| file.top_path.clone())
            .collect()
    }

    /// The note of a commit carrying outputs: `None` without one.
    pub(crate) fn note(&self) -> Option<Message> {
        (!self.files.is_empty()).then(|| {
            Message::Note(format!(
                "the commit carries the index's outputs too: {}",
                self.paths().join(", ")
            ))
        })
    }

    /// Step 8, after the target: each output written atomically, an
    /// existing one replaced through a dot-named sibling, an absent one
    /// created, never replacing anything. A write that fails puts back
    /// those before it and narrows the commit to the target (its warning,
    /// `messages` naming what could not be put back).
    pub(crate) fn write(&mut self, worktree: &str, messages: &mut Vec<Message>) {
        let mut failure = None;
        for file in &mut self.files {
            let written = match &file.prior {
                Some(_) => replace_file(&file.file, &file.bytes).map_err(|error| error.to_string()),
                None => create_file_at(file),
            };
            match written {
                Ok(()) => file.written = true,
                Err(error) => {
                    failure = Some(format!("cannot write `{}`: {error}", file.path));
                    break;
                }
            }
        }
        if let Some(why) = failure {
            self.put_back(worktree, messages);
            self.narrow(&why);
        }
    }

    /// Step 9, a step of its own before the commit: an intent-to-add entry
    /// for each output git's index lacks, so `git commit --only` takes it.
    /// A staging that fails drops the entries this run made, puts the
    /// outputs back and narrows the commit to the target (its warning):
    /// never a refusal.
    pub(crate) fn stage(&mut self, git: &WorktreeGit, worktree: &str, messages: &mut Vec<Message>) {
        let mut failure = None;
        for file in &mut self.files {
            if file.tracked {
                continue;
            }
            match git.intent_to_add(&file.top_path) {
                Ok(()) => file.staged = true,
                Err(error) => {
                    failure = Some(format!("staging `{}` failed: {error}", file.path));
                    break;
                }
            }
        }
        if let Some(why) = failure {
            self.restore(git, worktree, messages);
            self.narrow(&why);
        }
    }

    /// The commit failed with no commit of this ID: the index entries this
    /// run made dropped, each written output back to its prior bytes while
    /// it holds this run's (one absent before removed); how each fared, by
    /// root-relative path.
    pub(crate) fn restore(
        &mut self,
        git: &WorktreeGit,
        worktree: &str,
        messages: &mut Vec<Message>,
    ) -> Vec<(String, Restored)> {
        for file in &mut self.files {
            if !file.staged {
                continue;
            }
            // Only while it is still this run's intent-to-add entry: a
            // person's own staging (a hook's `git add`) is never dropped.
            let dropped = match git.is_intent_to_add(&file.top_path) {
                Ok(true) => git.remove_cached(&file.top_path).map(|()| true),
                Ok(false) => Ok(false),
                Err(cause) => Err(cause),
            };
            match dropped {
                Ok(_) => file.staged = false,
                Err(cause) => messages.push(Message::Warning(format!(
                    "cannot drop the index entry of `{}` in {worktree}: {cause}; `git rm \
                     --cached -- {}` drops it while it is an intent-to-add entry",
                    file.path,
                    crate::pathspec_word(&file.top_path)
                ))),
            }
        }
        self.put_back(worktree, messages)
    }

    /// Every written output back to its prior bytes while it holds this
    /// run's (one absent before removed); how each fared.
    fn put_back(&mut self, worktree: &str, messages: &mut Vec<Message>) -> Vec<(String, Restored)> {
        let mut fared = Vec::new();
        for file in &mut self.files {
            if !file.written {
                continue;
            }
            let restored = match &file.prior {
                Some(prior) => restore(
                    &file.file,
                    &file.top_path,
                    worktree,
                    &file.bytes,
                    prior,
                    messages,
                ),
                None => match &file.created {
                    Some(created) if fs::read(&file.file).ok().as_deref() == Some(&file.bytes) => {
                        let left = created.remove();
                        if left.is_empty() {
                            Restored::Back
                        } else {
                            messages.extend(left.into_iter().map(Message::Warning));
                            Restored::Failed
                        }
                    }
                    _ => Restored::Left,
                },
            };
            file.written = false;
            fared.push((file.path.clone(), restored));
        }
        fared
    }

    /// Step 10, once the commit is this run's: each written output's blob
    /// in `commit`'s tree compared with this run's bytes; one warning per
    /// output that differs (a hook rewrote it, another apply in the
    /// worktree, a path the commit left as `HEAD` held it). Never a refusal.
    pub(crate) fn compare(&self, git: &WorktreeGit, commit: &str) -> Vec<Message> {
        let command = self.command.as_deref().unwrap_or_default();
        self.files
            .iter()
            .filter(|file| {
                git.blob_at(commit, &file.top_path)
                    .map_or(true, |blob| blob != file.bytes)
            })
            .map(|file| {
                Message::Warning(format!(
                    "the committed `{}` is not the bytes this apply rendered; `{command}` writes \
                     them",
                    file.path
                ))
            })
            .collect()
    }
}

/// What [`render_tree`] made of a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TreeRender {
    /// The tree's config has no `index = true` entry: nothing rendered.
    NoIndex,
    /// The outputs, root-relative, in the render's order, and their bytes.
    Rendered(Vec<(String, Vec<u8>)>),
    /// Nothing rendered: why, one line.
    Gap(String),
}

/// The index entry's outputs rendered from the tree-ish `tree` under the
/// root `root` (task spec `hub-git`, "Outputs": a sync's merge commit), as
/// [`Outputs::render`] renders from a committed tree: the tree's own config
/// and entry, every listed document read from the tree with no lazy fetch.
/// A gap: the config unknown or invalid, `[paths] index` unset, a path that
/// can never be an output or one the tree holds as a document that is not
/// generated or cannot read, a walk gap, an unread blob. Git runs with
/// `git`'s variables. Nothing is written.
pub(crate) fn render_tree(root: &Path, git: &GitEnv, tree: &str) -> TreeRender {
    let committed = match committed_tree(root, git, tree, &[]) {
        CommittedTree::NoIndex => return TreeRender::NoIndex,
        CommittedTree::Unknown(why) => return TreeRender::Gap(why),
        CommittedTree::Indexed(committed) => committed,
    };
    let Some(generator) = committed.check.index_generator() else {
        return TreeRender::NoIndex;
    };
    let project = &committed.project;
    let Some(index_path) = project.paths.index.as_deref() else {
        return TreeRender::Gap("`[paths] index` is not set".to_owned());
    };
    for path in output_paths(index_path, generator) {
        let why = never_output(path, "", &project.paths)
            .map(str::to_owned)
            .or_else(|| match committed.at_head(path) {
                HeadFile::Bytes(bytes) => (document_class(path, bytes, &project.scheme)
                    != Class::Generated)
                    .then(|| SPEC_DOCUMENT.to_owned()),
                HeadFile::Absent => None,
                HeadFile::Unreadable(why) => Some(why.clone()),
            });
        if let Some(why) = why {
            return TreeRender::Gap(format!("`{path}` can never be an output ({why})"));
        }
    }
    let mut input = check_input(&committed.tree, &project.scheme);
    input
        .files
        .retain(|file| !committed.unread.contains_key(&file.path));
    if let Some(why) = incomplete(&input, &project.paths) {
        return TreeRender::Gap(why);
    }
    if let Some(why) = committed.unread.values().next() {
        return TreeRender::Gap(why.clone());
    }
    TreeRender::Rendered(
        render_index_set(&input, index_path, generator)
            .into_iter()
            .map(|output| (output.path, output.bytes.into_bytes()))
            .collect(),
    )
}

/// The outputs of the `index = true` entry of a config read as `project`
/// and `check`, root-relative, in the render's order; none without such an
/// entry or without `[paths] index`.
pub(crate) fn index_outputs(project: &ProjectConfig, check: &CheckConfig) -> Vec<String> {
    match (check.index_generator(), project.paths.index.as_deref()) {
        (Some(generator), Some(index)) => output_paths(index, generator)
            .into_iter()
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// The outputs of the index entry `generator`, root-relative, in the
/// render's order: `[paths] index`, the shards, the roadmap view.
fn output_paths<'a>(index: &'a str, generator: &'a Generator) -> Vec<&'a str> {
    let mut paths = vec![index];
    paths.extend(generator.shards.iter().map(|shard| shard.path.as_str()));
    paths.extend(generator.roadmap.iter().map(|view| view.path.as_str()));
    paths
}

/// Why the root-relative `path` can never be an index output of an apply
/// whose target is `target` (root-relative) under `paths`: the target
/// itself, a file named `spec.lock`, `specengine.toml` or `.spec-debt.toml`
/// in any directory, a path outside the walk. `None`: it may be one.
pub(crate) fn never_output(path: &str, target: &str, paths: &Paths) -> Option<&'static str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if path == target {
        Some("the apply's target")
    } else if name == LOCK_FILE {
        Some(LOCK_FILE)
    } else if name == CONFIG_FILE {
        Some(CONFIG_FILE)
    } else if name == BASELINE_FILE {
        Some(BASELINE_FILE)
    } else if !paths.in_walk_scope(path) {
        Some("outside the walk")
    } else {
        None
    }
}

/// Why a path a tree holds as a document that is not `class: generated`
/// (its front-matter unread included) is never an output.
const SPEC_DOCUMENT: &str = "a spec document";

/// Why a file on disk whose bytes do not read as a document is never an
/// output: a person's file, never overwritten.
const NOT_GENERATED_ON_DISK: &str = "not a generated document on disk";

/// How a document's bytes read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Its front-matter reads `class: generated`.
    Generated,
    /// Its front-matter reads, with another class or none.
    Other,
    /// It does not read: not UTF-8, a front-matter that fails, a parser
    /// failure.
    Unread,
}

/// How `bytes`, the document `path`, read under `scheme`.
fn document_class(path: &str, bytes: &[u8], scheme: &IdScheme) -> Class {
    let Ok(parsed) = panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(path, bytes, scheme)
    })) else {
        return Class::Unread;
    };
    if front_matter_failure(&parsed).is_some() {
        return Class::Unread;
    }
    let class = parsed
        .document()
        .and_then(|document| document.fields.as_ref())
        .and_then(|fields| fields.class.as_deref());
    if class == Some(DocClass::Generated.as_str()) {
        Class::Generated
    } else {
        Class::Other
    }
}

/// The bytes of the regular file `<root>/<path>`, reached through real
/// directories only; `None` when anything on the way is absent, a symlink
/// or another kind of file, or cannot be read (inspection names it).
fn disk_document(root: &Path, path: &str) -> Option<Vec<u8>> {
    if !is_clean_relative(path) {
        return None;
    }
    let mut file = root.to_path_buf();
    let mut components = path.split('/').peekable();
    while let Some(component) = components.next() {
        file.push(component);
        let kind = fs::symlink_metadata(&file).ok()?.file_type();
        let last = components.peek().is_none();
        if kind.is_symlink() || (last && !kind.is_file()) || (!last && !kind.is_dir()) {
            return None;
        }
    }
    fs::read(&file).ok()
}

/// The index entry's `command` in the recorded root's own config on disk:
/// `None` when it cannot be read or has no `index = true` entry.
fn disk_command(root: &Path) -> Option<String> {
    let config = NamedBytes::read(CONFIG_FILE, &root.join(CONFIG_FILE));
    let (_, check) = load_config(&config).ok()?;
    check.index_generator().map(|entry| entry.command.clone())
}

/// The changed output `path` judged on disk and in git: inspected as `spec
/// export index` inspects (no symlink or non-directory below the root, its
/// parent there, absent or a regular file, its bytes read for a restore);
/// one `HEAD` lacks (`at_head` false) not ignored by git; on disk under its
/// exact name; not skip-worktree. `Err`: why not, in the judge's words.
fn judged(
    git: &WorktreeGit,
    root: &Path,
    proposal: &Proposal,
    path: String,
    bytes: Vec<u8>,
    at_head: bool,
) -> Result<Output, String> {
    if !is_clean_relative(&path) {
        return Err(format!(
            "the index output `{path}` is not a root-relative path"
        ));
    }
    let components: Vec<&str> = path.split('/').collect();
    let mut file = root.to_path_buf();
    let mut prior = None;
    for (at, component) in components.iter().enumerate() {
        file.push(component);
        let shown = components[..=at].join("/");
        let last = at + 1 == components.len();
        let metadata = match fs::symlink_metadata(&file) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound && last => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(format!(
                    "the directory `{shown}` of the index output `{path}` does not exist (it is \
                     never created)"
                ));
            }
            Err(error) => return Err(format!("cannot inspect `{shown}`: {error}")),
        };
        let kind = metadata.file_type();
        if kind.is_symlink() {
            return Err(format!(
                "`{shown}` is a symlink: the index is written only through real directories, \
                 to a regular file"
            ));
        }
        if !last && !kind.is_dir() {
            return Err(format!("`{shown}` is not a directory"));
        }
        if last && !kind.is_file() {
            return Err(format!("`{shown}` is not a regular file"));
        }
        if last {
            prior =
                Some(fs::read(&file).map_err(|error| format!("cannot read `{path}`: {error}"))?);
        }
    }
    let top_path = top_of(proposal, &path);
    let unreadable = |error: GitError| format!("cannot tell how git holds `{path}`: {error}");
    if !at_head && git.is_ignored(&top_path).map_err(unreadable)? {
        return Err(format!("`{path}` is ignored by git"));
    }
    match disk_name(root, &path) {
        Ok(DiskName::AsAsked) => {}
        Ok(DiskName::Held(held)) => {
            return Err(format!("`{path}` is `{held}` on disk (a case variant)"));
        }
        Ok(DiskName::CannotTell) => {
            return Err(format!("cannot tell which file `{path}` names on disk"));
        }
        Err(error) => return Err(format!("cannot inspect `{path}`: {error}")),
    }
    if git.is_skip_worktree(&top_path).map_err(unreadable)? {
        return Err(format!("`{path}` is skip-worktree"));
    }
    let tracked = git.is_tracked(&top_path).map_err(unreadable)?;
    Ok(Output {
        path,
        top_path,
        file,
        bytes,
        prior,
        tracked,
        created: None,
        written: false,
        staged: false,
    })
}

/// The absent output `file` created, never replacing anything (its parent
/// exists: inspected).
fn create_file_at(file: &mut Output) -> Result<(), String> {
    let Some(parent) = file.file.parent() else {
        return Err("the path names no file".to_owned());
    };
    let Some(name) = file.file.file_name().and_then(|name| name.to_str()) else {
        return Err("the path names no file".to_owned());
    };
    let created = create_file(parent, name, &file.bytes).map_err(|error| error.to_string())?;
    file.created = Some(created);
    Ok(())
}

/// Two changed outputs that are one file are refused, as `spec export
/// index` refuses them: existing, the same file on disk (device and
/// inode); existing or not, paths equal but for case.
fn one_file_each(files: &[Output]) -> Result<(), String> {
    for (at, later) in files.iter().enumerate() {
        for earlier in &files[..at] {
            let (first, second) = (&earlier.path, &later.path);
            let same = match (fs::metadata(&earlier.file), fs::metadata(&later.file)) {
                (Ok(a), Ok(b)) => identity(&a).is_some() && identity(&a) == identity(&b),
                _ => false,
            };
            if same {
                return Err(format!(
                    "the index outputs `{first}` and `{second}` are one file on disk (names \
                     differing in case or Unicode form, or a hard link): each output needs a \
                     file of its own"
                ));
            }
            if first.to_lowercase() == second.to_lowercase() {
                return Err(format!(
                    "the index outputs `{first}` and `{second}` differ only in case: one file \
                     on a case-insensitive file system; each output needs a path of its own"
                ));
            }
        }
    }
    Ok(())
}

/// The outputs a completion accepts beside the path it judges, from the
/// worktree's top: those of the `index = true` entry in `parent`'s root
/// config (`<root_rel>/specengine.toml` in its tree, the tree the apply
/// rendered from; read with no lazy fetch), never one an apply may not
/// write ([`never_output`] of `target`, root-relative, under the parent's
/// `[paths]`; a path the parent holds as a document that is not `class:
/// generated`, or whose blob it cannot read). None when that config is
/// absent, unreadable or invalid, or has no such entry: a commit never
/// names its own extra paths.
pub(crate) fn parent_outputs(
    git: &WorktreeGit,
    parent: &str,
    proposal: &Proposal,
    target: &str,
) -> BTreeSet<String> {
    let config = top_of(proposal, CONFIG_FILE);
    let Ok(bytes) = git.local_blob_at(parent, &config) else {
        return BTreeSet::new();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return BTreeSet::new();
    };
    let (Ok(project), Ok(check)) = (
        ProjectConfig::from_toml(&text),
        CheckConfig::from_toml(&text),
    ) else {
        return BTreeSet::new();
    };
    let (Some(generator), Some(index)) = (check.index_generator(), project.paths.index.as_deref())
    else {
        return BTreeSet::new();
    };
    output_paths(index, generator)
        .into_iter()
        .filter(|path| never_output(path, target, &project.paths).is_none())
        .map(|path| (path, top_of(proposal, path)))
        .filter(|(path, top)| match git.local_blob_at(parent, top) {
            // The parent holds it: only as a generated document.
            Ok(bytes) => document_class(path, &bytes, &project.scheme) == Class::Generated,
            // Absent from the parent, an output its apply may create;
            // there but unread: not known to be generated.
            Err(_) => matches!(git.blob_oid(parent, top), Ok(None)),
        })
        .map(|(_, top)| top)
        .collect()
}

/// What a commit may change beside the path a judge looks at: index
/// outputs, their bytes unjudged there.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Beside<'a> {
    /// Step 10: the outputs this run wrote (from the top), whatever git
    /// made of them; step 10's blob compare judges their bytes.
    Written(&'a BTreeSet<String>),
    /// Completion: the outputs the parent's config names (from the top,
    /// [`parent_outputs`]), each added or modified (`A`, `M`), never
    /// deleted.
    Outputs(&'a BTreeSet<String>),
}

impl Beside<'_> {
    /// `changed` without the outputs allowed beside it.
    pub(crate) fn rest<'c>(&self, changed: &'c [ChangedPath]) -> Vec<&'c ChangedPath> {
        changed
            .iter()
            .filter(|path| match self {
                Self::Written(outputs) => !outputs.contains(&path.path),
                Self::Outputs(outputs) => {
                    !(outputs.contains(&path.path) && matches!(path.status.as_str(), "A" | "M"))
                }
            })
            .collect()
    }
}

/// The paths a failed commit's refusal names inside one pair of backticks:
/// `` a`, `b` and `c ``, so `` `{names}` `` reads `` `a`, `b` and `c` ``.
pub(crate) fn names(paths: &[&str]) -> String {
    match paths.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{}` and `{last}", rest.join("`, `")),
    }
}

/// How the outputs fared when put back after a failed commit, each named
/// as `restored_clause` names one file: `` `<path>` restored `` and the
/// rest; empty without one.
pub(crate) fn outputs_clause(fared: &[(String, Restored)]) -> String {
    fared
        .iter()
        .map(|(path, restored)| crate::revisions::restored_clause(&[(path.as_str(), *restored)]))
        .collect::<Vec<_>>()
        .join(", ")
}
