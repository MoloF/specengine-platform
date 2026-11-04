//! docs/features/hub-git.md (RM-0051, Phase 7 step 10b), the daemon's half: a
//! hub with `--push` fetches, merges and pushes its checkout's branch, never
//! rebasing, and shows the push status at `GET .../sync`. Through the real
//! daemon over a clone of a bare repository in the test's scratch, which a
//! developer's clone pushes to as well; over `fixtures/card-press` (a print
//! shop, no code) and `fixtures/apply-outputs/forge` (a smithing game whose
//! index is generated and whose roots reach beyond `docs/`), so the sync is
//! tried on projects of different nature (ADR-0008). Every expected value is
//! worked out from the spec, never read back from a run.
//!
//! Each hub runs `--public-origin --push --push-deadline 3` (the deadline
//! named where a criterion names another), its every git child through a
//! `PATH` wrapper that logs the child's argv and its `GIT_NO_LAZY_FETCH`
//! before it execs the real git. A test waits for `GET .../sync`'s `at` to
//! change, a second after the last one (`at` has a second's resolution), then
//! asserts with git.
//!
//! - P1 `p1_*`: after a confirm's sync the remote is `applied_commit`, `ok`,
//!   `ahead` 0; a hand commit goes with the next timer's sync. M: no push.
//! - P2 `p2_*`: a developer's commit on another path: a merge commit, first
//!   parent `applied_commit`, the queue unchanged. M: `git pull --rebase`.
//! - P3 `p3_*`: P2 under `pull.rebase`, `branch.main.rebase`, `merge.autoStash`
//!   and `merge.ff=only`. M: the sync as `git pull`.
//! - P4 `p4_*`: one span both ways: `held`, the path named, nothing touched,
//!   the next confirm a 200. M: the merge left in progress; `-X ours`.
//! - P5 `p5_*` (forge): two creates conflicting in `docs/index.md` alone: the
//!   merge commit holds both, its outputs its own render, no `index-drift`.
//!   M: abort on any conflict; git's conflicted bytes kept.
//! - P6 `p6_*`: a `receivepack` wrapper lands a developer's commit first: both
//!   reachable, no `--force*`. M: no retry; a retry with `--force`.
//! - P7 `p7_*`: a `pre-receive` saying `frozen` and U+202E: `held`, escaped,
//!   `HEAD` kept, reads and confirms 200. M: reset to the remote.
//! - P8 `p8_*`: the wrapper exits 1 after `receive-pack` landed: `retrying`
//!   with `ahead` 0 from git, the next `ok`, no merge commit. M: the status
//!   from the push's exit code.
//! - P9 `p9_*`: a `pre-receive` sleeping 30 s: killed at the deadline,
//!   `retrying`, named; meanwhile `GET .../tree` within 1 s and a confirm
//!   commits. M: the push inside the turn; no deadline.
//! - P10 `p10_*`: an orphan history: `held`, the remote unchanged. M:
//!   `--allow-unrelated-histories`.
//! - P11 `p11_*`: the remote forced back to the hub commit's parent: pushed
//!   back. M: reset to the remote.
//! - P12 `p12_*`: the checkout's `pre-push` exits 1: `retrying`, its words,
//!   the remote unchanged. M: `--no-verify`.
//! - P13 `p13_*`: a modified and an untracked file: `stopped`, both named,
//!   bytes and mtimes kept. M: `stash`, `reset --hard`, `clean`.
//! - P14 `p14_*`: logging `uploadpack` and `receivepack` wrappers never run for
//!   a local daemon, a hub without `--push` (its confirm too), `spec serve`, a
//!   terminal's approve and reject; `--push` without `--public-origin` exits 2
//!   creating nothing. M: a sync on every daemon; a push from `apply.rs`.
//! - P15 `p15_*` and [`assert_pins`]: the source's string literals; every git
//!   child of P1-P14 (the terminal's applies included) by the wrapper, matched
//!   on the verb and on push flags, never on substrings. M:
//!   `--force`; `GIT_SSH_COMMAND`; the variable dropped from
//!   `WorktreeGit::command`.
//! - P16 `p16_*`: no manifest names `git2`, `gix`, `reqwest`, `ureq`, `ssh2`,
//!   `russh`. M: add `gix`.
//! - P17 `p17_*`: three URLs holding a secret: `stopped`, no fetch, the secret
//!   nowhere; `ssh://git@...` passes the look. M: no check; `remote.origin.url`
//!   alone.
//! - P18 `p18_*`: the route on a hub with `--push` alone, five keys; the GET
//!   pin is `tests/hub.rs`, the key set `tests/daemon_keys.rs` (`HubSync`). M:
//!   mounted locally; mounted on every hub.
//! - P19 `p19_*`: the merge commit's message and identity are git's and the
//!   checkout's; none: `stopped`. M: `identified()`'s login.
//! - P20: [`World::assert_tidy`] after every outcome of P1-P12 and P17. M: a
//!   scratch file left in the worktree.
//! - P21 `p21_*`, the hub's half (forge, its config made the spec's synthetic
//!   one): a sparse checkout lacking `design/` stops the sync, also when a
//!   merge brings the root. The CLI's half is `specengine-cli`'s
//!   `tests/sparse_set.rs`. M: `docs/` hardcoded.
//! - P22 `p22_*`: the harness's own: under `product_env` an `ssh://` fetch is
//!   git's transport refusal. M: `GIT_ALLOW_PROTOCOL` removed.
//! - P23 `p23_*`: `spec check --json`, `spec health --json`, `spec gate`, `GET
//!   .../tree`, `.../inbox`, `.../check` byte for byte in step, two commits
//!   ahead and with the remote moved away. M: a note or row from push state.
//! - P24 `p24_*` (iteration 2): non-ASCII characters before `://` in a
//!   remote's and a hook's words: no panic, `ok` / `held` / `retrying` as git
//!   says, each URL token replaced whole. The store's half is
//!   `specengine-store`'s `tests/sync_urls.rs`. M: byte-index slicing.
//! - P25 `p25_*`: a smudge filter hanging the fast-forward past the deadline:
//!   no `index.lock` left, the next confirm commits. M: SIGKILL at once.
//! - P26 `p26_*`: a second worktree served: its status `stopped`, not synced;
//!   its sparse gap stops nothing. M: the first worktree's status shared.
//! - P27 `p27_*`: a confirm held in `pre-commit` while the timer's sync looks:
//!   never `stopped`, then `ok` with the commit pushed. M: cleanliness decided
//!   in the look.
//!
//! Beyond the criteria, from "Data" and "Rules and edge cases": the flags'
//! ranges and refusals and the start line; the look's other stops (no
//! upstream, `ahead` and `behind` null; a detached `HEAD`; a shallow and a
//! partial clone; a `bisect` in progress); a push URL and the other schemes of
//! the URL rule; three races in a row `retrying`, nothing forced; a long
//! non-ASCII refusal cut within 4096 bytes; the one stderr line per change; a
//! URL never in `error`.
//!
//! U1 is the UI's (`ui/src/app/SyncLine.test.tsx`,
//! `ui/src/inbox/ConfirmDialog.test.tsx`).
//!
//! docs/features/read-trust.md (RM-0054) AC-6 (a)-(e), the hub's half, over
//! the same worlds: a hub's reads trust the index its writers keep, so after
//! each event the next `search`, made after a read that walked, shows the
//! disk. `rt6a_*` a confirm; `rt6b_*` a fast-forward (a developer's commit
//! built on the confirm's, pushed by the checkout's `post-commit` hook, so
//! the confirm's own sync fast-forwards); `rt6c_*` a merge commit;
//! `rt6d_*` a merge that adds a `[paths]` root holding a file; `rt6e_*` a
//! hub restarted over an index made stale while it was down. M: the merge's
//! `update_paths` dropped; the update run with the config as read before
//! the merge; a trust that starts walked.
//!
//! read-trust R11 and R12, through `specengine_cli::sync_once` itself over
//! the same worlds (no daemon), its merge run inside `walking` of each
//! served root's trust, as the daemon's sync runs it, and the dirty mark
//! read back (`IndexTrust::is_dirty`): `rt11_*` a fast-forward updates the
//! index and doubts nothing; a failed `git diff` doubts every root; a
//! merged config that does not parse doubts its own root alone; `rt12_*` a
//! failed fast-forward doubts every root. The failures come from the
//! wrapper on `PATH`, failing that one git verb. M: each doubt dropped.
//! Review iteration 2: a diff naming a non-UTF-8 path (the wrapper writes
//! one) doubts every root; a served root outside the checkout's top is
//! doubted; a merge that gives a missing `[paths]` root its first file, or
//! takes a root's last, leaves the trusted read's warnings the walk's; a
//! confirm that step 10 refuses after its commit (a `pre-commit` hook adds
//! another file to it) doubts the project, so the next read shows that
//! file.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use common::hub::{ORIGIN, Proxied, Setup, hub_args, through};
use common::{
    Git, Reply, Scratch, Server, alarmed, product_env, product_path, propose_update, read_text,
    real_git, replace, repository_root, root_args, run_product, snapshot, spec, spec_bin,
    spec_json, write,
};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveRequest, Env, Exit, GitEnv, Globals, IndexRequest, IndexTrust, RejectRequest,
    SearchRequest, SyncOutcome, SyncRepository, approve, reject, sync_once, trusted, utc_now,
    walking,
};

/// The print shop's project and the game's.
const PRESS: &str = "/api/projects/card-press";
const FORGE: &str = "/api/projects/ember-forge";

/// The one login of each project's `[team]`: its owner stages and confirms.
const TEAM: &str = "\n[team.ann]\nrole = \"owner\"\nemail = \"ann@example.org\"\n";
const ANN: &str = "ann <ann@example.org>";

/// The checkout's own identity: a sync's merge commit is made under it.
const OWNER_NAME: &str = "Owner";
const OWNER_EMAIL: &str = "owner@example.invalid";
const OWNER: &str = "Owner <owner@example.invalid>";

/// The flags of every hub here but where a criterion names others.
const PUSH: [&str; 3] = ["--push", "--push-deadline", "3"];

/// A sync a signal starts ends well within this.
const SYNC_WAIT: Duration = Duration::from_secs(60);
/// A sync the timer starts (`--push-every 1`) ends within this.
const TICK_WAIT: Duration = Duration::from_secs(200);
/// The wait after which a sync that should not run has not.
const QUIET_WAIT: Duration = Duration::from_secs(3);
/// More than `at`'s resolution, a second.
const NEXT_SECOND: Duration = Duration::from_millis(1100);
const POLL: Duration = Duration::from_millis(100);

/// The five keys of the status document, in order ("Data", `HubSync`).
const KEYS: [&str; 5] = ["ahead", "behind", "state", "error", "at"];

/// A merge commit's message ("Merge"): git's default, no trailer.
const MERGE_MESSAGE: &str = "Merge remote-tracking branch 'origin/main'\n";

const BACK_MD: &str = "docs/spec/sht-back.md";
const INNER_MD: &str = "docs/spec/sht-inner.md";

/// The secret-bearing URLs of P17 and the words that must never show.
const PASSWORD_URL: &str = "https://u:s3cret@example.invalid/r.git";
const TOKEN_URL: &str = "https://tok@example.invalid/r.git";
const SECRETS: [&str; 2] = ["s3cret", "tok"];

// ------------------------------------------------------------- the lines

/// "Data"'s `error` lines, their fixed part: an error may go on with git's
/// stderr, so each is a prefix.
fn conflict_line(paths: &[&str], worktree: &Path) -> String {
    format!(
        "merging `origin/main` conflicts in {}: resolve it on the host's terminal in {}",
        paths.join(", "),
        worktree.display()
    )
}

const NO_HISTORY_LINE: &str = "`origin/main` shares no history with `main`: nothing merged";
const SECRET_LINE: &str = "the remote `origin` has a URL holding a user name or password: \
                           refused before any connection";
const REFUSED_LINE: &str = "the remote refused `main`: ";
const PUSH_FAILED_LINE: &str = "push to `origin` failed: ";
const FETCH_FAILED_LINE: &str = "fetch from `origin` failed: ";
const UPSTREAM_LINE: &str =
    "`main` has no upstream on a remote: set one with git branch --set-upstream-to";
const SHALLOW_LINE: &str = "a shallow clone: a sync needs the whole history";
const PARTIAL_LINE: &str = "a partial clone: a sync needs every blob";
const SPARSE_LINE: &str =
    "outside this checkout: design/: widen the sparse checkout (spec sparse-set)";

fn deadline_line(seconds: u32) -> String {
    format!("the push passed its deadline of {seconds} s")
}

fn identity_line(worktree: &Path) -> String {
    format!(
        "a merge commit needs an identity: set user.name and user.email in {}",
        worktree.display()
    )
}

fn dirty_line(count: usize, worktree: &Path) -> String {
    format!("{count} paths are not clean in {}: ", worktree.display())
}

fn operation_line(operation: &str, worktree: &Path) -> String {
    format!(
        "a {operation} is in progress in {}: finish or abort it first",
        worktree.display()
    )
}

/// The daemon's stderr line of a change ("Data").
fn stderr_line(worktree: &Path, state: &str, error: Option<&str>) -> String {
    match error {
        Some(error) => format!(
            "specengine-http: sync {}: {state}: {error}",
            worktree.display()
        ),
        None => format!("specengine-http: sync {}: {state}", worktree.display()),
    }
}

// ------------------------------------------------------------- the world

/// A project's git world: a bare `origin.git`, the developer's clone (where
/// the fixture was first committed) and the hub's checkout, cloned from the
/// bare repository, `main` tracking `origin/main`, the checkout's identity the
/// owner's. `server` is first: it stops before the scratch goes.
struct World {
    server: Option<Server>,
    setup: Setup,
    base: &'static str,
    bare: PathBuf,
    dev: PathBuf,
    dev_home: PathBuf,
    wrap: PathBuf,
    log: PathBuf,
}

impl World {
    /// The print shop with its `[team]`, then `edit` of its config.
    fn press_with(name: &str, edit: impl FnOnce(&Path)) -> Self {
        let seed = Setup::new(name, "card-press");
        append(&seed.root, TEAM);
        edit(&seed.root);
        commit_all(&seed.scratch.git(), &seed.root, "the team");
        Self::over(seed, PRESS)
    }

    fn press(name: &str) -> Self {
        Self::press_with(name, |_| {})
    }

    /// The game with `edit` of its config, its index's outputs rendered by
    /// `spec export index` and its `[team]`, committed.
    fn forge_with(name: &str, edit: impl FnOnce(&Path)) -> Self {
        let seed = Setup::new(name, "apply-outputs/forge");
        edit(&seed.root);
        let renderer = seed.scratch.home("renderer");
        spec(&renderer, &seed.root, &["export", "index"]).code(0);
        append(&seed.root, TEAM);
        commit_all(&seed.scratch.git(), &seed.root, "the index and the team");
        Self::over(seed, FORGE)
    }

    fn over(seed: Setup, base: &'static str) -> Self {
        let Setup {
            root: dev,
            home,
            cwd,
            scratch,
        } = seed;
        let git = scratch.git();
        let bare = scratch.dir("origin.git");
        git.run(
            &bare,
            &[
                "init",
                "-q",
                "--bare",
                "--template=",
                "--initial-branch=main",
            ],
        );
        git.quiet(&bare);
        git.run(&bare, &["config", "receive.autogc", "false"]);
        git.run(&dev, &["remote", "add", "origin", text(&bare)]);
        git.run(&dev, &["push", "-q", "-u", "origin", "main"]);
        let hub = scratch.join("hub");
        git.run(
            scratch.path(),
            &["clone", "-q", "--template=", text(&bare), text(&hub)],
        );
        let hub = fs::canonicalize(hub).expect("the hub's checkout");
        git.quiet(&hub);
        git.run(&hub, &["config", "user.name", OWNER_NAME]);
        git.run(&hub, &["config", "user.email", OWNER_EMAIL]);
        let wrap = scratch.dir("wrap");
        let log = scratch.join("git-children.log");
        script(
            &wrap.join("git"),
            &format!(
                "us=$(printf '\\037')\n\
                 line=\"${{GIT_NO_LAZY_FETCH-unset}}\"\n\
                 for arg in \"$@\"; do line=\"$line$us$arg\"; done\n\
                 printf '%s\\n' \"$line\" >> '{}'\n\
                 exec '{}' \"$@\"",
                log.display(),
                real_git().display()
            ),
        );
        let dev_home = scratch.home("developer");
        Self {
            server: None,
            setup: Setup {
                root: hub,
                home,
                cwd,
                scratch,
            },
            base,
            bare,
            dev,
            dev_home,
            wrap,
            log,
        }
    }

    fn root(&self) -> &Path {
        &self.setup.root
    }

    fn git(&self) -> Git {
        self.setup.scratch.git()
    }

    /// `git args` in `dir`, which must succeed: its stdout, trimmed.
    fn run(&self, dir: &Path, args: &[&str]) -> String {
        self.git().run(dir, args).trim_end().to_owned()
    }

    fn rev(&self, dir: &Path, revision: &str) -> String {
        self.run(dir, &["rev-parse", "--verify", "-q", revision])
    }

    fn head(&self) -> String {
        self.rev(self.root(), "HEAD")
    }

    /// The bare repository's `main`.
    fn remote_main(&self) -> String {
        self.rev(&self.bare, "refs/heads/main")
    }

