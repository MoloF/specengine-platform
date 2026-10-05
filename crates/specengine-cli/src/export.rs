//! `spec export index [--stdout]`: the generated index, rendered by core's
//! `render_index_set` with the `[[generators]]` entry that has `index =
//! true` over a fresh walk of the working tree, written to `<root>/<[paths]
//! index>` and to each shard the entry lists, in config order, and nowhere
//! else (the owner's Q3: a mode of `spec export`, whose bare form is Phase
//! 2's queue export). The header names the registered `command` and `gate`;
//! there is no default path, command, gate or shard. No database, slug or
//! `HOME`; the baseline is not read.
//!
//! Refused, exit 2, nothing written: a config error (one line per cause), no
//! `index = true` entry, an incomplete walk (a file that cannot be read or
//! parsed, a directory that cannot be listed, a missing written root: the
//! render would leave documents out; `--stdout` included), and, when
//! writing, for any output — every one inspected before the first write — a
//! symlink or a non-directory on the way, a missing parent directory (never
//! created), an output that is no regular file or cannot be read, an
//! existing output whose bytes differ that cannot be opened for writing or
//! is no longer the file inspected (device and inode); and two outputs that
//! are one file: the same file on disk, or paths equal but for case. Then
//! the outputs in order: equal bytes are not rewritten (`unchanged`, the
//! file not opened for writing); an existing file is truncated and written in
//! place through the handle opened at inspection; an absent one is created
//! exclusively. A failed write stops there, exit 2: the outputs written
//! before it stay written. Nothing is deleted. Written anyway, with a
//! `warning:`: names skipped for not being UTF-8 (their documents are not
//! listed), each output outside the walk.
//!
//! `--stdout` prints the render and writes nothing; with shards, each output
//! in order after a `==> <path> <==` line.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_core::check::{
    CheckInput, IndexOutput, ProblemKind, Report, WalkGap, render_index_set, walk_gap,
};
use specengine_core::{Paths, is_clean_relative};
use specengine_store::{NamedBytes, StoreError, WorkingTree, check_input, load_config};

use crate::project::locate;
use crate::{CliError, Env, Globals, Message, one_line};

/// `spec export index` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportIndexRequest {
    /// `--stdout`: print the render, write nothing.
    pub stdout: bool,
}

/// What `spec export index` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOutcome {
    /// `[paths] index`, root-relative.
    pub path: String,
    /// The render's length in bytes.
    pub bytes: usize,
    /// The file was written: `false` when it already held the render, and
    /// with `--stdout`.
    pub written: bool,
    /// `--stdout`: the render, printed instead of written.
    pub render: Option<String>,
    /// The shards of the index, in config order; empty without shards.
    pub shards: Vec<ShardOutcome>,
    pub messages: Vec<Message>,
}

/// What `spec export index` did with one shard: as for the root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShardOutcome {
    /// The shard's `path`, root-relative.
    pub path: String,
    /// The render's length in bytes.
    pub bytes: usize,
    /// The file was written: `false` when it already held the render, and
    /// with `--stdout`.
    pub written: bool,
    /// `--stdout`: the render, printed instead of written.
    #[serde(skip)]
    pub render: Option<String>,
}

