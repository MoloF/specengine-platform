//! `spec init [--slug S] [--pack PATH]`: creates exactly one file,
//! `specengine.toml` at the project root (`--root` or the current directory,
//! never a walk up), holding `[project]\nslug = "<slug>"\n`, a blank line and
//! comments that parse to nothing. Created exclusively: an existing file (or
//! anything by that name) is left as it is, exit 2, never merged. Nothing
//! else: no directory, no ignore file, no database, no active `[ids]`.
//!
//! The slug is `--slug` when given, else derived from the directory name:
//! ASCII letters and digits lower-cased, every other run of bytes (non-UTF-8
//! bytes included) one `-`, the ends trimmed. Either must be a slug of at
//! most 64 bytes, else exit 2 naming `--slug`.
//!
//! With `--pack PATH` (task spec `pack`, PK1 to PK5), a file's path resolved
//! against the current directory and echoed as given, never a name: the file
//! is `[project]`, the slug and a blank line, then the pack's bytes
//! verbatim, comments included, and no scaffold. Before anything is written,
//! the pack is read (a regular file of at most [`PACK_MAX_BYTES`], UTF-8, no
//! leading byte-order mark), read alone as TOML (its first line that is
//! neither blank nor a comment a table header; no `project` and no `team`
//! key), then the composed text goes
//! through the core readers: [`ProjectConfig::from_toml`] (itself running
//! the `[ids]` and `[paths]` readers, in that order), [`card_from_toml`], and
//! [`WorkspaceConfig::from_toml`], whose every note refuses. Each refusal is
//! exit 2 naming the pack's own line, `<PATH>:<L>`, the file's line less the
//! header's three. No other table is judged here: `spec check` does.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_core::paths_toml::DEFAULT_SPEC;
use specengine_core::{ProjectConfig, ProjectError, WorkspaceConfig, card_from_toml, slug_problem};
use toml::de::DeTable;

use crate::project::{CONFIG_FILE, canonical_dir, find_config_dir};
use crate::{CliError, Env, Globals, Message};

/// The largest pack `spec init --pack` reads, in bytes (task spec `pack`,
/// PK2).
pub const PACK_MAX_BYTES: usize = 1_048_576;

/// The lines above a pack in the file written: `[project]`, the slug and a
/// blank line. A pack's line L is the file's line L + 3.
const HEADER_LINES: usize = 3;

/// What follows the two written lines: why to commit the file, and the tables
/// a first document and a flow need, all comments, the paths under the default
/// spec directory.
fn scaffold() -> String {
    format!(
        r#"
# Commit this file: a proposal needs a commit, and the project's rules live here.
# [ids]
# NODE = {{ kind = "node", shape = "name" }}
# [[flow.start]]
# paths = ["{DEFAULT_SPEC}/first.md"]
# [[flow.end]]
# paths = ["{DEFAULT_SPEC}/last.md"]
"#
    )
}

/// `spec init` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitRequest {
    /// `--slug S`: the `[project] slug` to write.
    pub slug: Option<String>,
    /// `--pack PATH`: the file whose bytes follow the header, resolved
    /// against the current directory.
    pub pack: Option<PathBuf>,
}

/// What `spec init` wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOutcome {
    /// The file created, root-relative.
    pub path: String,
    pub slug: String,
    /// `--pack` as given; `None` without it.
    pub pack: Option<String>,
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
    let header = format!("[project]\nslug = \"{slug}\"\n");
    let (content, pack) = match &request.pack {
        None => (format!("{header}{}", scaffold()), None),
        Some(path) => {
            let shown = path.display().to_string();
            let text = read_pack(&env.cwd.join(path), &shown)?;
            check_pack(&shown, &text)?;
            let content = format!("{header}\n{text}");
            check_composed(&shown, &content)?;
            (content, Some(shown))
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
        pack,
        messages,
    })
}

/// Reads the pack at `path` (task spec `pack`, PK2 and PK5): a regular file,
/// judged on the open handle ([`open_pack`]), of at most [`PACK_MAX_BYTES`],
/// UTF-8. `shown` is the path as given.
fn read_pack(path: &Path, shown: &str) -> Result<String, CliError> {
    let cannot_read = |error: &dyn std::fmt::Display| {
        CliError::spec(format!(
            "--pack {shown}: cannot read: {error}; a pack is a file's path"
        ))
    };
    let too_big = |bytes: u64| {
        CliError::spec(format!(
            "--pack {shown}: {bytes} bytes; a pack is at most {PACK_MAX_BYTES}"
        ))
    };
    let file = open_pack(path).map_err(|error| cannot_read(&error))?;
    let meta = file.metadata().map_err(|error| cannot_read(&error))?;
    if !meta.is_file() {
        return Err(cannot_read(&"not a regular file"));
    }
    if meta.len() > PACK_MAX_BYTES as u64 {
        return Err(too_big(meta.len()));
    }
    let mut bytes = Vec::new();
    file.take(PACK_MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| cannot_read(&error))?;
    if bytes.len() > PACK_MAX_BYTES {
        return Err(too_big(bytes.len() as u64));
    }
    String::from_utf8(bytes).map_err(|_| CliError::spec(format!("--pack {shown}: not UTF-8")))
}

