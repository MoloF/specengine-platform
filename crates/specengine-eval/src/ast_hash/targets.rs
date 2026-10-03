//! Cargo target tables for the `qpath` part of `ast-hash`
//! (`docs/canon/code-identity.md`): `<cargo> metadata --format-version 1
//! --no-deps --offline --color never --manifest-path <dir>/Cargo.toml` over
//! the package dirs, shortest first, skipping those an earlier run covered
//! (its packages, its `workspace_root`); `<cargo>` is `$SPECENGINE_CARGO` or
//! `cargo`. A package dir no run covers — cargo missing, a broken manifest, a
//! nested workspace, no answer in time — gets Cargo's auto-discovery over its
//! files ([`qpath::layout_targets`]), never an error.
//!
//! Each call may take [`CARGO_BUDGET`], cut to what the run's `--timeout`
//! leaves less [`RUN_END_MARGIN`]. A call that gives no answer in that time
//! is killed and reaped and, like a cargo that cannot start, ends the cargo
//! calls of the run: every package dir not yet covered gets the layout. No
//! cargo process the harness started outlives the run.
//!
//! Cargo never builds, patches or runs anything here: `--no-deps` resolves
//! no dependency and writes no lock file, `--offline` keeps it off the
//! network. It runs at the filesystem root, owned by the system and outside
//! the corpus, so no `rust-toolchain` file or `.cargo/config.toml` of the
//! corpus — or one planted in a world-writable directory such as the system
//! temporary directory — is picked up through the working directory,
//! wherever the harness was started; a corpus at the root itself gets the
//! layout for every package, cargo is not called. Its stdout is parsed, its
//! stderr captured: one line reaches the harness's stderr, the corpus root in
//! it replaced by the run's label wherever it stands as a whole path; nothing
//! reaches stdout.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;
use specengine_code::qpath::{self, PackageTargets, Target, TargetSource};

/// Overrides the cargo command; default `cargo`.
pub const ENV_CARGO: &str = "SPECENGINE_CARGO";

/// Time one `cargo metadata` call may take before it is killed and the
/// package dirs not yet covered fall back to the layout.
pub const CARGO_BUDGET: Duration = Duration::from_secs(60);

/// How long before the end of the run's `--timeout` a call is killed, so it
/// is gone before the harness reports the timeout and exits.
pub const RUN_END_MARGIN: Duration = Duration::from_secs(1);

const MANIFEST: &str = "Cargo.toml";

/// The target table of every package dir in `packages` (relative to `root`,
/// `""` for the root itself), each mapped to its files relative to it.
/// `corpus_label` names the corpus on stderr; `run_end` is the end of the run's
/// `--timeout` (`None`: unbounded).
pub fn tables(
    root: &Path,
    corpus_label: &str,
    packages: &BTreeMap<PathBuf, Vec<PathBuf>>,
    run_end: Option<Instant>,
) -> BTreeMap<PathBuf, PackageTargets> {
    let cargo = std::env::var(ENV_CARGO)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "cargo".to_owned());
    let mut order: Vec<&PathBuf> = packages.keys().collect();
    order.sort_by_key(|dir| (dir.components().count(), *dir));
    let mut found: BTreeMap<PathBuf, PackageTargets> = BTreeMap::new();
    let mut covered: BTreeSet<PathBuf> = BTreeSet::new();
    match outside_dir(root) {
        None if !order.is_empty() => eprintln!(
            "ast-hash: no working directory outside the corpus for {cargo}; layout targets for every package"
        ),
        None => {}
        Some(cwd) => {
            let runner = Runner {
                cargo: &cargo,
                cwd: &cwd,
                root,
                corpus_label,
                run_end,
            };
            for dir in order {
                if covered.contains(dir) {
                    continue;
                }
                match runner.metadata(&root.join(dir).join(MANIFEST)) {
                    Ok(output) => absorb(root, &output, &mut found, &mut covered),
                    Err(Failure::Spawn(error)) => {
                        eprintln!(
                            "ast-hash: cannot run {cargo} ({error}); layout targets for every package"
                        );
                        break;
                    }
                    Err(Failure::Overrun(budget)) => {
                        eprintln!(
                            "ast-hash: cargo metadata gave no answer within {:.1} s for package dir {}, killed; layout targets for it and every package dir left",
                            budget.as_secs_f64(),
                            label(dir)
                        );
                        break;
                    }
                    Err(Failure::NoTime) => {
                        eprintln!(
                            "ast-hash: --timeout leaves no time for cargo metadata (package dir {}); layout targets for it and every package dir left",
                            label(dir)
                        );
                        break;
                    }
                    Err(Failure::Answer(reason)) => eprintln!(
                        "ast-hash: cargo metadata failed for package dir {}: {reason}; layout targets used",
                        label(dir)
                    ),
                }
            }
        }
    }
    packages
        .iter()
        .map(|(dir, files)| {
            let table = match found.remove(dir) {
                Some(table) => table,
                // A virtual workspace root: covered by its run, no targets.
                None if covered.contains(dir) => PackageTargets {
                    source: TargetSource::Metadata,
                    targets: Vec::new(),
                },
                None => qpath::layout_targets(
                    &package_name(root, dir),
                    files.iter().map(PathBuf::as_path),
                ),
            };
            (dir.clone(), table)
        })
        .collect()
}