/// `spec export index`: renders the index and writes it (or prints it).
pub fn export_index(
    env: &Env,
    globals: &Globals,
    request: &ExportIndexRequest,
) -> Result<ExportOutcome, CliError> {
    let located = locate(env, globals)?;
    let label = located.config_label.as_str();
    let config = NamedBytes::read(label, &located.config_file);
    let (project, check_config) =
        load_config(&config).map_err(|report| config_errors(label, &report))?;

    let Some(generator) = check_config.index_generator() else {
        let missing = match &check_config.generators {
            None => "has no `[[generators]]` table",
            Some(_) => "has no `[[generators]]` entry with `index = true`",
        };
        return Err(CliError::spec(format!(
            "{label} {missing}: register the command that writes the index (`command`, \
             `writes`, `index = true`) and set `[paths] index`; nothing was written"
        )));
    };
    let Some(index_path) = project.paths.index.clone() else {
        return Err(CliError::spec(format!(
            "{label}: the `[[generators]]` entry `{}` (line {}) has `index = true`, but \
             `[paths] index` is not set; nothing was written",
            generator.command, generator.line
        )));
    };

    let tree = WorkingTree::new(&located.root, &project.paths).map_err(|error| {
        CliError::spec(format!(
            "the root cannot be read: {}; nothing was written",
            io_message(&error)
        ))
    })?;
    let input = check_input(&tree, &project.scheme);
    if let Some(why) = incomplete(&input, &project.paths) {
        return Err(CliError::spec(format!(
            "the walk is incomplete: {why}: the index would leave documents out; \
             nothing was written"
        )));
    }

    let mut messages = skipped_warnings(&input);
    let scope = project.paths.walk_scope();
    if !scope.in_walk_scope(&index_path) {
        messages.push(Message::Warning(format!(
            "`[paths] index` `{index_path}` is outside the walk (the roots, `exclude`): \
             `spec check` reports it `index-missing`"
        )));
    }
    for shard in &generator.shards {
        if !scope.in_walk_scope(&shard.path) {
            messages.push(Message::Warning(format!(
                "the index shard `{}` is outside the walk (the roots, `exclude`): \
                 `spec check` reports it `index-missing`",
                shard.path
            )));
        }
    }

    let outputs = render_index_set(&input, &index_path, generator);
    let written = if request.stdout {
        vec![false; outputs.len()]
    } else {
        write_outputs(&located.root, &outputs)?
    };
    let render = |text: &str| request.stdout.then(|| text.to_owned());
    let mut outcomes = outputs
        .iter()
        .zip(written)
        .map(|(output, written)| ShardOutcome {
            path: output.path.clone(),
            bytes: output.bytes.len(),
            written,
            render: render(&output.bytes),
        });
    let Some(root) = outcomes.next() else {
        return Err(CliError::spec(
            "the index render has no root; nothing was written".to_owned(),
        ));
    };
    Ok(ExportOutcome {
        path: root.path,
        bytes: root.bytes,
        written: root.written,
        render: root.render,
        shards: outcomes.collect(),
        messages,
    })
}

/// Inspects every output before writing any (a refusal writes nothing),
/// then writes them in order, each only when its bytes differ: per output,
/// whether it was written.
fn write_outputs(root: &Path, outputs: &[IndexOutput]) -> Result<Vec<bool>, CliError> {
    let refused = |why: String| CliError::spec(format!("{why}; nothing was written"));
    let mut targets = Vec::with_capacity(outputs.len());
    for (at, output) in outputs.iter().enumerate() {
        let role = if at == 0 { Role::Root } else { Role::Shard };
        let target = inspect(root, &output.path, role, output.bytes.as_bytes()).map_err(refused)?;
        targets.push(target);
    }
    one_file_each(&targets).map_err(refused)?;
    let mut written: Vec<bool> = Vec::with_capacity(outputs.len());
    for (target, output) in targets.into_iter().zip(outputs) {
        let wrote = write(target, output.bytes.as_bytes()).map_err(|why| {
            let before: Vec<String> = outputs
                .iter()
                .zip(&written)
                .filter(|(_, wrote)| **wrote)
                .map(|(output, _)| format!("`{}`", output.path))
                .collect();
            if before.is_empty() {
                CliError::spec(format!("{why}; nothing was written"))
            } else {
                CliError::spec(format!("{why}; written before it: {}", before.join(", ")))
            }
        })?;
        written.push(wrote);
    }
    Ok(written)
}

/// One stderr line per cause of a config error: `<config>:<line>: message`,
/// else `spec: <config>: message`.
fn config_errors(label: &str, report: &Report) -> CliError {
    let lines: Vec<String> = report
        .cannot_check
        .iter()
        .map(|cause| {
            if cause.path == label {
                format!("spec: {}: {}", cause.path, cause.message)
            } else {
                format!("{}: {}", cause.path, cause.message)
            }
        })
        .collect();
    if lines.is_empty() {
        return CliError::spec(format!("{label}: the config cannot be used"));
    }
    CliError::lines(lines)
}

/// The first path (in byte order) the render could not account for, and
/// why: core's [`walk_gap`], the §11.5 stop conditions of the index
/// comparison.
fn incomplete(input: &CheckInput, paths: &Paths) -> Option<String> {
    let gap = walk_gap(input, paths)?;
    let path = gap.path();
    let shown = if path.is_empty() { "." } else { path };
    Some(match gap {
        WalkGap::Unreadable { error, .. } => format!("`{path}` cannot be read ({error})"),
        WalkGap::UnlistedDir { .. } => format!("the directory `{shown}` cannot be listed"),
        WalkGap::MissingRoot { .. } => {
            format!("the `[paths] roots` entry `{shown}` names no directory and no `.md` file")
        }
    })
}

