//! `spec export index [--stdout]`: the generated index, rendered by core's
//! `render_index` with the `[[generators]]` entry that has `index = true`
//! over a fresh walk of the working tree, written to `<root>/<[paths]
//! index>` and nowhere else (the owner's Q3: a mode of `spec export`, whose
//! bare form is Phase 2's queue export). The header names the registered
//! `command` and `gate`; there is no default path, command or gate. No
//! database, slug or `HOME`; the baseline is not read.
//!
//! Refused, exit 2, nothing written: a config error (one line per cause), no
//! `index = true` entry, an incomplete walk (a file that cannot be read or
//! parsed, a directory that cannot be listed, a missing written root: the
//! render would leave documents out; `--stdout` included), and, when
//! writing, a symlink or a non-directory on the way to the index, a missing
//! parent directory (never created), an index that is no regular file, a
//! failed write, a file replaced between its inspection and its opening.
//! Equal bytes are not rewritten (`unchanged`, the file not opened for
//! writing); else the file is opened, checked to be the one inspected
//! (device and inode), truncated and written in place.
//! Written anyway, with a `warning:`: names skipped for not being UTF-8
//! (their documents are not listed), an index outside the walk.
//!
//! `--stdout` prints the render and writes nothing.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::Path;

use serde::Serialize;
use specengine_core::check::{CheckInput, ProblemKind, Report, WalkGap, render_index, walk_gap};
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
    pub messages: Vec<Message>,
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
    if !project.paths.walk_scope().in_walk_scope(&index_path) {
        messages.push(Message::Warning(format!(
            "`[paths] index` `{index_path}` is outside the walk (the roots, `exclude`): \
             `spec check` reports it `index-missing`"
        )));
    }

    let render = render_index(&input, &index_path, generator);
    let bytes = render.len();
    if request.stdout {
        return Ok(ExportOutcome {
            path: index_path,
            bytes,
            written: false,
            render: Some(render),
            messages,
        });
    }
    let written = write_index(&located.root, &index_path, render.as_bytes())?;
    Ok(ExportOutcome {
        path: index_path,
        bytes,
        written,
        render: None,
        messages,
    })
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

/// Writes `bytes` to `<root>/<index>` unless the file holds them already
/// (`false`). Every existing component below the root is a real directory
/// and the file, if present, a regular file; the parent must exist.
fn write_index(root: &Path, index: &str, bytes: &[u8]) -> Result<bool, CliError> {
    let refuse = |why: String| CliError::spec(format!("{why}; nothing was written"));
    if !is_clean_relative(index) {
        return Err(refuse(format!(
            "`[paths] index` `{index}` is not a root-relative path"
        )));
    }
    let components: Vec<&str> = index.split('/').collect();
    let mut path = root.to_path_buf();
    let mut seen = None;
    for (at, component) in components.iter().enumerate() {
        path.push(component);
        let shown = components[..=at].join("/");
        let last = at + 1 == components.len();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound && last => {
                return create(&path, bytes)
                    .map(|()| true)
                    .map_err(|error| refuse(format!("cannot create `{index}`: {error}")));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(refuse(format!(
                    "the directory `{shown}` of `[paths] index` does not exist (it is \
                     never created)"
                )));
            }
            Err(error) => return Err(refuse(format!("cannot inspect `{shown}`: {error}"))),
        };
        let kind = metadata.file_type();
        if kind.is_symlink() {
            return Err(refuse(format!(
                "`{shown}` is a symlink: the index is written only through real \
                 directories, to a regular file"
            )));
        }
        if !last && !kind.is_dir() {
            return Err(refuse(format!("`{shown}` is not a directory")));
        }
        if last && !kind.is_file() {
            return Err(refuse(format!("`{shown}` is not a regular file")));
        }
        if last {
            seen = Some(metadata);
        }
    }
    let Some(seen) = seen else {
        return Err(refuse(format!(
            "`[paths] index` `{index}` is not a root-relative path"
        )));
    };
    let current =
        fs::read(&path).map_err(|error| refuse(format!("cannot read `{index}`: {error}")))?;
    if current == bytes {
        return Ok(false);
    }
    // Opened without truncating, then checked to be the very file inspected
    // above: a symlink or another file put in its place meanwhile is refused
    // before a byte changes.
    let cannot_write = |error: io::Error| refuse(format!("cannot write `{index}`: {error}"));
    let mut file = OpenOptions::new()
        .write(true)
        .open(&path)
        .map_err(cannot_write)?;
    let opened = file.metadata().map_err(cannot_write)?;
    if !same_file(&seen, &opened) {
        return Err(refuse(format!(
            "`{index}` was replaced while it was being written"
        )));
    }
    file.set_len(0)
        .and_then(|()| file.write_all(bytes))
        .map_err(cannot_write)?;
    Ok(true)
}

/// The two metadata name the same file (device and inode).
#[cfg(unix)]
fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    a.dev() == b.dev() && a.ino() == b.ino()
}

/// Without file identities: the opened file is at least a regular file.
#[cfg(not(unix))]
fn same_file(_: &fs::Metadata, b: &fs::Metadata) -> bool {
    b.is_file()
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

/// `wrote <path>: <n> bytes`, `unchanged …`, or the render (`--stdout`).
pub(crate) fn render_text(outcome: &ExportOutcome) -> String {
    if let Some(render) = &outcome.render {
        return render.clone();
    }
    let verb = if outcome.written {
        "wrote"
    } else {
        "unchanged"
    };
    format!(
        "{verb} {}: {} bytes\n",
        one_line(&outcome.path),
        outcome.bytes
    )
}

#[derive(Serialize)]
struct ExportJson<'a> {
    path: &'a str,
    bytes: usize,
    written: bool,
}

/// `{path, bytes, written}`, as [`crate::render_json`] prints it.
impl Serialize for ExportOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ExportJson {
            path: &self.path,
            bytes: self.bytes,
            written: self.written,
        }
        .serialize(serializer)
    }
}