/// The working directory for the cargo calls: the filesystem root, which the
/// system owns, unless the corpus is the root itself. Never a world-writable
/// directory such as the system temporary one, where anyone could plant a
/// `rust-toolchain.toml` that makes the rustup proxy run a foreign cargo.
fn outside_dir(root: &Path) -> Option<PathBuf> {
    fs::canonicalize("/")
        .ok()
        .filter(|dir| dir.is_dir() && !dir.starts_with(root))
}

enum Failure {
    /// The command could not be started at all.
    Spawn(std::io::Error),
    /// No answer within this budget; the child was killed and reaped.
    Overrun(Duration),
    /// The run's `--timeout` leaves no time for a call; none was started.
    NoTime,
    /// It ran and gave no usable answer.
    Answer(String),
}

/// What every `cargo metadata` call of one run shares.
struct Runner<'a> {
    cargo: &'a str,
    /// Outside the corpus ([`outside_dir`]).
    cwd: &'a Path,
    root: &'a Path,
    corpus_label: &'a str,
    run_end: Option<Instant>,
}

impl Runner<'_> {
    /// The time the next call may take: [`CARGO_BUDGET`], cut to what the
    /// run's `--timeout` leaves less [`RUN_END_MARGIN`]; `None` when nothing
    /// is left.
    fn budget(&self) -> Option<Duration> {
        let Some(end) = self.run_end else {
            return Some(CARGO_BUDGET);
        };
        let left = end
            .checked_duration_since(Instant::now())?
            .checked_sub(RUN_END_MARGIN)?;
        (!left.is_zero()).then(|| left.min(CARGO_BUDGET))
    }

    /// One `cargo metadata` call, bounded by [`Self::budget`]; both pipes are
    /// drained on their own threads so a large answer never stalls the child.
    fn metadata(&self, manifest: &Path) -> Result<Value, Failure> {
        let budget = self.budget().ok_or(Failure::NoTime)?;
        let mut child = Command::new(self.cargo)
            .args([
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--offline",
                "--color",
                "never",
                "--manifest-path",
            ])
            .arg(manifest)
            .current_dir(self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(Failure::Spawn)?;
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let deadline = Instant::now() + budget;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Failure::Overrun(budget));
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Failure::Answer(format!("cannot wait for cargo: {error}")));
                }
            }
        };
        let stdout = stdout.join().unwrap_or_default();
        let stderr = stderr.join().unwrap_or_default();
        if !status.success() {
            let stderr = String::from_utf8_lossy(&stderr);
            let reason = stderr
                .lines()
                .find(|line| line.starts_with("error"))
                .or_else(|| stderr.lines().find(|line| !line.trim().is_empty()))
                .map_or_else(
                    || status.to_string(),
                    |line| truncate(&scrub(line, self.root, self.corpus_label), 200),
                );
            return Err(Failure::Answer(reason));
        }
        serde_json::from_slice(&stdout)
            .map_err(|error| Failure::Answer(format!("unreadable output: {error}")))
    }
}