    fn parents(&self, dir: &Path, commit: &str) -> Vec<String> {
        self.run(dir, &["rev-list", "--parents", "-n", "1", commit])
            .split_whitespace()
            .skip(1)
            .map(str::to_owned)
            .collect()
    }

    /// Whether `ancestor` is reachable from `tip` in `dir`.
    /// Not when `dir` lacks `ancestor` at all (a commit never pushed there).
    fn reaches(&self, dir: &Path, tip: &str, ancestor: &str) -> bool {
        let object = format!("{ancestor}^{{commit}}");
        if !self
            .git()
            .output(dir, &["cat-file", "-e", &object])
            .status
            .success()
        {
            return false;
        }
        let output = self
            .git()
            .output(dir, &["merge-base", "--is-ancestor", ancestor, tip]);
        match output.status.code() {
            Some(0) => true,
            Some(1) => false,
            other => panic!(
                "merge-base --is-ancestor {ancestor} {tip}: {other:?} {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }

    /// The product environment of every daemon here: the logging wrapper first
    /// on `PATH`.
    fn env(&self) -> Vec<(OsString, OsString)> {
        let mut path = OsString::from(self.wrap.as_os_str());
        path.push(":");
        path.push(product_path());
        product_env(&self.setup.home)
            .into_iter()
            .map(|(name, value)| {
                if name == "PATH" {
                    (name, path.clone())
                } else {
                    (name, value)
                }
            })
            .collect()
    }

    /// A hub over the checkout with `flags` after `--public-origin`, listening;
    /// no wait for its sync.
    fn launch(&mut self, flags: &[&str]) -> &Server {
        let root = self.root().to_path_buf();
        self.launch_roots(&[root.as_path()], flags)
    }

    /// [`World::launch`] serving `roots`, in this order.
    fn launch_roots(&mut self, roots: &[&Path], flags: &[&str]) -> &Server {
        self.server = None;
        let mut args = hub_args(roots, ORIGIN);
        args.extend(flags.iter().map(OsStr::new));
        let server = Server::try_start_env(&self.env(), &self.setup.cwd, &args)
            .unwrap_or_else(|refused| panic!("specengine-http {args:?}: {refused:?}"));
        self.server.insert(server)
    }

    /// [`World::launch`], then the start's sync ended: its status.
    fn start(&mut self, flags: &[&str]) -> Value {
        self.launch(flags);
        self.after(&json!({"at": null}), SYNC_WAIT)
    }

    fn start_push(&mut self) -> Value {
        self.start(&PUSH)
    }

    fn stop(&mut self) {
        self.server = None;
    }

    fn server(&self) -> &Server {
        self.server.as_ref().expect("a running daemon")
    }

    /// `GET path` by `ann`.
    fn get(&self, path: &str) -> Reply {
        through(self.server(), "GET", path, &Proxied::fetch("ann"), b"")
    }

    /// `POST <base>/path` of `body` by `ann`.
    fn post(&self, path: &str, body: &Value) -> Reply {
        through(
            self.server(),
            "POST",
            &format!("{}/{path}", self.base),
            &Proxied::write("ann"),
            body.to_string().as_bytes(),
        )
    }

    /// `GET .../sync`: a 200, "Data"'s five keys in order, its `error` shown
    /// as "Data" says ([`assert_shown`]).
    fn status(&self) -> Value {
        let reply = self.get(&format!("{}/sync", self.base));
        assert_eq!(reply.status, 200, "GET sync: {}", reply.text());
        assert_eq!(keys_of(reply.text()), KEYS, "{}", reply.text());
        let document = reply.json();
        self.assert_shown(&document, &reply.body);
        document
    }

    /// "Data", **`error`**: no control but LF and no Cf raw, in the body
    /// either; a URL never shown (the bare repository's path is the URL here).
    fn assert_shown(&self, document: &Value, body: &[u8]) {
        let Some(error) = document["error"].as_str() else {
            assert!(document["error"].is_null(), "{document}");
            return;
        };
        assert!(
            !error.chars().any(raw_control),
            "a raw control or Cf in error: {error:?}"
        );
        assert!(
            !String::from_utf8_lossy(body).chars().any(raw_control),
            "a raw control or Cf in the body"
        );
        assert!(
            !error.contains(text(&self.bare)) && !error.contains("origin.git"),
            "\"A URL is never shown\": {error:?}"
        );
    }

    /// The first status after `before`'s whose `at` differs and is set.
    fn after(&self, before: &Value, within: Duration) -> Value {
        let started = Instant::now();
        loop {
            let status = self.status();
            if !status["at"].is_null() && status["at"] != before["at"] {
                return status;
            }
            assert!(
                started.elapsed() < within,
                "no sync ended within {within:?} after {before}: {status}\n--- the daemon's \
                 stderr\n{}",
                self.server().stderr()
            );
            thread::sleep(POLL);
        }
    }

    /// `act` a second after the last sync's end, then the next sync's status.
    fn synced<T>(&self, act: impl FnOnce() -> T) -> (T, Value) {
        let before = self.status();
        thread::sleep(NEXT_SECOND);
        let out = act();
        (out, self.after(&before, SYNC_WAIT))
    }

    /// `target`'s text with `from` become `to`, raised by `spec propose update`
    /// on the hub's host, its data directory: the ID.
    fn raise(&self, target: &str, from: &str, to: &str) -> String {
        let setup = &self.setup;
        let shown = spec_json(&setup.home, &setup.cwd, &setup.root, &["show", target]).json();
        let text = shown["nodes"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{target}: {shown}"));
        assert!(text.contains(from), "{target} holds no {from:?}: {text}");
        propose_update(
            &setup.home,
            &setup.cwd,
            &setup.root,
            target,
            &text.replacen(from, to, 1),
        )
    }

    /// A new file `path` of `body`, raised by `spec propose create` on the
    /// hub's host: the ID.
    fn raise_create(&self, path: &str, body: &str) -> String {
        let setup = &self.setup;
        let run = run_product(
            &spec_bin(),
            &setup.home,
            &setup.cwd,
            &[
                "--root",
                text(&setup.root),
                "propose",
                "create",
                path,
                "--text-file",
                "-",
                "--rationale",
                "The smith tires.",
            ],
            body.as_bytes(),
        );
        run.code(0);
        let id = run.stdout.lines().next().unwrap_or_default().to_owned();
        assert!(id.starts_with("PR-"), "not stored: {}", run.show());
        id
    }

    fn review(&self, id: &str) -> Value {
        let setup = &self.setup;
        spec_json(&setup.home, &setup.cwd, &setup.root, &["review", id]).json()
    }

    /// `id` staged by `ann` and confirmed in one click: the POST's document,
    /// `applied`.
    fn confirm(&self, id: &str) -> Value {
        let stage = self.post(
            &format!("proposals/{id}/decision"),
            &json!({"decision": "approve", "updated_at": self.review(id)["updated_at"]}),
        );
        assert_eq!(stage.status, 200, "ann stages {id}: {}", stage.text());
        let route = format!("{}/proposals/{id}/confirm", self.base);
        let read = self.get(&route);
        assert_eq!(read.status, 200, "ann reads {id}: {}", read.text());
        let read = read.json();
        let body = json!({"question": read["question"], "updated_at": read["updated_at"]});
        let reply = through(
            self.server(),
            "POST",
            &route,
            &Proxied::write("ann"),
            body.to_string().as_bytes(),
        );
        assert_eq!(reply.status, 200, "ann confirms {id}: {}", reply.text());
        let document = reply.json();
        assert_eq!(document["status"], json!("applied"), "{document}");
        document
    }

    /// A commit of `path` made by hand on the host: its SHA.
    fn hand_commit(&self, path: &str, body: &str) -> String {
        write(self.root(), path, body);
        commit_all(&self.git(), self.root(), "by hand on the host");
        self.head()
    }

    /// A developer's commit of `path` on the remote's `main`, pushed when
    /// `push`: its SHA.
    fn developer(&self, path: &str, body: &str, push: bool) -> String {
        let git = self.git();
        git.run(&self.dev, &["pull", "-q", "--ff-only", "origin", "main"]);
        write(&self.dev, path, body);
        commit_all(&git, &self.dev, "a developer's change");
        if push {
            git.run(&self.dev, &["push", "-q", "origin", "main"]);
        }
        self.rev(&self.dev, "HEAD")
    }

    /// `name` as a hook of the bare repository, `body` its shell.
    fn remote_hook(&self, name: &str, body: &str) {
        script(&self.bare.join("hooks").join(name), body);
    }

    /// `name` as a hook of the hub's checkout.
    fn checkout_hook(&self, name: &str, body: &str) {
        script(&self.root().join(".git").join("hooks").join(name), body);
    }

    /// Every git child the wrapper logged.
    fn children(&self) -> Vec<Child> {
        let Ok(log) = fs::read_to_string(&self.log) else {
            return Vec::new();
        };
        log.lines()
            .map(|line| {
                let mut fields = line.split('\u{1f}');
                Child {
                    lazy: fields.next().unwrap_or_default().to_owned(),
                    args: fields.map(str::to_owned).collect(),
                }
            })
            .collect()
    }

    /// The children whose subcommand is `name`.
    fn children_running(&self, name: &str) -> Vec<Child> {
        self.children()
            .into_iter()
            .filter(|child| child.subcommand() == Some(name))
            .collect()
    }

    /// P20: `git status --porcelain=v1 --untracked-files=all` empty, no
    /// operation in progress, no stash. M: a scratch file left in the
    /// worktree.
    #[track_caller]
    fn assert_tidy(&self, context: &str) {
        let root = self.root();
        assert_eq!(
            self.run(root, &["status", "--porcelain=v1", "--untracked-files=all"]),
            "",
            "{context}: P20, the checkout clean"
        );
        for marker in [
            "MERGE_HEAD",
            "MERGE_MSG",
            "AUTO_MERGE",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "BISECT_LOG",
            "rebase-merge",
            "rebase-apply",
            "sequencer",
        ] {
            assert!(
                !root.join(".git").join(marker).exists(),
                "{context}: P20, .git/{marker} left"
            );
        }
        assert_eq!(
            self.run(root, &["stash", "list"]),
            "",
            "{context}: P20, a stash"
        );
    }

    /// The daemon's stderr lines that are a sync's.
    fn sync_lines(&self) -> Vec<String> {
        self.server()
            .stderr()
            .lines()
            .filter(|line| line.starts_with("specengine-http: sync "))
            .map(str::to_owned)
            .collect()
    }
}

/// One git child, as the wrapper logged it.
#[derive(Debug, Clone)]
struct Child {
    /// `GIT_NO_LAZY_FETCH`'s value, `unset` when absent.
    lazy: String,
    args: Vec<String>,
}

impl Child {
    /// The subcommand: the first word past git's own options (`-C <dir>`, `-c
    /// <k=v>`, `--…`).
    fn subcommand(&self) -> Option<&str> {
        let mut args = self.args.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-C" | "-c" => {
                    args.next();
                }
                other if other.starts_with('-') => {}
                other => return Some(other),
            }
        }
        None
    }

    /// The words after the subcommand.
    fn operands(&self) -> Vec<&str> {
        let Some(name) = self.subcommand() else {
            return Vec::new();
        };
        self.args
            .iter()
            .map(String::as_str)
            .skip_while(|arg| *arg != name)
            .skip(1)
            .collect()
    }
}

impl fmt::Display for Child {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GIT_NO_LAZY_FETCH={} git {}",
            self.lazy,
            self.args.join(" ")
        )
    }
}

/// P15's argv pins over `children` ("Data", **Git children**; D2), matched on
/// the git verb (the first word past git's own options) and on push flags,
/// never on substrings (`rev-parse --git-path rebase-merge` is no `rebase`):
/// - no `pull`, `rebase`, `reset` (but one with a `--` pathspec), `stash`,
///   `clean`, `checkout`, nor
///   `--allow-unrelated-histories`, `-X…` or `--strategy-option`;
/// - a push with no `--force*`, `-f`, `+` refspec, `--delete`, `-d`,
///   `--mirror`, `--all`, `--tags` or `--no-verify`;
/// - a `merge` only `--ff-only --no-autostash` (D2: its flags override
///   `merge.ff` and `merge.autoStash`);
/// - `GIT_NO_LAZY_FETCH=1` on every child but `fetch` and `push`.
#[track_caller]
fn assert_pins(children: &[Child], context: &str) {
    assert!(
        !children.is_empty(),
        "{context}: the wrapper logged no git child"
    );
    for child in children {
        let name = child.subcommand().unwrap_or_default();
        // `reset -q -- <path>` (apply's recovery) moves no ref: a `reset`
        // counts only without a `--` pathspec.
        let pathspec_reset = name == "reset" && child.operands().contains(&"--");
        assert!(
            !["pull", "rebase", "reset", "stash", "clean", "checkout"].contains(&name)
                || pathspec_reset,
            "{context}: P15, {child}"
        );
        for arg in &child.args {
            assert!(
                arg != "--allow-unrelated-histories"
                    && !arg.starts_with("-X")
                    && !arg.starts_with("--strategy-option"),
                "{context}: P15, {child}"
            );
        }
        if name == "push" {
            for arg in child.operands() {
                assert!(
                    !arg.starts_with("--force")
                        && !arg.starts_with('+')
                        && ![
                            "-f",
                            "--delete",
                            "-d",
                            "--mirror",
                            "--all",
                            "--tags",
                            "--no-verify",
                        ]
                        .contains(&arg),
                    "{context}: P15, {child}"
                );
            }
        }
        if name == "merge" {
            let operands = child.operands();
            assert!(
                operands.contains(&"--ff-only") && operands.contains(&"--no-autostash"),
                "{context}: D2, {child}"
            );
        }
        if name != "fetch" && name != "push" {
            assert_eq!(
                child.lazy, "1",
                "{context}: P15, GIT_NO_LAZY_FETCH on {child}"
            );
        }
    }
}

// ------------------------------------------------------------- helpers

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 scratch path")
}

/// `table` at the end of `root`'s config.
fn append(root: &Path, table: &str) {
    let mut config = read_text(root, "specengine.toml");
    config.push_str(table);
    write(root, "specengine.toml", config);
}

fn commit_all(git: &Git, dir: &Path, message: &str) {
    git.run(dir, &["add", "-A"]);
    git.run(dir, &["commit", "-q", "-m", message]);
}

/// An executable `#!/bin/sh` script at `path`.
fn script(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("a directory")).expect("the script's directory");
    fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("the script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// Whether `text` holds `word` with no letter or digit either side: the token
/// `tok` shown, never a key such as `tokens`.
fn holds_word(text: &str, word: &str) -> bool {
    let alnum = |letter: Option<char>| letter.is_some_and(char::is_alphanumeric);
    text.match_indices(word).any(|(at, _)| {
        !alnum(text[..at].chars().next_back()) && !alnum(text[at + word.len()..].chars().next())
    })
}

/// A control character but LF, or a Cf character (`hub-confirm.md`,
/// **Escaping**): what `error` never holds raw.
fn raw_control(letter: char) -> bool {
    (letter.is_control() && letter != '\n')
        || matches!(
            letter,
            '\u{00ad}'
                | '\u{0600}'..='\u{0605}'
                | '\u{061c}'
                | '\u{06dd}'
                | '\u{070f}'
                | '\u{180e}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206f}'
                | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}'
        )
}

/// A JSON object's keys, in the order written.
struct Keys(Vec<String>);

impl<'de> Deserialize<'de> for Keys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Seen;
        impl<'de> Visitor<'de> for Seen {
            type Value = Keys;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Keys, M::Error> {
                let mut keys = Vec::new();
                while let Some((key, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                    keys.push(key);
                }
                Ok(Keys(keys))
            }
        }
        deserializer.deserialize_map(Seen)
    }
}

fn keys_of(json: &str) -> Vec<String> {
    serde_json::from_str::<Keys>(json)
        .unwrap_or_else(|error| panic!("{error}: {json}"))
        .0
}

#[track_caller]
fn assert_state(status: &Value, state: &str, context: &str) {
    assert_eq!(status["state"], json!(state), "{context}: {status}");
}

/// `status`'s `error`, which must start with `line`.
#[track_caller]
fn assert_error(status: &Value, line: &str, context: &str) -> String {
    let error = status["error"]
        .as_str()
        .unwrap_or_else(|| panic!("{context}: no error: {status}"));
    assert!(
        error.starts_with(line),
        "{context}: {error:?} does not start with {line:?}"
    );
    error.to_owned()
}

/// `path`'s bytes and mtime under `root`.
fn stamp(root: &Path, path: &str) -> (Vec<u8>, SystemTime) {
    let full = root.join(path);
    let bytes = fs::read(&full).unwrap_or_else(|error| panic!("{path}: {error}"));
    let modified = fs::metadata(full)
        .and_then(|meta| meta.modified())
        .expect("mtime");
    (bytes, modified)
}

/// `path`'s mtime set far in the past, so a rewrite shows.
fn aged(root: &Path, path: &str) {
    fs::File::options()
        .write(true)
        .open(root.join(path))
        .expect("the file opens")
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000))
        .expect("an old mtime");
}

// --------------------------------------------------------------------- P1

