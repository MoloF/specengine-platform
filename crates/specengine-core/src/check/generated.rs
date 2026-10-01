//! §11.5–6 of the documentation convention, over the generator registry of
//! `specengine.toml` (`[[generators]]`), errors both:
//!
//! - `index-drift`, `index-missing`: with an `index = true` entry, the
//!   render of each output of the index (the root at `[paths] index`, then
//!   each shard) is compared byte for byte with its walked bytes, each
//!   output on its own. Nothing is written; bytes not supplied → cannot check;
//!   an incomplete walk (an unreadable file or directory, a missing written
//!   root: each a cause of "cannot check") → not compared. A skipped
//!   non-UTF-8 name does not stop the comparison.
//! - `generator-unknown`, `generator-path`: with the table (even empty),
//!   every `class: generated` document names a registered `command` in
//!   `generator:`, and its path is in that entry's `writes`.
//!
//! Without the table, or without an `index = true` entry, the rules are off.

use specengine_model::Severity;

use super::config::{CheckConfig, DocClass, Generator};
use super::engine::Corpus;
use super::input::{CheckInput, ProblemKind};
use super::render::{IndexOutput, readable_fields, render_index_set};
use super::report::{Cause, Finding};
use crate::Paths;

/// Both rules; causes when the index bytes were not supplied or the index
/// entry has no `[paths] index` to compare (a config built in code: the TOML
/// reader rejects it).
pub(crate) fn run(
    input: &CheckInput,
    corpus: &Corpus<'_>,
    paths: &Paths,
    config: &CheckConfig,
    findings: &mut Vec<Finding>,
    causes: &mut Vec<Cause>,
) {
    if let Some(generators) = &config.generators {
        registry(corpus, generators, findings);
    }
    let Some(generator) = config.index_generator() else {
        return;
    };
    let Some(index_path) = &paths.index else {
        causes.push(Cause {
            path: String::new(),
            message: format!(
                "the `[[generators]]` entry `{}` (line {}) has `index = true`, but `[paths] index` is not set: the index cannot be compared with its render",
                generator.command, generator.line
            ),
        });
        return;
    };
    if walk_gap(input, paths).is_some() {
        return;
    }
    index(input, corpus, index_path, generator, findings, causes);
}

/// Why a walk cannot vouch for every document, at its path: the render of
/// the index would leave documents out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkGap<'a> {
    /// A listed file that could not be read or parsed (its `read_error`).
    Unreadable { path: &'a str, error: &'a str },
    /// A root written in `[paths] roots` that names no directory and no
    /// `.md` file.
    MissingRoot { path: &'a str },
    /// A directory that could not be listed; `""`: the root itself.
    UnlistedDir { path: &'a str },
}

impl<'a> WalkGap<'a> {
    /// The root-relative path concerned.
    pub fn path(&self) -> &'a str {
        match *self {
            Self::Unreadable { path, .. }
            | Self::MissingRoot { path }
            | Self::UnlistedDir { path } => path,
        }
    }

    /// Ties on the path: an unreadable file, a missing root, an unlisted
    /// directory.
    fn rank(&self) -> u8 {
        match self {
            Self::Unreadable { .. } => 0,
            Self::MissingRoot { .. } => 1,
            Self::UnlistedDir { .. } => 2,
        }
    }
}

/// The first gap of the walk by path (byte order), `None` when the walk is
/// complete (§11.5's stop conditions): a file that could not be read (its
/// `read_error`), a directory that could not be listed, a written root that
/// is missing — each already a cause of "cannot check", so the run cannot
/// vouch for the index either way, and a comparison would report drift the
/// generator cannot fix. A directory or `.md` name that is not UTF-8 (the
/// warning `name-skipped`) is no gap: a lossy generator line for such a
/// file shows as `index-drift`, and renaming the file, which `name-skipped`
/// already asks for, fixes both. The index writer refuses on the same
/// predicate.
pub fn walk_gap<'a>(input: &'a CheckInput, paths: &Paths) -> Option<WalkGap<'a>> {
    let files = input.files.iter().filter_map(|file| {
        file.read_error.as_deref().map(|error| WalkGap::Unreadable {
            path: &file.path,
            error,
        })
    });
    let problems = input
        .problems
        .iter()
        .filter_map(|problem| match problem.kind {
            ProblemKind::UnreadableDir => Some(WalkGap::UnlistedDir {
                path: &problem.path,
            }),
            ProblemKind::MissingRoot if paths.roots_written => Some(WalkGap::MissingRoot {
                path: &problem.path,
            }),
            ProblemKind::MissingRoot | ProblemKind::SkippedName => None,
        });
    files
        .chain(problems)
        .min_by(|a, b| (a.path(), a.rank()).cmp(&(b.path(), b.rank())))
}

