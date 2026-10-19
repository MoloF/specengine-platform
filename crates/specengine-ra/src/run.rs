//! One run of protocol v1 after its request is accepted
//! (`docs/features/layer-c.md` "Binary"): each workspace owning a requested
//! file discovered without proc macros, loaded, scanned and closed in turn,
//! one at a time; files under no manifest parsed alone; then the `file`
//! lines in request order and `done`.
//!
//! A file's workspace is the one its nearest `Cargo.toml` belongs to,
//! looking up from the file's directory to the root and never above it.
//! Workspaces run in the byte order of those manifest directories (an
//! already answered one skipped); a load answers every requested file whose
//! manifest is the workspace root's or a member's. A file the load did not
//! take (outside every package root, excluded) and a file under no manifest
//! are parsed alone: `in_crate` false, every item `not_loaded`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use specengine_ra::{
    FileScan, LoadError, LoadOptions, MonikerStatus, Project, Workspace, scan_unloaded,
};

use crate::reply::{
    self, ByState, DatabaseStep, DiscoverStep, Done, Ending, ErrorLine, FileLine, ItemLine,
};
use crate::request::Request;
use crate::sys;

/// The file name of a Cargo manifest, a workspace's or a package's.
const MANIFEST: &str = "Cargo.toml";

/// The `workspace` of an `error` line for a file under no manifest.
const NO_WORKSPACE: usize = 0;

/// Runs the request; `Err` only when stdout is gone.
pub fn run(request: &Request, started: Instant) -> io::Result<Ending> {
    let owners: Vec<Option<PathBuf>> = request
        .files
        .iter()
        .map(|file| nearest_manifest_dir(&request.root, file))
        .collect();
    let mut pending: BTreeMap<String, PathBuf> = owners
        .iter()
        .flatten()
        .map(|dir| (relative_dir(&request.root, dir), dir.clone()))
        .collect();
    let mut scans: Vec<Option<FileScan>> = vec![None; request.files.len()];
    let mut number = 0;
    while let Some((_, manifest_dir)) = pending.pop_first() {
        number += 1;
        let answered = answer_workspace(
            request,
            number,
            &manifest_dir,
            &mut pending,
            &owners,
            &mut scans,
        )?;
        if let Err(category) = answered {
            return reply::emit_error(&ErrorLine::new(number, category));
        }
    }
    for (index, owner) in owners.iter().enumerate() {
        if owner.is_some() {
            continue;
        }
        match parse_alone(&request.root, &request.files[index]) {
            Ok(scan) => scans[index] = Some(scan),
            Err(category) => return reply::emit_error(&ErrorLine::new(NO_WORKSPACE, category)),
        }
    }
    // Every manifest directory in `pending` is claimed by the workspace that
    // pops it or by an earlier one, and each claimed directory's files are
    // scanned; a file under no manifest was parsed alone just above.
    let Some(scans) = scans.into_iter().collect::<Option<Vec<FileScan>>>() else {
        unreachable!("every requested file is answered before `done`");
    };
    finish(request, scans, started)
}

/// One workspace: discover from `manifest_dir`, claim the pending manifest
/// directories it answers for, load, read their files, scan them, close.
/// The inner `Err` is the category of the `error` line.
fn answer_workspace(
    request: &Request,
    number: usize,
    manifest_dir: &Path,
    pending: &mut BTreeMap<String, PathBuf>,
    owners: &[Option<PathBuf>],
    scans: &mut [Option<FileScan>],
) -> io::Result<Result<(), &'static str>> {
    let options = LoadOptions {
        root: manifest_dir.to_path_buf(),
        target_dir: request.target_dir.clone(),
        proc_macros: false,
    };
    let step = Instant::now();
    let project = match guarded("discover", || Project::discover(&options)) {
        Ok(project) => project,
        Err(category) => return Ok(Err(category)),
    };
    let ms = millis(step);
    let summary = project.summary();
    let dir = relative_dir(&request.root, &project.workspace_root());
    reply::emit(&DiscoverStep::new(
        number,
        &dir,
        ms,
        summary.toolchain.as_deref(),
        summary.sysroot_packages,
        summary.metadata_error.is_some(),
    ))?;
    if let Some(error) = &summary.metadata_error {
        eprintln!("specengine-ra: workspace {number} ({dir}): metadata degraded: {error}");
    }
    if let Some(error) = &summary.sysroot_error {
        eprintln!("specengine-ra: workspace {number} ({dir}): sysroot: {error}");
    }

    let members = project.member_manifests();
    let mut claimed = vec![manifest_dir.to_path_buf()];
    pending.retain(|_, dir| {
        let member = members.contains(&dir.join(MANIFEST));
        if member {
            claimed.push(dir.clone());
        }
        !member
    });

    let step = Instant::now();
    let workspace = match guarded("database", || project.into_workspace()) {
        Ok(workspace) => workspace,
        Err(category) => return Ok(Err(category)),
    };
    reply::emit(&DatabaseStep::new(number, &dir, millis(step)))?;

    // Every claimed file's bytes before any scan: load-cargo leaves the text
    // of a file that is not UTF-8 unset, and scanning any file of its crate
    // builds the def map that reads it, a requested module file included.
    let mut texts = Vec::new();
    for (index, owner) in owners.iter().enumerate() {
        if !owner.as_ref().is_some_and(|owner| claimed.contains(owner)) {
            continue;
        }
        match read_text(&request.root, &request.files[index]) {
            Ok(text) => texts.push((index, text)),
            Err(category) => return Ok(Err(category)),
        }
    }
    for (index, text) in texts {
        match scan_in(&workspace, &request.root, &request.files[index], &text) {
            Ok(scan) => scans[index] = Some(scan),
            Err(category) => return Ok(Err(category)),
        }
    }
    workspace.close();
    Ok(Ok(()))
}