/// P1: the hub only ahead. After a confirm's sync the remote's `main` is
/// `applied_commit`, `ahead` 0, `ok`; a commit made by hand on the host goes
/// with the next sync, the timer's (`--push-every 1`). "Data": one stderr line
/// per change, `ok` once through three `ok` syncs. M: no push.
#[test]
fn p1_a_confirm_and_a_hand_commit_reach_the_remote() {
    let mut world = World::press("git-ahead");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    let started = world.start(&["--push", "--push-deadline", "3", "--push-every", "1"]);
    assert_state(&started, "ok", "the start, in step");
    assert_eq!(started["error"], Value::Null, "{started}");

    let (document, status) = world.synced(|| world.confirm(&id));
    let applied = document["applied_commit"]
        .as_str()
        .expect("applied_commit")
        .to_owned();
    assert_eq!(world.remote_main(), applied, "the remote's `main`");
    assert_state(&status, "ok", "after the confirm");
    assert_eq!(
        (&status["ahead"], &status["behind"]),
        (&json!(0), &json!(0)),
        "{status}"
    );
    world.assert_tidy("after the confirm");

    let before = world.status();
    let hand = world.hand_commit("notes/hand.md", "Set by hand on the host.\n");
    let status = world.after(&before, TICK_WAIT);
    assert_eq!(
        world.remote_main(),
        hand,
        "the hand commit, by the timer's sync"
    );
    assert_state(&status, "ok", "after the timer");
    assert_eq!(status["ahead"], json!(0), "{status}");
    world.assert_tidy("after the timer");
    assert_eq!(
        world.sync_lines(),
        [stderr_line(world.root(), "ok", None)],
        "one line per change"
    );
    assert_pins(&world.children(), "P1");
}

// ------------------------------------------------------------ P2, P3, P19

/// A developer's commit on a path the hub never touches, pushed after the
/// start's sync, then a confirm on the hub: the sync's merge. The applied
/// commit, the developer's, the proposal's ID and the status.
fn diverged(world: &mut World) -> (String, String, String, Value) {
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    assert_state(&world.start_push(), "ok", "the start, in step");
    let theirs = world.developer("notes/elsewhere.md", "A developer's note.\n", true);
    let (document, status) = world.synced(|| world.confirm(&id));
    let applied = document["applied_commit"]
        .as_str()
        .expect("applied_commit")
        .to_owned();
    (applied, theirs, id, status)
}

/// P2 and P3's result: the remote tip a merge commit, `HEAD`, first parent
/// `applied_commit`, second the developer's; the queue's `applied_commit` the
/// same; `ok`, `ahead` 0.
#[track_caller]
fn assert_merged(world: &World, applied: &str, theirs: &str, id: &str, status: &Value) {
    assert_state(status, "ok", "the merge pushed");
    assert_eq!(status["ahead"], json!(0), "{status}");
    let tip = world.remote_main();
    assert_eq!(tip, world.head(), "the remote holds HEAD");
    assert_eq!(
        world.parents(&world.bare, &tip),
        [applied, theirs],
        "a merge commit: applied_commit first, the developer's second"
    );
    assert_eq!(
        world.review(id)["applied_commit"],
        json!(applied),
        "the queue keeps its SHA"
    );
    world.assert_tidy("after the merge");
}

/// P2: diverged on an unrelated path, the remote tip has two parents, first
/// `applied_commit`, unchanged in the queue, then the developer's commit. M:
/// `git pull --rebase`.
#[test]
fn p2_a_divergence_on_another_path_is_merged_never_rebased() {
    let mut world = World::press("git-diverged");
    let (applied, theirs, id, status) = diverged(&mut world);
    assert_merged(&world, &applied, &theirs, &id, &status);
    assert_pins(&world.children(), "P2");
}

/// P3: P2 with `pull.rebase`, `branch.main.rebase`, `merge.autoStash` `true`
/// and `merge.ff` `only` in the checkout's config: the same result. M: the
/// sync as `git pull`.
#[test]
fn p3_rebase_and_ff_settings_change_nothing() {
    let mut world = World::press("git-diverged-config");
    for (key, value) in [
        ("pull.rebase", "true"),
        ("branch.main.rebase", "true"),
        ("merge.autoStash", "true"),
        ("merge.ff", "only"),
    ] {
        world.run(world.root(), &["config", key, value]);
    }
    let (applied, theirs, id, status) = diverged(&mut world);
    assert_merged(&world, &applied, &theirs, &id, &status);
    assert_pins(&world.children(), "P3");
}

/// P19: P2's merge commit carries no SpecEngine trailer: its message is git's
/// default and LF, its author and committer the checkout's `user.name` and
/// `user.email`, never the confirming login. M: reuse `identified()`'s login.
#[test]
fn p19_the_merge_commit_is_gits_message_under_the_checkouts_identity() {
    let mut world = World::press("git-merge-identity");
    let (applied, _, _, status) = diverged(&mut world);
    assert_state(&status, "ok", "the merge pushed");
    let tip = world.remote_main();
    assert_ne!(tip, applied, "a merge commit");
    let raw = world.git().run(&world.bare, &["cat-file", "commit", &tip]);
    let message = raw
        .split_once("\n\n")
        .map(|(_, message)| message)
        .expect("a message");
    assert_eq!(message, MERGE_MESSAGE, "git's default, no trailer");
    let shown = |format: &str| {
        world.run(
            &world.bare,
            &["log", "-1", &format!("--format={format}"), &tip],
        )
    };
    assert_eq!(shown("%an <%ae>"), OWNER, "the author");
    assert_eq!(shown("%cn <%ce>"), OWNER, "the committer");
    assert_ne!(shown("%an <%ae>"), ANN, "never the login");
    assert_eq!(
        world.run(&world.bare, &["log", "-1", "--format=%an <%ae>", &applied]),
        ANN,
        "the confirm's own commit is the login's"
    );
    assert_pins(&world.children(), "P19");
}

/// P19: with neither `user.name` nor `user.email` in the checkout (none in the
/// process either: `product_env`), the sync is `stopped`, nothing merged: `HEAD`
/// the applied commit, the remote the developer's. M: reuse `identified()`'s
/// login.
#[test]
fn p19_with_no_identity_the_sync_stops_and_merges_nothing() {
    let mut world = World::press("git-merge-no-identity");
    for key in ["user.name", "user.email"] {
        world.run(world.root(), &["config", "--unset", key]);
    }
    let (applied, theirs, _, status) = diverged(&mut world);
    assert_state(&status, "stopped", "no identity");
    assert_error(&status, &identity_line(world.root()), "no identity");
    assert_eq!(world.head(), applied, "nothing merged");
    assert_eq!(world.remote_main(), theirs, "nothing pushed");
    world.assert_tidy("no identity");
    assert_pins(&world.children(), "P19");
}

// --------------------------------------------------------------------- P4

/// P4: one span, the hub's confirm and a developer's edit: `held`, the path
/// named; no operation in progress, `git status` empty, `HEAD` and the remote
/// unchanged; the next confirm answers 200 and its sync is `held` again. M:
/// the merge left in progress; `-X ours`.
#[test]
fn p4_a_conflict_on_one_span_holds_and_touches_nothing() {
    let mut world = World::press("git-conflict");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    let next = world.raise(
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the season",
    );
    assert_state(&world.start_push(), "ok", "the start");
    replace(&world.dev, BACK_MD, "the tide table", "the sun table");
    let theirs = {
        let git = world.git();
        commit_all(&git, &world.dev, "the sun table");
        git.run(&world.dev, &["push", "-q", "origin", "main"]);
        world.rev(&world.dev, "HEAD")
    };
    let (document, status) = world.synced(|| world.confirm(&id));
    let applied = document["applied_commit"].as_str().expect("applied_commit");
    assert_state(&status, "held", "a conflict");
    assert_error(
        &status,
        &conflict_line(&[BACK_MD], world.root()),
        "a conflict",
    );
    assert_eq!(world.head(), applied, "HEAD unchanged");
    assert_eq!(world.remote_main(), theirs, "the remote unchanged");
    assert!(
        read_text(world.root(), BACK_MD).contains("the moon table"),
        "the worktree holds the hub's text"
    );
    world.assert_tidy("a conflict");

    let (document, status) = world.synced(|| world.confirm(&next));
    assert_eq!(world.head(), document["applied_commit"].as_str().unwrap());
    assert_state(&status, "held", "the next sync");
    assert_eq!(world.remote_main(), theirs, "the remote unchanged");
    world.assert_tidy("the next sync");
    assert_pins(&world.children(), "P4");
}

// --------------------------------------------------------------------- P5

/// The hub's new file and the developer's: two features of the game whose
/// index lines meet.
const HAMMER_MD: &str = "docs/features/hammer.md";
const HAMMER_TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n\
     # Hammer weight {#MEC-HAMMER}\n\nA heavier hammer strikes harder and tires the smith \
     sooner.\n";
const HEARTH_MD: &str = "docs/features/hearth.md";
const HEARTH_TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [forge]\n---\n\n\
     # Hearth heat {#MEC-HEARTH}\n\nThe hearth cools one step each turn the bellows rest.\n";
const OUTPUTS: [&str; 2] = ["docs/index.md", "docs/index-archive.md"];

/// P5: two creates, the hub's confirm and a developer's commit with its own
/// render, conflict in `docs/index.md` alone (asked of `git merge-tree` here
/// too). The remote tip is a merge commit holding both documents; a clean
/// clone of it renders its outputs byte for byte (`spec export index
/// --stdout`) and `spec check` finds no `index-drift`. M: abort on any
/// conflict; keep git's conflicted bytes.
#[test]
fn p5_an_index_only_conflict_is_merged_with_the_outputs_rendered_again() {
    let mut world = World::forge_with("git-index-conflict", |_| {});
    let id = world.raise_create(HAMMER_MD, HAMMER_TEXT);
    assert_state(&world.start_push(), "ok", "the start");
    let theirs = {
        let git = world.git();
        git.run(&world.dev, &["pull", "-q", "--ff-only", "origin", "main"]);
        write(&world.dev, HEARTH_MD, HEARTH_TEXT);
        spec(&world.dev_home, &world.dev, &["export", "index"]).code(0);
        commit_all(&git, &world.dev, "the hearth");
        git.run(&world.dev, &["push", "-q", "origin", "main"]);
        world.rev(&world.dev, "HEAD")
    };
    let (document, status) = world.synced(|| world.confirm(&id));
    let applied = document["applied_commit"]
        .as_str()
        .expect("applied_commit")
        .to_owned();

    let trial = world.git().output(
        world.root(),
        &[
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            &applied,
            &theirs,
        ],
    );
    let conflicted: Vec<String> = String::from_utf8_lossy(&trial.stdout)
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        (trial.status.code(), conflicted),
        (Some(1), vec![OUTPUTS[0].to_owned()]),
        "the fixture's premise: the two commits conflict in docs/index.md alone"
    );

    assert_state(&status, "ok", "an index-only conflict");
    let tip = world.remote_main();
    assert_eq!(tip, world.head(), "pushed");
    assert_eq!(
        world.parents(&world.bare, &tip),
        [applied.as_str(), theirs.as_str()]
    );
    for path in [HAMMER_MD, HEARTH_MD] {
        world.run(&world.bare, &["cat-file", "-e", &format!("{tip}:{path}")]);
    }
    let clean = world.setup.scratch.join("clean");
    world.run(
        world.setup.scratch.path(),
        &[
            "clone",
            "-q",
            "--template=",
            text(&world.bare),
            text(&clean),
        ],
    );
    let reader = world.setup.scratch.home("reader");
    let render = spec(&reader, &clean, &["export", "index", "--stdout"]);
    render.code(0);
    let committed: String = OUTPUTS
        .iter()
        .map(|path| {
            format!(
                "==> {path} <==\n{}",
                world
                    .git()
                    .run(&world.bare, &["show", &format!("{tip}:{path}")])
            )
        })
        .collect();
    assert_eq!(
        render.stdout, committed,
        "the merge commit equals its own render"
    );
    let report = spec_json(&reader, &clean, &clean, &["check"]).json();
    let drift: Vec<&Value> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| finding["code"] == json!("index-drift"))
        .collect();
    assert!(drift.is_empty(), "no index-drift: {drift:?}");
    world.assert_tidy("an index-only conflict");
    assert_pins(&world.children(), "P5");
}

// --------------------------------------------------------------------- P6

/// P6: a `remote.origin.receivepack` wrapper that, on its first run, lands a
/// developer's commit, then execs `git receive-pack`: the hub's push meets a
/// remote that moved, fetches, merges and pushes again. Both commits are
/// reachable from the remote tip, `ok`, and no push carries `--force*`. M: no
/// retry after a non-fast-forward; a retry with `--force`.
#[test]
fn p6_a_race_at_the_push_is_merged_and_pushed_again() {
    let mut world = World::press("git-race");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    assert_state(&world.start_push(), "ok", "the start");
    let theirs = world.developer("notes/race.md", "Landed first.\n", false);
    let marker = world.setup.scratch.join("raced");
    let wrapper = world.setup.scratch.join("racing-receive-pack");
    script(
        &wrapper,
        &format!(
            "if [ ! -e '{marker}' ]; then\n\
             : > '{marker}'\n\
             ( unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
             GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_QUARANTINE_PATH GIT_PROTOCOL\n\
             '{git}' -C '{dev}' push -q origin main ) >&2 || exit 3\n\
             fi\n\
             exec '{git}' receive-pack \"$@\"",
            marker = marker.display(),
            git = real_git().display(),
            dev = world.dev.display(),
        ),
    );
    world.run(
        world.root(),
        &["config", "remote.origin.receivepack", text(&wrapper)],
    );

    let (document, status) = world.synced(|| world.confirm(&id));
    let applied = document["applied_commit"].as_str().expect("applied_commit");
    assert!(marker.exists(), "the wrapper raced");
    assert_state(&status, "ok", "after the race");
    let tip = world.remote_main();
    assert!(
        world.reaches(&world.bare, &tip, applied),
        "applied_commit is reachable"
    );
    assert!(
        world.reaches(&world.bare, &tip, &theirs),
        "the developer's commit is reachable"
    );
    assert!(
        world.children_running("push").len() >= 2,
        "pushed again: {:?}",
        world.children_running("push")
    );
    world.assert_tidy("after the race");
    assert_pins(&world.children(), "P6");
}

// --------------------------------------------------------------------- P7

/// P7: a `pre-receive` prints `frozen` and U+202E, then exits 1: `held`, the
/// remote's refusal shown with `\u{202e}` written out, never the character, in
/// the body and on stderr; `HEAD` keeps the commit, reads and confirms answer
/// 200. M: reset the branch to the remote.
#[test]
fn p7_a_refusing_remote_holds_and_shows_its_words_escaped() {
    let mut world = World::press("git-refused");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    world.remote_hook(
        "pre-receive",
        "printf 'frozen\\342\\200\\256\\n' >&2\nexit 1",
    );
    let hand = world.hand_commit("notes/hand.md", "Set by hand.\n");
    let base = world.remote_main();
    let status = world.start_push();
    assert_state(&status, "held", "a refusal");
    let error = assert_error(&status, REFUSED_LINE, "a refusal");
    assert!(error.contains("frozen"), "{error:?}");
    assert!(error.contains("\\u{202e}"), "written out: {error:?}");
    assert!(!error.contains('\u{202e}'), "never raw: {error:?}");
    assert!(
        !world.server().stderr().contains('\u{202e}'),
        "never raw on stderr"
    );
    assert_eq!(world.head(), hand, "HEAD keeps the commit");
    assert_eq!(world.remote_main(), base, "the remote unchanged");
    world.assert_tidy("a refusal");

    world.get(&format!("{}/tree", world.base)).status(200);
    let (document, status) = world.synced(|| world.confirm(&id));
    let applied = document["applied_commit"].as_str().expect("applied_commit");
    assert_eq!(world.head(), applied);
    assert_eq!(world.parents(world.root(), applied), [hand.as_str()]);
    assert_state(&status, "held", "refused again once HEAD moved");
    assert_eq!(world.remote_main(), base, "the remote unchanged");
    world.assert_tidy("refused again");
    assert_pins(&world.children(), "P7");
}

// --------------------------------------------------------------------- P8

/// P8: a `receivepack` wrapper exits 1, once, after `git receive-pack`
/// succeeded: that sync is `retrying`, yet `ahead` is 0 (git's own count, the
/// tracking ref moved with the accepted ref) and the remote holds the commit;
/// the next sync is `ok`, `ahead` 0, no merge commit anywhere. M: the status
/// from the push's exit code.
#[test]
fn p8_a_push_that_landed_reads_from_git_not_from_its_exit_code() {
    let mut world = World::press("git-lost-answer");
    let first = world.raise("SHT-BACK", "the tide table", "the moon table");
    let second = world.raise(
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the season",
    );
    assert_state(&world.start_push(), "ok", "the start");
    let base = world.remote_main();
    let marker = world.setup.scratch.join("failed-once");
    let wrapper = world.setup.scratch.join("failing-receive-pack");
    script(
        &wrapper,
        &format!(
            "'{git}' receive-pack \"$@\"\n\
             code=$?\n\
             if [ ! -e '{marker}' ]; then : > '{marker}'; exit 1; fi\n\
             exit $code",
            git = real_git().display(),
            marker = marker.display(),
        ),
    );
    world.run(
        world.root(),
        &["config", "remote.origin.receivepack", text(&wrapper)],
    );

    let (document, status) = world.synced(|| world.confirm(&first));
    let applied = document["applied_commit"].as_str().expect("applied_commit");
    assert!(marker.exists(), "the wrapper failed once");
    assert_state(&status, "retrying", "the push's exit 1");
    assert_eq!(world.remote_main(), applied, "it landed");
    assert_eq!(status["ahead"], json!(0), "git's count: {status}");
    world.assert_tidy("the push's exit 1");

    let (document, status) = world.synced(|| world.confirm(&second));
    let applied = document["applied_commit"].as_str().expect("applied_commit");
    assert_state(&status, "ok", "the next sync");
    assert_eq!(status["ahead"], json!(0), "{status}");
    assert_eq!(world.remote_main(), applied, "pushed");
    assert_eq!(
        world.run(
            &world.bare,
            &["rev-list", "--merges", &format!("{base}..{applied}")]
        ),
        "",
        "no merge commit"
    );
    world.assert_tidy("the next sync");
    assert_pins(&world.children(), "P8");
}