/// One warning per place names were skipped for not being UTF-8, with the
/// count.
fn skipped_warnings(input: &CheckInput) -> Vec<Message> {
    let mut skipped: BTreeMap<&str, usize> = BTreeMap::new();
    for problem in &input.problems {
        if problem.kind == ProblemKind::SkippedName {
            *skipped.entry(problem.path.as_str()).or_default() += 1;
        }
    }
    skipped
        .into_iter()
        .map(|(path, count)| {
            let place = if path.is_empty() {
                String::new()
            } else {
                format!(" under `{path}`")
            };
            Message::Warning(format!(
                "{count} directory or `.md` names skipped{place}: not UTF-8; \
                 their documents are not in the index"
            ))
        })
        .collect()
}

/// Which output of the index a path is, for the messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// `[paths] index`.
    Root,
    /// A shard of the index entry.
    Shard,
}

impl Role {
    /// How a message names the output `path`.
    fn name(self, path: &str) -> String {
        match self {
            Self::Root => format!("`[paths] index` `{path}`"),
            Self::Shard => format!("the index shard `{path}`"),
        }
    }

    /// How a message names the output whose directory is missing.
    fn owner(self, path: &str) -> String {
        match self {
            Self::Root => "`[paths] index`".to_owned(),
            Self::Shard => format!("the index shard `{path}`"),
        }
    }
}

/// An output inspected for writing.
struct Target {
    /// Root-relative, as configured.
    path: String,
    /// `<root>/<path>`.
    file: PathBuf,
    /// What writing does with it.
    plan: Plan,
}

/// What writing does with an inspected output.
enum Plan {
    /// Absent: created exclusively.
    Create,
    /// It holds the render already (as inspected): not opened for writing.
    Keep(fs::Metadata),
    /// Its bytes differ: `handle` was opened for writing at inspection and
    /// checked to be the file `seen`; it is truncated and written through.
    Rewrite {
        seen: fs::Metadata,
        handle: fs::File,
    },
}

impl Target {
    /// The existing file as inspected; `None`: absent.
    fn seen(&self) -> Option<&fs::Metadata> {
        match &self.plan {
            Plan::Create => None,
            Plan::Keep(seen) | Plan::Rewrite { seen, .. } => Some(seen),
        }
    }
}