/// A load step, a panic of rust-analyzer's included: the error's category,
/// its text on stderr only (it may name paths).
fn guarded<T>(
    step: &'static str,
    call: impl FnOnce() -> Result<T, LoadError>,
) -> Result<T, &'static str> {
    match panic::catch_unwind(AssertUnwindSafe(call)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            eprintln!("specengine-ra: {step} failed: {error}");
            Err(error.category())
        }
        Err(_) => {
            eprintln!("specengine-ra: {step} failed: rust-analyzer panicked");
            Err(if step == "discover" {
                "metadata"
            } else {
                "database"
            })
        }
    }
}

/// A file of a loaded workspace, `text` its bytes already read as UTF-8:
/// rust-analyzer's answer when the load took it, else parsed alone from
/// `text`. A file that is not UTF-8 never gets here: it is a `path` error
/// before any file of its workspace is scanned, as it is under no manifest.
fn scan_in(
    workspace: &Workspace,
    root: &Path,
    file: &str,
    text: &str,
) -> Result<FileScan, &'static str> {
    let Some(loaded) = workspace.file_at(&root.join(file)) else {
        return Ok(scan_unloaded(text));
    };
    panic::catch_unwind(AssertUnwindSafe(|| workspace.scan(loaded))).map_err(|_| {
        eprintln!("specengine-ra: {file}: rust-analyzer panicked while scanning it");
        "database"
    })
}

/// A file no load took: its items from its raw text, every one `not_loaded`.
fn parse_alone(root: &Path, file: &str) -> Result<FileScan, &'static str> {
    read_text(root, file).map(|text| scan_unloaded(&text))
}

/// A requested file's text; unreadable or not UTF-8 is the `path` category.
fn read_text(root: &Path, file: &str) -> Result<String, &'static str> {
    let bytes = fs::read(root.join(file)).map_err(|error| {
        eprintln!("specengine-ra: {file}: cannot be read: {error}");
        "path"
    })?;
    String::from_utf8(bytes).map_err(|_| {
        eprintln!("specengine-ra: {file}: not UTF-8");
        "path"
    })
}

/// The `file` lines in request order, then `done` counted from them.
fn finish(request: &Request, scans: Vec<FileScan>, started: Instant) -> io::Result<Ending> {
    let mut items = 0;
    let mut by_state = ByState::default();
    for (file, mut scan) in request.files.iter().zip(scans) {
        scan.items.sort_by_key(|item| item.start);
        let lines: Vec<ItemLine<'_>> = scan
            .items
            .iter()
            .map(|item| {
                items += 1;
                by_state.count(&item.status);
                ItemLine {
                    start: item.start,
                    end: item.end,
                    kind: item.kind.as_str(),
                    state: item.status.as_str(),
                    moniker: match &item.status {
                        MonikerStatus::Moniker(moniker) => Some(moniker.as_str()),
                        _ => None,
                    },
                }
            })
            .collect();
        reply::emit(&FileLine::new(file, scan.in_crate, lines))?;
    }
    reply::emit_done(&Done::new(
        request.files.len(),
        items,
        by_state,
        sys::peak_rss(),
        millis(started),
    ))
}

/// The directory of the `Cargo.toml` nearest to `file`, from its directory
/// up to the root, never above it.
fn nearest_manifest_dir(root: &Path, file: &str) -> Option<PathBuf> {
    let mut dir = root.join(file);
    while dir.pop() && dir.starts_with(root) {
        if dir.join(MANIFEST).is_file() {
            return Some(dir);
        }
    }
    None
}

/// `dir` relative to the root, `/`-separated: `.` for the root itself, `..`
/// segments for a workspace root above it.
fn relative_dir(root: &Path, dir: &Path) -> String {
    let words = |path: &Path| -> Vec<String> {
        path.components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect()
    };
    let root = words(root);
    let dir = words(dir);
    let common = root
        .iter()
        .zip(&dir)
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts: Vec<String> = vec!["..".to_owned(); root.len() - common];
    parts.extend(dir[common..].iter().cloned());
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

fn millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
