//! docs/features/layer-c-resolve.md R6 and "Freshness" through the CLI
//! library (`specengine_cli::layer_c_state`, public and refreshing both
//! indexes first, as reads do: the developer's iteration 1, deviation 11)
//! over a scratch ra-mini resolved against the golden fake
//! (`layer_c_common`):
//!
//! - `absent` without a snapshot, `fresh` after a run;
//! - `stale` once any `tree_fp` input changes, its rows kept, `fresh` again
//!   once the input is back: a requested file's bytes, a new requested
//!   file, a package manifest, and (L11, iteration 2) every `TREE_FILES`
//!   name (`Cargo.toml`, `Cargo.lock`, `rust-toolchain`,
//!   `rust-toolchain.toml`, `.cargo/config`, `.cargo/config.toml`) in each
//!   directory from a walked file's up to the root: the root, a package
//!   directory, a directory between, a virtual workspace root's manifest
//!   that is no walked file's nearest; an unreadable one too (an entry of
//!   its own);
//! - never an input: a file the request does not name (a `.ron`, a
//!   non-UTF-8 `.rs`, a `.rs` outside the walk), a tree file in a
//!   directory no walked file is under, one above the root, a directory
//!   standing at such a name.
//!
//! And the library's own `--timeout` check (deviation 12): out of range,
//! `spec: --timeout N is out of range: 1 to 3600 seconds`, nothing spawned.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod code_index_common;
mod common;
mod layer_c_common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use specengine_cli::{Env, Globals, ResolveRequest};

use common::{read_text, write};
use layer_c_common::{CONFIG, Lab, Reply, golden_text, swap_in};

/// The library's view of the lab's project.
fn library(lab: &Lab) -> (Env, Globals) {
    (
        Env {
            cwd: lab.root.clone(),
            home: Some(lab.home.as_os_str().to_owned()),
            xdg_data_home: None,
        },
        Globals {
            root: Some(lab.root.clone()),
            config: None,
        },
    )
}

/// `layer_c_state()` of the lab's worktree: `absent`, `fresh` or `stale`.
fn state(lab: &Lab) -> String {
    let (env, globals) = library(lab);
    specengine_cli::layer_c_state(&env, &globals)
        .unwrap_or_else(|error| panic!("layer_c_state: {error}"))
        .as_str()
        .to_owned()
}

/// An edit of `relative` under `root` and its undo: an absent file is
/// created with `text`, a present one gets `text` appended.
struct Edit<'a> {
    relative: &'a str,
    text: &'a str,
}

impl Edit<'_> {
    /// Applies the edit; the bytes to restore (`None`: delete).
    fn apply(&self, root: &Path) -> Option<Vec<u8>> {
        let path = root.join(self.relative);
        let before = fs::read(&path).ok();
        let mut bytes = before.clone().unwrap_or_default();
        bytes.extend_from_slice(self.text.as_bytes());
        write(root, self.relative, bytes);
        before
    }

    fn undo(&self, root: &Path, before: Option<Vec<u8>>) {
        match before {
            Some(bytes) => write(root, self.relative, bytes),
            None => {
                let path = root.join(self.relative);
                fs::remove_file(&path)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                let emptied = path.parent().filter(|parent| {
                    *parent != root
                        && fs::read_dir(parent).is_ok_and(|mut dir| dir.next().is_none())
                });
                if let Some(parent) = emptied {
                    let _ = fs::remove_dir(parent);
                }
            }
        }
    }
}

