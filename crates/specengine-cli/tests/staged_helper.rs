//! AC-15 of docs/features/spec-cli-staged.md: the scratch-git helper
//! (`common::git::Sandbox`) isolates every git process. Over an
//! environment whose `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`,
//! `GIT_OBJECT_DIRECTORY` and `GIT_COMMON_DIR` point into a second
//! repository, and whose `HOME/.gitconfig` (and `XDG_CONFIG_HOME`) set a
//! failing `core.hooksPath`, `commit.gpgsign` and an `init.templateDir`
//! with a failing hook: `init`, `add` and `commit` succeed in a scratch
//! repository, and the second repository is byte for byte unchanged.

#![cfg(unix)]

mod common;

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use common::git::Sandbox;
use common::{Scratch, snapshot, write};

fn failing_hook(dir: &Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    let hook = dir.join(name);
    fs::write(&hook, "#!/bin/sh\necho hostile hook ran >&2\nexit 1\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A hostile `HOME` and `XDG_CONFIG_HOME` under `dir`.
fn hostile_config(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let home = dir.join("hostile-home");
    let xdg = dir.join("hostile-xdg");
    let hooks = dir.join("hostile-hooks");
    let template = dir.join("hostile-template");
    for hook in ["pre-commit", "commit-msg", "post-commit"] {
        failing_hook(&hooks, hook);
        failing_hook(&template.join("hooks"), hook);
    }
    let config = format!(
        "[core]\n\thooksPath = {}\n[commit]\n\tgpgsign = true\n[gpg]\n\tprogram = /usr/bin/false\n[init]\n\ttemplateDir = {}\n[user]\n\tname = Hostile\n\temail = hostile@example.invalid\n",
        hooks.display(),
        template.display()
    );
    write(&home, ".gitconfig", &config);
    write(&xdg, "git/config", &config);
    (
        fs::canonicalize(&home).unwrap(),
        fs::canonicalize(&xdg).unwrap(),
    )
}

#[test]
fn the_helper_isolates_git_from_the_inherited_environment() {
    let scratch = Scratch::new("staged-helper");
    let clean = Sandbox::new(scratch.path());

    // The second repository the inherited variables point into.
    let second = scratch.dir("second");
    clean.init(&second);
    write(&second, "kept.txt", "kept\n");
    clean.add_all(&second);
    clean.commit(&second, "second");
    let second_before = snapshot(&second);

    let (home, xdg) = hostile_config(scratch.path());
    let dot_git = second.join(".git");
    let mut base: Vec<(OsString, OsString)> = std::env::vars_os().collect();
    base.retain(|(name, _)| !name.to_string_lossy().starts_with("GIT_"));
    for (name, value) in [
        ("GIT_DIR", dot_git.clone()),
        ("GIT_WORK_TREE", second.clone()),
        ("GIT_INDEX_FILE", dot_git.join("index")),
        ("GIT_OBJECT_DIRECTORY", dot_git.join("objects")),
        ("GIT_COMMON_DIR", dot_git.clone()),
        ("HOME", home.clone()),
        ("XDG_CONFIG_HOME", xdg.clone()),
    ] {
        base.push((name.into(), value.into_os_string()));
    }

    // The hostile config is in force for a git that is not isolated: a
    // commit in a throwaway repository fails.
    let throwaway = scratch.dir("throwaway");
    clean.init(&throwaway);
    write(&throwaway, "a.txt", "a\n");
    clean.add_all(&throwaway);
    let hostile = std::process::Command::new(clean.git_program())
        .current_dir(&throwaway)
        .env_clear()
        .envs(
            base.iter()
                .filter(|(name, _)| !name.to_string_lossy().starts_with("GIT_"))
                .map(|(name, value)| (name.clone(), value.clone())),
        )
        .args(["commit", "-q", "-m", "hostile"])
        .output()
        .unwrap();
    assert!(
        !hostile.status.success(),
        "the hostile HOME makes a commit fail: {}",
        String::from_utf8_lossy(&hostile.stderr)
    );

    // Through the helper over that environment: init, add, commit succeed.
    let sandbox = Sandbox::over(scratch.path(), base);
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
    ] {
        assert_eq!(sandbox.var(name), None, "{name} reaches git");
    }
    assert_eq!(sandbox.var("GIT_CONFIG_GLOBAL"), Some("/dev/null".as_ref()));
    assert_eq!(sandbox.var("GIT_CONFIG_NOSYSTEM"), Some("1".as_ref()));
    assert_eq!(sandbox.var("HOME"), Some(sandbox.home().as_os_str()));
    assert_eq!(
        sandbox.var("XDG_CONFIG_HOME"),
        Some(sandbox.xdg().as_os_str())
    );
    assert_eq!(
        sandbox.var("GIT_CEILING_DIRECTORIES"),
        Some(scratch.path().as_os_str())
    );

    let mine = scratch.path().join("mine");
    sandbox.init(&mine);
    assert!(
        !mine.join(".git/hooks").exists(),
        "no template was copied into the new repository"
    );
    write(&mine, "docs/a.md", "# A\n");
    sandbox.add_all(&mine);
    sandbox.git(&mine, &["commit", "-q", "-m", "mine"]);
    let log = sandbox.git_text(&mine, &["log", "--format=%an <%ae> %cn <%ce> %ad"]);
    assert_eq!(
        log,
        "Scratch Author <author@example.invalid> Scratch Committer <committer@example.invalid> Thu Jan 1 00:00:00 2026 +0000"
    );
    assert_eq!(sandbox.git_text(&mine, &["ls-files"]), "docs/a.md");
    let remotes = sandbox.git_text(&mine, &["remote"]);
    assert_eq!(remotes, "", "no remote");

    assert!(
        snapshot(&second) == second_before,
        "the second repository changed"
    );
    assert!(
        snapshot(sandbox.home()).is_empty(),
        "nothing written under the scratch HOME"
    );
}
