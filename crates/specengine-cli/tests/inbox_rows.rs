//! docs/features/mixed-load-memory.md (RM-0134) MX1 [identical answers]:
//! what the inbox and its siblings print over one seeded queue stays byte
//! for byte what 540db8ff printed. The scene: a scratch git copy of
//! `fixtures/spec-a` (main worktree, linked worktree `t1`), another
//! repository of the project (it exists), a gone one (never made), and the
//! queue of the store's `mixed_load_oracle/queue_rows.rs` (a `#[cfg(test)]`
//! module, included here by `#[path]`) written into the pair's
//! data directory: every kind in every state, a staged reject and a staged
//! approve, recorded items, another repository's rows, 12 rows of the gone
//! one, corrupt `author`, `branch` and `base_commit`, a NULL and an unknown
//! state, a corrupt row in a state the inbox leaves out.
//!
//! - `mx1_the_reads_are_the_goldens`: `spec inbox`, `--json`, `--all`,
//!   `--all --json` (the binary: code, stdout, stderr), `GET
//!   /api/projects/lantern-keep/inbox` of `spec serve` (status, type,
//!   body), and `spec approve --open` through the library (a terminal that
//!   answers nothing: its text and JSON, every line it was told, every
//!   question it was asked) equal [`GOLDEN`], recorded at 540db8ff by
//!   `record_the_inbox_rows_golden`. Scratch paths are placeholders.
//!   M: a bare `status IN (…)` prefilter (the NULL and unknown states'
//!   notes gone); only the inbox's columns selected (the corrupt
//!   `author`'s note gone): red.
//!
//! The daemon is the build directory's `specengine-http` beside `spec`
//! (`cargo build -p specengine-http` when this binary runs alone).

#![cfg(unix)]

mod common;

/// The store's public items, as `queue_rows` names them.
mod store {
    pub use specengine_store::*;
}

#[path = "../../specengine-store/src/queue/mixed_load_oracle/queue_rows.rs"]
mod queue_rows;

use std::ffi::OsString;
use std::fs;
use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::proposal::Pair;
use common::repository_root;
use common::serve::Daemon;
use serde::{Deserialize, Serialize};
use specengine_cli::{
    ApproveOpenRequest, Globals, Outcome, RoundTerminal, approve_open, render_json, render_text,
};
use specengine_store::Place;

/// The golden, beside this file.
const GOLDEN: &str = "inbox_rows_before.json";

/// The build the golden was recorded with.
const RECORDED_WITH: &str = "spec and specengine-http built from 540db8ff (0.49.2), before any \
                             src change of mixed-load-memory";

/// The walk's one clock reading.
const WALK_NOW: &str = "2026-11-20T12:00:00Z";

