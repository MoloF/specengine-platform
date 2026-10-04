//! Plumbing shared by every measurement: corpus resolution, the read-only
//! guard, file enumeration and the self-terminating timeout.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::CommonArgs;

/// A resolved corpus: where to read, where to write, how to call it.
pub struct Corpus {
    /// `pilot-a`, `pilot-b`, `fixtures` or `pilot`; the only name that reaches stdout.
    pub label: String,
    /// Canonical corpus root; read-only.
    pub root: PathBuf,
    /// Scratch directory for per-file detail; never under `root`.
    pub out: PathBuf,
}

pub enum HarnessError {
    /// Exit 2: nothing was written.
    Refused(String),
    /// Exit 1.
    Internal(String),
}

/// The environment variables that stand in for a flag under a pilot label:
/// the first under `--label pilot-a`, the second under `--label pilot-b`.
/// Every `SPECENGINE_*` name of this crate's flags is declared here once.
#[derive(Clone, Copy)]
pub struct PilotVariable([&'static str; 2]);

/// The corpus root (`--pilot`).
pub const PILOT_CORPUS: PilotVariable = PilotVariable(["SPECENGINE_PILOT_A", "SPECENGINE_PILOT_B"]);
/// The ID scheme of `parse`, `index` and `check` (`--scheme`).
pub const PILOT_SCHEME: PilotVariable =
    PilotVariable(["SPECENGINE_SCHEME_A", "SPECENGINE_SCHEME_B"]);
/// The census config of `census` and `parse` (`--config`).
pub const PILOT_CENSUS_CONFIG: PilotVariable =
    PilotVariable(["SPECENGINE_CENSUS_CONFIG_A", "SPECENGINE_CENSUS_CONFIG_B"]);

impl PilotVariable {
    /// The variable read under `label`; none for a label that is not a pilot's.
    fn name(self, label: &str) -> Option<&'static str> {
        match label {
            "pilot-a" => Some(self.0[0]),
            "pilot-b" => Some(self.0[1]),
            _ => None,
        }
    }
}

/// The path in `variable` under `label`, when it is set and not empty.
fn from_environment(variable: PilotVariable, label: Option<&str>) -> Option<PathBuf> {
    path_in(variable.name(label?)?)
}

/// The path in `variable`, when it is set and not empty.
fn path_in(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The file a measurement reads: the path of `flag` (`flag_name`) when
/// given, else the path in `variable` under a pilot label, else `default`
/// at the corpus root. None of them: refused, naming what was looked for.
/// The path is not opened here: reading it is the caller's.
pub fn resolve_file(
    flag_name: &str,
    flag: Option<&Path>,
    variable: PilotVariable,
    label: Option<&str>,
    root: &Path,
    default: &str,
) -> Result<PathBuf, String> {
    if let Some(path) = flag
        .map(Path::to_path_buf)
        .or_else(|| from_environment(variable, label))
    {
        return Ok(path);
    }
    let path = root.join(default);
    if path.is_file() {
        return Ok(path);
    }
    Err(match label.and_then(|label| variable.name(label)) {
        Some(name) => {
            format!("no {flag_name} given, no {name} and no {default} at the corpus root")
        }
        None => format!("no {flag_name} given and no {default} at the corpus root"),
    })
}

/// The label of a run: `--label`, else `pilot` with `--pilot`, else
/// `fixtures`.
pub fn label_of(args: &CommonArgs) -> &str {
    args.label.as_deref().unwrap_or(if args.pilot.is_some() {
        "pilot"
    } else {
        "fixtures"
    })
}

/// Resolves the corpus and the scratch directory.
///
/// Refuses — before opening anything for writing — when `--label` is not a
/// single plain path component (it names a directory under `--out`), when
/// the corpus is unreadable or when `--out` resolves under the corpus root. The
/// measurement-specific `setup` over the canonical corpus root (reading a
/// config, say) runs after that guard and before `--out` is created; its error
/// is a refusal too: exit 2, nothing written.
pub fn prepare_with<C>(
    args: &CommonArgs,
    fixture: &str,
    setup: impl FnOnce(&Path) -> Result<C, String>,
) -> Result<(Corpus, C), HarnessError> {
    if let Some(label) = &args.label
        && !is_single_component(label)
    {
        return Err(HarnessError::Refused(format!(
            "--label {label:?} is not a single plain path component (it names a directory under --out); nothing written"
        )));
    }
    let (requested, label) = match &args.pilot {
        Some(pilot) => (pilot.clone(), label_of(args).to_owned()),
        None => {
            let label = label_of(args);
            match PILOT_CORPUS.name(label) {
                Some(variable) => match path_in(variable) {
                    Some(path) => (path, label.to_owned()),
                    None => {
                        return Err(HarnessError::Refused(format!(
                            "--label {label} needs --pilot or the {variable} environment variable"
                        )));
                    }
                },
                None => (fixture_dir(fixture), label.to_owned()),
            }
        }
    };
    let root = fs::canonicalize(&requested).map_err(|error| {
        HarnessError::Refused(format!(
            "corpus path unreadable: {}: {error}",
            requested.display()
        ))
    })?;
    if !root.is_dir() {
        return Err(HarnessError::Refused(format!(
            "corpus path is not a directory: {}",
            root.display()
        )));
    }
    fs::read_dir(&root).map_err(|error| {
        HarnessError::Refused(format!(
            "corpus path unreadable: {}: {error}",
            root.display()
        ))
    })?;
    let out = absolutize(&args.out);
    if out.starts_with(&root) {
        return Err(HarnessError::Refused(format!(
            "--out {} lies under the corpus {}; nothing written",
            out.display(),
            root.display()
        )));
    }
    let prepared = setup(&root).map_err(HarnessError::Refused)?;
    fs::create_dir_all(&out).map_err(|error| {
        HarnessError::Internal(format!("cannot create --out {}: {error}", out.display()))
    })?;
    Ok((Corpus { label, root, out }, prepared))
}

/// `label` is exactly one normal path component: not empty, no separator,
/// not `.` or `..`, not absolute — so `<out>/<measurement>/<label>` stays
/// under `--out`.
fn is_single_component(label: &str) -> bool {
    let mut components = Path::new(label).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) => name == label && !label.contains('/'),
        _ => false,
    }
}

