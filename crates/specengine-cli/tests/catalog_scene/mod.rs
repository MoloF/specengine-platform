//! docs/features/direction-catalog.md (RM-0106) "Acceptance criteria", the
//! copy every catalog test reads ("the copy" of its criteria): a scratch git
//! repository holding `fixtures/studio-lamp` (one commit by the sandbox's
//! identity), then two commits on top of it, `Ann Owner` applying PR-0001 to
//! `docs/spec/scn-arrival.md` at 2026-01-02T10:00:00Z and `Bob Hand` editing
//! `docs/spec/scn-storm.md` by hand at 2026-01-03T10:00:00Z, every author and
//! committer named and dated here; then, through the CLI library, C-0001
//! open on `SCN-ARRIVAL` and PR-0040 an open question naming it.
//! `specengine-http`'s `tests/catalog.rs` and `tests/daemon_keys.rs` include
//! this file by path, beside `common/git.rs` as their `git` module, so the
//! scene holds no `specengine-core` call (the daemon's crate has none): the
//! apply message is written out here, and the CLI's `tests/catalog.rs`
//! checks it equals what core's `commit_message_raising` writes.
//!
//! Every name here is invented.

#![allow(dead_code)]

use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use specengine_cli::{
    CardAcceptRequest, CardCancelRequest, CardMoveRequest, CardNewRequest, Env, Exit, GitEnv,
    Globals, QuestionRequest, card_accept, card_cancel, card_move, card_new, propose_question,
};

use crate::git::Sandbox;

pub const SLUG: &str = "studio-lamp";
pub const ARRIVAL: &str = "docs/spec/scn-arrival.md";
pub const STORM: &str = "docs/spec/scn-storm.md";
pub const SHORE: &str = "docs/spec/chp-shore.md";

/// A person as git and the trailers write one.
#[derive(Clone, Copy, Debug)]
pub struct Ident {
    pub name: &'static str,
    pub email: &'static str,
}

impl Ident {
    /// `Name <email>`.
    pub fn value(self) -> String {
        format!("{} <{}>", self.name, self.email)
    }
}

/// The owner who decides PR-0001.
pub const ANN: Ident = Ident {
    name: "Ann Owner",
    email: "ann@example.org",
};
/// The hand editor of the storm.
pub const BOB: Ident = Ident {
    name: "Bob Hand",
    email: "bob@example.org",
};
/// A committer who is never a `by`.
pub const CLEO: Ident = Ident {
    name: "Cleo Commit",
    email: "cleo@example.org",
};

/// The pinned text, `{applied}`-free: OIDs never print.
pub const PINNED_TEXT: &str = "catalog scene | rows 2\nSCN-ARRIVAL | version 1 | in work | open \
                               items 1 | tasks 1, open 1 | 2026-01-02T10:00:00Z | Ann Owner \
                               <ann@example.org> | PR-0001 | Arrival\nSCN-STORM | version 1 | no \
                               card | open items 0 | tasks 0, open 0 | 2026-01-03T10:00:00Z | Bob \
                               Hand <bob@example.org> | hand | The storm\n";

/// The pinned JSON, compact, with the two commits' OIDs in place: one line
/// and its LF, as `spec catalog --kind scene --json` prints it.
pub fn pinned_json(applied: &str, hand: &str) -> String {
    format!(
        "{{\"kinds\":[\"scene\"],\"archive\":false,\"notes\":[],\"rows\":[\
         {{\"id\":\"SCN-ARRIVAL\",\"kind\":\"scene\",\"title\":\"Arrival\",\"path\":\"{ARRIVAL}\",\
         \"line\":1,\"rev\":1,\"status\":null,\"archived\":false,\"state\":\"in work\",\
         \"open_items\":1,\"tasks\":1,\"tasks_open\":1,\"last_change\":{{\"at\":\
         \"2026-01-02T10:00:00Z\",\"by\":\"Ann Owner <ann@example.org>\",\"by_name\":\"Ann Owner\",\
         \"commit\":\"{applied}\",\"proposal\":\"PR-0001\",\"of_file\":false}}}},\
         {{\"id\":\"SCN-STORM\",\"kind\":\"scene\",\"title\":\"The storm\",\"path\":\"{STORM}\",\
         \"line\":1,\"rev\":1,\"status\":null,\"archived\":false,\"state\":\"no card\",\
         \"open_items\":0,\"tasks\":0,\"tasks_open\":0,\"last_change\":{{\"at\":\
         \"2026-01-03T10:00:00Z\",\"by\":\"Bob Hand <bob@example.org>\",\"by_name\":\"Bob Hand\",\
         \"commit\":\"{hand}\",\"proposal\":null,\"of_file\":false}}}}],\
         \"truncated\":false,\"omitted\":null}}\n"
    )
}

