//! Helpers of the graph-read tests (docs/features/spec-cli-graph.md):
//! `spec` runs killed after a short deadline (a walk without a visited set
//! must not hang the machine), the parts of a `tree` / `graph` / `show
//! --links` stdout, and a scratch copy of this repository's walked
//! documents with the committed config (its `[project] slug` included).

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use specengine_core::ProjectConfig;
use specengine_store::{WorkingTree, check_input};

use super::{Run, SPEC, Scratch, read, repository_root, write};

/// The deadline of every graph-read run that could loop (AC-07).
pub const DEADLINE: Duration = Duration::from_secs(30);

/// `spec args` in `cwd` with `HOME=home` only; `Err` when it runs longer
/// than `limit` (the child is then killed and reaped).
pub fn spec_within(home: &Path, cwd: &Path, args: &[&str], limit: Duration) -> Result<Run, String> {
    let mut child = Command::new(SPEC)
        .env_clear()
        .env("HOME", home.as_os_str())
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn spec");
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait on spec") {
            break status;
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("spec {args:?} ran longer than {limit:?}; killed"));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = out.join().expect("stdout reader");
    let stderr = err.join().expect("stderr reader");
    Ok(Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().expect("spec exited, not killed by a signal"),
        stdout: String::from_utf8(stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(stderr).expect("stderr is UTF-8"),
    })
}

/// [`spec_within`] the [`DEADLINE`], which must not pass.
pub fn spec30(home: &Path, cwd: &Path, args: &[&str]) -> Run {
    spec_within(home, cwd, args, DEADLINE).unwrap_or_else(|problem| panic!("{problem}"))
}

/// The item lines of a `tree` or `graph` stdout: every line but the
/// summary (`nodes …`) and a tail (`[truncated: …]`).
pub fn item_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| !line.starts_with("nodes ") && !line.starts_with("[truncated: "))
        .collect()
}

/// The summary line of a `tree` or `graph` stdout.
pub fn summary_line(stdout: &str) -> &str {
    stdout
        .lines()
        .find(|line| line.starts_with("nodes "))
        .unwrap_or_else(|| panic!("no summary line in {stdout:?}"))
}

/// The node lines of a `graph` stdout: `<distance> <name> | …`.
pub fn graph_nodes(stdout: &str) -> Vec<&str> {
    item_lines(stdout)
        .into_iter()
        .filter(|line| {
            line.split(' ')
                .next()
                .is_some_and(|d| d.parse::<usize>().is_ok())
        })
        .collect()
}

/// The edge lines of a `graph` stdout: `<src> --<type>--> <dst> | …`.
pub fn graph_edges(stdout: &str) -> Vec<&str> {
    item_lines(stdout)
        .into_iter()
        .filter(|line| line.contains(" --") && line.contains("--> "))
        .collect()
}

/// `(distance, name)` of each node line of a `graph` stdout.
pub fn distances(stdout: &str) -> Vec<(usize, String)> {
    graph_nodes(stdout)
        .into_iter()
        .map(|line| {
            let (distance, rest) = line.split_once(' ').unwrap();
            let name = rest.split(" | ").next().unwrap();
            (distance.parse().unwrap(), name.to_owned())
        })
        .collect()
}

/// `(depth, name)` of each node line of a `tree` stdout (two spaces per
/// level).
pub fn tree_depths(stdout: &str) -> Vec<(usize, String)> {
    item_lines(stdout)
        .into_iter()
        .map(|line| {
            let trimmed = line.trim_start_matches(' ');
            let indent = line.len() - trimmed.len();
            assert_eq!(indent % 2, 0, "odd indent: {line:?}");
            (indent / 2, trimmed.split(" | ").next().unwrap().to_owned())
        })
        .collect()
}

/// The links block of a `show --links` stdout for its first node: the
/// lines after the header up to and including `  links <o> out, <i> in…`.
pub fn links_block(stdout: &str) -> Vec<&str> {
    let mut block = Vec::new();
    for line in stdout.lines().skip(1) {
        block.push(line);
        if line.starts_with("  links ") {
            return block;
        }
    }
    panic!("no `  links ` line in {stdout:?}")
}

/// The link lines (`  out …` / `  in …`) of [`links_block`].
pub fn link_lines(stdout: &str) -> Vec<&str> {
    links_block(stdout)
        .into_iter()
        .filter(|line| line.starts_with("  out ") || line.starts_with("  in "))
        .collect()
}

/// The keys of a JSON object.
pub fn keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("not an object: {value}"))
        .keys()
        .map(String::as_str)
        .collect()
}

/// A field of every element of a JSON array, as text.
pub fn field(items: &Value, key: &str) -> Vec<String> {
    items
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {items}"))
        .iter()
        .map(|item| match &item[key] {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
        .collect()
}

/// A scratch copy of this repository's walked documents (the committed
/// root config's walk) at `<scratch>/<dir>`, its `specengine.toml` the
/// committed one as is (it names `[project] slug`, Q8 of
/// docs/features/spec-cli-bundle.md). The repository is only read.
pub fn repository_copy(scratch: &Scratch, dir: &str) -> PathBuf {
    let repository = repository_root();
    let toml = read(&repository, "specengine.toml");
    let text = std::str::from_utf8(&toml).expect("UTF-8 config");
    let project = ProjectConfig::from_toml(text).expect("the root config");
    let tree = WorkingTree::new(&repository, &project.paths).expect("the working tree");
    let input = check_input(&tree, &project.scheme);
    let copy = scratch.dir(dir);
    for file in &input.files {
        write(&copy, &file.path, read(&repository, &file.path));
    }
    write(&copy, "specengine.toml", &toml);
    copy
}

/// `HOME` for [`super::spec_with`] callers.
pub fn home_env(home: &Path) -> [(&'static str, &OsStr); 1] {
    [("HOME", home.as_os_str())]
}