/// Opens the pack for reading without waiting on it: `O_NONBLOCK`, so a pipe
/// or a device at the path, even one swapped in after any earlier look,
/// cannot hold the command before [`read_pack`] judges the handle. On a
/// regular file the flag changes nothing: a read of one never waits.
#[cfg(unix)]
fn open_pack(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_pack(path: &Path) -> io::Result<File> {
    File::open(path)
}

/// The pack read alone (task spec `pack`, PK2 (a), (b), (c), (g)): no
/// leading byte-order mark, which the composed text could not read in its
/// middle; TOML; its first line that is neither blank nor a comment a table
/// header, as a key above one would join `[project]`; no `project` and no
/// `team` key.
fn check_pack(shown: &str, pack: &str) -> Result<(), CliError> {
    if pack.starts_with('\u{feff}') {
        return Err(refusal(
            shown,
            Some(1),
            "a pack starts with a byte-order mark; save it as UTF-8 without one",
        ));
    }
    let root = DeTable::parse(pack).map_err(|error| {
        let line = error.span().map(|span| line_of(pack, span.start));
        refusal(shown, line, error.message().trim())
    })?;
    let first = pack.lines().enumerate().find_map(|(index, line)| {
        let line = line.trim();
        (!line.is_empty() && !line.starts_with('#')).then_some((index + 1, line))
    });
    if let Some((line, text)) = first
        && !text.starts_with('[')
    {
        return Err(refusal(
            shown,
            Some(line),
            "a pack starts with a table header; a key above it would join [project]",
        ));
    }
    for (name, message) in [
        ("project", "a pack holds no [project]: spec init writes it"),
        (
            "team",
            "a pack holds no [team]: a project's people are its own",
        ),
    ] {
        if let Some((key, _)) = root
            .get_ref()
            .iter()
            .find(|(key, _)| key.get_ref().as_ref() == name)
        {
            return Err(refusal(
                shown,
                Some(line_of(pack, key.span().start)),
                message,
            ));
        }
    }
    Ok(())
}

/// The composed text through the core readers, the first error refusing
/// (task spec `pack`, PK2 (d), (e), (f)): [`ProjectConfig::from_toml`], which
/// runs the `[ids]` and the `[paths]` readers itself, in that order, then
/// [`card_from_toml`]; then [`WorkspaceConfig::from_toml`], whose first note
/// by line refuses. A line is the file's, given as the pack's.
fn check_composed(shown: &str, content: &str) -> Result<(), CliError> {
    let pack_line = |line: Option<usize>| {
        line.and_then(|line| line.checked_sub(HEADER_LINES))
            .filter(|&line| line > 0)
    };
    let refuse = |error: ProjectError| refusal(shown, pack_line(error.line), &error.message);
    ProjectConfig::from_toml(content).map_err(refuse)?;
    card_from_toml(content).map_err(refuse)?;
    let workspaces = WorkspaceConfig::from_toml(content);
    if let Some(note) = workspaces.notes.first() {
        return Err(refusal(shown, pack_line(Some(note.line)), &note.message));
    }
    Ok(())
}

/// `spec: <PATH>:<L>: <message>`, or `spec: <PATH>: <message>` without a line.
fn refusal(shown: &str, line: Option<usize>, message: &str) -> CliError {
    match line {
        Some(line) => CliError::spec(format!("{shown}:{line}: {message}")),
        None => CliError::spec(format!("{shown}: {message}")),
    }
}

/// The 1-based line of byte `offset` in `text`.
fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
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
    match &outcome.pack {
        None => format!("created {} with slug {}\n", outcome.path, outcome.slug),
        Some(pack) => format!(
            "created {} with slug {} from {pack}\n",
            outcome.path, outcome.slug
        ),
    }
}

#[derive(Serialize)]
struct InitJson<'a> {
    path: &'a str,
    slug: &'a str,
    pack: Option<&'a str>,
}

fn view(outcome: &InitOutcome) -> InitJson<'_> {
    InitJson {
        path: &outcome.path,
        slug: &outcome.slug,
        pack: outcome.pack.as_deref(),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for InitOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