/// `spec: apply <id>` as an apply writes it (core's `commit_message_raising`
/// with no interpretation and no revision trailer): the rationale, then the
/// trailer paragraph.
pub fn apply_message(id: &str, rationale: &str, decided_by: Ident, base: &str) -> String {
    format!(
        "spec: apply {id}\n\n{rationale}\n\nProposal: {id}\nDecided-by: {}\nProposed-by: agent \
         role=spec-writer model=m run=r\nBase-commit: {base}\n",
        decided_by.value()
    )
}

/// git's date of a UTC stamp `YYYY-MM-DDTHH:MM:SSZ`.
pub fn git_date(utc: &str) -> String {
    format!("{}+0000", utc.trim_end_matches('Z'))
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// The scratch repository and its `HOME`, removed on drop.
pub struct Lamp {
    dir: PathBuf,
    pub git: Sandbox,
    pub root: PathBuf,
    pub home: PathBuf,
    /// The fixture's commit.
    pub first: String,
    messages: Cell<usize>,
}

impl Drop for Lamp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// The pinned copy's two commits on top of the fixture.
pub struct Pinned {
    pub copy: Lamp,
    /// Ann's apply of PR-0001.
    pub applied: String,
    /// Bob's hand edit.
    pub hand: String,
}

/// `from` and every directory under it copied to `to`.
pub fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture").flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).expect("a copied file");
        }
    }
}

/// `fixtures/<name>` of this repository.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