/// `generator-unknown`, `generator-path` on every generated document whose
/// front-matter was read.
fn registry(corpus: &Corpus<'_>, generators: &[Generator], findings: &mut Vec<Finding>) {
    for (index, parsed) in corpus.parses.iter().enumerate() {
        let Some(fields) = parsed.and_then(readable_fields) else {
            continue;
        };
        if fields.class.as_deref() != Some(DocClass::Generated.as_str()) {
            continue;
        }
        let path = corpus.paths[index];
        let line = corpus.texts[index].key_line("generator").unwrap_or(1);
        let value = fields.generator.as_deref();
        let registered =
            value.and_then(|value| generators.iter().find(|entry| entry.command == value));
        match (value, registered) {
            (_, None) => {
                let message = match value {
                    Some(value) => format!(
                        "`generator: {value}` names no registered `[[generators]]` command"
                    ),
                    None => "a generated document without `generator:`: name a registered `[[generators]]` command".to_owned(),
                };
                findings.push(error(
                    "generator-unknown",
                    path,
                    line,
                    value.unwrap_or_default(),
                    message,
                ));
            }
            (Some(value), Some(entry)) if !entry.writes.iter().any(|written| written == path) => {
                findings.push(error(
                    "generator-path",
                    path,
                    line,
                    value,
                    format!(
                        "`{value}` writes {}, not this file",
                        entry.writes.join(", ")
                    ),
                ));
            }
            _ => {}
        }
    }
}

/// Each output of the index (the root, then its shards) judged on its own:
/// `index-missing` when it was not walked; `index-drift` when its bytes
/// differ from its render, on the line of the first differing byte.
fn index(
    input: &CheckInput,
    corpus: &Corpus<'_>,
    index_path: &str,
    generator: &Generator,
    findings: &mut Vec<Finding>,
    causes: &mut Vec<Cause>,
) {
    for (at, output) in render_index_set(input, index_path, generator)
        .iter()
        .enumerate()
    {
        compare(corpus, output, at == 0, generator, findings, causes);
    }
}

/// One output against its walked bytes; `root`: the `[paths] index` file,
/// else a shard.
fn compare(
    corpus: &Corpus<'_>,
    output: &IndexOutput,
    root: bool,
    generator: &Generator,
    findings: &mut Vec<Finding>,
    causes: &mut Vec<Cause>,
) {
    let command = &generator.command;
    let path = output.path.as_str();
    let (what, bytes) = if root {
        ("the `[paths] index` file", "the index bytes were")
    } else {
        ("the index shard", "the index shard's bytes were")
    };
    let Some(&at) = corpus.by_path.get(path) else {
        findings.push(error(
            "index-missing",
            path,
            1,
            "",
            format!(
                "{what} is not walked (absent, outside the roots or excluded); `{command}` writes it"
            ),
        ));
        return;
    };
    let file = corpus.files[at];
    if file.size > 0 && file.bytes.is_empty() {
        causes.push(Cause {
            path: path.to_owned(),
            message: format!(
                "{bytes} not supplied: it cannot be compared with the render of `{command}`"
            ),
        });
        return;
    }
    let walked = file.bytes.as_slice();
    let rendered = output.bytes.as_bytes();
    let first_difference = walked
        .iter()
        .zip(rendered)
        .position(|(a, b)| a != b)
        .or_else(|| (walked.len() != rendered.len()).then(|| walked.len().min(rendered.len())));
    let Some(offset) = first_difference else {
        return;
    };
    let line = 1 + walked[..offset]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count();
    findings.push(error(
        "index-drift",
        path,
        line,
        "",
        format!(
            "differs from the render of `{command}` from this line on: rebuild it with `{command}`; manual edits are overwritten"
        ),
    ));
}

fn error(code: &str, path: &str, line: usize, subject: &str, message: String) -> Finding {
    Finding {
        code: code.to_owned(),
        severity: Severity::Error,
        path: path.to_owned(),
        line,
        subject: subject.to_owned(),
        message,
        fix: None,
        debt: None,
        introduced: None,
    }
}
