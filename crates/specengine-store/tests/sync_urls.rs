//! docs/features/hub-git.md (RM-0051) P24, the store's half: a URL's credential
//! judged, and git's words cleared of URLs, when non-ASCII characters stand
//! just before `://` or just before a scheme. Never a panic (no byte-index
//! slicing inside a character); each whitespace-delimited token holding a URL
//! replaced whole by `<url>`, never cut inside, whatever characters precede it
//! ("Data", **`error`**); the remote's own URL written as its name. The
//! redaction is reached through the public look on a real clone of a bare
//! repository (`SyncGit::look`, then `Upstream::redacted`), every git process
//! in the CLI tests' sandbox. The daemon's half is `specengine-http`'s
//! `tests/hub_git.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use common::{Scratch, write};
use git::Sandbox;
use specengine_store::{GitEnv, Looked, SyncGit, holds_secret};

/// P24: `holds_secret` on URLs whose scheme follows, or whose `://` follows, a
/// non-ASCII character: a password in any scheme, a user name on `https`,
/// none at all. M: byte-index slicing.
#[test]
fn p24_a_credential_is_judged_whatever_precedes_the_scheme() {
    for (url, holds) in [
        ("x\u{e9}://u:p@example.invalid/q.git", true),
        ("na\u{ef}vehttps://u:p@example.invalid/s.git", true),
        ("\u{e9}https://tok@example.invalid/r.git", true),
        ("refus\u{e9} https://u:p@example.invalid/r.git", true),
        ("\u{e9}://example.invalid/x", false),
        ("\u{e9}\u{e9}\u{e9}", false),
        ("ssh://git@example.invalid/r.git", false),
    ] {
        assert_eq!(holds_secret(url), holds, "{url:?}");
    }
}

/// A clone of a bare repository, `main` tracking `origin/main`, clean: what a
/// look passes.
struct Clone {
    _scratch: Scratch,
    git: Sandbox,
    clone: PathBuf,
    bare: PathBuf,
}

fn clone(label: &str) -> Clone {
    let scratch = Scratch::new(label);
    let git = Sandbox::new(scratch.path());
    let seed = scratch.join("seed");
    write(&seed, "notes.md", "A note.\n");
    git.init(&seed);
    git.add_all(&seed);
    git.commit(&seed, "first");
    let bare = scratch.join("origin.git");
    git.init_with(&bare, &["--bare"]);
    git.git(&bare, &["config", "receive.autogc", "false"]);
    let bare = fs::canonicalize(bare).expect("the bare repository");
    let bare_text = bare.to_str().expect("UTF-8");
    git.git(&seed, &["remote", "add", "origin", bare_text]);
    git.git(&seed, &["push", "-q", "-u", "origin", "main"]);
    let clone = scratch.join("clone");
    git.git(
        scratch.path(),
        &[
            "clone",
            "-q",
            "--template=",
            bare_text,
            clone.to_str().expect("UTF-8"),
        ],
    );
    let clone = fs::canonicalize(clone).expect("the clone");
    git.quiet(&clone);
    Clone {
        _scratch: scratch,
        git,
        clone,
        bare,
    }
}

/// P24: git's words holding U+00E9 then a space and a URL, U+00E9 right before
/// `://` and U+00EF right before a scheme, each URL with `u:p@`, then the
/// remote's own URL in quotes: the three tokens become `<url>` whole, the words
/// around them stay, and the remote's URL becomes its name. M: byte-index
/// slicing.
#[test]
fn p24_each_url_token_is_replaced_whole_whatever_precedes_it() {
    let repo = clone("sync-urls-redacted");
    let env = GitEnv::new(repo.clone.clone(), repo.git.vars());
    let sync = SyncGit::open(&repo.clone, &env, Duration::from_secs(30)).expect("open");
    let upstream = match sync.look(&[]).expect("the look") {
        Looked::Ready { upstream, .. } => upstream,
        Looked::Stopped(why) => panic!("the look stopped: {why}"),
    };
    let bare = repo.bare.to_str().expect("UTF-8");
    let spoken = format!(
        "remote: refus\u{e9} https://u:p@example.invalid/r.git \
         x\u{e9}://u:p@example.invalid/q.git na\u{ef}vehttps://u:p@example.invalid/s.git fin\n\
         error: failed to push some refs to '{bare}'"
    );
    let shown = upstream.redacted(&spoken);
    let (first, second) = shown.split_once('\n').expect("two lines");
    assert_eq!(first, "remote: refus\u{e9} <url> <url> <url> fin");
    assert!(
        !second.contains(bare) && second.contains("origin"),
        "the remote's URL as its name: {second:?}"
    );
    for leak in ["u:p", "example.invalid", "x\u{e9}", "na\u{ef}ve"] {
        assert!(!shown.contains(leak), "{leak:?} in {shown:?}");
    }
}