/// `fixtures/<name>` of this repository, located from the crate directory.
fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join(name)
}

/// Absolute path resolved component by component: every prefix that exists is
/// canonicalised (symlinks resolved) as soon as it is complete, `..` pops the
/// prefix resolved so far and `.` is dropped, and components that do not exist
/// yet are appended lexically. The result is always canonical wherever the
/// file system has an answer, so it compares with the canonical corpus root
/// even when a `..` follows a directory that does not exist (the OS would
/// create it and step back into the corpus) or a symlink into the corpus.
pub fn absolutize(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => {
                result.push(other.as_os_str());
                if let Ok(canonical) = fs::canonicalize(&result) {
                    result = canonical;
                }
            }
        }
    }
    result
}

/// All `.rs` files under `root`, as sorted paths relative to it. Skips hidden
/// directories, `target*` build directories and symlinks; an unreadable
/// subdirectory is reported on stderr and skipped, never fatal.
pub fn rust_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    files_with_extension(root, ".rs")
}

/// All `.ron` files under `root`; same rules as [`rust_files`].
pub fn ron_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    files_with_extension(root, ".ron")
}

/// Every `Cargo.toml` under `root`; same rules as [`rust_files`].
pub fn cargo_manifests(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = files_with_extension(root, "Cargo.toml")?;
    files.retain(|path| path.file_name().is_some_and(|name| name == "Cargo.toml"));
    Ok(files)
}

fn files_with_extension(root: &Path, extension: &str) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    walk(root, root, extension, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk(root: &Path, dir: &Path, extension: &str, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if dir != root => {
            eprintln!("skipped {}: {error}", dir.display());
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let path = entry.path();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if name.starts_with('.') || name == "target" || name.starts_with("target.") {
                continue;
            }
            walk(root, &path, extension, out)?;
        } else if name.ends_with(extension) {
            out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
    Ok(())
}

pub enum Outcome<T> {
    Finished(T),
    TimedOut,
    Panicked,
}

/// Runs `work` on a helper thread and waits at most `budget`. On overrun the
/// caller prints its envelope and exits, which ends the helper with the
/// process: the self-terminating form of a timeout, nothing survives the run.
pub fn run_with_timeout<T: Send + 'static>(
    budget: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Outcome<T> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(work());
    });
    match receiver.recv_timeout(budget) {
        Ok(value) => Outcome::Finished(value),
        Err(mpsc::RecvTimeoutError::Timeout) => Outcome::TimedOut,
        Err(mpsc::RecvTimeoutError::Disconnected) => Outcome::Panicked,
    }
}

/// `numerator / denominator` in percent, one decimal; 0.0 for an empty denominator.
pub fn percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        (numerator as f64 * 1000.0 / denominator as f64).round() / 10.0
    }
}

/// Like [`percent`] but vacuously 100.0 when there is nothing to compare.
pub fn stability_percent(equal: usize, compared: usize) -> f64 {
    if compared == 0 {
        100.0
    } else {
        percent(equal, compared)
    }
}