/// The reads the binary plays, in order.
const READS: [&[&str]; 4] = [
    &["inbox"],
    &["inbox", "--json"],
    &["inbox", "--all"],
    &["inbox", "--all", "--json"],
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Read {
    args: Vec<String>,
    code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Fetched {
    status: String,
    content_type: Option<String>,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Walked {
    /// The walk's text and JSON, or its error.
    text: String,
    json: String,
    told: Vec<String>,
    reported: Vec<String>,
    asked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Played {
    reads: Vec<Read>,
    get_inbox: Fetched,
    approve_open: Walked,
}

#[derive(Debug, Serialize, Deserialize)]
struct Golden {
    recorded_with: String,
    procedure: String,
    played: Played,
}

/// The seeded scene.
struct Scene {
    pair: Pair,
}

fn place(git_common_dir: &Path, worktree: &Path, branch: &str, base: &str) -> Place {
    Place {
        git_common_dir: git_common_dir.to_str().expect("UTF-8").to_owned(),
        worktree: worktree.to_str().expect("UTF-8").to_owned(),
        root_rel: String::new(),
        branch: branch.to_owned(),
        base_commit: base.to_owned(),
    }
}

fn scene(label: &str) -> Scene {
    let pair = Pair::new(label, "spec-a");
    let other = pair.scratch.join("other");
    pair.git.init(&other);
    let other = fs::canonicalize(other).expect("the other repository");
    let gone = pair.scratch.join("gone");
    let base = pair.rev(&pair.main, "HEAD");
    let common = fs::canonicalize(pair.main.join(".git")).expect("the common dir");
    let places = queue_rows::Places {
        ours: place(&common, &pair.linked, "t1", &base),
        other: place(&other.join(".git"), &other, "main", &base),
        gone: place(&gone.join(".git"), &gone, "main", &base),
    };
    let db = pair.db();
    fs::create_dir_all(db.parent().expect("the data directory")).expect("the data directory");
    queue_rows::seed(&pair.scratch.join("template.db"), &db, &pair.slug, &places);
    Scene { pair }
}

/// `text` with the scene's scratch paths as placeholders.
fn placed(pair: &Pair, text: &str) -> String {
    let mut out = text.to_owned();
    for (path, name) in [
        (&pair.linked, "<linked>"),
        (&pair.main, "<main>"),
        (&pair.home, "<home>"),
    ] {
        out = out.replace(path.to_str().expect("UTF-8"), name);
    }
    out.replace(pair.scratch.path().to_str().expect("UTF-8"), "<scratch>")
}

/// The daemon's variables: the sandbox's, the pair's `HOME`.
fn vars(pair: &Pair) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = pair
        .git
        .vars()
        .into_iter()
        .filter(|(name, _)| name != "HOME")
        .collect();
    vars.push(("HOME".into(), pair.home.clone().into()));
    vars
}

/// `GET path` of the daemon at `url` (`http://127.0.0.1:PORT`), one
/// connection, closed by the answer: its status line, `Content-Type` and
/// body (a chunked body joined).
fn get(url: &str, path: &str) -> Fetched {
    let authority = url.strip_prefix("http://").expect("an http URL");
    let mut stream = TcpStream::connect(authority).expect("the daemon listens");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .expect("a timeout");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )
    .expect("the request");
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).expect("the answer");
    let text = String::from_utf8(bytes).expect("a UTF-8 answer");
    let (head, body) = text.split_once("\r\n\r\n").expect("a head and a body");
    let mut lines = head.split("\r\n");
    let status = lines.next().expect("a status line").to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let header = |name: &str| {
        headers
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, value)| value.clone())
    };
    let body = if header("transfer-encoding").is_some_and(|value| value == "chunked") {
        let mut joined = String::new();
        let mut rest = body;
        loop {
            let (size, after) = rest.split_once("\r\n").expect("a chunk size");
            let size = usize::from_str_radix(size.trim(), 16).expect("a hex size");
            if size == 0 {
                break;
            }
            joined.push_str(&after[..size]);
            rest = &after[size + 2..];
        }
        joined
    } else {
        body.to_owned()
    };
    Fetched {
        status,
        content_type: header("content-type"),
        body,
    }
}

/// A terminal with no input: every question unanswered.
#[derive(Default)]
struct Silent {
    told: Vec<String>,
    reported: Vec<String>,
    asked: Vec<String>,
}

impl RoundTerminal for Silent {
    fn report(&mut self, text: &str) {
        self.reported.push(text.to_owned());
    }

    fn ask(&mut self, question: &str) -> Option<bool> {
        self.asked.push(question.to_owned());
        None
    }

    fn tell(&mut self, line: &str) {
        self.told.push(line.to_owned());
    }
}

/// Every read of the golden on a fresh scene, in its order: the binary's
/// reads, the daemon's, then the walk (the only one that may write).
fn play(label: &str) -> Played {
    let Scene { pair } = scene(label);
    let reads = READS
        .iter()
        .map(|args| {
            let done = pair.spec_piped(&pair.main, args, b"");
            Read {
                args: args.iter().map(|arg| (*arg).to_owned()).collect(),
                code: done.code,
                stdout: placed(&pair, &done.stdout),
                stderr: placed(&pair, &done.stderr),
            }
        })
        .collect();
    let daemon = Daemon::start(&vars(&pair), &pair.main, &[&pair.main]);
    let mut get_inbox = get(&daemon.url, &format!("/api/projects/{}/inbox", pair.slug));
    drop(daemon);
    get_inbox.body = placed(&pair, &get_inbox.body);
    let mut terminal = Silent::default();
    let walked = approve_open(
        &pair.env(&pair.main),
        &Globals::default(),
        &ApproveOpenRequest {
            now: WALK_NOW.to_owned(),
            git: pair.git_env(&pair.main),
        },
        &mut terminal,
    );
    let (text, json) = match walked {
        Ok(walk) => {
            let outcome = Outcome::OpenWalk(walk);
            (render_text(&outcome), render_json(&outcome))
        }
        Err(error) => (
            format!("error {:?}: {}", error.exit, error.message),
            String::new(),
        ),
    };
    let all = |lines: Vec<String>| lines.iter().map(|line| placed(&pair, line)).collect();
    Played {
        reads,
        get_inbox,
        approve_open: Walked {
            text: placed(&pair, &text),
            json: placed(&pair, &json),
            told: all(terminal.told),
            reported: all(terminal.reported),
            asked: all(terminal.asked),
        },
    }
}

