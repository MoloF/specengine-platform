//! argv and the request of protocol v1 (`docs/features/layer-c.md`
//! "Protocol v1"), checked whole before anything is discovered: a wrong one
//! is a `refused` line and exit 2, and no cargo, rustc or file write has
//! happened by then.
//!
//! The request is one stdin line, a JSON object with exactly the keys
//! `protocol`, `root`, `files`, `target_dir`, `tmp_dir`, `budget_ms`:
//! `files` byte-sorted, unique, clean root-relative paths of regular files;
//! `root` a clean absolute directory; `target_dir`, `tmp_dir` clean absolute
//! paths, neither the root nor under it (compared as given and resolved, so
//! a symlinked spelling of the root is caught too).

use std::ffi::OsString;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::reply::PROTOCOL;

/// Upper bound of the request line, bytes: a tree of 100 000 files with
/// 100-byte paths stays well below it.
const REQUEST_CAP: u64 = 64 * 1024 * 1024;

const USAGE: &str = "usage: specengine-ra --protocol 1, the request as one line on stdin";

/// A checked request; paths absolute, the root canonical.
#[derive(Debug)]
pub struct Request {
    /// The canonical root: every requested file and manifest is looked up
    /// under it, never above it.
    pub root: PathBuf,
    /// Root-relative, `/`-separated, as the request named them.
    pub files: Vec<String>,
    pub target_dir: PathBuf,
    pub tmp_dir: PathBuf,
    pub budget: Duration,
}

/// The request's JSON: exactly these keys, each required.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    protocol: u64,
    root: String,
    files: Vec<String>,
    target_dir: String,
    tmp_dir: String,
    budget_ms: u64,
}

/// argv after the program name: exactly `--protocol 1`.
pub fn check_argv(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    let args: Vec<OsString> = args.into_iter().collect();
    let unexpected = |arg: &OsString| {
        Err(format!(
            "unexpected argument `{}`; {USAGE}",
            arg.to_string_lossy()
        ))
    };
    match args.as_slice() {
        [] => Err(format!("no arguments; {USAGE}")),
        [flag] if flag == "--protocol" => Err(format!("`--protocol` names no version; {USAGE}")),
        [flag, version, rest @ ..] if flag == "--protocol" => {
            if version.to_str() != Some("1") {
                Err(format!(
                    "protocol {} is not spoken here: this binary speaks protocol {PROTOCOL}",
                    version.to_string_lossy()
                ))
            } else if let Some(extra) = rest.first() {
                unexpected(extra)
            } else {
                Ok(())
            }
        }
        [first, ..] => unexpected(first),
    }
}

/// The request line from `input`, up to and without its newline (EOF ends
/// a last line too). The rest of `input` stays unread.
pub fn read_line(input: impl BufRead) -> Result<Vec<u8>, String> {
    let mut line = Vec::new();
    input
        .take(REQUEST_CAP + 1)
        .read_until(b'\n', &mut line)
        .map_err(|error| format!("cannot read the request from stdin: {error}"))?;
    if u64::try_from(line.len()).unwrap_or(u64::MAX) > REQUEST_CAP {
        return Err(format!("the request line is over {REQUEST_CAP} bytes"));
    }
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    if line.iter().all(u8::is_ascii_whitespace) {
        return Err("no request on stdin: one JSON line is expected".to_owned());
    }
    Ok(line)
}