/// `line` with the corpus root replaced by the run's label wherever it
/// stands as a whole path, so no absolute corpus path reaches stderr: after
/// the start of the line or a character no path continues through
/// (whitespace, a quote, a backtick, `(`, `=`, `:` …), and before the end or
/// a character no name continues through (`/`, whitespace, a quote …).
/// `/mnt<root>/y` and `<root>.bak` name other paths and stay.
fn scrub(line: &str, root: &Path, label: &str) -> String {
    let root = root.to_string_lossy();
    let root = root.as_ref();
    let Some(first) = root.chars().next() else {
        return line.to_owned();
    };
    let mut scrubbed = String::with_capacity(line.len());
    let mut copied = 0;
    let mut from = 0;
    while let Some(found) = line[from..].find(root) {
        let at = from + found;
        let end = at + root.len();
        let starts = line[..at]
            .chars()
            .next_back()
            .is_none_or(|prev| !(continues_name(prev) || matches!(prev, '/' | '\\')));
        let ends = line[end..]
            .chars()
            .next()
            .is_none_or(|next| !continues_name(next));
        if starts && ends {
            scrubbed.push_str(&line[copied..at]);
            scrubbed.push_str(label);
            copied = end;
            from = end;
        } else {
            from = at + first.len_utf8();
        }
    }
    scrubbed.push_str(&line[copied..]);
    scrubbed
}

/// A character a path component name continues through.
fn continues_name(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.')
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    })
}

/// Records every package of one answer that lies in the corpus, and the dirs
/// it covers: its packages and its workspace root.
fn absorb(
    root: &Path,
    output: &Value,
    found: &mut BTreeMap<PathBuf, PackageTargets>,
    covered: &mut BTreeSet<PathBuf>,
) {
    if let Some(workspace) = output
        .get("workspace_root")
        .and_then(Value::as_str)
        .and_then(|dir| fs::canonicalize(dir).ok())
        && let Ok(relative) = workspace.strip_prefix(root)
    {
        covered.insert(relative.to_path_buf());
    }
    let packages = output.get("packages").and_then(Value::as_array);
    for package in packages.into_iter().flatten() {
        let Some(manifest) = package
            .get("manifest_path")
            .and_then(Value::as_str)
            .and_then(|path| fs::canonicalize(path).ok())
        else {
            continue;
        };
        let Some(dir) = manifest.parent() else {
            continue;
        };
        let Ok(relative) = dir.strip_prefix(root) else {
            continue;
        };
        let targets = package.get("targets").and_then(Value::as_array);
        let targets = targets
            .into_iter()
            .flatten()
            .filter_map(|target| target_of(dir, target))
            .collect();
        covered.insert(relative.to_path_buf());
        found.insert(
            relative.to_path_buf(),
            PackageTargets {
                source: TargetSource::Metadata,
                targets,
            },
        );
    }
}

/// One target of an answer: Cargo's first kind, its name, its root relative
/// to the (canonical) package dir; a root outside the package is dropped.
fn target_of(package_dir: &Path, target: &Value) -> Option<Target> {
    let kind = target.get("kind")?.as_array()?.first()?.as_str()?;
    let name = target.get("name")?.as_str()?;
    let source = fs::canonicalize(target.get("src_path")?.as_str()?).ok()?;
    let root = source.strip_prefix(package_dir).ok()?;
    Some(Target {
        kind: kind.to_owned(),
        name: name.to_owned(),
        root: root.to_path_buf(),
    })
}

#[derive(Deserialize)]
struct Manifest {
    package: Option<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: Option<String>,
}

/// `[package] name` when the manifest parses, else the directory's name.
fn package_name(root: &Path, dir: &Path) -> String {
    let manifest = fs::read_to_string(root.join(dir).join(MANIFEST))
        .ok()
        .and_then(|text| toml::from_str::<Manifest>(&text).ok());
    manifest
        .and_then(|manifest| manifest.package?.name)
        .unwrap_or_else(|| {
            root.join(dir)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

/// A package dir as the stderr notes name it: relative, `.` for the root.
fn label(dir: &Path) -> String {
    let text = dir.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        ".".to_owned()
    } else {
        text
    }
}

fn truncate(line: &str, max_chars: usize) -> String {
    match line.char_indices().nth(max_chars) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}