fn golden_path() -> PathBuf {
    repository_root()
        .join("crates/specengine-cli/tests")
        .join(GOLDEN)
}

/// MX1: the binary's, the daemon's and the walk's answers over the seeded
/// queue are the golden's, byte for byte.
#[test]
fn mx1_the_reads_are_the_goldens() {
    let text = fs::read_to_string(golden_path())
        .unwrap_or_else(|error| panic!("{GOLDEN}: {error}; record it once at 540db8ff"));
    let golden: Golden =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{GOLDEN}: {error}"));
    assert_eq!(golden.recorded_with, RECORDED_WITH);
    let now = play("mx1-play");
    for (read, recorded) in now.reads.iter().zip(&golden.played.reads) {
        assert_eq!(read, recorded, "spec {:?}", recorded.args);
    }
    assert_eq!(now.reads.len(), golden.played.reads.len());
    assert_eq!(now.get_inbox, golden.played.get_inbox, "GET …/inbox");
    assert_eq!(
        now.approve_open, golden.played.approve_open,
        "approve --open"
    );
}

/// Writes [`GOLDEN`] from this build, at 540db8ff before any `src/` change
/// of mixed-load-memory. Run once, by hand: `cargo build -p
/// specengine-http && cargo nextest run -p specengine-cli --test
/// inbox_rows record_the_inbox_rows_golden --run-ignored only`. The scene
/// is played twice and must answer alike; it refuses when the golden
/// exists.
#[test]
#[ignore = "manual: records mixed-load-memory's MX1 golden at 540db8ff, once"]
fn record_the_inbox_rows_golden() {
    let path = golden_path();
    assert!(
        !path.exists(),
        "{GOLDEN} exists: it is recorded once, never again"
    );
    let played = play("mx1-record-a");
    assert_eq!(
        played,
        play("mx1-record-b"),
        "one scene answers alike twice"
    );
    // What the golden must hold to judge MX1: every note the rows make.
    let inbox = &played.reads[0];
    assert_eq!(inbox.code, 0, "{inbox:?}");
    for needle in [
        "the stored `author` cannot be read",
        "the stored `branch` cannot be read",
        "the stored `base_commit` cannot be read",
        "the stored `status` cannot be read: it is NULL",
        "`parked` is no state",
        "proposal(s) of another repository",
        "12 proposal(s) of a repository that no longer exists",
        "… and 2 more",
        "open (staged)",
    ] {
        assert!(
            inbox.stdout.contains(needle) || inbox.stderr.contains(needle),
            "spec inbox shows {needle:?}: {inbox:?}"
        );
    }
    let all = &played.reads[2];
    assert_eq!(
        all.stdout.matches("cannot be read").count() + all.stderr.matches("cannot be read").count(),
        6,
        "--all names six rows that do not decode: {all:?}"
    );
    assert!(
        played.get_inbox.status.contains(" 200 "),
        "{:?}",
        played.get_inbox
    );
    assert!(
        played.approve_open.text.contains("has a reject staged"),
        "{:?}",
        played.approve_open
    );
    let golden = Golden {
        recorded_with: RECORDED_WITH.to_owned(),
        procedure: "inbox_rows.rs `play`: a fresh spec-a pair, another repository and a gone \
                    one; the queue of the store's mixed_load_oracle/queue_rows.rs restored into \
                    the pair's data directory; the reads in the main worktree with the pair's HOME; scratch \
                    paths as <linked>, <main>, <home>, <scratch>"
            .to_owned(),
        played,
    };
    let mut text = serde_json::to_string_pretty(&golden).expect("JSON");
    text.push('\n');
    fs::write(&path, text).expect("the golden");
}