// --------------------------------------------------------------------- P9

/// P9: a `pre-receive` that sleeps 30 s. With `--push-deadline 3` the push is
/// killed at 3 s, `retrying`, the deadline named, long before the hook ends.
/// Then a hub with `--push-deadline 20`, whose start's push hangs the same way:
/// during the hang the status is a start's (`none`, `at` null), `GET .../tree`
/// answers within 1 s, a confirm commits and answers while the push still
/// hangs; then the deadline ends it. M: the push inside the turn; no deadline.
#[test]
fn p9_a_hanging_push_is_killed_at_its_deadline_and_blocks_no_read_or_confirm() {
    let mut world = World::press("git-hang");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    let hung = world.setup.scratch.join("hung.log");
    world.remote_hook(
        "pre-receive",
        &format!("echo hang >> '{}'\nsleep 30\nexit 1", hung.display()),
    );
    let hand = world.hand_commit("notes/hand.md", "Set by hand.\n");
    let base = world.remote_main();
    let hangs = || {
        fs::read_to_string(hung.as_path())
            .map(|log| log.lines().count())
            .unwrap_or(0)
    };

    let launched = Instant::now();
    let status = world.start_push();
    let took = launched.elapsed();
    assert_eq!(hangs(), 1, "the push reached the hook");
    assert_state(&status, "retrying", "a hang");
    let error = assert_error(&status, "", "a hang");
    assert!(error.contains(&deadline_line(3)), "{error:?}");
    assert!(
        (Duration::from_secs(3)..Duration::from_secs(25)).contains(&took),
        "killed at its deadline, not at the hook's end: {took:?}"
    );
    world.get(&format!("{}/tree", world.base)).status(200);
    world.stop();

    world.launch(&["--push", "--push-deadline", "20"]);
    let waited = Instant::now();
    while hangs() < 2 {
        assert!(
            waited.elapsed() < SYNC_WAIT,
            "the second hub's push never reached the hook: {}",
            world.server().stderr()
        );
        thread::sleep(POLL);
    }
    let during = world.status();
    assert_eq!(
        (&during["state"], &during["error"], &during["at"]),
        (&json!("none"), &Value::Null, &Value::Null),
        "a start's status while its sync runs: {during}"
    );
    let asked = Instant::now();
    let tree = world.get(&format!("{}/tree", world.base));
    let answered = asked.elapsed();
    assert_eq!(tree.status, 200, "{}", tree.text());
    assert!(
        answered < Duration::from_secs(1),
        "GET tree during the hang: {answered:?}"
    );
    let document = world.confirm(&id);
    assert_eq!(world.head(), document["applied_commit"].as_str().unwrap());
    assert_eq!(world.parents(world.root(), &world.head()), [hand]);
    let still = world.status();
    assert_eq!(
        still["at"],
        Value::Null,
        "the confirm answered while the push still hung: {still}"
    );
    let ended = world.after(&still, SYNC_WAIT);
    assert_state(&ended, "retrying", "the second hang");
    let error = assert_error(&ended, "", "the second hang");
    assert!(error.contains(&deadline_line(20)), "{error:?}");
    assert_eq!(world.remote_main(), base, "nothing landed");
    world.assert_tidy("a hang");
    assert_pins(&world.children(), "P9");
}

// -------------------------------------------------------------------- P10

/// P10: the remote's `main` replaced by an orphan history: `held`, naming it,
/// the remote unchanged, `HEAD` the applied commit. M:
/// `--allow-unrelated-histories`.
#[test]
fn p10_an_orphan_history_on_the_remote_is_held() {
    let mut world = World::press("git-orphan");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    assert_state(&world.start_push(), "ok", "the start");
    let orphan = {
        let git = world.git();
        git.run(&world.dev, &["checkout", "-q", "--orphan", "other"]);
        git.run(&world.dev, &["rm", "-rqf", "."]);
        write(&world.dev, "README.md", "Another project altogether.\n");
        commit_all(&git, &world.dev, "an orphan");
        git.run(
            &world.dev,
            &["push", "-q", "--force", "origin", "other:main"],
        );
        world.rev(&world.dev, "HEAD")
    };
    let (document, status) = world.synced(|| world.confirm(&id));
    assert_state(&status, "held", "unrelated histories");
    assert_error(&status, NO_HISTORY_LINE, "unrelated histories");
    assert_eq!(world.remote_main(), orphan, "the remote unchanged");
    assert_eq!(world.head(), document["applied_commit"].as_str().unwrap());
    world.assert_tidy("unrelated histories");
    assert_pins(&world.children(), "P10");
}

// -------------------------------------------------------------------- P11

/// P11: after a sync pushed `applied_commit`, the remote is force-pushed back to
/// its parent; the next sync makes `applied_commit` reachable from the remote
/// tip again, merging nothing. M: reset the branch to the remote.
#[test]
fn p11_a_remote_forced_back_gets_the_hubs_commits_again() {
    let mut world = World::press("git-forced-back");
    let first = world.raise("SHT-BACK", "the tide table", "the moon table");
    let second = world.raise(
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the season",
    );
    let base = world.remote_main();
    assert_state(&world.start_push(), "ok", "the start");
    let (document, status) = world.synced(|| world.confirm(&first));
    let applied = document["applied_commit"]
        .as_str()
        .expect("applied_commit")
        .to_owned();
    assert_state(&status, "ok", "pushed");
    assert_eq!(world.remote_main(), applied);
    assert_eq!(world.parents(world.root(), &applied), [base.as_str()]);
    world.run(
        &world.dev,
        &[
            "push",
            "-q",
            "--force",
            "origin",
            &format!("{base}:refs/heads/main"),
        ],
    );
    assert_eq!(world.remote_main(), base, "forced back");

    let (document, status) = world.synced(|| world.confirm(&second));
    let next = document["applied_commit"].as_str().expect("applied_commit");
    assert_state(&status, "ok", "pushed again");
    let tip = world.remote_main();
    assert_eq!(tip, next, "the hub's branch");
    assert!(
        world.reaches(&world.bare, &tip, &applied),
        "applied_commit is reachable again"
    );
    assert_eq!(
        world.parents(&world.bare, &tip),
        [applied.as_str()],
        "no merge"
    );
    world.assert_tidy("pushed again");
    assert_pins(&world.children(), "P11");
}

// -------------------------------------------------------------------- P12

/// P12: the checkout's `pre-push` exits 1: the remote unchanged, `retrying`,
/// the hook's words shown. M: `--no-verify`.
#[test]
fn p12_the_checkouts_pre_push_hook_runs_and_its_no_is_retried() {
    let mut world = World::press("git-pre-push");
    world.checkout_hook("pre-push", "echo 'the pre-push hook says no' >&2\nexit 1");
    world.hand_commit("notes/hand.md", "Set by hand.\n");
    let base = world.remote_main();
    let status = world.start_push();
    assert_state(&status, "retrying", "pre-push");
    let error = assert_error(&status, PUSH_FAILED_LINE, "pre-push");
    assert!(
        error.contains("the pre-push hook says no"),
        "the hook's words: {error:?}"
    );
    assert_eq!(world.remote_main(), base, "the remote unchanged");
    world.assert_tidy("pre-push");
    assert_pins(&world.children(), "P12");
}

// -------------------------------------------------------------------- P13

/// P13: a modified tracked file and an untracked one: `stopped`, both named,
/// one stderr line; both keep their bytes and mtime, `HEAD` and the remote
/// theirs. Beyond: once both are gone, a confirm's signal looks again and the
/// sync pushes. M: `stash`, `reset --hard` or `clean`.
#[test]
fn p13_a_dirty_checkout_stops_the_sync_and_keeps_every_byte() {
    let mut world = World::press("git-dirty");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    let hand = world.hand_commit("notes/hand.md", "Set by hand.\n");
    let base = world.remote_main();
    let original = fs::read(world.root().join(INNER_MD)).expect(INNER_MD);
    let mut edited = original.clone();
    edited.extend_from_slice(b"\nAn edit in progress.\n");
    write(world.root(), INNER_MD, &edited);
    write(world.root(), "scratch-notes.txt", "Not for git.\n");
    aged(world.root(), INNER_MD);
    aged(world.root(), "scratch-notes.txt");
    let kept = [
        stamp(world.root(), INNER_MD),
        stamp(world.root(), "scratch-notes.txt"),
    ];

    let status = world.start_push();
    assert_state(&status, "stopped", "a dirty checkout");
    let error = assert_error(&status, &dirty_line(2, world.root()), "a dirty checkout");
    assert!(
        error.contains(INNER_MD) && error.contains("scratch-notes.txt"),
        "both named: {error:?}"
    );
    assert_eq!(
        [
            stamp(world.root(), INNER_MD),
            stamp(world.root(), "scratch-notes.txt")
        ],
        kept,
        "bytes and mtimes"
    );
    assert_eq!(world.head(), hand, "HEAD unchanged");
    assert_eq!(world.remote_main(), base, "the remote unchanged");
    assert_eq!(
        world.sync_lines(),
        [stderr_line(world.root(), "stopped", Some(error.as_str()))],
        "one line"
    );

    write(world.root(), INNER_MD, &original);
    fs::remove_file(world.root().join("scratch-notes.txt")).expect("removed");
    let (document, status) = world.synced(|| world.confirm(&id));
    assert_state(&status, "ok", "looked again on the signal");
    assert_eq!(
        world.remote_main(),
        document["applied_commit"].as_str().unwrap()
    );
    world.assert_tidy("looked again");
    assert_pins(&world.children(), "P13");
}

// -------------------------------------------------------------------- P14