/// Inspects `<root>/<path>` without writing: every existing component below
/// the root is a real directory, the parent exists, and the file, if
/// present, is a regular file that can be read and, unless it holds `bytes`
/// already, opened for writing (not truncated, not created) and checked to
/// be the file inspected. `Err`: why it is refused.
fn inspect(root: &Path, path: &str, role: Role, bytes: &[u8]) -> Result<Target, String> {
    if !is_clean_relative(path) {
        return Err(format!("{} is not a root-relative path", role.name(path)));
    }
    let components: Vec<&str> = path.split('/').collect();
    let mut file = root.to_path_buf();
    let mut seen = None;
    for (at, component) in components.iter().enumerate() {
        file.push(component);
        let shown = components[..=at].join("/");
        let last = at + 1 == components.len();
        let metadata = match fs::symlink_metadata(&file) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound && last => {
                return Ok(Target {
                    path: path.to_owned(),
                    file,
                    plan: Plan::Create,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(format!(
                    "the directory `{shown}` of {} does not exist (it is never created)",
                    role.owner(path)
                ));
            }
            Err(error) => return Err(format!("cannot inspect `{shown}`: {error}")),
        };
        let kind = metadata.file_type();
        if kind.is_symlink() {
            return Err(format!(
                "`{shown}` is a symlink: the index is written only through real \
                 directories, to a regular file"
            ));
        }
        if !last && !kind.is_dir() {
            return Err(format!("`{shown}` is not a directory"));
        }
        if last && !kind.is_file() {
            return Err(format!("`{shown}` is not a regular file"));
        }
        if last {
            seen = Some(metadata);
        }
    }
    let Some(seen) = seen else {
        return Err(format!("{} is not a root-relative path", role.name(path)));
    };
    let current = fs::read(&file).map_err(|error| format!("cannot read `{path}`: {error}"))?;
    if current == bytes {
        return Ok(Target {
            path: path.to_owned(),
            file,
            plan: Plan::Keep(seen),
        });
    }
    // Opened without truncating, then checked to be the very file inspected
    // above: a read-only file, a symlink or another file put in its place
    // meanwhile is refused before any output changes.
    let opened = OpenOptions::new()
        .write(true)
        .open(&file)
        .map_err(|error| format!("cannot open `{path}` for writing: {error}"))?;
    let metadata = opened
        .metadata()
        .map_err(|error| format!("cannot inspect `{path}`: {error}"))?;
    if !same_file(&seen, &metadata) {
        return Err(format!(
            "`{path}` was replaced while it was being inspected"
        ));
    }
    Ok(Target {
        path: path.to_owned(),
        file,
        plan: Plan::Rewrite {
            seen,
            handle: opened,
        },
    })
}

/// Two outputs that are one file are refused: existing, the same file on
/// disk (device and inode: a name differing in case or Unicode form on a
/// file system that ignores it, or a hard link); existing or not, paths
/// equal but for case (one file on a case-insensitive file system, and in a
/// checkout there). Pairs in config order, the first found.
fn one_file_each(targets: &[Target]) -> Result<(), String> {
    for (at, later) in targets.iter().enumerate() {
        for earlier in &targets[..at] {
            let (first, second) = (&earlier.path, &later.path);
            if let (Some(a), Some(b)) = (earlier.seen(), later.seen())
                && identity(a).is_some()
                && identity(a) == identity(b)
            {
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

/// Writes `bytes` to the inspected `target` unless it holds them already
/// (`false`, the file not opened for writing). `Err`: why the write failed.
fn write(target: Target, bytes: &[u8]) -> Result<bool, String> {
    let path = target.path.as_str();
    match target.plan {
        Plan::Keep(_) => Ok(false),
        Plan::Create => create(&target.file, bytes)
            .map(|()| true)
            .map_err(|error| format!("cannot create `{path}`: {error}")),
        Plan::Rewrite { mut handle, .. } => handle
            .set_len(0)
            .and_then(|()| handle.write_all(bytes))
            .map(|()| true)
            .map_err(|error| format!("cannot write `{path}`: {error}")),
    }
}

/// The two metadata name the same file.
fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    match (identity(a), identity(b)) {
        (Some(a), Some(b)) => a == b,
        // Without file identities: the opened file is at least a regular
        // file.
        _ => b.is_file(),
    }
}

/// The file's identity: device and inode.
#[cfg(unix)]
pub(crate) fn identity(metadata: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    Some((metadata.dev(), metadata.ino()))
}

/// No file identities here.
#[cfg(not(unix))]
pub(crate) fn identity(_: &fs::Metadata) -> Option<(u64, u64)> {
    None
}

/// A new file, created exclusively (a symlink appearing there is not
/// followed).
fn create(path: &Path, bytes: &[u8]) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)
}

/// A store error without the absolute path it may carry.
fn io_message(error: &StoreError) -> String {
    match error {
        StoreError::Io { source, .. } => source.to_string(),
        other => other.to_string(),
    }
}

/// One `wrote <path>: <n> bytes` or `unchanged …` line per output, the root
/// first; `--stdout`: the render, or with shards each output after a
/// `==> <path> <==` line.
pub(crate) fn render_text(outcome: &ExportOutcome) -> String {
    let outputs = std::iter::once((
        outcome.path.as_str(),
        outcome.bytes,
        outcome.written,
        outcome.render.as_deref(),
    ))
    .chain(outcome.shards.iter().map(|shard| {
        (
            shard.path.as_str(),
            shard.bytes,
            shard.written,
            shard.render.as_deref(),
        )
    }));
    if let Some(render) = &outcome.render
        && outcome.shards.is_empty()
    {
        return render.clone();
    }
    let mut text = String::new();
    for (path, bytes, written, render) in outputs {
        if let Some(render) = render {
            text.push_str(&format!("==> {} <==\n{render}", one_line(path)));
            continue;
        }
        let verb = if written { "wrote" } else { "unchanged" };
        text.push_str(&format!("{verb} {}: {bytes} bytes\n", one_line(path)));
    }
    text
}

#[derive(Serialize)]
struct ExportJson<'a> {
    path: &'a str,
    bytes: usize,
    written: bool,
    #[serde(skip_serializing_if = "<[ShardOutcome]>::is_empty")]
    shards: &'a [ShardOutcome],
}

/// `{path, bytes, written}`, as [`crate::render_json`] prints it; with
/// shards, also `shards`: the same keys per shard, in config order.
impl Serialize for ExportOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ExportJson {
            path: &self.path,
            bytes: self.bytes,
            written: self.written,
            shards: &self.shards,
        }
        .serialize(serializer)
    }
}