/// `reply` stored; each edit makes the snapshot stale, its rows kept;
/// undone, fresh again.
fn assert_each_makes_stale(lab: &Lab, reply: &str, edits: &[Edit<'_>]) {
    let stored = lab.store_reply(reply);
    assert_eq!(state(lab), "fresh", "after a run");
    for edit in edits {
        let before = edit.apply(&lab.root);
        assert_eq!(state(lab), "stale", "{} edited", edit.relative);
        assert_eq!(
            lab.layer_c(),
            stored,
            "{}: a stale snapshot keeps its rows",
            edit.relative
        );
        edit.undo(&lab.root, before);
        assert_eq!(state(lab), "fresh", "{} back as it was", edit.relative);
    }
}

/// Each edit leaves a fresh snapshot fresh.
fn assert_each_keeps_fresh(lab: &Lab, edits: &[Edit<'_>]) {
    assert_eq!(state(lab), "fresh", "before");
    for edit in edits {
        let before = edit.apply(&lab.root);
        assert_eq!(
            state(lab),
            "fresh",
            "{}: not a tree_fp input",
            edit.relative
        );
        edit.undo(&lab.root, before);
    }
}

/// R6. `absent` before any run, `fresh` after one.
#[test]
fn layer_c_state_is_fresh_after_a_run() {
    let lab = Lab::ra_mini("lcs-fresh");
    assert_eq!(state(&lab), "absent", "no snapshot yet");
    lab.store_golden();
    assert_eq!(state(&lab), "fresh");
}

/// R6. M: per-file freshness. An edit of one requested file, then of
/// another, and a new `.rs` file under a walked root: the whole snapshot
/// is stale, its rows kept; undone, fresh. A `.ron`, a non-UTF-8 `.rs` and
/// a `.rs` outside every `[code]` root are no input.
#[test]
fn layer_c_state_is_stale_after_another_file_edit() {
    let lab = Lab::ra_mini("lcs-file");
    assert_each_makes_stale(
        &lab,
        &golden_text(),
        &[
            Edit {
                relative: "arena/src/stray.rs",
                text: "// an edit\n",
            },
            Edit {
                relative: "tools/probe.rs",
                text: "// another edit\n",
            },
            Edit {
                relative: "arena/src/spare.rs",
                text: "pub fn spare() {}\n",
            },
        ],
    );
    assert_each_keeps_fresh(
        &lab,
        &[
            Edit {
                relative: "arena/src/tuning.ron",
                text: "(regen: 3)\n",
            },
            Edit {
                relative: "outside/loose.rs",
                text: "pub fn loose() {}\n",
            },
        ],
    );
    let latin = lab.root.join("arena/src/latin1.rs");
    fs::write(&latin, b"// caf\xe9\nfn latin() {}\n").expect("a non-UTF-8 file");
    assert_eq!(state(&lab), "fresh", "a non-UTF-8 .rs is never requested");
    fs::remove_file(latin).expect("remove the non-UTF-8 file");
}

/// R6. M: manifests out of `tree_fp`. Each package manifest and the
/// workspace's: stale; undone, fresh.
#[test]
fn manifest_edit_makes_the_snapshot_stale() {
    let lab = Lab::ra_mini("lcs-manifest");
    assert_each_makes_stale(
        &lab,
        &golden_text(),
        &[
            Edit {
                relative: "arena/Cargo.toml",
                text: "# an edit\n",
            },
            Edit {
                relative: "arena-derive/Cargo.toml",
                text: "# an edit\n",
            },
            Edit {
                relative: "Cargo.toml",
                text: "# an edit\n",
            },
        ],
    );
}

const TOOLCHAIN: &str = "[toolchain]\nchannel = \"1.98.1\"\n";
const CARGO_CONFIG: &str = "[build]\njobs = 1\n";

/// R6. M: one of the lock, toolchain or cargo config files out of
/// `tree_fp`. `Cargo.lock` edited; `rust-toolchain`, `rust-toolchain.toml`,
/// `.cargo/config`, `.cargo/config.toml` created in the root and in a
/// package directory: each stale, undone fresh. The same files above the
/// root: fresh (never read above it, L8).
#[test]
fn lock_toolchain_or_cargo_config_edit_makes_the_snapshot_stale() {
    let lab = Lab::ra_mini("lcs-lock");
    assert_each_makes_stale(
        &lab,
        &golden_text(),
        &[
            Edit {
                relative: "Cargo.lock",
                text: "\n",
            },
            Edit {
                relative: "rust-toolchain.toml",
                text: TOOLCHAIN,
            },
            Edit {
                relative: "rust-toolchain",
                text: "1.98.1\n",
            },
            Edit {
                relative: ".cargo/config.toml",
                text: CARGO_CONFIG,
            },
            Edit {
                relative: ".cargo/config",
                text: CARGO_CONFIG,
            },
            Edit {
                relative: "arena/rust-toolchain.toml",
                text: TOOLCHAIN,
            },
            Edit {
                relative: "arena/.cargo/config.toml",
                text: CARGO_CONFIG,
            },
        ],
    );
    let above = lab.root.parent().expect("the root's parent").to_path_buf();
    for (relative, text) in [
        ("Cargo.lock", "version = 4\n"),
        ("rust-toolchain.toml", TOOLCHAIN),
        (".cargo/config.toml", CARGO_CONFIG),
    ] {
        let path = above.join(relative);
        assert!(!path.exists(), "{}: the scratch holds none", path.display());
        write(&above, relative, text);
        assert_eq!(state(&lab), "fresh", "{relative} above the root");
        fs::remove_file(path).expect("remove it");
    }
}

/// L11 (iteration 2). M: tree files read beside the nearest manifest only.
/// Each `TREE_FILES` name in a directory between a walked file and the
/// root (`arena/src`, no manifest of its own) and in a walked file's own
/// directory under no package (`tools`): stale, undone fresh. In a
/// directory no walked file is under (`docs`): fresh.
#[test]
fn tree_files_from_each_walked_dir_up_to_the_root_are_inputs() {
    let lab = Lab::ra_mini("lcs-between");
    assert_each_makes_stale(
        &lab,
        &golden_text(),
        &[
            Edit {
                relative: "arena/src/Cargo.lock",
                text: "version = 4\n",
            },
            Edit {
                relative: "arena/src/rust-toolchain.toml",
                text: TOOLCHAIN,
            },
            Edit {
                relative: "arena/src/.cargo/config.toml",
                text: CARGO_CONFIG,
            },
            Edit {
                relative: "tools/Cargo.lock",
                text: "version = 4\n",
            },
            Edit {
                relative: "tools/rust-toolchain",
                text: "1.98.1\n",
            },
        ],
    );
    assert_each_keeps_fresh(
        &lab,
        &[
            Edit {
                relative: "docs/Cargo.lock",
                text: "version = 4\n",
            },
            Edit {
                relative: "docs/.cargo/config.toml",
                text: CARGO_CONFIG,
            },
        ],
    );
}

/// L11 (iteration 2): the developer's iteration 1 observation. With
/// `tools` out of the walk no walked file's nearest manifest is the
/// virtual workspace root's `Cargo.toml`; an edit of it alone (a
/// `[workspace.dependencies]` or a `[patch]` table, its `members`
/// reordered) still makes the snapshot stale; undone, fresh. `tools`, now
/// outside the walk, holds no input.
#[test]
fn virtual_workspace_root_edit_makes_the_snapshot_stale() {
    let lab = Lab::ra_mini("lcs-virtual-root");
    write(
        &lab.root,
        "specengine.toml",
        swap_in(
            CONFIG,
            "roots = [\"arena\", \"arena-derive\", \"tools\"]",
            "roots = [\"arena\", \"arena-derive\"]",
        ),
    );
    let mut reply = Reply::golden();
    reply.files.retain(|file| file.path != "tools/probe.rs");
    assert_each_makes_stale(
        &lab,
        &reply.render(),
        &[
            Edit {
                relative: "Cargo.toml",
                text: "[workspace.dependencies]\narena-derive = { path = \"arena-derive\" }\n",
            },
            Edit {
                relative: "Cargo.toml",
                text: "[patch.crates-io]\narena-derive = { path = \"arena-derive\" }\n",
            },
        ],
    );
    let manifest = read_text(&lab.root, "Cargo.toml");
    let reordered = swap_in(
        &manifest,
        "members = [\"arena\", \"arena-derive\"]",
        "members = [\"arena-derive\", \"arena\"]",
    );
    write(&lab.root, "Cargo.toml", &reordered);
    assert_eq!(state(&lab), "stale", "the virtual root's members reordered");
    write(&lab.root, "Cargo.toml", &manifest);
    assert_eq!(state(&lab), "fresh", "the virtual root back as it was");
    assert_each_keeps_fresh(
        &lab,
        &[Edit {
            relative: "tools/Cargo.lock",
            text: "version = 4\n",
        }],
    );
}

/// The review's freshness edges (deviation 7): an unreadable `Cargo.lock`
/// is an input of its own (stale; readable again, fresh); a directory
/// standing at `.cargo/config` is skipped (fresh).
#[test]
fn unreadable_tree_file_is_an_input_and_a_directory_is_not() {
    let lab = Lab::ra_mini("lcs-unreadable");
    lab.store_golden();
    assert_eq!(state(&lab), "fresh");
    let lock = lab.root.join("Cargo.lock");
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let unreadable = state(&lab);
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o644)).expect("chmod back");
    assert_eq!(unreadable, "stale", "an unreadable Cargo.lock");
    assert_eq!(state(&lab), "fresh", "readable again");

    let directory = lab.root.join(".cargo/config");
    fs::create_dir_all(&directory).expect("a directory at .cargo/config");
    assert_eq!(state(&lab), "fresh", "a directory at a tree file's name");
    fs::remove_dir(directory).expect("remove it");
}

/// Deviation 12: the library checks `--timeout` too, for bridges: 0 and
/// 3601 give `spec: --timeout N is out of range: 1 to 3600 seconds`,
/// before any lookup or spawn.
#[test]
fn resolve_library_refuses_a_timeout_out_of_range() {
    let lab = Lab::ra_mini("lcs-timeout");
    let (env, globals) = library(&lab);
    for timeout in [0, 3601] {
        let request = ResolveRequest {
            timeout,
            spec_path: Some(lab.spec.clone()),
            path: None,
        };
        let Err(error) = specengine_cli::resolve(&env, &globals, &request) else {
            panic!("--timeout {timeout} is refused");
        };
        assert_eq!(
            error.message,
            format!("spec: --timeout {timeout} is out of range: 1 to 3600 seconds")
        );
    }
    assert!(lab.fake.invocations().is_empty(), "nothing spawned");
}