/// P14: with logging `remote.origin.uploadpack` and `receivepack` wrappers and a
/// commit to push, neither runs for a local daemon, a hub without `--push` (its
/// confirm included), `spec serve`, nor a terminal's approve and reject (the
/// library a terminal runs, consent yes); no git child of theirs fetches or
/// pushes, and each holds `GIT_NO_LAZY_FETCH=1` (P15). The wrappers do run for
/// a fetch, which the test makes last. `--push` without `--public-origin` exits
/// 2 with "Data"'s line and creates nothing. M: a sync on every daemon; a push
/// from `apply.rs`.
#[test]
fn p14_no_daemon_without_push_and_no_terminal_reaches_the_remote() {
    let mut world = World::press("git-offline");
    let on_hub = world.raise("SHT-BACK", "the tide table", "the moon table");
    let approved = world.raise(
        "SHT-FRONT-HEAD",
        "stays the same",
        "changes with the season",
    );
    let rejected = world.raise("SHT-FRONT", "the lead story", "the weather");
    world.hand_commit("notes/hand.md", "A commit to push.\n");
    let base = world.remote_main();
    let net = world.setup.scratch.join("network.log");
    for (key, program) in [
        ("remote.origin.uploadpack", "upload-pack"),
        ("remote.origin.receivepack", "receive-pack"),
    ] {
        let wrapper = world.setup.scratch.join(&format!("logging-{program}"));
        script(
            &wrapper,
            &format!(
                "echo {program} >> '{}'\nexec '{}' {program} \"$@\"",
                net.display(),
                real_git().display()
            ),
        );
        world.run(world.root(), &["config", key, text(&wrapper)]);
    }
    let root = world.root().to_path_buf();
    let cwd = world.setup.cwd.clone();
    let sync_route = format!("{PRESS}/sync");

    let local =
        Server::try_start_env(&world.env(), &cwd, &root_args(&[&root])).expect("a local daemon");
    local.get(&format!("{PRESS}/tree")).status(200);
    local.get(&sync_route).status(404);
    thread::sleep(QUIET_WAIT);
    drop(local);

    world.launch(&[]);
    world.confirm(&on_hub);
    world.get(&sync_route).status(404);
    thread::sleep(QUIET_WAIT);
    world.stop();

    let os: Vec<&OsStr> = ["--root", text(&root), "serve", "--port", "0"]
        .into_iter()
        .map(OsStr::new)
        .collect();
    let served =
        Server::try_start_program(&spec_bin(), &world.env(), &cwd, &os).expect("spec serve");
    assert!(
        served
            .lines
            .iter()
            .any(|line| line.starts_with("listening ")),
        "{:?}",
        served.lines
    );
    served.get(&format!("{PRESS}/tree")).status(200);
    thread::sleep(QUIET_WAIT);
    drop(served);

    let library = (
        Env {
            cwd: root.clone(),
            home: Some(world.setup.home.as_os_str().to_os_string()),
            xdg_data_home: None,
        },
        Globals {
            root: Some(root.clone()),
            config: None,
        },
    );
    let terminal = GitEnv::new(root.as_path(), world.env());
    let before = world.head();
    let outcome = approve(
        &library.0,
        &library.1,
        &ApproveRequest {
            id: approved.clone(),
            note: None,
            now: utc_now(),
            git: terminal.clone(),
        },
        &mut |_: &str| true,
    )
    .unwrap_or_else(|error| panic!("approve {approved}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_ne!(world.head(), before, "the terminal's apply committed");
    let outcome = reject(
        &library.0,
        &library.1,
        &RejectRequest {
            id: rejected.clone(),
            reason: Some("Not this week.".to_owned()),
            now: utc_now(),
            git: terminal,
        },
        &mut |_: &str| true,
    )
    .unwrap_or_else(|error| panic!("reject {rejected}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    thread::sleep(QUIET_WAIT);

    assert!(
        !net.exists(),
        "a wrapper ran: {:?}",
        fs::read_to_string(&net)
    );
    assert_eq!(world.remote_main(), base, "the remote unchanged");
    let children = world.children();
    for child in &children {
        assert!(
            !matches!(
                child.subcommand(),
                Some("fetch" | "push" | "pull" | "ls-remote")
            ),
            "D1: {child}"
        );
    }
    assert_pins(&children, "P14");

    world.run(&root, &["fetch", "-q", "origin"]);
    assert_eq!(
        fs::read_to_string(net.as_path()).expect("the control's log"),
        "upload-pack\n",
        "the control: a fetch runs the wrapper"
    );

    let home = world.setup.scratch.home("never");
    let mut args = root_args(&[&root]);
    args.push(OsStr::new("--push"));
    let refused = Server::try_start(&home, &cwd, &args)
        .err()
        .expect("--push without --public-origin started");
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(
        refused.stderr,
        "specengine-http: --push needs --public-origin: only a hub pushes\n"
    );
    assert!(refused.stdout.is_empty(), "{refused:?}");
    assert!(
        snapshot(&home).is_empty(),
        "no data directory: {:?}",
        snapshot(&home)
    );
}

// -------------------------------------------------------------------- P15

/// Whether a raw string (`r"`, `r#"`, `br"`, `cr"` …) opens at `at`: its
/// number of `#`.
fn raw_string_at(bytes: &[u8], at: usize) -> Option<usize> {
    let ident = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let prefix_ok = at == 0
        || !ident(bytes[at - 1])
        || (matches!(bytes[at - 1], b'b' | b'c') && (at < 2 || !ident(bytes[at - 2])));
    if bytes[at] != b'r' || !prefix_ok {
        return None;
    }
    let hashes = bytes[at + 1..]
        .iter()
        .take_while(|byte| **byte == b'#')
        .count();
    (bytes.get(at + 1 + hashes) == Some(&b'"')).then_some(hashes)
}

/// The string literals of Rust `source`, comments not read: `"…"`, `b"…"`,
/// `r#"…"#` and their kin, their text as written (escapes kept).
fn literals(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let rest = &bytes[at..];
        if rest.starts_with(b"//") {
            at += rest
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap_or(rest.len());
        } else if rest.starts_with(b"/*") {
            let mut depth = 0;
            while at < bytes.len() {
                if bytes[at..].starts_with(b"/*") {
                    depth += 1;
                    at += 2;
                } else if bytes[at..].starts_with(b"*/") {
                    depth -= 1;
                    at += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    at += 1;
                }
            }
        } else if let Some(hashes) = raw_string_at(bytes, at) {
            let start = at + 2 + hashes;
            let mut close = vec![b'"'];
            close.extend(std::iter::repeat_n(b'#', hashes));
            let end = bytes[start..]
                .windows(close.len())
                .position(|window| window == close.as_slice())
                .map(|offset| start + offset)
                .expect("a closed raw string");
            out.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
            at = end + close.len();
        } else if bytes[at] == b'"' {
            let start = at + 1;
            let mut end = start;
            while bytes[end] != b'"' {
                end += if bytes[end] == b'\\' { 2 } else { 1 };
            }
            out.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
            at = end + 1;
        } else if bytes[at] == b'\'' {
            if rest.get(1) == Some(&b'\\') {
                let close = rest[3..]
                    .iter()
                    .position(|byte| *byte == b'\'')
                    .expect("a closed char");
                at += 4 + close;
            } else {
                let width = source[at + 1..].chars().next().map_or(1, char::len_utf8);
                if rest.get(1 + width) == Some(&b'\'') {
                    at += 2 + width;
                } else {
                    at += 1;
                }
            }
        } else {
            at += 1;
        }
    }
    out
}

/// P15, the source: across every `crates/*/src/**/*.rs`, the literals `"fetch"`
/// and `"push"` stand in store `sync.rs` alone, which holds both; no literal
/// holds `GIT_SSH_COMMAND`, `core.sshCommand`, `credential.` or `/run/secrets`
/// (git's keys in any case). The reader's own probe first. M: add
/// `GIT_SSH_COMMAND`; a push from `apply.rs`.
#[test]
fn p15_no_source_but_store_sync_names_fetch_or_push_and_none_names_a_credential() {
    let probe = "// \"push\"\n/* \"fetch\" /* \"push\" */ */\nlet a = 'a'; let q = '\"';\n\
                 fn f<'a>(x: &'a str) {}\nlet s = \"fetch\"; let r = r#\"push \"x\"\"#;\n\
                 let b = b\"push\"; let e = \"a\\\"b\"; let w = br\"credential.helper\";\n";
    assert_eq!(
        literals(probe),
        ["fetch", "push \"x\"", "push", "a\\\"b", "credential.helper"]
    );
    let crates = repository_root().join("crates");
    let sync = Path::new("specengine-store/src/sync.rs");
    let mut seen_in_sync = Vec::new();
    for entry in fs::read_dir(crates).expect("crates/").flatten() {
        let src = entry.path().join("src");
        for file in snapshot(&src)
            .into_iter()
            .filter(|(path, bytes)| bytes.is_some() && path.ends_with(".rs"))
            .map(|(path, _)| path)
        {
            let relative = Path::new(&entry.file_name()).join("src").join(&file);
            let source = read_text(&src, &file);
            for literal in literals(&source) {
                if literal == "fetch" || literal == "push" {
                    assert_eq!(
                        relative,
                        sync,
                        "D1: {literal:?} spawned from {} (only store sync.rs)",
                        relative.display()
                    );
                    seen_in_sync.push(literal.clone());
                }
                let lower = literal.to_ascii_lowercase();
                for named in [
                    "git_ssh_command",
                    "core.sshcommand",
                    "credential.",
                    "/run/secrets",
                ] {
                    assert!(
                        !lower.contains(named),
                        "D6: {} names {literal:?}",
                        relative.display()
                    );
                }
            }
        }
    }
    for verb in ["fetch", "push"] {
        assert!(
            seen_in_sync.iter().any(|literal| literal == verb),
            "store sync.rs spawns {verb}: {seen_in_sync:?}"
        );
    }
}

// -------------------------------------------------------------------- P16

/// The dependency names of a `Cargo.toml`: every key of a `[*dependencies*]`
/// table, every `[*dependencies.<name>]` table, every `package = "<name>"`.
fn dependencies(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            let header = line.trim_matches(['[', ']']);
            inside = header.ends_with("dependencies");
            if let Some((_, name)) = header.rsplit_once("dependencies.") {
                names.push(name.trim_matches('"').to_owned());
            }
            continue;
        }
        if let Some((_, rest)) = line.split_once("package") {
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix('=') {
                let value = value.trim().trim_start_matches('"');
                names.push(value.split('"').next().unwrap_or_default().to_owned());
            }
        }
        if inside && !line.is_empty() && !line.starts_with('#') {
            let name = line.split(['=', '.']).next().unwrap_or_default().trim();
            names.push(name.trim_matches('"').to_owned());
        }
    }
    names
}

/// P16: neither the root manifest nor any `crates/*/Cargo.toml` names `git2`,
/// `gix`, `reqwest`, `ureq`, `ssh2` or `russh`, read as `tests/hub_confirm.rs`'s
/// B9 reads one, plus a table per dependency and a renamed `package`. M: add
/// `gix`.
#[test]
fn p16_no_manifest_names_a_git_or_network_crate() {
    let probe = "[dependencies]\n# gix\nserde.workspace = true\nfoo = { package = \"git2\" }\n\
                 [dependencies.ureq]\nversion = \"2\"\n[target.'cfg(unix)'.dependencies]\n\
                 russh = \"0.4\"\n[features]\ngix = []\n";
    let names = dependencies(probe);
    for name in ["serde", "foo", "git2", "ureq", "russh"] {
        assert!(names.iter().any(|known| known == name), "{name}: {names:?}");
    }
    assert!(!names.iter().any(|known| known == "gix"), "{names:?}");
    let root = repository_root();
    let mut manifests = vec![root.join("Cargo.toml")];
    for entry in fs::read_dir(root.join("crates"))
        .expect("crates/")
        .flatten()
    {
        let manifest = entry.path().join("Cargo.toml");
        if manifest.is_file() {
            manifests.push(manifest);
        }
    }
    assert!(manifests.len() > 5, "{manifests:?}");
    for manifest in manifests {
        let text = fs::read_to_string(&manifest).expect("a manifest");
        for name in dependencies(&text) {
            assert!(
                !["git2", "gix", "reqwest", "ureq", "ssh2", "russh"].contains(&name.as_str()),
                "{} names {name}",
                manifest.display()
            );
        }
    }
}

// -------------------------------------------------------------------- P17

/// P17: `origin` with a password, with a token as the user name, and with a
/// clean name that `url.<base>.insteadOf` turns into the first: each start's
/// sync is `stopped`, no fetch or push child runs, and neither `s3cret` nor
/// `tok` shows in a body, the daemon's stderr or the children's argv.
/// `ssh://git@example.invalid/r.git` passes the look: its fetch runs and fails
/// in git (`GIT_ALLOW_PROTOCOL=file`), `retrying`, no URL shown. M: no check; a
/// check of `remote.origin.url` alone.
#[test]
fn p17_a_url_holding_a_secret_stops_before_any_fetch() {
    let mut world = World::press("git-urls");
    world.hand_commit("notes/hand.md", "A commit to push.\n");
    let root = world.root().to_path_buf();
    let bare = text(&world.bare).to_owned();
    let rewrite = "url.https://u:s3cret@example.invalid/.insteadOf";
    let cases: [(&str, &str, Option<&str>); 3] = [
        ("a password", PASSWORD_URL, None),
        ("a token as the user name", TOKEN_URL, None),
        (
            "a clean name insteadOf rewrites",
            "forge:r.git",
            Some("forge:"),
        ),
    ];
    for (label, url, instead_of) in cases {
        let _ = fs::remove_file(&world.log);
        world.run(&root, &["remote", "set-url", "origin", url]);
        if let Some(prefix) = instead_of {
            world.run(&root, &["config", rewrite, prefix]);
        }
        let status = world.start_push();
        assert_state(&status, "stopped", label);
        assert_error(&status, SECRET_LINE, label);
        assert!(
            world.children_running("fetch").is_empty() && world.children_running("push").is_empty(),
            "{label}: no network child: {:?}",
            world.children()
        );
        let bodies = [
            world.get(&format!("{PRESS}/sync")).text().to_owned(),
            world.get("/api/projects").text().to_owned(),
            world.get(&format!("{PRESS}/tree")).text().to_owned(),
        ];
        let log = fs::read_to_string(&world.log).unwrap_or_default();
        let stderr = world.server().stderr();
        for secret in SECRETS {
            for (place, seen) in [
                ("a body", bodies.join("\n")),
                ("stderr", stderr.clone()),
                ("the argv log", log.clone()),
            ] {
                assert!(!holds_word(&seen, secret), "{label}: {secret:?} in {place}");
            }
        }
        world.stop();
        if instead_of.is_some() {
            world.run(&root, &["config", "--unset", rewrite]);
        }
        world.run(&root, &["remote", "set-url", "origin", &bare]);
        world.assert_tidy(label);
    }

    let _ = fs::remove_file(&world.log);
    world.run(
        &root,
        &[
            "remote",
            "set-url",
            "origin",
            "ssh://git@example.invalid/r.git",
        ],
    );
    let status = world.start_push();
    assert_state(&status, "retrying", "an ssh user name");
    let error = assert_error(&status, FETCH_FAILED_LINE, "an ssh user name");
    assert!(!error.contains("example.invalid"), "no URL: {error:?}");
    assert_eq!(world.children_running("fetch").len(), 1, "the fetch ran");
    world.stop();
    world.run(&root, &["remote", "set-url", "origin", &bare]);
    world.assert_tidy("an ssh user name");
    assert_pins(&world.children(), "P17");
}

/// "Rules and edge cases", D6, beyond P17: a password after U+00E9 just before
/// `://` (P24: judged with no panic), a push URL holding a password (`git
/// remote get-url --push --all`), a user name on `ftps`, a password on `ssh`:
/// `stopped`, no network child; an scp-like `git@host:path` passes.
#[test]
fn a_push_url_and_the_other_schemes_of_the_url_rule() {
    let mut world = World::press("git-urls-more");
    let root = world.root().to_path_buf();
    let bare = text(&world.bare).to_owned();
    let cases: [(&str, &str, &str); 4] = [
        (
            "a password after U+00E9 just before `://` (P24)",
            "remote.origin.url",
            "x\u{e9}://u:p@example.invalid/q.git",
        ),
        (
            "a push URL with a password",
            "remote.origin.pushurl",
            PASSWORD_URL,
        ),
        (
            "a user name on ftps",
            "remote.origin.url",
            "ftps://u@example.invalid/r.git",
        ),
        (
            "a password on ssh",
            "remote.origin.url",
            "ssh://git:s3cret@example.invalid/r.git",
        ),
    ];
    for (label, key, url) in cases {
        let _ = fs::remove_file(&world.log);
        world.run(&root, &["config", key, url]);
        let status = world.start_push();
        assert_state(&status, "stopped", label);
        assert_error(&status, SECRET_LINE, label);
        assert!(
            world.children_running("fetch").is_empty() && world.children_running("push").is_empty(),
            "{label}: {:?}",
            world.children()
        );
        world.stop();
        if key == "remote.origin.pushurl" {
            world.run(&root, &["config", "--unset", key]);
        }
        world.run(&root, &["remote", "set-url", "origin", &bare]);
    }
    let _ = fs::remove_file(&world.log);
    world.run(
        &root,
        &["remote", "set-url", "origin", "git@example.invalid:r.git"],
    );
    let status = world.start_push();
    assert_state(&status, "retrying", "scp-like");
    assert_error(&status, FETCH_FAILED_LINE, "scp-like");
    world.stop();
}

// -------------------------------------------------------------------- P18

/// P18: on a hub with `--push`, `GET .../sync` answers 200 with exactly
/// "Data"'s five keys in order, to a login outside `[team]` too, `at` RFC 3339
/// UTC to the second, and the start line `push every 5 min` stands between
/// `public` and `listening`. A local daemon and a hub without `--push` answer
/// it with their unknown-route 404, byte for byte. M: mounted locally; mounted
/// on every hub.
#[test]
fn p18_the_status_route_is_a_push_hubs_alone() {
    let mut world = World::press("git-route");
    let status = world.start_push();
    assert_eq!(
        world.server().lines,
        [
            format!("serving card-press {}", world.root().display()),
            format!("public {ORIGIN}"),
            "push every 5 min".to_owned(),
            format!("listening http://127.0.0.1:{}", world.server().port),
        ]
    );
    assert_state(&status, "ok", "in step");
    assert_eq!(
        (&status["ahead"], &status["behind"], &status["error"]),
        (&json!(0), &json!(0), &Value::Null)
    );
    let at = status["at"].as_str().expect("at");
    assert!(
        at.len() == 20
            && at.ends_with('Z')
            && at.as_bytes()[10] == b'T'
            && at.chars().filter(char::is_ascii_digit).count() == 14,
        "RFC 3339 UTC: {at}"
    );
    let outside = through(
        world.server(),
        "GET",
        &format!("{PRESS}/sync"),
        &Proxied::fetch("eve"),
        b"",
    );
    assert_eq!(outside.status, 200, "{}", outside.text());
    assert_eq!(keys_of(outside.text()), KEYS);
    world.stop();

    // The unknown-route 404 names the path it was asked: the other route's
    // answer, its path read as `/sync`.
    let unknown = |reply: Reply, other: Reply, context: &str| {
        assert_eq!(reply.status, 404, "{context}: {}", reply.text());
        let other_text = other.text().replace("/no-such-route", "/sync");
        assert_eq!(
            (reply.status, reply.text()),
            (other.status, other_text.as_str()),
            "{context}: the unknown-route 404"
        );
    };
    world.launch(&[]);
    unknown(
        world.get(&format!("{PRESS}/sync")),
        world.get(&format!("{PRESS}/no-such-route")),
        "a hub without --push",
    );
    world.stop();
    let local = Server::try_start_env(&world.env(), &world.setup.cwd, &root_args(&[world.root()]))
        .expect("a local daemon");
    unknown(
        local.get(&format!("{PRESS}/sync")),
        local.get(&format!("{PRESS}/no-such-route")),
        "a local daemon",
    );
}

/// "Data", **Flags**: judged with `--public-origin` before anything is created,
/// each refusal exit 2 on stderr naming the flag, a fresh `HOME` left empty;
/// the ranges' ends start and print their start line.
#[test]
fn the_push_flags_are_judged_before_anything_is_created() {
    let world = World::press("git-flags");
    let root = world.root().to_path_buf();
    let cwd = world.setup.cwd.clone();
    let refusals: [(&[&str], &str); 11] = [
        (&["--push-every", "5"], "--push-every"),
        (&["--push-deadline", "3"], "--push-deadline"),
        (&["--push", "--push-every", "0"], "--push-every"),
        (&["--push", "--push-every", "1441"], "--push-every"),
        (&["--push", "--push-every", "x"], "--push-every"),
        (&["--push", "--push-every", "-1"], "--push-every"),
        (&["--push", "--push-deadline", "0"], "--push-deadline"),
        (&["--push", "--push-deadline", "3601"], "--push-deadline"),
        (&["--push", "--push-deadline", "-1"], "--push-deadline"),
        (&["--push", "--push-deadline", "1.5"], "--push-deadline"),
        (&["--push", "--push-deadline", ""], "--push-deadline"),
    ];
    let mut wrong = Vec::new();
    for (index, (flags, named)) in refusals.iter().enumerate() {
        let home = world.setup.scratch.home(&format!("refused-{index}"));
        let mut args = hub_args(&[&root], ORIGIN);
        args.extend(flags.iter().map(OsStr::new));
        let refused = Server::try_start(&home, &cwd, &args)
            .err()
            .unwrap_or_else(|| panic!("{flags:?} started a hub"));
        let named_right = refused.code == Some(2)
            && refused.stdout.is_empty()
            && refused.stderr.starts_with("specengine-http: ")
            && refused.stderr.contains(named);
        if !named_right || !snapshot(&home).is_empty() {
            wrong.push(format!(
                "{flags:?}: {refused:?}, HOME {:?}",
                snapshot(&home)
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "exit 2 naming the flag, nothing created:\n{}",
        wrong.join("\n")
    );
    for (flags, line) in [
        (
            ["--push-every", "1440", "--push-deadline", "3600"],
            "push every 1440 min",
        ),
        (
            ["--push-every", "1", "--push-deadline", "1"],
            "push every 1 min",
        ),
    ] {
        let home = world.setup.scratch.home(&format!("accepted-{}", flags[1]));
        let mut args = hub_args(&[&root], ORIGIN);
        args.push(OsStr::new("--push"));
        args.extend(flags.iter().map(OsStr::new));
        let server = Server::try_start(&home, &cwd, &args)
            .unwrap_or_else(|refused| panic!("{flags:?}: {refused:?}"));
        assert_eq!(
            server.lines[server.lines.len() - 2..],
            [
                line.to_owned(),
                format!("listening http://127.0.0.1:{}", server.port)
            ],
            "{flags:?}"
        );
        assert_eq!(
            server.lines[server.lines.len() - 3],
            format!("public {ORIGIN}")
        );
    }
}

// ----------------------------------------------------------- three races

/// "Data", beyond P6: a `receivepack` wrapper that lands a new developer's
/// commit before every push: the sync fetches, merges and pushes again at most
/// three rounds, then is `retrying` with `` `origin/main` moved during three
/// rounds: trying again later ``; the developer's commits all stand on the
/// remote, no push carries `--force*`, the checkout is clean.
#[test]
fn three_races_in_a_row_are_retrying_and_force_nothing() {
    let mut world = World::press("git-three-races");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    assert_state(&world.start_push(), "ok", "the start");
    let races = world.setup.scratch.join("races.log");
    let wrapper = world.setup.scratch.join("always-racing-receive-pack");
    script(
        &wrapper,
        &format!(
            "echo race >> '{races}'\n\
             ( unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
             GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_QUARANTINE_PATH GIT_PROTOCOL\n\
             '{git}' -C '{dev}' commit -q --allow-empty -m 'a racing commit' && \
             '{git}' -C '{dev}' push -q origin main ) >&2 || exit 3\n\
             exec '{git}' receive-pack \"$@\"",
            races = races.display(),
            git = real_git().display(),
            dev = world.dev.display(),
        ),
    );
    world.run(
        world.root(),
        &["config", "remote.origin.receivepack", text(&wrapper)],
    );

    let (document, status) = world.synced(|| world.confirm(&id));
    let applied = document["applied_commit"].as_str().expect("applied_commit");
    assert_state(&status, "retrying", "three races");
    assert_error(
        &status,
        "`origin/main` moved during three rounds: trying again later",
        "three races",
    );
    let rounds = fs::read_to_string(races.as_path()).expect("the wrapper ran");
    // "at most three rounds": three pushes, or the first and three more; the
    // spec's words allow either.
    assert!(
        (3..=4).contains(&rounds.lines().count()),
        "three rounds: {rounds:?}"
    );
    let tip = world.remote_main();
    assert_eq!(tip, world.rev(&world.dev, "HEAD"), "the developer's last");
    assert!(
        !world.reaches(&world.bare, &tip, applied),
        "the hub's commit never forced in"
    );
    assert!(
        world.reaches(world.root(), &world.head(), applied),
        "HEAD keeps it"
    );
    world.assert_tidy("three races");
    assert_pins(&world.children(), "three races");
}

// ------------------------------------------------------------- the look

/// "Data"'s look, beyond the criteria, each on a checkout one commit ahead,
/// each a start's sync `stopped` with nothing touched: a branch with no
/// upstream (and then no tracking ref: `ahead` and `behind` null), a detached
/// `HEAD`, a `bisect` in progress, a shallow clone, a partial clone.
#[test]
fn the_looks_other_stops_touch_nothing() {
    let mut world = World::press("git-look");
    let root = world.root().to_path_buf();
    let hand = world.hand_commit("notes/hand.md", "A commit to push.\n");
    let base = world.remote_main();

    world.run(&root, &["branch", "--unset-upstream"]);
    let status = world.start_push();
    assert_state(&status, "stopped", "no upstream");
    assert_error(&status, UPSTREAM_LINE, "no upstream");
    assert_eq!(
        (&status["ahead"], &status["behind"]),
        (&Value::Null, &Value::Null),
        "no tracking ref: {status}"
    );
    world.stop();
    world.run(&root, &["branch", "--set-upstream-to=origin/main", "main"]);

    world.run(&root, &["switch", "-q", "--detach", "HEAD"]);
    let status = world.start_push();
    assert_state(&status, "stopped", "a detached HEAD");
    world.stop();
    world.run(&root, &["switch", "-q", "main"]);

    world.run(&root, &["bisect", "start"]);
    let status = world.start_push();
    assert_state(&status, "stopped", "a bisect");
    assert_error(&status, &operation_line("bisect", &root), "a bisect");
    world.stop();
    world.run(&root, &["bisect", "reset"]);
    assert_eq!(world.head(), hand);
    assert_eq!(world.remote_main(), base, "the remote unchanged");

    world.run(&world.bare, &["config", "uploadpack.allowFilter", "true"]);
    for (label, flag, line) in [
        ("a shallow clone", "--depth=1", SHALLOW_LINE),
        ("a partial clone", "--filter=blob:none", PARTIAL_LINE),
    ] {
        let clone = world
            .setup
            .scratch
            .join(&format!("clone{flag}").replace(['=', ':'], "-"));
        world.run(
            world.setup.scratch.path(),
            &[
                "clone",
                "-q",
                "--template=",
                flag,
                &format!("file://{}", world.bare.display()),
                text(&clone),
            ],
        );
        let clone = fs::canonicalize(clone).expect("the clone");
        world.git().quiet(&clone);
        world.setup.root = clone;
        let status = world.start_push();
        assert_state(&status, "stopped", label);
        assert_error(&status, line, label);
        world.stop();
    }
    assert_eq!(world.remote_main(), base, "the remote unchanged");
}

// -------------------------------------------------------------------- P21

/// P21's synthetic config ("Data"): roots beyond `docs/` in this order, the
/// records outside them, the index writing `docs/index.md`; `roots` as given.
fn synthetic(roots: &str) -> impl FnOnce(&Path) {
    let roots = roots.to_owned();
    move |root: &Path| {
        replace(
            root,
            "specengine.toml",
            "roots = [\"README.md\", \"docs\", \"design\"]",
            &roots,
        );
        replace(
            root,
            "specengine.toml",
            "dir      = \"docs/records/DEC\"",
            "dir      = \"records\"",
        );
        write(root, "records/README.md", "The decision records.\n");
    }
}

/// The hub's checkout made sparse (cone `docs`, `records`, `templates`): no
/// `design/`.
fn sparse(world: &World) {
    world.run(
        world.root(),
        &[
            "sparse-checkout",
            "set",
            "--cone",
            "docs",
            "records",
            "templates",
        ],
    );
    assert!(!world.root().join("design").exists(), "design/ is out");
}

/// P21, on a hub: the synthetic config lists `design`, the sparse checkout
/// lacks it: the start's sync is `stopped`, naming it, nothing pushed. M:
/// `docs/` hardcoded.
#[test]
fn p21_a_sparse_checkout_lacking_a_root_stops_the_sync() {
    let mut world = World::forge_with(
        "git-sparse",
        synthetic("roots = [\"docs\", \"design\", \"README.md\"]"),
    );
    sparse(&world);
    world.hand_commit("notes.txt", "A commit to push.\n");
    let base = world.remote_main();
    let status = world.start_push();
    assert_state(&status, "stopped", "a gap");
    assert_error(&status, SPARSE_LINE, "a gap");
    assert_eq!(world.remote_main(), base, "nothing pushed");
    assert_pins(&world.children(), "P21");
}

/// P21, on a hub, "also when a merge brings it": the synthetic config without
/// `design` at first, the checkout sparse; a developer's commit adds the root
/// (its index rendered again) before the hub starts. The start's sync
/// fast-forwards to it, `ok`; the timer's next sync (`--push-every 1`) reads the
/// merged config and is `stopped`, naming `design/`. M: `docs/` hardcoded; the
/// set computed once.
#[test]
fn p21_a_merge_that_brings_a_root_outside_the_checkout_stops_the_next_sync() {
    let mut world = World::forge_with(
        "git-sparse-merged",
        synthetic("roots = [\"docs\", \"README.md\"]"),
    );
    sparse(&world);
    let theirs = {
        let git = world.git();
        replace(
            &world.dev,
            "specengine.toml",
            "roots = [\"docs\", \"README.md\"]",
            "roots = [\"docs\", \"design\", \"README.md\"]",
        );
        spec(&world.dev_home, &world.dev, &["export", "index"]).code(0);
        commit_all(&git, &world.dev, "the sketches are a root");
        git.run(&world.dev, &["push", "-q", "origin", "main"]);
        world.rev(&world.dev, "HEAD")
    };
    let started = world.start(&["--push", "--push-deadline", "3", "--push-every", "1"]);
    assert_state(&started, "ok", "the fast-forward");
    assert_eq!(world.head(), theirs, "merged");
    let status = world.after(&started, TICK_WAIT);
    assert_state(&status, "stopped", "the merged config's gap");
    assert_error(&status, SPARSE_LINE, "the merged config's gap");
    assert_eq!(world.remote_main(), theirs);
    world.assert_tidy("the merged config's gap");
    assert_pins(&world.children(), "P21");
}

// -------------------------------------------------------------------- P22

/// P22, the harness's own: under [`product_env`] (and under the hubs' `PATH`
/// wrapper here) `git fetch ssh://example.invalid/x` and `git ls-remote
/// https://example.invalid/x` fail with git's transport refusal, never a
/// resolver's error: no test's mistaken URL reaches the network. M: remove
/// `GIT_ALLOW_PROTOCOL`.
#[test]
fn p22_a_mistaken_network_url_fails_in_git_before_any_connection() {
    let scratch = Scratch::new("git-protocol");
    let home = scratch.home("h");
    let repo = scratch.dir("repo");
    scratch.git().run(
        &repo,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    for (args, transport) in [
        (["fetch", "ssh://example.invalid/x"], "ssh"),
        (["ls-remote", "https://example.invalid/x"], "https"),
    ] {
        let os: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        let output = alarmed(&real_git(), &os, &product_env(&home))
            .current_dir(repo.as_path())
            .stdin(Stdio::null())
            .output()
            .expect("git runs");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{args:?}: {stderr}");
        assert!(
            stderr.contains(&format!("transport '{transport}' not allowed")),
            "{args:?}: {stderr}"
        );
        for resolver in ["resolve", "Could not", "nodename", "unable to access"] {
            assert!(!stderr.contains(resolver), "{args:?}: {stderr}");
        }
    }
}

// -------------------------------------------------------------------- P23

/// What P23 compares: the CLI's and the daemon's reads, as bytes.
#[derive(Debug, PartialEq)]
struct Reads {
    check: String,
    health: String,
    gate: (i32, String),
    tree: Vec<u8>,
    inbox: Vec<u8>,
    daemon_check: Vec<u8>,
}

/// The print shop's write gate on the back sheet, for `spec gate`'s fixed
/// payload.
const GATE: &str = "\n[zones]\nback = [\"docs/spec/sht-back.md\"]\n\n[gate]\nmode = \"selective\"\nzones = [\"back\"]\n";

fn reads(world: &World) -> Reads {
    let setup = &world.setup;
    let check = spec_json(&setup.home, &setup.cwd, &setup.root, &["check"]);
    let health = spec_json(&setup.home, &setup.cwd, &setup.root, &["health"]);
    let gate = run_product(
        &spec_bin(),
        &setup.home,
        &setup.cwd,
        &[
            "gate",
            "--worktree",
            text(&setup.root),
            "--path",
            BACK_MD,
            "--json",
        ],
        b"",
    );
    let page = |path: &str| {
        let reply = world.get(&format!("{PRESS}/{path}"));
        assert_eq!(reply.status, 200, "{path}: {}", reply.text());
        reply.body
    };
    Reads {
        check: check.stdout,
        health: health.stdout,
        gate: (gate.code, gate.stdout),
        tree: page("tree"),
        inbox: page("inbox"),
        daemon_check: page("check"),
    }
}

/// P23: `spec check --json`, `spec health --json`, `spec gate` on a fixed
/// payload, `GET .../tree`, `.../inbox` and `.../check`, each read once a hub's
/// start's sync has ended, byte for byte the same in step (`ok`), with the hub
/// two commits ahead (`held`: a `pre-receive` refuses) and with the remote
/// moved away to an orphan commit (`held`), the checkout and the queue the
/// same throughout. M: a note or row derived from push state.
#[test]
fn p23_no_read_derives_anything_from_push_state() {
    let mut world = World::press_with("git-reads", |root| append(root, GATE));
    world.raise("SHT-BACK", "the tide table", "the moon table");
    let root = world.root().to_path_buf();
    world.hand_commit("notes/one.md", "One.\n");
    let mut inner = read_text(&root, INNER_MD);
    inner.push_str("\nThe fold carries the weather.\n");
    write(&root, INNER_MD, inner);
    commit_all(&world.git(), &root, "two");
    let head = world.head();
    world.run(&root, &["push", "-q", "origin", "main"]);
    world.remote_hook("pre-receive", "echo frozen >&2\nexit 1");

    let status = world.start_push();
    assert_state(&status, "ok", "in step");
    let in_step = reads(&world);
    world.stop();

    let behind = world.rev(&root, "HEAD~2");
    world.run(&world.bare, &["update-ref", "refs/heads/main", &behind]);
    let status = world.start_push();
    assert_state(&status, "held", "two commits ahead");
    assert_eq!(status["ahead"], json!(2), "{status}");
    let ahead = reads(&world);
    world.stop();

    let empty = world.run(&world.bare, &["mktree"]);
    let orphan = world.run(&world.bare, &["commit-tree", &empty, "-m", "elsewhere"]);
    world.run(&world.bare, &["update-ref", "refs/heads/main", &orphan]);
    let status = world.start_push();
    assert_state(&status, "held", "moved away");
    assert_ne!(status["behind"], json!(0), "{status}");
    let away = reads(&world);
    world.stop();

    assert_eq!(world.head(), head, "the checkout the same throughout");
    assert_eq!(ahead, in_step, "two commits ahead");
    assert_eq!(away, in_step, "the remote moved away");
    assert_ne!(in_step.gate.1, "", "the gate answered a document");
}

// -------------------------------------------------------------------- P24

/// A hook's words with non-ASCII characters near `://` (docs/features/hub-git.md
/// P24): U+00E9 then a space and a URL, U+00E9 right before `://`, U+00EF right
/// before a scheme, each URL holding `u:p@`.
const ACCENTED: &str = "printf 'refus\\303\\251 https://u:p@example.invalid/r.git \
                        x\\303\\251://u:p@example.invalid/q.git \
                        na\\303\\257vehttps://u:p@example.invalid/s.git fin\\n' >&2";

/// What `error` shows of [`ACCENTED`]: each token holding a URL replaced whole.
const ACCENTED_SHOWN: &str = "refus\u{e9} <url> <url> <url> fin";

#[track_caller]
fn assert_redacted(error: &str, context: &str) {
    assert!(
        error.contains(ACCENTED_SHOWN),
        "{context}: {ACCENTED_SHOWN:?} not in {error:?}"
    );
    for leak in ["u:p", "example.invalid", "x\u{e9}", "na\u{ef}ve"] {
        assert!(!error.contains(leak), "{context}: {leak:?} in {error:?}");
    }
}

/// P24: git speaks [`ACCENTED`] in three places, and each sync reads as git
/// says, the daemon alive, every URL token replaced whole: a `post-receive` on
/// a push that lands (`ok`), a `pre-receive` that refuses (`held`, its reason
/// then its words), the checkout's `pre-push` (`retrying`, its words). M:
/// byte-index slicing.
#[test]
fn p24_non_ascii_before_a_url_never_panics_and_the_token_goes_whole() {
    let mut world = World::press("git-accents");
    world.remote_hook("post-receive", ACCENTED);
    world.hand_commit("notes/one.md", "One.\n");
    let status = world.start_push();
    assert_state(&status, "ok", "a push that landed, with words");
    assert_eq!(world.remote_main(), world.head(), "it landed");
    assert!(
        world.server.as_mut().expect("a daemon").alive(),
        "the daemon lives"
    );
    world.stop();

    fs::remove_file(world.bare.join("hooks").join("post-receive")).expect("removed");
    world.remote_hook("pre-receive", &format!("{ACCENTED}\nexit 1"));
    world.hand_commit("notes/two.md", "Two.\n");
    let status = world.start_push();
    assert_state(&status, "held", "a refusal with words");
    let error = assert_error(
        &status,
        "the remote refused `main`: pre-receive hook declined; ",
        "a refusal with words",
    );
    assert_redacted(&error, "a refusal with words");
    world.stop();

    fs::remove_file(world.bare.join("hooks").join("pre-receive")).expect("removed");
    world.checkout_hook("pre-push", &format!("{ACCENTED}\nexit 1"));
    let status = world.start_push();
    assert_state(&status, "retrying", "a pre-push with words");
    let error = assert_error(&status, PUSH_FAILED_LINE, "a pre-push with words");
    assert_redacted(&error, "a pre-push with words");
    let stderr = world.server().stderr();
    assert!(
        !stderr.contains("u:p@") && !stderr.contains("panicked"),
        "the daemon's stderr: {stderr}"
    );
    world.assert_tidy("P24");
    assert_pins(&world.children(), "P24");
}

/// "Data", **`error`**, beyond P24: a refusal whose hook prints `x` and 3000
/// U+00E9 (6001 bytes, so 4096 falls inside a character after git's `remote: `):
/// `held`, the git part cut so that `error` stays within 4096 bytes, on a
/// character boundary, never a panic. M: a cut at a byte index.
#[test]
fn a_long_non_ascii_refusal_is_cut_within_4096_bytes() {
    let mut world = World::press("git-long-words");
    world.remote_hook(
        "pre-receive",
        "/usr/bin/perl -e 'print STDERR \"x\", \"\\xc3\\xa9\" x 3000, \"\\n\"'\nexit 1",
    );
    world.hand_commit("notes/one.md", "One.\n");
    let status = world.start_push();
    assert_state(&status, "held", "a long refusal");
    let error = assert_error(&status, REFUSED_LINE, "a long refusal");
    assert!(error.len() <= 4096, "{} bytes", error.len());
    assert!(
        error.contains("\u{e9}\u{e9}\u{e9}"),
        "git's part kept: {error:?}"
    );
    world.assert_tidy("a long refusal");
}

// -------------------------------------------------------------------- P25

/// P25: a developer's commit of `notes/slow.txt`, which the checkout smudges
/// through a filter that sleeps 30 s: the start's sync fetches and its
/// fast-forward hangs in the filter, inside the turns, until the deadline (3
/// s) ends it with SIGTERM, SIGKILL only 2 s later, so git removes its own
/// `index.lock`. Afterwards no `index.lock` exists, `HEAD` has not moved, and,
/// the filter gone, the next confirm commits and its sync merges and pushes.
/// M: SIGKILL at once.
#[test]
fn p25_a_local_child_past_the_deadline_is_ended_and_leaves_no_lock() {
    let mut world = World::press("git-slow-checkout");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    let root = world.root().to_path_buf();
    let smudged = world.setup.scratch.join("smudged.log");
    write(
        &root,
        ".git/info/attributes",
        "notes/slow.txt filter=slow\n",
    );
    let smudge = format!("echo smudging >> '{}'; sleep 30; cat", smudged.display());
    world.run(&root, &["config", "filter.slow.smudge", &smudge]);
    world.run(&root, &["config", "filter.slow.clean", "cat"]);
    let theirs = world.developer("notes/slow.txt", "Checked out slowly.\n", true);
    let before = world.head();

    let launched = Instant::now();
    let status = world.start_push();
    let took = launched.elapsed();
    assert!(smudged.exists(), "the fast-forward reached the filter");
    assert!(
        took < Duration::from_secs(25),
        "ended by the deadline, not by the filter: {took:?}"
    );
    assert_ne!(status["state"], json!("ok"), "{status}");
    let lock = root.join(".git").join("index.lock");
    let waited = Instant::now();
    while lock.exists() && waited.elapsed() < Duration::from_secs(5) {
        thread::sleep(POLL);
    }
    assert!(!lock.exists(), "P25: .git/index.lock left behind");
    assert_eq!(
        world.head(),
        before,
        "the killed fast-forward moved nothing"
    );

    world.run(&root, &["config", "--unset", "filter.slow.smudge"]);
    world.run(&root, &["config", "--unset", "filter.slow.clean"]);
    fs::remove_file(root.join(".git/info/attributes")).expect("removed");
    let (document, status) = world.synced(|| world.confirm(&id));
    let applied = document["applied_commit"].as_str().expect("applied_commit");
    assert_eq!(
        world.parents(&root, applied),
        [before.as_str()],
        "committed"
    );
    assert_state(&status, "ok", "the next sync");
    let tip = world.remote_main();
    assert!(world.reaches(&world.bare, &tip, applied), "pushed");
    assert!(world.reaches(&world.bare, &tip, &theirs), "merged");
    world.assert_tidy("P25");
    assert_pins(&world.children(), "P25");
}

// -------------------------------------------------------------------- P26

/// P26: the hub's checkout and a second worktree of its repository (branch
/// `side`, its project `card-press-two` with a root `notes` its sparse
/// checkout lacks), both served, the first first. The first's sync ends `ok`
/// and pushes, the second's gap left out of its look; the second's `GET
/// .../sync` reads `stopped`, `this worktree is not synced: the sync runs in
/// <top>`. M: share the first worktree's status; look at every root of the
/// repository.
#[test]
fn p26_a_second_worktree_is_not_synced_and_its_gap_stops_nothing() {
    let mut world = World::press("git-worktrees");
    let root = world.root().to_path_buf();
    let second = world.setup.scratch.join("hub-two");
    world.run(
        &root,
        &["worktree", "add", "-q", "-b", "side", text(&second)],
    );
    let second = fs::canonicalize(second).expect("the second worktree");
    replace(
        &second,
        "specengine.toml",
        "slug = \"card-press\"",
        "slug = \"card-press-two\"",
    );
    append(&second, "\n[paths]\nroots = [\"docs\", \"notes\"]\n");
    write(
        &second,
        "notes/plan.md",
        "---\nclass: spec\nstatus: draft\nscope: [weekly]\n---\n\n# A plan\n\nTo sort.\n",
    );
    commit_all(&world.git(), &second, "the second project");
    world.run(&second, &["sparse-checkout", "set", "--cone", "docs"]);
    assert!(!second.join("notes").exists(), "the second worktree's gap");
    assert!(
        root.join(BACK_MD).is_file(),
        "the first worktree keeps every file"
    );
    let hand = world.hand_commit("notes/hand.md", "A commit to push.\n");

    world.launch_roots(&[root.as_path(), second.as_path()], &PUSH);
    let first = world.after(&json!({"at": null}), SYNC_WAIT);
    assert_state(&first, "ok", "the first worktree's sync");
    assert_eq!(world.remote_main(), hand, "pushed");
    let reply = world.get("/api/projects/card-press-two/sync");
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(keys_of(reply.text()), KEYS, "{}", reply.text());
    let other = reply.json();
    assert_state(&other, "stopped", "the second worktree");
    assert_eq!(
        other["error"],
        json!(format!(
            "this worktree is not synced: the sync runs in {}",
            root.display()
        )),
        "{other}"
    );
    world.assert_tidy("P26");
    assert_pins(&world.children(), "P26");
}

// -------------------------------------------------------------------- P27

/// P27: a confirm in mid-apply while a sync looks. The checkout's
/// `pre-commit`, armed once, holds the confirm's commit (its target written,
/// not yet committed: the status unclean) until the wrapper's log shows the
/// timer's sync (`--push-every 1`) running `git status` or `git fetch`, then
/// lets it go. That sync is never `stopped`: its look is provisional, and
/// inside the turns, once the confirm is done, it pushes the confirm's commit,
/// `ok`. The premise is checked: the hook saw the sync while it held the
/// commit. M: decide cleanliness in the look.
#[test]
fn p27_a_confirm_in_mid_apply_never_stops_the_sync() {
    let mut world = World::press("git-mid-apply");
    let id = world.raise("SHT-BACK", "the tide table", "the moon table");
    let armed = world.setup.scratch.join("gate-armed");
    let gate = world.setup.scratch.join("gate.log");
    world.checkout_hook(
        "pre-commit",
        &format!(
            "if [ -e '{armed}' ]; then\n\
             rm -f '{armed}'\n\
             start=$(wc -l < '{log}' | tr -d ' ')\n\
             us=$(printf '\\037')\n\
             echo waiting >> '{gate}'\n\
             i=0\n\
             while [ $i -lt 300 ]; do\n\
             if tail -n +$((start + 1)) '{log}' | grep -qF -e \"${{us}}status${{us}}\" -e \"${{us}}fetch${{us}}\"; then\n\
             echo saw >> '{gate}'; exit 0\n\
             fi\n\
             sleep 0.1; i=$((i + 1))\n\
             done\n\
             echo timeout >> '{gate}'\n\
             fi\n\
             exit 0",
            armed = armed.display(),
            log = world.log.display(),
            gate = gate.display(),
        ),
    );
    let started = world.start(&["--push", "--push-deadline", "3", "--push-every", "1"]);
    let ended_at = Instant::now();
    assert_state(&started, "ok", "the start");
    // The timer's next sync comes 60 s after the start's ended: the confirm
    // starts 42 s in, and the hook holds it for at most 30 s.
    thread::sleep(Duration::from_secs(42).saturating_sub(ended_at.elapsed()));
    fs::write(armed.as_path(), "").expect("armed");
    let before = world.status();
    let document = world.confirm(&id);
    let applied = document["applied_commit"]
        .as_str()
        .expect("applied_commit")
        .to_owned();
    let held = fs::read_to_string(gate.as_path()).unwrap_or_default();
    assert_eq!(
        held, "waiting\nsaw\n",
        "the premise: the hook held the commit while a sync ran"
    );
    let next = world.after(&before, SYNC_WAIT);
    assert_ne!(next["state"], json!("stopped"), "P27: {next}");
    let waited = Instant::now();
    let mut last = next;
    while !(last["state"] == json!("ok") && world.remote_main() == applied) {
        assert!(
            waited.elapsed() < SYNC_WAIT,
            "the confirm's commit never pushed: {last}"
        );
        thread::sleep(POLL);
        last = world.status();
    }
    assert!(
        !world
            .sync_lines()
            .iter()
            .any(|line| line.contains(": stopped")),
        "never stopped: {:?}",
        world.sync_lines()
    );
    world.assert_tidy("P27");
    assert_pins(&world.children(), "P27");
}

// ------------------------------------------------------------- read-trust AC-6

/// The node a developer adds, its file and the word only it holds.
const ZETA_ID: &str = "SHT-ZETA";
const ZETA_WORD: &str = "qorvexa";

fn zeta_file() -> String {
    format!(
        "---\nid: {ZETA_ID}\nclass: canon\ntitle: The zeta sheet\nparent: PRN-WEEKLY\n\
         owner: owner\nreviewed: 2026-10-01\n---\n\n# The zeta sheet\n\n\
         The zeta sheet carries the {ZETA_WORD} notices.\n"
    )
}

impl World {
    /// `GET <base>/search?query=<word>` by `ann`: a 200, its body.
    fn search(&self, word: &str) -> String {
        let reply = self.get(&format!("{}/search?query={word}", self.base));
        assert_eq!(reply.status, 200, "search {word}: {}", reply.text());
        reply.text().to_owned()
    }

    /// A first read, which walks: the trust is fresh and not dirty after it.
    fn walked(&self) {
        let _ = self.search("notices");
    }
}

/// AC-6 (a): after a confirm, a trusted `search` finds the new text.
#[test]
fn rt6a_a_confirm_is_found_by_the_next_trusted_search() {
    let mut world = World::press("trust-confirm");
    let id = world.raise("SHT-BACK", "the tide table", "the zavoqui table");
    assert_state(&world.start_push(), "ok", "the start");
    world.walked();
    assert!(
        !world.search("zavoqui").contains("SHT-BACK"),
        "premise: not found before the confirm"
    );
    world.confirm(&id);
    let found = world.search("zavoqui");
    assert!(
        found.contains("SHT-BACK"),
        "the confirm's commit updated the index: {found}"
    );
}

/// AC-6 (b): a fast-forward from the remote. The checkout's `post-commit`
/// hook has the developer pull the confirm's commit from the checkout,
/// build a commit adding a node on it and push it, all before the confirm
/// returns; the confirm's sync then fast-forwards to it. A trusted
/// `search` finds the node. M: the merge's `update_paths` dropped.
#[test]
fn rt6b_a_fast_forward_is_found_by_the_next_trusted_search() {
    let mut world = World::press("trust-fast-forward");
    let id = world.raise("SHT-BACK", "the tide table", "the zavoqui table");
    let body = zeta_file();
    world.checkout_hook(
        "post-commit",
        &format!(
            "unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR\n\
             cd '{dev}' || exit 0\n\
             git pull -q --ff-only '{hub}' main || exit 0\n\
             cat > docs/spec/sht-zeta.md <<'ZETA'\n{body}ZETA\n\
             git add -A && git commit -q -m 'a developer builds on the hub' && \
             git push -q origin main\n\
             rm -f '{hook}'\n",
            dev = text(&world.dev),
            hub = text(world.root()),
            hook = text(&world.root().join(".git/hooks/post-commit")),
        ),
    );
    assert_state(&world.start_push(), "ok", "the start");
    world.walked();
    assert!(
        !world.search(ZETA_WORD).contains(ZETA_ID),
        "premise: not found before"
    );
    let (document, status) = world.synced(|| world.confirm(&id));
    assert_state(&status, "ok", "the sync after the confirm");
    let applied = document["applied_commit"]
        .as_str()
        .expect("applied_commit")
        .to_owned();
    let theirs = world.rev(&world.dev, "HEAD");
    assert_ne!(theirs, applied, "premise: the hook built a commit");
    assert_eq!(
        world.parents(&world.dev, &theirs),
        [applied],
        "premise: the developer's commit is built on the confirm's"
    );
    assert_eq!(world.head(), theirs, "premise: the sync fast-forwarded");
    let found = world.search(ZETA_WORD);
    assert!(
        found.contains(ZETA_ID),
        "the fast-forward updated the index: {found}"
    );
}

/// AC-6 (c): a merge commit from the remote: a developer's commit adding a
/// node, pushed before a confirm whose sync merges it. A trusted `search`
/// finds the node. M: the merge's `update_paths` dropped.
#[test]
fn rt6c_a_merge_commit_is_found_by_the_next_trusted_search() {
    let mut world = World::press("trust-merge");
    let id = world.raise("SHT-BACK", "the tide table", "the zavoqui table");
    assert_state(&world.start_push(), "ok", "the start");
    world.walked();
    let theirs = world.developer("docs/spec/sht-zeta.md", &zeta_file(), true);
    assert!(
        !world.search(ZETA_WORD).contains(ZETA_ID),
        "premise: not found before the merge"
    );
    let (document, status) = world.synced(|| world.confirm(&id));
    assert_state(&status, "ok", "the merge pushed");
    let applied = document["applied_commit"].as_str().expect("applied_commit");
    assert_eq!(
        world.parents(world.root(), &world.head()),
        [applied, theirs.as_str()],
        "premise: a merge commit"
    );
    let found = world.search(ZETA_WORD);
    assert!(
        found.contains(ZETA_ID),
        "the merge updated the index: {found}"
    );
    assert!(
        world.search("zavoqui").contains("SHT-BACK"),
        "the confirm's own text too"
    );
}

/// AC-6 (d): a merge whose `specengine.toml` adds a `[paths]` root holding a
/// file: that file's node is found. M: the update run with the config as
/// read before the merge.
#[test]
fn rt6d_a_merge_adding_a_root_is_found_by_the_next_trusted_search() {
    let mut world = World::press("trust-roots");
    let id = world.raise("SHT-BACK", "the tide table", "the zavoqui table");
    assert_state(&world.start_push(), "ok", "the start");
    world.walked();
    let git = world.git();
    git.run(&world.dev, &["pull", "-q", "--ff-only", "origin", "main"]);
    append(
        &world.dev,
        "\n[paths]\nroots = [\"docs/spec\", \"docs/records\", \"notes\"]\n",
    );
    write(&world.dev, "notes/sht-zeta.md", zeta_file());
    commit_all(&git, &world.dev, "a developer adds a root");
    git.run(&world.dev, &["push", "-q", "origin", "main"]);
    assert!(
        !world.search(ZETA_WORD).contains(ZETA_ID),
        "premise: not found before the merge"
    );
    let (_, status) = world.synced(|| world.confirm(&id));
    assert_state(&status, "ok", "the merge pushed");
    assert!(
        read_text(world.root(), "specengine.toml").contains("\"notes\"]"),
        "premise: the merge brought the new root"
    );
    let found = world.search(ZETA_WORD);
    assert!(
        found.contains(ZETA_ID),
        "the merge's update read the merged config: {found}"
    );
}

/// AC-6 (e): a hub restarted over an index made stale while it was down
/// (a hand edit on the host): its first `search` finds the edit. M: a
/// trust that starts walked.
#[test]
fn rt6e_a_restarted_hub_walks_first() {
    let mut world = World::press("trust-restart");
    world.launch(&[]);
    world.walked();
    assert!(
        !world.search("zavoqui").contains("SHT-BACK"),
        "premise: not found before the edit"
    );
    world.stop();
    replace(world.root(), BACK_MD, "the tide table", "the zavoqui table");
    world.launch(&[]);
    let found = world.search("zavoqui");
    assert!(
        found.contains("SHT-BACK"),
        "a restarted hub's first read walks: {found}"
    );
}

// ------------------------------------------------------------ read-trust R11, R12

/// The second project of a two-root repository: its directory, its slug,
/// its node and the word only that node holds.
const SECOND_DIR: &str = "second";
const SECOND_SLUG: &str = "second-press";
const SECOND_ID: &str = "SHT-SECOND";
const SECOND_WORD: &str = "pelvanu";

/// The print shop with a second project under [`SECOND_DIR`], committed
/// before the hub's clone: one repository serving two roots.
fn two_roots(name: &str) -> World {
    World::press_with(name, |root| {
        write(
            root,
            &format!("{SECOND_DIR}/specengine.toml"),
            format!(
                "[project]\nslug = \"{SECOND_SLUG}\"\n\n[ids]\n\
                 SHT = {{ kind = \"broadsheet\", shape = \"name\" }}\n"
            ),
        );
        write(
            root,
            &format!("{SECOND_DIR}/docs/spec/sht-second.md"),
            format!(
                "---\nid: {SECOND_ID}\nclass: canon\ntitle: The second sheet\n\
                 owner: owner\nreviewed: 2026-10-01\n---\n\n# The second sheet\n\n\
                 The second sheet carries the {SECOND_WORD} notices.\n"
            ),
        );
    })
}

/// What the library calls of the hub's host take: its `HOME`, its working
/// directory, git's environment with the wrapper first on `PATH`.
struct Host {
    env: Env,
    git: GitEnv,
}

impl Host {
    fn of(world: &World) -> Self {
        Self {
            env: Env {
                cwd: world.setup.cwd.clone(),
                home: Some(world.setup.home.as_os_str().to_owned()),
                xdg_data_home: None,
            },
            git: GitEnv::new(world.setup.cwd.clone(), world.env()),
        }
    }

    fn globals(root: &Path) -> Globals {
        Globals {
            root: Some(root.to_path_buf()),
            config: None,
        }
    }

    /// `spec index` of `root`.
    fn index(&self, root: &Path) {
        specengine_cli::index(
            &self.env,
            &Self::globals(root),
            &IndexRequest { full: false },
        )
        .unwrap_or_else(|error| panic!("spec index of {}: {error}", root.display()));
    }

    /// `search word` over `root`: whether its text names `id`.
    fn finds(&self, root: &Path, word: &str, id: &str) -> bool {
        let outcome = specengine_cli::search(
            &self.env,
            &Self::globals(root),
            &SearchRequest {
                terms: vec![word.to_owned()],
                kinds: Vec::new(),
                limit: None,
                archive: false,
            },
        )
        .unwrap_or_else(|error| panic!("search {word}: {error}"));
        specengine_cli::render_text(&specengine_cli::Outcome::Search(outcome)).contains(id)
    }

    /// One sync of `world`'s repository serving `roots`, its merge inside
    /// `walking` of `trusts`, as the daemon's sync runs it (a write: nothing
    /// in it trusts the index; its doubts reach the trusts).
    fn sync(&self, world: &World, roots: &[PathBuf], trusts: &[&IndexTrust]) -> SyncOutcome {
        let repository = SyncRepository {
            top: world.root().to_path_buf(),
            roots: roots.to_vec(),
        };
        sync_once(
            &repository,
            &self.env,
            &self.git,
            Duration::from_secs(30),
            &SyncOutcome::default(),
            &mut |merge| walking(trusts, merge),
        )
    }
}

/// Every root indexed and its trust walked once, clean.
fn walked_trusts(host: &Host, roots: &[PathBuf]) -> Vec<IndexTrust> {
    roots
        .iter()
        .map(|root| {
            host.index(root);
            let trust = IndexTrust::new(root.clone());
            trusted(&[&trust], || host.finds(root, "notices", "-"));
            assert_eq!(trust.walks(), 1, "{}: the first read walks", root.display());
            assert!(!trust.is_dirty(), "{}: premise, clean", root.display());
            trust
        })
        .collect()
}

/// The wrapper on `PATH` made to fail git's `verb` when its arguments hold
/// `flag`, writing `marker` when it does; every other child runs as before.
fn fail_git(world: &World, verb: &str, flag: &str, marker: &Path) {
    script(
        &world.wrap.join("git"),
        &format!(
            "case \" $* \" in\n  *\" {verb} \"*\"{flag}\"*) printf 'failed\\n' > '{}'; \
             echo 'injected failure' >&2; exit 1 ;;\nesac\nexec '{}' \"$@\"",
            marker.display(),
            real_git().display()
        ),
    );
}

/// R11: a fast-forward over two served roots updates each index inside
/// the turns and doubts nothing; a trusted read finds the merged node
/// without a walk (AC-6 (b) through the library).
#[test]
fn rt11_a_fast_forward_updates_each_root_and_doubts_nothing() {
    let world = two_roots("trust-r11-ff");
    let host = Host::of(&world);
    let roots = [world.root().to_path_buf(), world.root().join(SECOND_DIR)];
    let trusts = walked_trusts(&host, &roots);
    let theirs = world.developer("docs/spec/sht-zeta.md", &zeta_file(), true);
    let outcome = host.sync(&world, &roots, &trusts.iter().collect::<Vec<_>>());
    assert_eq!(world.head(), theirs, "premise: fast-forwarded: {outcome:?}");
    for (root, trust) in roots.iter().zip(&trusts) {
        assert!(!trust.is_dirty(), "{}: nothing doubted", root.display());
    }
    assert!(
        trusted(&[&trusts[0]], || host.finds(&roots[0], ZETA_WORD, ZETA_ID)),
        "the merge's update indexed the developer's node"
    );
    assert_eq!(trusts[0].walks(), 1, "the trusted read did not walk");
}

/// R11: a fast-forward whose `git diff` fails doubts every served root.
#[test]
fn rt11_a_failed_diff_doubts_every_root() {
    let world = two_roots("trust-r11-diff");
    let host = Host::of(&world);
    let roots = [world.root().to_path_buf(), world.root().join(SECOND_DIR)];
    let trusts = walked_trusts(&host, &roots);
    let theirs = world.developer("docs/spec/sht-zeta.md", &zeta_file(), true);
    let marker = world.setup.scratch.join("diff-failed");
    fail_git(&world, "diff", "--no-renames", &marker);
    let outcome = host.sync(&world, &roots, &trusts.iter().collect::<Vec<_>>());
    assert!(
        marker.is_file(),
        "premise: the merge's diff ran and failed: {outcome:?}"
    );
    assert_eq!(world.head(), theirs, "premise: fast-forwarded: {outcome:?}");
    for (root, trust) in roots.iter().zip(&trusts) {
        assert!(
            trust.is_dirty(),
            "{}: a failed diff doubts it",
            root.display()
        );
    }
    assert!(
        trusted(&[&trusts[0]], || host.finds(&roots[0], ZETA_WORD, ZETA_ID)),
        "the doubted root's next read walks and finds the merged node"
    );
    assert_eq!(trusts[0].walks(), 2, "that read walked");
}

/// R11: a fast-forward bringing a `specengine.toml` that does not parse:
/// that root's discovery fails, and it alone is doubted.
#[test]
fn rt11_a_failed_discovery_doubts_its_own_root() {
    let world = two_roots("trust-r11-config");
    let host = Host::of(&world);
    let roots = [world.root().to_path_buf(), world.root().join(SECOND_DIR)];
    let trusts = walked_trusts(&host, &roots);
    let mut broken = read_text(&world.dev, "specengine.toml");
    broken.push_str("\n[ids\n");
    let theirs = world.developer("specengine.toml", &broken, true);
    let outcome = host.sync(&world, &roots, &trusts.iter().collect::<Vec<_>>());
    assert_eq!(world.head(), theirs, "premise: fast-forwarded: {outcome:?}");
    assert!(
        trusts[0].is_dirty(),
        "the root whose merged config does not parse is doubted"
    );
    assert!(
        !trusts[1].is_dirty(),
        "a root the merge did not touch is not doubted"
    );
}

/// R12: a fast-forward that fails (the wrapper refuses `merge --ff-only`)
/// stops the sync and doubts every served root: their files may be half
/// moved.
#[test]
fn rt12_a_failed_fast_forward_doubts_every_root() {
    let world = two_roots("trust-r12");
    let host = Host::of(&world);
    let roots = [world.root().to_path_buf(), world.root().join(SECOND_DIR)];
    let trusts = walked_trusts(&host, &roots);
    let before = world.head();
    world.developer("docs/spec/sht-zeta.md", &zeta_file(), true);
    let marker = world.setup.scratch.join("merge-failed");
    fail_git(&world, "merge", "--ff-only", &marker);
    let outcome = host.sync(&world, &roots, &trusts.iter().collect::<Vec<_>>());
    assert!(
        marker.is_file(),
        "premise: the fast-forward ran and failed: {outcome:?}"
    );
    assert_eq!(world.head(), before, "premise: nothing moved");
    assert_eq!(
        outcome.state,
        specengine_cli::SyncState::Stopped,
        "the sync stops: {outcome:?}"
    );
    for (root, trust) in roots.iter().zip(&trusts) {
        assert!(
            trust.is_dirty(),
            "{}: a failed fast-forward doubts it",
            root.display()
        );
    }
}

// ------------------------------------------------- read-trust, review iteration 2

/// The wrapper on `PATH` made to add a non-UTF-8 name to the merge's
/// `git diff --name-only -z` (APFS cannot hold one), writing `marker`.
fn fake_diff_name(world: &World, marker: &Path) {
    script(
        &world.wrap.join("git"),
        &format!(
            "case \" $* \" in\n  *\" diff \"*\"--no-renames\"*) printf 'faked\\n' > '{}'; \
             printf 'docs/spec/bad-\\377.md\\000'; exec '{git}' \"$@\" ;;\nesac\n\
             exec '{git}' \"$@\"",
            marker.display(),
            git = real_git().display()
        ),
    );
}

/// A merge diff naming a path that is not UTF-8: its skipped name doubts
/// every served root, as the walk would skip it.
#[test]
fn rt11_a_skipped_name_in_the_diff_doubts_every_root() {
    let world = two_roots("trust-r11-skipped");
    let host = Host::of(&world);
    let roots = [world.root().to_path_buf(), world.root().join(SECOND_DIR)];
    let trusts = walked_trusts(&host, &roots);
    let theirs = world.developer("docs/spec/sht-zeta.md", &zeta_file(), true);
    let marker = world.setup.scratch.join("diff-faked");
    fake_diff_name(&world, &marker);
    let outcome = host.sync(&world, &roots, &trusts.iter().collect::<Vec<_>>());
    assert!(
        marker.is_file(),
        "premise: the merge's diff ran: {outcome:?}"
    );
    assert_eq!(world.head(), theirs, "premise: fast-forwarded: {outcome:?}");
    for (root, trust) in roots.iter().zip(&trusts) {
        assert!(
            trust.is_dirty(),
            "{}: a skipped name in the diff doubts it",
            root.display()
        );
    }
}

/// A served root that does not lie below the checkout's top (a repository
/// of its own: no path from the top): the merge cannot say which of its
/// paths moved, so it is doubted; the top's root is updated and not
/// doubted.
#[test]
fn rt11_a_root_outside_the_top_is_doubted() {
    let world = World::press("trust-r11-outside");
    let host = Host::of(&world);
    let top = world.root().to_path_buf();
    let outside = fs::canonicalize(world.setup.scratch.dir("outside")).expect("canonical");
    world.git().run(&outside, &["init", "-q"]);
    let trusts = walked_trusts(&host, std::slice::from_ref(&top));
    let elsewhere = IndexTrust::new(outside.clone());
    let theirs = world.developer("docs/spec/sht-zeta.md", &zeta_file(), true);
    let roots = [top.clone(), outside];
    let outcome = host.sync(&world, &roots, &[&trusts[0], &elsewhere]);
    assert_eq!(world.head(), theirs, "premise: fast-forwarded: {outcome:?}");
    assert!(elsewhere.is_dirty(), "a root outside the top is doubted");
    assert!(
        !trusts[0].is_dirty(),
        "the top's root is updated, not doubted"
    );
    assert!(
        trusted(&[&trusts[0]], || host.finds(&top, ZETA_WORD, ZETA_ID)),
        "the top's index holds the merged node"
    );
}

/// The print shop's `[paths]` written: its spec and records roots, and a
/// `notes` root.
const NOTES_ROOTS: &str = "\n[paths]\nroots = [\"docs/spec\", \"docs/records\", \"notes\"]\n";
const NOTES_FILE: &str = "notes/sht-zeta.md";

impl Host {
    /// `search word` over `root`: its stderr lines.
    fn search_lines(&self, root: &Path, word: &str) -> Vec<String> {
        specengine_cli::search(
            &self.env,
            &Self::globals(root),
            &SearchRequest {
                terms: vec![word.to_owned()],
                kinds: Vec::new(),
                limit: None,
                archive: false,
            },
        )
        .map(|outcome| specengine_cli::Outcome::Search(outcome).stderr_lines())
        .unwrap_or_else(|error| panic!("search {word}: {error}"))
    }
}

/// The warnings of a trusted `search` (no walk) and of a walking one, over
/// `root` whose trust is `trust`.
fn warnings_both_ways(host: &Host, root: &Path, trust: &IndexTrust) -> (Vec<String>, Vec<String>) {
    let walks = trust.walks();
    let trusted_lines = trusted(&[trust], || host.search_lines(root, "notices"));
    assert_eq!(
        trust.walks(),
        walks,
        "premise: the trusted read did not walk"
    );
    let walking_lines = host.search_lines(root, "notices");
    (trusted_lines, walking_lines)
}

fn names_notes(lines: &[String]) -> bool {
    lines.iter().any(|line| line.contains("`notes`"))
}

/// A merge giving the missing `notes` root its first file: the trusted
/// read's warnings are the walk's, the missing root no longer named.
#[test]
fn rt11_a_merge_giving_a_missing_root_its_first_file_moves_the_warnings() {
    let world = World::press_with("trust-r11-first", |root| append(root, NOTES_ROOTS));
    let host = Host::of(&world);
    let root = world.root().to_path_buf();
    let trusts = walked_trusts(&host, std::slice::from_ref(&root));
    let (before, _) = warnings_both_ways(&host, &root, &trusts[0]);
    assert!(
        names_notes(&before),
        "premise: `notes` is missing: {before:?}"
    );
    world.developer(NOTES_FILE, &zeta_file(), true);
    let outcome = host.sync(&world, std::slice::from_ref(&root), &[&trusts[0]]);
    assert!(
        root.join(NOTES_FILE).is_file(),
        "premise: fast-forwarded: {outcome:?}"
    );
    assert!(!trusts[0].is_dirty(), "premise: nothing doubted");
    let (trusted_lines, walking_lines) = warnings_both_ways(&host, &root, &trusts[0]);
    assert_eq!(
        trusted_lines, walking_lines,
        "the trusted read's warnings are the walk's"
    );
    assert!(
        !names_notes(&trusted_lines),
        "`notes` is no longer missing: {trusted_lines:?}"
    );
}

/// A merge taking the `notes` root's last file: the trusted read's warnings
/// are the walk's, the root named missing again.
#[test]
fn rt11_a_merge_taking_a_roots_last_file_moves_the_warnings() {
    let world = World::press_with("trust-r11-last", |root| {
        append(root, NOTES_ROOTS);
        write(root, NOTES_FILE, zeta_file());
    });
    let host = Host::of(&world);
    let root = world.root().to_path_buf();
    let trusts = walked_trusts(&host, std::slice::from_ref(&root));
    let (before, _) = warnings_both_ways(&host, &root, &trusts[0]);
    assert!(
        !names_notes(&before),
        "premise: `notes` holds a file: {before:?}"
    );
    let git = world.git();
    git.run(&world.dev, &["pull", "-q", "--ff-only", "origin", "main"]);
    git.run(&world.dev, &["rm", "-q", NOTES_FILE]);
    git.run(
        &world.dev,
        &["commit", "-q", "-m", "a developer empties notes"],
    );
    git.run(&world.dev, &["push", "-q", "origin", "main"]);
    let outcome = host.sync(&world, std::slice::from_ref(&root), &[&trusts[0]]);
    assert!(
        !root.join(NOTES_FILE).exists(),
        "premise: fast-forwarded: {outcome:?}"
    );
    let (trusted_lines, walking_lines) = warnings_both_ways(&host, &root, &trusts[0]);
    assert_eq!(
        trusted_lines, walking_lines,
        "the trusted read's warnings are the walk's"
    );
    assert!(
        names_notes(&trusted_lines),
        "`notes` is missing again: {trusted_lines:?}"
    );
}

impl World {
    /// `id` staged by `ann` and confirmed, the confirm's reply as it comes.
    fn confirm_reply(&self, id: &str) -> Reply {
        let stage = self.post(
            &format!("proposals/{id}/decision"),
            &json!({"decision": "approve", "updated_at": self.review(id)["updated_at"]}),
        );
        assert_eq!(stage.status, 200, "ann stages {id}: {}", stage.text());
        let route = format!("{}/proposals/{id}/confirm", self.base);
        let read = self.get(&route);
        assert_eq!(read.status, 200, "ann reads {id}: {}", read.text());
        let read = read.json();
        let body = json!({"question": read["question"], "updated_at": read["updated_at"]});
        through(
            self.server(),
            "POST",
            &route,
            &Proxied::write("ann"),
            body.to_string().as_bytes(),
        )
    }
}

/// A confirm whose commit step 10 refuses (the checkout's `pre-commit` hook
/// adds another file's change to it): the files are written, no
/// post-commit update runs, and the refusal doubts the project, so the
/// next trusted read walks and shows the other file. M: the refused
/// confirm's doubt dropped.
#[test]
fn rt9_a_confirm_refused_after_its_commit_doubts_the_project() {
    let mut world = World::press("trust-refused-confirm");
    let id = world.raise("SHT-BACK", "the tide table", "the zavoqui table");
    world.checkout_hook(
        "pre-commit",
        &format!(
            "perl -pi -e 's/the long reads/the vexotra reads/' {INNER_MD} && \
             git add {INNER_MD}\nrm -f \"$0\""
        ),
    );
    world.launch(&[]);
    world.walked();
    assert!(
        !world.search("vexotra").contains("SHT-INNER"),
        "premise: not found before"
    );
    let before = world.head();
    let reply = world.confirm_reply(&id);
    let status = serde_json::from_slice::<Value>(&reply.body)
        .ok()
        .map(|document| document["status"].clone());
    assert_ne!(
        status,
        Some(json!("applied")),
        "premise: step 10 refuses the commit: {} {}",
        reply.status,
        reply.text()
    );
    let head = world.head();
    assert_ne!(
        head,
        before,
        "premise: the commit was made: {}",
        reply.text()
    );
    let changed = world.run(world.root(), &["show", "--name-only", "--format=", &head]);
    assert!(
        changed.lines().any(|line| line == INNER_MD),
        "premise: the hook's file is in the commit: {changed}"
    );
    let found = world.search("vexotra");
    assert!(
        found.contains("SHT-INNER"),
        "the refused confirm doubted the project: the next read walked: {found}"
    );
}

/// R11: a merge whose `specengine.toml` changes `[paths]` is indexed under
/// the merged config: the stored fingerprint moves with it, so the next
/// trusted read trusts (no walk) and finds the new root's file. M: the
/// update run with the config as read before the merge (the read's
/// fingerprint check would then walk).
#[test]
fn rt11_a_merge_changing_paths_is_indexed_under_the_merged_config() {
    let world = World::press("trust-r11-paths");
    let host = Host::of(&world);
    let root = world.root().to_path_buf();
    let trusts = walked_trusts(&host, std::slice::from_ref(&root));
    let git = world.git();
    git.run(&world.dev, &["pull", "-q", "--ff-only", "origin", "main"]);
    append(&world.dev, NOTES_ROOTS);
    write(&world.dev, NOTES_FILE, zeta_file());
    commit_all(&git, &world.dev, "a developer adds the notes root");
    git.run(&world.dev, &["push", "-q", "origin", "main"]);
    let outcome = host.sync(&world, std::slice::from_ref(&root), &[&trusts[0]]);
    assert!(
        root.join(NOTES_FILE).is_file(),
        "premise: fast-forwarded: {outcome:?}"
    );
    assert!(!trusts[0].is_dirty(), "nothing doubted");
    let walks = trusts[0].walks();
    assert!(
        trusted(&[&trusts[0]], || host.finds(&root, ZETA_WORD, ZETA_ID)),
        "the merged root's file is indexed"
    );
    assert_eq!(
        trusts[0].walks(),
        walks,
        "the index already holds the merged config: the read trusts it"
    );
}
