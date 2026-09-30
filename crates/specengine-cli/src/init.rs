//! `spec init [--slug S]`: creates exactly one file, `specengine.toml` at the
//! project root (`--root` or the current directory, never a walk up),
//! holding exactly `[project]\nslug = "<slug>"\n`. Created exclusively: an
//! existing file (or anything by that name) is left as it is, exit 2.
//! Nothing else: no directory, no ignore file, no database, no `[ids]`.
//!
//! The slug is `--slug` when given, else derived from the directory name:
//! ASCII letters and digits lower-cased, every other run of bytes (non-UTF-8
//! bytes included) one `-`, the ends trimmed. Either must be a slug of at
//! most 64 bytes, else exit 2 naming `--slug`.

use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::Path;

use serde::Serialize;
use specengine_core::slug_problem;

use crate::project::{CONFIG_FILE, canonical_dir, find_config_dir};
use crate::{CliError, Env, Globals, Message};

/// `spec init` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitRequest {
    /// `--slug S`: the `[project] slug` to write.
    pub slug: Option<String>,
}

/// What `spec init` wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOutcome {
    /// The file created, root-relative.
    pub path: String,
    pub slug: String,
    pub messages: Vec<Message>,
}

/// `spec init`: writes the project's `specengine.toml`.
pub fn init(env: &Env, globals: &Globals, request: &InitRequest) -> Result<InitOutcome, CliError> {
    if globals.config.is_some() {
        return Err(CliError::spec(format!(
            "`spec init` takes no --config: it creates {CONFIG_FILE} at the project root \
             (--root, else the current directory)"
        )));
    }
    let dir = match &globals.root {
        Some(root) => canonical_dir(&env.cwd.join(root), &format!("--root {}", root.display()))?,
        None => canonical_dir(&env.cwd, "the current directory")?,
    };
    let slug = match &request.slug {
        Some(slug) => {
            if let Some(problem) = slug_problem(slug) {
                return Err(CliError::spec(format!("--slug: {problem}")));
            }
            slug.clone()
        }
        None => {
            let name = dir.file_name().unwrap_or_default();
            let derived = derive_slug(name_bytes(name).as_ref());
            if let Some(problem) = slug_problem(&derived) {
                return Err(CliError::spec(format!(
                    "the directory name {:?} gives no slug ({problem}); pass --slug",
                    name.to_string_lossy()
                )));
            }
            derived
        }
    };

    let mut messages = Vec::new();
    if let Some(parent) = dir.parent()
        && let Some(above) = find_config_dir(parent)
    {
        let depth = dir
            .strip_prefix(&above)
            .map_or(0, |below| below.components().count());
        messages.push(Message::Warning(format!(
            "{}{CONFIG_FILE} already configures a project above this directory",
            "../".repeat(depth)
        )));
    }

    let content = format!("[project]\nslug = \"{slug}\"\n");
    write_new(&dir.join(CONFIG_FILE), content.as_bytes()).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            CliError::spec(format!("{CONFIG_FILE} already exists; nothing was written"))
        } else {
            CliError::spec(format!("cannot create {CONFIG_FILE}: {error}"))
        }
    })?;
    Ok(InitOutcome {
        path: CONFIG_FILE.to_owned(),
        slug,
        messages,
    })
}

/// Creates `path` exclusively and writes `bytes`. A failed write removes
/// the file it just created, so a rerun is not refused over a partial one.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(bytes) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

/// The slug a directory name gives: ASCII letters and digits lower-cased,
/// every other run of bytes one `-`, leading and trailing `-` trimmed
/// (`My Project_2` → `my-project-2`). Not validated.
pub fn derive_slug(name: &[u8]) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut in_run = false;
    for &byte in name {
        if byte.is_ascii_alphanumeric() {
            slug.push(char::from(byte.to_ascii_lowercase()));
            in_run = false;
        } else if !in_run {
            slug.push('-');
            in_run = true;
        }
    }
    slug.trim_matches('-').to_owned()
}

#[cfg(unix)]
fn name_bytes(name: &std::ffi::OsStr) -> std::borrow::Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt as _;
    std::borrow::Cow::Borrowed(name.as_bytes())
}

#[cfg(not(unix))]
fn name_bytes(name: &std::ffi::OsStr) -> std::borrow::Cow<'_, [u8]> {
    match name.to_string_lossy() {
        std::borrow::Cow::Borrowed(text) => std::borrow::Cow::Borrowed(text.as_bytes()),
        std::borrow::Cow::Owned(text) => std::borrow::Cow::Owned(text.into_bytes()),
    }
}

pub(crate) fn render_text(outcome: &InitOutcome) -> String {
    format!("created {} with slug {}\n", outcome.path, outcome.slug)
}

#[derive(Serialize)]
struct InitJson<'a> {
    path: &'a str,
    slug: &'a str,
}

fn view(outcome: &InitOutcome) -> InitJson<'_> {
    InitJson {
        path: &outcome.path,
        slug: &outcome.slug,
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for InitOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