/// Checks the request line, in a fixed order, the first fault refusing it.
pub fn parse(line: &[u8]) -> Result<Request, String> {
    let raw: Raw = serde_json::from_slice(line)
        .map_err(|error| format!("the request is not a protocol 1 request object: {error}"))?;
    if raw.protocol != u64::from(PROTOCOL) {
        return Err(format!(
            "the request speaks protocol {}: this binary speaks protocol {PROTOCOL}",
            raw.protocol
        ));
    }
    check_absolute("root", &raw.root)?;
    let root = fs::canonicalize(&raw.root)
        .map_err(|error| format!("root `{}` cannot be resolved: {error}", raw.root))?;
    if !root.is_dir() {
        return Err(format!("root `{}` is not a directory", raw.root));
    }
    let target_dir = outside_root("target_dir", &raw.target_dir, &raw.root, &root)?;
    let tmp_dir = outside_root("tmp_dir", &raw.tmp_dir, &raw.root, &root)?;
    if raw.budget_ms == 0 {
        return Err("budget_ms is 0: at least 1 is expected".to_owned());
    }
    for (index, file) in raw.files.iter().enumerate() {
        check_relative(index, file)?;
        if let Some(previous) = index.checked_sub(1).map(|before| &raw.files[before]) {
            if previous == file {
                return Err(format!(
                    "files[{index}] `{file}` repeats files[{}]",
                    index - 1
                ));
            }
            if previous > file {
                return Err(format!(
                    "files are not byte-sorted: files[{index}] `{file}` sorts before files[{}]",
                    index - 1
                ));
            }
        }
        let metadata = fs::metadata(root.join(file))
            .map_err(|error| format!("files[{index}] `{file}` cannot be read: {error}"))?;
        if !metadata.is_file() {
            return Err(format!("files[{index}] `{file}` is not a regular file"));
        }
    }
    Ok(Request {
        root,
        files: raw.files,
        target_dir,
        tmp_dir,
        budget: Duration::from_millis(raw.budget_ms),
    })
}

/// Creates the two directories the binary writes into (cargo's target
/// directory and the temporary one), both outside the root.
pub fn create_dirs(request: &Request) -> Result<(), String> {
    for (key, dir) in [
        ("target_dir", &request.target_dir),
        ("tmp_dir", &request.tmp_dir),
    ] {
        fs::create_dir_all(dir)
            .map_err(|error| format!("{key} `{}` cannot be created: {error}", dir.display()))?;
    }
    Ok(())
}

/// The segments of a `/`-separated path, each non-empty and neither `.`
/// nor `..`; `None` when one is not.
fn clean_segments(path: &str) -> Option<Vec<&str>> {
    let segments: Vec<&str> = path.split('/').collect();
    let clean = segments
        .iter()
        .all(|segment| !segment.is_empty() && *segment != "." && *segment != "..");
    (clean && !path.contains('\0')).then_some(segments)
}

fn check_absolute(key: &str, path: &str) -> Result<(), String> {
    let clean = path
        .strip_prefix('/')
        .and_then(clean_segments)
        .is_some_and(|_| Path::new(path).is_absolute());
    if clean {
        Ok(())
    } else {
        Err(format!(
            "{key} `{path}` is not a clean absolute path (no `.`, `..` or empty segment)"
        ))
    }
}

fn check_relative(index: usize, file: &str) -> Result<(), String> {
    if !file.starts_with('/') && clean_segments(file).is_some() {
        Ok(())
    } else {
        Err(format!(
            "files[{index}] `{file}` is not a clean root-relative path (no leading `/`, `.`, `..` or empty segment)"
        ))
    }
}

/// `path` checked clean and absolute, and neither the root nor under it:
/// as written against the root as written, and resolved (its longest
/// existing ancestor canonicalised) against the canonical root.
fn outside_root(key: &str, path: &str, root_given: &str, root: &Path) -> Result<PathBuf, String> {
    check_absolute(key, path)?;
    let given = Path::new(path);
    if given.starts_with(root_given) || resolved(given).starts_with(root) {
        return Err(format!(
            "{key} `{path}` is the root or lies under it: nothing is written there"
        ));
    }
    Ok(given.to_path_buf())
}

/// `path` with its longest existing ancestor canonicalised and the rest
/// appended (the rest does not exist yet, so it holds no symlink).
fn resolved(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut rest = Vec::new();
    loop {
        if let Ok(canonical) = fs::canonicalize(existing) {
            let mut out = canonical;
            out.extend(rest.iter().rev());
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}