impl Lamp {
    /// `fixtures/studio-lamp` copied, `git init`, committed as the sandbox's
    /// identity; no queue.
    pub fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-dc-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        let dir = fs::canonicalize(&dir).expect("canonical");
        let git = Sandbox::new(&dir);
        let root = dir.join("repo");
        copy_tree(&fixture(SLUG), &root);
        git.init(&root);
        let root = fs::canonicalize(&root).expect("canonical");
        git.add_all(&root);
        git.commit(&root, "the fixture");
        let first = git.git_text(&root, &["rev-parse", "HEAD"]);
        let home = dir.join("h");
        fs::create_dir_all(&home).expect("a home");
        Self {
            dir,
            git,
            root,
            home,
            first,
            messages: Cell::new(0),
        }
    }

    /// The scratch directory every repository of the test lives in.
    pub fn scratch(&self) -> &Path {
        &self.dir
    }

    /// The copy with the spec's two commits, no queue yet.
    pub fn committed(label: &str) -> Pinned {
        let copy = Self::new(label);
        copy.edit(
            ARRIVAL,
            "The boat runs aground on the shingle.",
            "The boat runs aground on the wet shingle.",
        );
        let applied = copy.commit(
            ANN,
            ANN,
            "2026-01-02T09:00:00Z",
            "2026-01-02T10:00:00Z",
            &apply_message("PR-0001", "Name the shingle wet.", ANN, &copy.first),
        );
        copy.edit(
            STORM,
            "Rain puts out the keeper's lantern.",
            "Rain puts out the keeper's lantern at once.",
        );
        let hand = copy.commit(
            BOB,
            BOB,
            "2026-01-03T09:00:00Z",
            "2026-01-03T10:00:00Z",
            "Reword the storm\n",
        );
        Pinned {
            copy,
            applied,
            hand,
        }
    }

    /// The whole of the spec's copy: its commits, C-0001 open on
    /// `SCN-ARRIVAL` and PR-0040 an open question naming it.
    pub fn pinned(label: &str) -> Pinned {
        let pinned = Self::committed(label);
        let copy = &pinned.copy;
        copy.number_items_from_40();
        let card = copy.card(
            "Stage the arrival",
            &["SCN-ARRIVAL"],
            "2026-01-04T10:00:00Z",
        );
        assert_eq!(card, "C-0001");
        let asked = copy.question(
            &["SCN-ARRIVAL"],
            "Is the brass key on the boat?",
            "2026-01-04T11:00:00Z",
        );
        assert_eq!(asked, "PR-0040");
        pinned
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn put(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).expect("a readable file")
    }

    /// `relative` with its one `from` replaced by `to`.
    pub fn edit(&self, relative: &str, from: &str, to: &str) {
        let text = self.read(relative);
        assert_eq!(text.matches(from).count(), 1, "{relative}: {from:?}");
        self.put(relative, &text.replacen(from, to, 1));
    }

    /// `git add -A` and a commit of `message` (verbatim) by `author` at
    /// `authored`, committed by `committer` at `committed`: its OID.
    pub fn commit(
        &self,
        author: Ident,
        committer: Ident,
        authored: &str,
        committed: &str,
        message: &str,
    ) -> String {
        self.git.add_all(&self.root);
        self.commit_staged(author, committer, authored, committed, message, &[])
    }

    /// A commit of what is staged, with `extra` arguments (`--allow-empty`).
    pub fn commit_staged(
        &self,
        author: Ident,
        committer: Ident,
        authored: &str,
        committed: &str,
        message: &str,
        extra: &[&str],
    ) -> String {
        let n = self.messages.get();
        self.messages.set(n + 1);
        let file = self.dir.join(format!("message-{n}.txt"));
        fs::write(&file, message).expect("a message file");
        let env = self.stamp(author, committer, authored, committed);
        let pairs: Vec<(&str, &OsStr)> = env
            .iter()
            .map(|(name, value)| (*name, value.as_os_str()))
            .collect();
        let mut args = vec![
            "commit",
            "-q",
            "--no-verify",
            "--cleanup=verbatim",
            "-F",
            file.to_str().expect("a UTF-8 scratch path"),
        ];
        args.extend_from_slice(extra);
        self.git.git_env(&self.root, &args, &pairs);
        self.git.git_text(&self.root, &["rev-parse", "HEAD"])
    }

    /// The six identity variables of one commit.
    pub fn stamp(
        &self,
        author: Ident,
        committer: Ident,
        authored: &str,
        committed: &str,
    ) -> Vec<(&'static str, OsString)> {
        vec![
            ("GIT_AUTHOR_NAME", OsString::from(author.name)),
            ("GIT_AUTHOR_EMAIL", OsString::from(author.email)),
            ("GIT_AUTHOR_DATE", OsString::from(git_date(authored))),
            ("GIT_COMMITTER_NAME", OsString::from(committer.name)),
            ("GIT_COMMITTER_EMAIL", OsString::from(committer.email)),
            ("GIT_COMMITTER_DATE", OsString::from(git_date(committed))),
        ]
    }

    /// `git args` in the root with `stamp`'s identity.
    pub fn git_as(&self, stamp: &[(&'static str, OsString)], args: &[&str]) -> String {
        let pairs: Vec<(&str, &OsStr)> = stamp
            .iter()
            .map(|(name, value)| (*name, value.as_os_str()))
            .collect();
        String::from_utf8(self.git.git_env(&self.root, args, &pairs))
            .expect("UTF-8")
            .trim()
            .to_owned()
    }

    /// The library's environment in the root.
    pub fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    /// The caller's git environment: the sandbox's.
    pub fn git_env(&self) -> GitEnv {
        GitEnv::new(self.root.clone(), self.git.vars())
    }

    /// The variables a `spec` process runs with: the sandbox's, `HOME` the
    /// copy's, then `extra` over them.
    pub fn vars(&self, extra: &[(&str, &OsStr)]) -> Vec<(String, OsString)> {
        let mut vars: Vec<(String, OsString)> = self
            .git
            .vars()
            .into_iter()
            .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
            .filter(|(name, _)| name != "HOME")
            .collect();
        vars.push(("HOME".to_owned(), self.home.as_os_str().to_owned()));
        for (name, value) in extra {
            vars.retain(|(known, _)| known != name);
            vars.push(((*name).to_owned(), value.to_os_string()));
        }
        vars
    }

    /// A card in the narrative flow on `targets`, `Ann` its reviewer, made
    /// at `now`: its ID.
    pub fn card(&self, title: &str, targets: &[&str], now: &str) -> String {
        let outcome = card_new(
            &self.env(),
            &Globals::default(),
            &CardNewRequest {
                title: title.to_owned(),
                discipline: Some("narrative".to_owned()),
                targets: targets.iter().map(|id| (*id).to_owned()).collect(),
                reviewer: Some("Ann".to_owned()),
                now: now.to_owned(),
                ..CardNewRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("card new {title}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.id.expect("a card's ID")
    }

    /// `card move id --to column` at `now`.
    pub fn move_to(&self, id: &str, column: &str, now: &str) {
        let outcome = card_move(
            &self.env(),
            &Globals::default(),
            &CardMoveRequest {
                id: id.to_owned(),
                to: Some(column.to_owned()),
                now: now.to_owned(),
                ..CardMoveRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("move {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "move {id}: {outcome:?}");
    }

    /// The card moved to the narrative flow's `ready-for-dev`, then
    /// accepted at `now`.
    pub fn accept(&self, id: &str, now: &str) {
        self.move_to(id, "ready-for-dev", now);
        let mut yes = |_: &str| true;
        let outcome = card_accept(
            &self.env(),
            &Globals::default(),
            &CardAcceptRequest {
                id: id.to_owned(),
                actor: "Ann".to_owned(),
                briefed: true,
                now: now.to_owned(),
                ..CardAcceptRequest::default()
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("accept {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "accept {id}: {outcome:?}");
    }

    /// `card cancel id --reason reason` at `now`.
    pub fn cancel(&self, id: &str, reason: &str, now: &str) {
        let outcome = card_cancel(
            &self.env(),
            &Globals::default(),
            &CardCancelRequest {
                id: id.to_owned(),
                reason: reason.to_owned(),
                actor: Some("Ann".to_owned()),
                now: now.to_owned(),
                ..CardCancelRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("cancel {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "cancel {id}: {outcome:?}");
    }

    /// A question naming `nodes`, raised in `cwd` (a worktree of some
    /// repository) under the copy's `HOME`: its ID.
    pub fn question_in(&self, cwd: &Path, nodes: &[&str], text: &str, now: &str) -> String {
        let request = QuestionRequest {
            node_ids: nodes.iter().map(|id| (*id).to_owned()).collect(),
            text: text.to_owned(),
            working_answer: "Yes, under the thwart.".to_owned(),
            price_of_other: "A scene rewritten.".to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            card: None,
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: now.to_owned(),
            git: GitEnv::new(cwd.to_path_buf(), self.git.vars()),
            gist: None,
            matters: None,
            options: None,
            recommendation: None,
        };
        let env = Env {
            cwd: cwd.to_path_buf(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        };
        let outcome = propose_question(&env, &Globals::default(), &request)
            .unwrap_or_else(|error| panic!("question {text}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        assert!(outcome.document.created, "not stored: {outcome:?}");
        outcome.document.id.clone().expect("an ID")
    }

    /// A question naming `nodes`, raised in the root: its ID.
    pub fn question(&self, nodes: &[&str], text: &str, now: &str) -> String {
        self.question_in(&self.root.clone(), nodes, text, now)
    }

    /// The next item raised is `PR-0040`: one question on the chapter (no
    /// scene row), renumbered `PR-0039`, its events dropped.
    pub fn number_items_from_40(&self) {
        let first = self.question(
            &["CHP-SHORE"],
            "Does the tide turn twice a day?",
            "2026-01-04T09:00:00Z",
        );
        assert_eq!(first, "PR-0001");
        self.sql("UPDATE proposals SET id = 'PR-0039' WHERE id = 'PR-0001'; DELETE FROM events;");
    }

    /// The queue's file under the copy's `HOME`.
    pub fn db(&self) -> PathBuf {
        db_under(&self.home)
    }

    pub fn sql(&self, sql: &str) -> String {
        sql_at(&self.db(), sql)
    }
}

/// The queue's file under `home`, as the host rule places it.
pub fn db_under(home: &Path) -> PathBuf {
    let data = if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("specengine")
    } else {
        home.join(".local").join("share").join("specengine")
    };
    data.join(format!("{SLUG}.db"))
}

/// `sqlite3 db sql`, which must answer.
pub fn sql_at(db: &Path, sql: &str) -> String {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(client)
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
