//! docs/features/node-history.md (RM-0035) "Acceptance criteria", the CLI
//! half: the `spec` binary over scratch git repositories built at run time
//! (an invented rules book, `docs/rules.md` holding `{#R-12}` and
//! `{#R-13}`), every commit with an explicit author and committer (name,
//! email, date), every apply message made by core's
//! `commit_message_raising` as an apply writes it, and one apply (with
//! the queue's items and cards) run through the library's `approve` at
//! fixed clocks.
//!
//! - NH1: `spec show R-12 --history` lists exactly the commits that
//!   changed R-12's span, newest first, with the table's keys; the text
//!   block is the spec's, byte for byte; the document lists every commit of
//!   its file; a copy at another root and `HOME` reads byte-equal. M: every
//!   commit of the file listed.
//! - NH2: the trailer block is the last paragraph of a `spec: apply <ID>`
//!   commit: a `Proposal:` line in the rationale never counts; a
//!   `Co-Authored-By:` paragraph, an apply subject with no `Proposal:` and
//!   one whose `Proposal:` names another ID are hand commits. M: every
//!   body line scanned. (The unit half is `specengine-core`'s
//!   `tests/node_history.rs`.)
//! - NH3: after `git mv` and one more edit, the history holds that edit and
//!   all of NH1's. M: `--follow` dropped.
//! - NH4: not asked, no `git log` and no `git cat-file` run (a `git` shim
//!   logging its arguments). M: the history always read.
//! - NH5: a `--depth 1` clone, no `git` on `PATH`, no repository: exit 0,
//!   the text, `history: null`, one `note: history:` line. M: git's error
//!   passed through.
//! - NH8: cards accepted on the node and items applied on it join, placed
//!   by `at`, ties commits, cards, items; open items, unaccepted cards and
//!   another node's cards stay out; an item listed as a commit's
//!   `Proposal:` is not listed again; a queue that cannot be read is a
//!   note. M: open items taken; an unaccepted card taken; the dedup
//!   dropped.
//! - NH9: the entry cap (64), the scan (128 commits, 129 read) and the
//!   stop at the node's creation (a `cat-file` stdin log as the seam). M:
//!   no entry cap; an unbounded scan; no stop at creation.
//! - NH10: the text cap: the node's text is cut first; a history over the
//!   cap is cut after a whole entry, the tail ending `; history not shown:
//!   <k>]`, the JSON holding the printed entries and `omitted` k. M: a cut
//!   inside an entry.
//!
//! NH6 and NH11 are `specengine-mcp`'s `tests/node_history.rs`; NH12 and
//! NH10's daemon half `specengine-http`'s `tests/node_history.rs`. Every
//! expected value is worked out from the spec and the commits made here,
//! never read back from a run of the read under test.

#![cfg(unix)]

mod common;

use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use common::git::Sandbox;
use common::proposal::sqlite3;
use common::{Run, Scratch, copy_dir, data_dir, read_text, spec_with, write};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, CardAcceptRequest, CardAssignRequest, CardMoveRequest,
    CardNewRequest, DecisionInput, DecisionRequest, Env, Exit, Globals, IntakeOutcome,
    ProposeRequest, ProposedText, QuestionRequest, RejectRequest, approve, approve_with,
    card_accept, card_assign, card_move, card_new, propose, propose_decision, propose_question,
    reject,
};
use specengine_core::proposal::{
    ApplyTrailers, Author, AuthorType, CommitFacts, Revised, commit_message_raising,
};
use specengine_store::{GitEnv, ProposalQueue as _, SqliteQueue};

// ------------------------------------------------------------- the scene

/// The project's slug.
const SLUG: &str = "quill-rules";

/// The holder of both sections.
const RULES: &str = "docs/rules.md";

/// `HISTORY_MAX`, `HISTORY_SCAN_MAX`, `HISTORY_TEXT_MAX` as the spec fixes
/// them (NH9, NH10): the CLI's own numbers are checked against these.
const HISTORY_MAX: usize = 64;
const HISTORY_SCAN_MAX: usize = 128;
const HISTORY_TEXT_MAX: usize = 300;

/// The read's character ceiling (`docs/canon/spec-cli-read.md` "The cap").
const CAP: usize = 40_000;

/// The config every scene commits first: `docs` walked, `R` and `DEC`,
/// decision records under `docs/records/DEC`.
const CONFIG: &str = "[project]\nslug = \"quill-rules\"\n\n[paths]\nroots = [\"docs\"]\n\n\
                      [ids]\nR   = { kind = \"requirement\", width = 2 }\n\
                      DEC = { kind = \"decision\",    width = 4 }\n\n\
                      [decision_records]\nprefix   = \"DEC\"\ndir      = \"docs/records/DEC\"\n\
                      template = \"templates/decision.md\"\n";

/// A record template outside the walk.
const TEMPLATE: &str = "---\nid: {{id}}\nclass: decision\nstatus: {{status}}\ndate: {{date}}\n\
                        title: {{title}}\ncanon: {{canon}}\nlinks:\n  answers: {{targets}}\n\
                        ref: {{proposal}}\nscope: [decisions]\n---\n\n# {{title}}\n\nChoice:\n\n\
                        {{choice}}\n\nEffect:\n\n{{effect}}\n\nCost:\n\n{{cost}}\n\nSummary:\n\n\
                        {{summary}}\n\nOptions:\n\n{{options}}\n\nEvidence:\n\n{{evidence}}\n\n\
                        Note:\n\n{{note}}\n\nDecided by:\n\n{{decided_by}}\n";

/// A person as git and the trailers write one.
#[derive(Clone, Copy)]
struct Ident {
    name: &'static str,
    email: &'static str,
}

impl Ident {
    fn value(self) -> String {
        format!("{} <{}>", self.name, self.email)
    }
}

/// The hand author of NH1.
const BOB: Ident = Ident {
    name: "Bob Hand",
    email: "bob@example.org",
};
/// The committer of hand commits: never their `by`.
const DANA: Ident = Ident {
    name: "Dana Desk",
    email: "dana@example.org",
};
/// The owner who decides: the apply commits' `Decided-by:`.
const ANN: Ident = Ident {
    name: "Ann Owner",
    email: "ann@example.org",
};
/// The git author and committer of NH1's apply commits: never their `by`
/// (that is `Decided-by:`).
const CLEO: Ident = Ident {
    name: "Cleo Commit",
    email: "cleo@example.org",
};

/// The agent who proposed NH1's applies.
fn writer() -> Author {
    Author {
        author_type: AuthorType::Agent,
        role: Some("spec-writer".to_owned()),
        model: Some("m".to_owned()),
        run: Some("r".to_owned()),
        login: None,
    }
}

/// `Proposed-by:` of [`writer`], and its `proposer`.
const PROPOSED_BY: &str = "agent role=spec-writer model=m run=r";
const PROPOSER: &str = "agent spec-writer";

/// Who wrote and committed one commit, and when (git's date format).
#[derive(Clone, Copy)]
struct Stamp {
    author: Ident,
    committer: Ident,
    authored: &'static str,
    committed: &'static str,
}

/// `docs/rules.md` with R-12's and R-13's sections as given (heading line
/// and body, no trailing newline).
fn rules(window: &str, door: &str) -> String {
    format!("# Rules\n\nThe house rules.\n\n{window}\n\n{door}\n")
}

/// A scratch git repository built here, its `HOME` for the data
/// directory.
struct Repo {
    scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
    messages: Cell<usize>,
}

impl Repo {
    /// An empty repository on `main` with the config and the template in
    /// its tree (not committed).
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(&format!("nh-{label}"));
        let git = Sandbox::new(scratch.path());
        let root = scratch.dir("repo");
        git.init(&root);
        write(&root, "specengine.toml", CONFIG);
        write(&root, "templates/decision.md", TEMPLATE);
        let home = scratch.home("h");
        Self {
            scratch,
            git,
            root,
            home,
            messages: Cell::new(0),
        }
    }

    /// `git add -A` and a commit of `message` (verbatim) as `stamp` says:
    /// the new commit's OID.
    fn commit(&self, stamp: Stamp, message: &str) -> String {
        self.commit_in(&self.root, stamp, message)
    }

    fn commit_in(&self, dir: &Path, stamp: Stamp, message: &str) -> String {
        let n = self.messages.get();
        self.messages.set(n + 1);
        let file = self.scratch.join(&format!("message-{n}.txt"));
        fs::write(&file, message).expect("message file");
        self.git.add_all(dir);
        let env = [
            ("GIT_AUTHOR_NAME", OsString::from(stamp.author.name)),
            ("GIT_AUTHOR_EMAIL", OsString::from(stamp.author.email)),
            ("GIT_AUTHOR_DATE", OsString::from(stamp.authored)),
            ("GIT_COMMITTER_NAME", OsString::from(stamp.committer.name)),
            ("GIT_COMMITTER_EMAIL", OsString::from(stamp.committer.email)),
            ("GIT_COMMITTER_DATE", OsString::from(stamp.committed)),
        ];
        let extra: Vec<(&str, &OsStr)> = env
            .iter()
            .map(|(name, value)| (*name, value.as_os_str()))
            .collect();
        self.git.git_env(
            dir,
            &[
                "commit",
                "-q",
                "--no-verify",
                "--cleanup=verbatim",
                "-F",
                file.to_str().expect("a UTF-8 scratch path"),
            ],
            &extra,
        );
        self.git.git_text(dir, &["rev-parse", "HEAD"])
    }

    /// The variables `spec` runs with: the sandbox's, `HOME` = `home`,
    /// then `extra` over them.
    fn vars(&self, home: &Path, extra: &[(&str, &OsStr)]) -> Vec<(String, OsString)> {
        let mut vars: Vec<(String, OsString)> = self
            .git
            .vars()
            .into_iter()
            .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
            .filter(|(name, _)| name != "HOME")
            .collect();
        vars.push(("HOME".to_owned(), home.as_os_str().to_owned()));
        for (name, value) in extra {
            vars.retain(|(known, _)| known != name);
            vars.push(((*name).to_owned(), value.to_os_string()));
        }
        vars
    }

    /// `spec args` in `cwd` with `home` and `extra`.
    fn spec_at(&self, cwd: &Path, home: &Path, args: &[&str], extra: &[(&str, &OsStr)]) -> Run {
        let vars = self.vars(home, extra);
        let env: Vec<(&str, &OsStr)> = vars
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_os_str()))
            .collect();
        spec_with(cwd, args, &env)
    }

    /// `spec args` in the root.
    fn spec(&self, args: &[&str]) -> Run {
        self.spec_at(&self.root, &self.home, args, &[])
    }

    /// `spec show <reference> --history`: the text run and the JSON
    /// document, each exit 0.
    fn history(&self, reference: &str) -> (Run, Value) {
        self.history_at(&self.root, &self.home, reference, &[])
    }

    fn history_at(
        &self,
        cwd: &Path,
        home: &Path,
        reference: &str,
        extra: &[(&str, &OsStr)],
    ) -> (Run, Value) {
        let text = self.spec_at(cwd, home, &["show", reference, "--history"], extra);
        text.code(0);
        let json = self.spec_at(
            cwd,
            home,
            &["--json", "show", reference, "--history"],
            extra,
        );
        json.code(0);
        (text, json.json())
    }

    /// The library's environment in the root.
    fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    /// The caller's git environment: the sandbox's, `extra` over it.
    fn git_env(&self, extra: &[(&str, &str)]) -> GitEnv {
        let mut env = GitEnv::new(self.root.clone(), self.git.vars());
        for (name, value) in extra {
            env = env.with_var(OsString::from(name), OsString::from(value));
        }
        env
    }

    /// A git environment committing as `who` at `date` (both author and
    /// committer).
    fn deciding(&self, who: Ident, date: &str) -> GitEnv {
        self.git_env(&[
            ("GIT_AUTHOR_NAME", who.name),
            ("GIT_AUTHOR_EMAIL", who.email),
            ("GIT_AUTHOR_DATE", date),
            ("GIT_COMMITTER_NAME", who.name),
            ("GIT_COMMITTER_EMAIL", who.email),
            ("GIT_COMMITTER_DATE", date),
        ])
    }

    fn queue(&self) -> SqliteQueue {
        SqliteQueue::open(data_dir(&self.home).join(format!("{SLUG}.db")), SLUG)
            .expect("the queue opens")
    }

    fn db(&self) -> PathBuf {
        data_dir(&self.home).join(format!("{SLUG}.db"))
    }
}

/// The entries of `node`'s history (`nodes[0]` unless `at` says).
fn entries(document: &Value) -> Vec<Value> {
    document["nodes"][0]["history"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("history.entries: {document}"))
        .clone()
}

/// Each entry's `id`.
fn ids(document: &Value) -> Vec<String> {
    entries(document)
        .iter()
        .map(|entry| entry["id"].as_str().expect("an id").to_owned())
        .collect()
}

/// Each entry's `change`.
fn changes(document: &Value) -> Vec<String> {
    entries(document)
        .iter()
        .map(|entry| entry["change"].as_str().expect("a change").to_owned())
        .collect()
}

/// The 15 keys of an entry, in the table's order (NH1, NH12).
const ENTRY_KEYS: [&str; 15] = [
    "source",
    "at",
    "id",
    "change",
    "created",
    "rev",
    "rev_before",
    "by",
    "by_name",
    "proposal",
    "proposed_by",
    "proposer",
    "record",
    "title",
    "rationale",
];

/// A commit entry's facts.
struct Commit<'a> {
    at: &'a str,
    oid: &'a str,
    change: &'a str,
    created: bool,
    rev: Option<u32>,
    rev_before: Option<u32>,
    by: Ident,
    /// The `Proposal:` of an apply; `None` a hand commit.
    apply: Option<&'a str>,
    /// An apply's rationale, else a hand commit's subject.
    text: &'a str,
}

/// A commit entry as the table builds it.
fn commit_entry(commit: &Commit<'_>) -> Value {
    let apply = commit.apply;
    json!({
        "source": "commit",
        "at": commit.at,
        "id": commit.oid,
        "change": commit.change,
        "created": commit.created,
        "rev": commit.rev,
        "rev_before": commit.rev_before,
        "by": commit.by.value(),
        "by_name": commit.by.name,
        "proposal": apply,
        "proposed_by": apply.map(|_| PROPOSED_BY),
        "proposer": apply.map(|_| PROPOSER),
        "record": null,
        "title": if apply.is_none() { json!(commit.text) } else { json!(null) },
        "rationale": if apply.is_some() { json!(commit.text) } else { json!(null) },
    })
}

/// Every entry's keys are exactly the 15, in order.
fn assert_keys(document: &Value, context: &str) {
    let history = &document["nodes"][0]["history"];
    let keys: Vec<&str> = history
        .as_object()
        .unwrap_or_else(|| panic!("{context}: history is an object: {document}"))
        .keys()
        .map(String::as_str)
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, ["entries", "omitted", "truncated"], "{context}");
    for entry in entries(document) {
        let keys: Vec<&str> = entry
            .as_object()
            .expect("an entry object")
            .keys()
            .map(String::as_str)
            .collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort_unstable();
        let mut expected = ENTRY_KEYS.to_vec();
        expected.sort_unstable();
        assert_eq!(sorted_keys, expected, "{context}: {entry}");
    }
}

/// A `spec: apply` message as an apply writes it.
fn apply_message(
    id: &str,
    rationale: &str,
    decided_by: Ident,
    base: &str,
    revised: &[Revised<'_>],
) -> String {
    let author = writer();
    let decided = decided_by.value();
    commit_message_raising(
        &CommitFacts {
            id,
            rationale,
            decided_by: &decided,
            author: &author,
            base_commit: base,
        },
        None,
        &ApplyTrailers {
            revised,
            locked: &[],
        },
    )
}

// ---------------------------------------------------------------- NH1

/// R-12's section at each of NH1's steps.
const WINDOW_A: &str = "## Window {#R-12}\n\nThe window opens at noon.";
const WINDOW_B: &str = "## Window {#R-12 rev=2}\n\nThe window opens at noon for ten minutes.";
const WINDOW_C: &str = "## Window {#R-12 rev=2}\n\nThe window opens at noon, for ten minutes.";
const WINDOW_D: &str = "## Window {#R-12 rev=2}\n\nThe window opens at noon, for ten minutes only.";
const DOOR_A: &str = "## Door {#R-13}\n\nThe door shuts at dusk.";
const DOOR_E: &str = "## Door {#R-13}\n\nThe door shuts at dusk sharp.";

/// NH1's five commits.
struct Book {
    repo: Repo,
    a: String,
    b: String,
    c: String,
    d: String,
    e: String,
}

/// NH1's repository: A by hand creating both, B PR-0001 raising R-12
/// 1 -> 2, C PR-0002 editorial on R-12, D by hand on R-12, E by hand on
/// R-13 alone. Committer times are 10:00 UTC on 2026-01-01 to -05, written
/// in other offsets where it shows; author times are other days.
fn book(label: &str) -> Book {
    let repo = Repo::new(label);
    write(&repo.root, RULES, rules(WINDOW_A, DOOR_A));
    let a = repo.commit(
        Stamp {
            author: BOB,
            committer: DANA,
            authored: "2025-12-20T09:00:00+0000",
            committed: "2026-01-01T10:00:00+0000",
        },
        "Add the rules\n\nThe window and the door.\n",
    );
    write(&repo.root, RULES, rules(WINDOW_B, DOOR_A));
    let b = repo.commit(
        Stamp {
            author: CLEO,
            committer: CLEO,
            authored: "2025-12-21T09:00:00+0000",
            committed: "2026-01-02T13:00:00+0300",
        },
        &apply_message(
            "PR-0001",
            "Tighten the window.",
            ANN,
            &a,
            &[Revised {
                id: "R-12",
                rev: 1,
                to: Some(2),
            }],
        ),
    );
    write(&repo.root, RULES, rules(WINDOW_C, DOOR_A));
    let c = repo.commit(
        Stamp {
            author: CLEO,
            committer: CLEO,
            authored: "2025-12-22T09:00:00+0000",
            committed: "2026-01-03T05:00:00-0500",
        },
        &apply_message(
            "PR-0002",
            "Wording only.",
            ANN,
            &b,
            &[Revised {
                id: "R-12",
                rev: 2,
                to: None,
            }],
        ),
    );
    write(&repo.root, RULES, rules(WINDOW_D, DOOR_A));
    let d = repo.commit(
        Stamp {
            author: BOB,
            committer: DANA,
            authored: "2025-12-23T09:00:00+0000",
            committed: "2026-01-04T10:00:00+0000",
        },
        "Fix a typo in the window rule\n",
    );
    write(&repo.root, RULES, rules(WINDOW_D, DOOR_E));
    let e = repo.commit(
        Stamp {
            author: BOB,
            committer: DANA,
            authored: "2025-12-24T09:00:00+0000",
            committed: "2026-01-05T10:00:00+0000",
        },
        "Shorten the door rule\n",
    );
    Book {
        repo,
        a,
        b,
        c,
        d,
        e,
    }
}

/// NH1's entries of R-12, newest first.
fn book_entries(book: &Book) -> Vec<Value> {
    vec![
        commit_entry(&Commit {
            at: "2026-01-04T10:00:00Z",
            oid: &book.d,
            change: "hand",
            created: false,
            rev: Some(2),
            rev_before: Some(2),
            by: BOB,
            apply: None,
            text: "Fix a typo in the window rule",
        }),
        commit_entry(&Commit {
            at: "2026-01-03T10:00:00Z",
            oid: &book.c,
            change: "editorial",
            created: false,
            rev: Some(2),
            rev_before: Some(2),
            by: ANN,
            apply: Some("PR-0002"),
            text: "Wording only.",
        }),
        commit_entry(&Commit {
            at: "2026-01-02T10:00:00Z",
            oid: &book.b,
            change: "raised",
            created: false,
            rev: Some(2),
            rev_before: Some(1),
            by: ANN,
            apply: Some("PR-0001"),
            text: "Tighten the window.",
        }),
        commit_entry(&Commit {
            at: "2026-01-01T10:00:00Z",
            oid: &book.a,
            change: "hand",
            created: true,
            rev: Some(1),
            rev_before: None,
            by: BOB,
            apply: None,
            text: "Add the rules",
        }),
    ]
}

/// NH1's text block for its repository, with its real OIDs.
fn book_block(book: &Book) -> String {
    let short = |oid: &str| oid[..12].to_owned();
    format!(
        "  2026-01-04T10:00:00Z | hand | rev 2 | - | Bob Hand <bob@example.org> | - | {d}\n    \
         Fix a typo in the window rule\n  2026-01-03T10:00:00Z | editorial | rev 2 | PR-0002 | Ann \
         Owner <ann@example.org> | agent role=spec-writer model=m run=r | {c}\n    Wording \
         only.\n  2026-01-02T10:00:00Z | raised | rev 1 -> 2 | PR-0001 | Ann Owner \
         <ann@example.org> | agent role=spec-writer model=m run=r | {b}\n    Tighten the \
         window.\n  2026-01-01T10:00:00Z | hand created | rev 1 | - | Bob Hand <bob@example.org> \
         | - | {a}\n    Add the rules\n  history 4\n",
        d = short(&book.d),
        c = short(&book.c),
        b = short(&book.b),
        a = short(&book.a),
    )
}

/// The text of `spec show REF` (not asked) with `block` after its header
/// line: what `--history` must print.
fn with_block(plain: &str, block: &str) -> String {
    let (header, rest) = plain
        .split_once('\n')
        .unwrap_or_else(|| panic!("a header line: {plain}"));
    format!("{header}\n{block}{rest}")
}

/// NH1: exactly D, C, B, A for R-12, with the table's keys and values;
/// the spec's text block byte for byte; R-13 lists E and A; the document lists
/// E too; a copy at another root and `HOME` reads byte-equal.
#[test]
fn nh1_only_commits_that_changed_the_span() {
    let book = book("nh1");
    let repo = &book.repo;
    let (text, document) = repo.history("R-12");
    assert_eq!(text.stderr, "", "no note: {}", text.show());
    assert_keys(&document, "NH1");
    assert_eq!(
        document["nodes"][0]["history"],
        json!({"entries": book_entries(&book), "truncated": false, "omitted": 0}),
        "NH1's four entries: {document:#}"
    );
    assert_eq!(document["notes"], json!([]), "no note");

    let plain = repo.spec(&["show", "R-12"]);
    plain.code(0);
    assert_eq!(
        text.stdout,
        with_block(&plain.stdout, &book_block(&book)),
        "\"Text\"'s block after the header line, before the node's text"
    );

    // R-13: its creation and E alone.
    let (_, door) = repo.history("R-13");
    assert_eq!(ids(&door), [book.e.clone(), book.a.clone()], "{door:#}");
    assert_eq!(changes(&door), ["hand", "hand"]);
    assert_eq!(entries(&door)[1]["created"], json!(true));

    // The document: its span is its file, so every commit of it.
    let (whole_text, whole) = repo.history(RULES);
    assert_eq!(
        ids(&whole),
        [book.e.as_str(), &book.d, &book.c, &book.b, &book.a],
        "{whole:#}"
    );
    assert_eq!(
        changes(&whole),
        ["hand", "hand", "applied", "applied", "hand"],
        "no `Revision:` names the document: `applied`"
    );
    let created: Vec<bool> = entries(&whole)
        .iter()
        .map(|entry| entry["created"].as_bool().expect("created"))
        .collect();
    assert_eq!(created, [false, false, false, false, true]);
    for entry in entries(&whole) {
        assert_eq!(entry["rev"], json!(1), "none written is 1: {entry}");
    }
    assert!(
        whole_text.stdout.contains("\n  history 5\n"),
        "{}",
        whole_text.show()
    );

    // Determinism: a copy at another root, read with another `HOME`.
    let copy = repo.scratch.join("copy");
    copy_dir(&repo.root, &copy);
    let other_home = repo.scratch.home("other");
    let (again_text, again) = repo.history_at(&copy, &other_home, "R-12", &[]);
    assert_eq!(again_text.stdout, text.stdout, "byte-equal text");
    assert_eq!(again_text.stderr, text.stderr);
    let json_again = repo.spec_at(
        &copy,
        &other_home,
        &["--json", "show", "R-12", "--history"],
        &[],
    );
    let json_first = repo.spec(&["--json", "show", "R-12", "--history"]);
    assert_eq!(json_again.stdout, json_first.stdout, "byte-equal JSON");
    assert!(
        json_first
            .stdout
            .contains("\"bindings\":null,\"history\":{\"entries\":[{\"source\":\"commit\",\"at\":"),
        "`history` the last key, after `bindings`; `entries` first, `source` first: {}",
        json_first.stdout
    );
    assert!(
        json_first
            .stdout
            .contains("\"rationale\":null}],\"truncated\":false,\"omitted\":0}}]}"),
        "`rationale` an entry's last key; `truncated`, `omitted` after `entries`"
    );
    assert_eq!(again, document);
}

/// "Writes": a history read writes no queue row, no event and nothing
/// under the root.
#[test]
fn nh1_a_history_read_writes_nothing() {
    let book = book("nh1-writes");
    let repo = &book.repo;
    repo.spec(&["index"]).code(0);
    let before = common::snapshot(&repo.root);
    let events = repo.queue().events().expect("events").len();
    repo.history("R-12");
    repo.history(RULES);
    assert_eq!(
        common::snapshot(&repo.root),
        before,
        "nothing under the root"
    );
    assert_eq!(
        repo.queue().events().expect("events").len(),
        events,
        "no event"
    );
}

// ---------------------------------------------------------------- NH2

/// NH2's commits on R-12, newest first, each with its expected `change`
/// and `proposal`.
#[test]
fn nh2_the_trailer_block_is_the_last_paragraph() {
    let repo = Repo::new("nh2");
    let hand = |authored: &'static str, committed: &'static str| Stamp {
        author: BOB,
        committer: DANA,
        authored,
        committed,
    };
    write(&repo.root, RULES, rules(WINDOW_A, DOOR_A));
    let a = repo.commit(
        hand("2025-12-20T09:00:00+0000", "2026-01-01T10:00:00+0000"),
        "Add the rules\n",
    );
    // B: the rationale holds a `Proposal:` line of its own.
    write(&repo.root, RULES, rules(WINDOW_B, DOOR_A));
    let b = repo.commit(
        Stamp {
            author: CLEO,
            committer: CLEO,
            authored: "2025-12-21T09:00:00+0000",
            committed: "2026-01-02T10:00:00+0000",
        },
        &apply_message(
            "PR-0001",
            "Tighten the window.\n\nProposal: PR-9999",
            ANN,
            &a,
            &[Revised {
                id: "R-12",
                rev: 1,
                to: Some(2),
            }],
        ),
    );
    // A hand commit ending in a `Co-Authored-By:` paragraph.
    write(&repo.root, RULES, rules(WINDOW_C, DOOR_A));
    let co = repo.commit(
        hand("2025-12-22T09:00:00+0000", "2026-01-03T10:00:00+0000"),
        "Tidy the window rule\n\nCo-Authored-By: Dee Pair <dee@example.org>\n",
    );
    // An apply subject whose `Proposal:` stands in a middle paragraph.
    write(&repo.root, RULES, rules(WINDOW_D, DOOR_A));
    let middle = repo.commit(
        hand("2025-12-23T09:00:00+0000", "2026-01-04T10:00:00+0000"),
        "spec: apply PR-0003\n\nProposal: PR-0003\n\nCo-Authored-By: Dee Pair <dee@example.org>\n",
    );
    // An apply subject with no `Proposal:` trailer at all.
    let window_e = "## Window {#R-12 rev=2}\n\nThe window opens at noon, for ten minutes, daily.";
    write(&repo.root, RULES, rules(window_e, DOOR_A));
    let bare = repo.commit(
        hand("2025-12-24T09:00:00+0000", "2026-01-05T10:00:00+0000"),
        "spec: apply PR-0004\n\nNo trailer here.\n",
    );
    // An apply subject whose last paragraph names another proposal.
    let window_f = "## Window {#R-12 rev=2}\n\nThe window opens at noon, daily, for ten minutes.";
    write(&repo.root, RULES, rules(window_f, DOOR_A));
    let other = repo.commit(
        hand("2025-12-25T09:00:00+0000", "2026-01-06T10:00:00+0000"),
        &apply_message("PR-0005", "Another one.", ANN, &bare, &[]).replacen(
            "Proposal: PR-0005",
            "Proposal: PR-0006",
            1,
        ),
    );

    let (_, document) = repo.history("R-12");
    assert_eq!(
        ids(&document),
        [other.as_str(), &bare, &middle, &co, &b, &a],
        "{document:#}"
    );
    assert_eq!(
        changes(&document),
        ["hand", "hand", "hand", "hand", "raised", "hand"],
        "only B is an apply: {document:#}"
    );
    let list = entries(&document);
    assert_eq!(
        list[4]["proposal"],
        json!("PR-0001"),
        "B's trailer, not PR-9999"
    );
    assert_eq!(
        list[4]["rationale"],
        json!("Tighten the window.  Proposal: PR-9999"),
        "the rationale before the trailer block, one_line's (each LF a space)"
    );
    assert_eq!(
        list[4]["by"],
        json!(ANN.value()),
        "Decided-by, not the author"
    );
    for (at, title) in [
        (0, "spec: apply PR-0005"),
        (1, "spec: apply PR-0004"),
        (2, "spec: apply PR-0003"),
        (3, "Tidy the window rule"),
    ] {
        assert_eq!(list[at]["proposal"], json!(null), "{}", list[at]);
        assert_eq!(list[at]["proposed_by"], json!(null), "{}", list[at]);
        assert_eq!(list[at]["proposer"], json!(null), "{}", list[at]);
        assert_eq!(list[at]["rationale"], json!(null), "{}", list[at]);
        assert_eq!(list[at]["title"], json!(title), "a hand commit's subject");
        assert_eq!(
            list[at]["by"],
            json!(BOB.value()),
            "the author, not the committer"
        );
    }
}

// ---------------------------------------------------------------- NH3

/// NH3: after `git mv docs/rules.md docs/law/rules.md` (F, R-12 kept) and
/// one more edit of R-12 (G), R-12 lists G and all of NH1's; F, which
/// changed no byte of R-12, is no creation.
#[test]
fn nh3_a_rename_carries_the_history() {
    let book = book("nh3");
    let repo = &book.repo;
    fs::create_dir_all(repo.root.join("docs/law")).expect("docs/law");
    repo.git
        .git(&repo.root, &["mv", RULES, "docs/law/rules.md"]);
    let f = repo.commit(
        Stamp {
            author: BOB,
            committer: DANA,
            authored: "2025-12-25T09:00:00+0000",
            committed: "2026-01-06T10:00:00+0000",
        },
        "Move the rules under law\n",
    );
    let window_g = "## Window {#R-12 rev=2}\n\nThe window opens at noon, for ten minutes, no more.";
    write(&repo.root, "docs/law/rules.md", rules(window_g, DOOR_E));
    let g = repo.commit(
        Stamp {
            author: BOB,
            committer: DANA,
            authored: "2025-12-26T09:00:00+0000",
            committed: "2026-01-07T10:00:00+0000",
        },
        "Cap the window\n",
    );
    let (_, document) = repo.history("R-12");
    assert_eq!(document["nodes"][0]["path"], json!("docs/law/rules.md"));
    let mut expected = vec![commit_entry(&Commit {
        at: "2026-01-07T10:00:00Z",
        oid: &g,
        change: "hand",
        created: false,
        rev: Some(2),
        rev_before: Some(2),
        by: BOB,
        apply: None,
        text: "Cap the window",
    })];
    expected.extend(book_entries(&book));
    assert_eq!(
        document["nodes"][0]["history"],
        json!({"entries": expected, "truncated": false, "omitted": 0}),
        "G, then NH1's four; F not listed: {document:#}"
    );
    assert!(
        !ids(&document).contains(&f),
        "a rename alone changes no span"
    );

    // The document follows too: G, F (a move is a change of the file's
    // commits? its bytes are the same, so it is not listed), E..A.
    let (_, whole) = repo.history("docs/law/rules.md");
    let listed = ids(&whole);
    assert_eq!(listed.first(), Some(&g), "{whole:#}");
    assert_eq!(listed.last(), Some(&book.a), "{whole:#}");
    assert_eq!(
        entries(&whole).last().expect("A")["created"],
        json!(true),
        "the creation is A, at the old path"
    );
    let created: usize = entries(&whole)
        .iter()
        .filter(|entry| entry["created"] == json!(true))
        .count();
    assert_eq!(created, 1, "one creation: {whole:#}");
}

// ---------------------------------------------------------------- NH4

/// A `git` shim first on `PATH` (in `dir`): it logs its arguments to
/// `args.log`, tees a `cat-file`'s stdin to `stdin.log`, and runs the real
/// git. Its directory and the two logs.
fn shim(repo: &Repo, dir: &str) -> (PathBuf, PathBuf, PathBuf) {
    let bin = repo.scratch.dir(dir);
    let args = bin.join("args.log");
    let stdin = bin.join("stdin.log");
    let git = repo.git.git_program().to_path_buf();
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{args}'\ncase \" $* \" in\n  *\" cat-file \"*) \
         /usr/bin/tee -a '{stdin}' | '{git}' \"$@\" ;;\n  *) exec '{git}' \"$@\" ;;\nesac\n",
        args = args.display(),
        stdin = stdin.display(),
        git = git.display(),
    );
    let path = bin.join("git");
    fs::write(&path, script).expect("the shim");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    (bin, args, stdin)
}

/// `PATH` with `first` before the sandbox's.
fn path_with(repo: &Repo, first: &Path) -> OsString {
    let mut dirs = vec![first.to_path_buf()];
    dirs.extend(std::env::split_paths(
        repo.git.var("PATH").expect("the sandbox's PATH"),
    ));
    std::env::join_paths(dirs).expect("a PATH")
}

/// Every logged git subcommand line.
fn logged(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Whether a logged argument line runs `sub` (a word of it).
fn runs(line: &str, sub: &str) -> bool {
    line.split_whitespace().any(|word| word == sub)
}

/// NH4: not asked, `spec show R-12 --json` exits 0 with `history: null`,
/// no `note: history:` and no `git log` or `git cat-file`; asked, the same
/// shim sees both (the shim is on the path the read takes).
#[test]
fn nh4_not_asked_runs_no_git_for_it() {
    let book = book("nh4");
    let repo = &book.repo;
    let (bin, args_log, _) = shim(repo, "shim");
    let path = path_with(repo, &bin);
    let extra: [(&str, &OsStr); 1] = [("PATH", path.as_os_str())];
    for args in [
        &["--json", "show", "R-12"][..],
        &["show", "R-12"],
        &["--json", "show", "R-12", "--links"],
        &["--json", "show", RULES],
    ] {
        let _ = fs::remove_file(&args_log);
        let done = repo.spec_at(&repo.root, &repo.home, args, &extra);
        done.code(0);
        assert!(
            !done.stderr.contains("note: history:"),
            "{args:?}: no history note\n{}",
            done.show()
        );
        if args[0] == "--json" {
            let document = done.json();
            for node in document["nodes"].as_array().expect("nodes") {
                assert_eq!(node["history"], json!(null), "{args:?}: {node}");
            }
        }
        let lines = logged(&args_log);
        assert!(
            !lines
                .iter()
                .any(|line| runs(line, "log") || runs(line, "cat-file")),
            "{args:?}: no git log, no cat-file: {lines:#?}"
        );
    }
    let _ = fs::remove_file(&args_log);
    repo.spec_at(
        &repo.root,
        &repo.home,
        &["--json", "show", "R-12", "--history"],
        &extra,
    )
    .code(0);
    let lines = logged(&args_log);
    assert!(
        lines.iter().any(|line| runs(line, "log"))
            && lines.iter().any(|line| runs(line, "cat-file")),
        "asked: the shim sees git log and cat-file: {lines:#?}"
    );
}

// ---------------------------------------------------------------- NH5

/// The one `note: history:` line of `run`'s stderr.
fn history_notes(run: &Run) -> Vec<String> {
    run.stderr
        .lines()
        .filter(|line| line.starts_with("note: history:"))
        .map(str::to_owned)
        .collect()
}

/// `spec show R-12 --history` and its `--json` in `cwd`: exit 0, the
/// text with no history block, `history: null`, exactly one note; the note.
fn unread(repo: &Repo, cwd: &Path, home: &Path, extra: &[(&str, &OsStr)], context: &str) -> String {
    let text = repo.spec_at(cwd, home, &["show", "R-12", "--history"], extra);
    text.code(0);
    let plain = repo.spec_at(cwd, home, &["show", "R-12"], extra);
    plain.code(0);
    assert_eq!(text.stdout, plain.stdout, "{context}: the text, no block");
    assert!(text.stdout.contains("The window opens"), "{context}");
    let notes = history_notes(&text);
    assert_eq!(notes.len(), 1, "{context}: one note\n{}", text.show());
    assert_eq!(
        text.stderr.lines().count(),
        1,
        "{context}: nothing else on stderr\n{}",
        text.show()
    );
    let json = repo.spec_at(cwd, home, &["--json", "show", "R-12", "--history"], extra);
    json.code(0);
    let document = json.json();
    assert_eq!(
        document["nodes"][0]["history"],
        json!(null),
        "{context}: {document}"
    );
    assert_eq!(
        history_notes(&json),
        notes,
        "{context}: the JSON run's note"
    );
    assert!(
        document["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|note| Some(note.as_str().unwrap_or_default()) == notes[0].strip_prefix("note: ")),
        "{context}: the JSON's notes hold it: {document}"
    );
    notes[0].clone()
}

/// NH5: a `--depth 1` clone, no `git` on `PATH`, no repository: each
/// exit 0, the text, `history: null`, one note naming the holder.
#[test]
fn nh5_no_history_to_read_is_a_note() {
    let book = book("nh5");
    let repo = &book.repo;

    // A shallow clone.
    let clone = repo.scratch.join("clone");
    let url = format!("file://{}", repo.root.display());
    repo.git.git(
        repo.scratch.path(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            &url,
            clone.to_str().expect("UTF-8"),
        ],
    );
    let clone_home = repo.scratch.home("clone");
    let note = unread(repo, &clone, &clone_home, &[], "a shallow clone");
    assert_eq!(
        note,
        format!("note: history: {RULES}: not read: a shallow clone")
    );

    // No git on PATH.
    let empty = repo.scratch.dir("no-git");
    let no_git_home = repo.scratch.home("no-git");
    let extra: [(&str, &OsStr); 1] = [("PATH", empty.as_os_str())];
    let note = unread(repo, &repo.root, &no_git_home, &extra, "git missing");
    let prefix = format!("note: history: {RULES}: not read: ");
    assert_eq!(
        note,
        format!("{prefix}git missing"),
        "the cause: git missing"
    );

    // Outside any repository: the tree without `.git`.
    let bare_tree = repo.scratch.dir("no-repository");
    for relative in ["specengine.toml", "templates/decision.md", RULES] {
        write(&bare_tree, relative, read_text(&repo.root, relative));
    }
    let outside_home = repo.scratch.home("outside");
    let note = unread(repo, &bare_tree, &outside_home, &[], "not a repository");
    assert_eq!(note, format!("{prefix}not a repository"));

    // A partial clone lacking the old blobs: never fetched (the read runs
    // git with `GIT_NO_LAZY_FETCH=1`), a note.
    repo.git
        .git(&repo.root, &["config", "uploadpack.allowFilter", "true"]);
    let partial = repo.scratch.join("partial");
    repo.git.git(
        repo.scratch.path(),
        &[
            "clone",
            "-q",
            "--filter=blob:none",
            &url,
            partial.to_str().expect("UTF-8"),
        ],
    );
    repo.git.quiet(&partial);
    let missing = repo.git.git_output(
        &partial,
        &["cat-file", "-e", &format!("{}:{RULES}", book.a)],
        &[("GIT_NO_LAZY_FETCH", OsStr::new("1"))],
    );
    assert!(
        !missing.status.success(),
        "premise: A's blob is not in the partial clone"
    );
    let partial_home = repo.scratch.home("partial");
    let note = unread(repo, &partial, &partial_home, &[], "a partial clone");
    assert_eq!(
        note,
        format!("{prefix}a blob is missing (a partial clone?)")
    );
    let still = repo.git.git_output(
        &partial,
        &["cat-file", "-e", &format!("{}:{RULES}", book.a)],
        &[("GIT_NO_LAZY_FETCH", OsStr::new("1"))],
    );
    assert!(!still.status.success(), "nothing fetched by the read");
}

// ------------------------------------------------- what git does not hold

/// What git does not hold (beside NH5): an uncommitted edit shows no commit entry (the
/// committed ones stay), a section or a file not committed yet lists
/// nothing, and an unborn branch gives no entry and no note; each exit 0
/// with no `note: history:`.
#[test]
fn what_git_does_not_hold_lists_nothing_and_says_nothing() {
    let book = book("nh-uncommitted");
    let repo = &book.repo;
    // An uncommitted edit of R-12, a new section R-14, a new file.
    let edited = rules(
        "## Window {#R-12 rev=2}\n\nThe window opens at noon, for ten minutes only, today.",
        &format!("{DOOR_E}\n\n## Gate {{#R-14}}\n\nThe gate stays open."),
    );
    write(&repo.root, RULES, edited);
    write(
        &repo.root,
        "docs/porch.md",
        "# Porch\n\n## Lamp {#R-15}\n\nThe lamp burns all night.\n",
    );
    let (text, document) = repo.history("R-12");
    assert_eq!(text.stderr, "", "{}", text.show());
    assert_eq!(
        ids(&document),
        [book.d.as_str(), &book.c, &book.b, &book.a],
        "the committed changes alone: {document:#}"
    );
    for reference in ["R-14", "R-15", "docs/porch.md"] {
        let (text, document) = repo.history(reference);
        assert_eq!(text.stderr, "", "{reference}: no note\n{}", text.show());
        assert_eq!(
            document["nodes"][0]["history"],
            json!({"entries": [], "truncated": false, "omitted": 0}),
            "{reference}: not committed, nothing listed"
        );
        assert!(text.stdout.contains("\n  history 0\n"), "{}", text.stdout);
    }

    // An unborn branch: no commit at all.
    let unborn = Repo::new("nh-unborn");
    write(&unborn.root, RULES, rules(WINDOW_A, DOOR_A));
    let (text, document) = unborn.history("R-12");
    assert_eq!(text.stderr, "", "no note: {}", text.show());
    assert_eq!(
        document["nodes"][0]["history"],
        json!({"entries": [], "truncated": false, "omitted": 0})
    );
    assert_eq!(document["notes"], json!([]));
}

// ------------------------------------------------------------- merges

/// The spec's rule on merges: a branch edits R-12, `main` edits R-13,
/// and a merge joins them. Judged against its first parent (`main`), the
/// merge changed R-12 (the branch's edit came in) and not R-13: R-12 lists
/// the merge and the branch's commit, R-13 `main`'s commit alone.
#[test]
fn a_merge_is_judged_against_its_first_parent() {
    let repo = Repo::new("nh-merge");
    let hand = |authored: &'static str, committed: &'static str| Stamp {
        author: BOB,
        committer: DANA,
        authored,
        committed,
    };
    write(&repo.root, RULES, rules(WINDOW_A, DOOR_A));
    let a = repo.commit(
        hand("2025-12-20T09:00:00+0000", "2026-01-01T10:00:00+0000"),
        "Add the rules\n",
    );
    repo.git.git(&repo.root, &["checkout", "-q", "-b", "side"]);
    write(&repo.root, RULES, rules(WINDOW_B, DOOR_A));
    let side = repo.commit(
        hand("2025-12-21T09:00:00+0000", "2026-01-02T10:00:00+0000"),
        "Widen the window on the side branch\n",
    );
    repo.git.git(&repo.root, &["checkout", "-q", "main"]);
    write(&repo.root, RULES, rules(WINDOW_A, DOOR_E));
    let door = repo.commit(
        hand("2025-12-22T09:00:00+0000", "2026-01-03T10:00:00+0000"),
        "Shorten the door rule\n",
    );
    let date: OsString = "2026-01-04T10:00:00+0000".into();
    let name: OsString = BOB.name.into();
    let email: OsString = BOB.email.into();
    repo.git.git_env(
        &repo.root,
        &[
            "merge",
            "-q",
            "--no-ff",
            "--no-edit",
            "side",
            "-m",
            "Merge the side branch",
        ],
        &[
            ("GIT_AUTHOR_DATE", date.as_os_str()),
            ("GIT_COMMITTER_DATE", date.as_os_str()),
            ("GIT_AUTHOR_NAME", name.as_os_str()),
            ("GIT_AUTHOR_EMAIL", email.as_os_str()),
        ],
    );
    let merge = repo.git.git_text(&repo.root, &["rev-parse", "HEAD"]);
    assert_eq!(
        read_text(&repo.root, RULES),
        rules(WINDOW_B, DOOR_E),
        "the merge holds both edits"
    );
    let (_, window) = repo.history("R-12");
    assert_eq!(ids(&window), [merge.as_str(), &side, &a], "{window:#}");
    assert_eq!(entries(&window)[0]["title"], json!("Merge the side branch"));
    assert_eq!(entries(&window)[0]["change"], json!("hand"));
    let (_, door_history) = repo.history("R-13");
    assert_eq!(
        ids(&door_history),
        [door.as_str(), &a],
        "the merge changed no byte of R-13 against main: {door_history:#}"
    );
}

// ---------------------------------------------------------------- NH8

/// The clocks of NH8's scene.
const RAISED: &str = "2026-01-01T12:00:00Z";
/// B's commit, C-0001's acceptance and the first decision: one tie.
const TIE: &str = "2026-01-02T10:00:00Z";

/// A stored intake item's ID.
fn stored(outcome: &IntakeOutcome) -> String {
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

fn hit_names(outcome: &IntakeOutcome) -> Vec<String> {
    outcome
        .document
        .hits
        .iter()
        .map(|hit| hit.name().to_owned())
        .collect()
}

impl Repo {
    /// A decision naming `node`, stored (raised again naming its hits).
    fn decision(&self, node: &str, text: &str) -> String {
        let mut request = DecisionRequest {
            input: DecisionInput {
                node_ids: vec![node.to_owned()],
                text: text.to_owned(),
                working_answer: format!("Yes: {text}"),
                price_of_other: "A rule rewritten.".to_owned(),
                why: None,
                severity: None,
                proposed_patch: None,
                distinct_from: None,
            },
            card: None,
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: RAISED.to_owned(),
            git: self.git_env(&[]),
        };
        let raise = |request: &DecisionRequest| {
            propose_decision(&self.env(), &Globals::default(), request)
                .unwrap_or_else(|error| panic!("decision {text}: {error}"))
        };
        let first = raise(&request);
        if first.document.created {
            return stored(&first);
        }
        request.input.distinct_from = Some(hit_names(&first));
        stored(&raise(&request))
    }

    /// A question naming `node`, stored.
    fn question(&self, node: &str, text: &str) -> String {
        let mut request = QuestionRequest {
            node_ids: vec![node.to_owned()],
            text: text.to_owned(),
            working_answer: "Yes, at once.".to_owned(),
            price_of_other: "A new edge case.".to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            card: None,
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: RAISED.to_owned(),
            git: self.git_env(&[]),
            gist: None,
            matters: None,
            options: None,
            recommendation: None,
        };
        let ask = |request: &QuestionRequest| {
            propose_question(&self.env(), &Globals::default(), request)
                .unwrap_or_else(|error| panic!("question {text}: {error}"))
        };
        let first = ask(&request);
        if first.document.created {
            return stored(&first);
        }
        request.distinct_from = hit_names(&first);
        stored(&ask(&request))
    }

    /// Approves an item at `now` as [`ANN`], its working answer: the
    /// record's ID.
    fn decide(&self, id: &str, now: &str) -> String {
        let mut yes = |_: &str| true;
        let outcome = approve_with(
            &self.env(),
            &Globals::default(),
            &ApproveRequest {
                id: id.to_owned(),
                note: None,
                now: now.to_owned(),
                git: self.deciding(ANN, &git_date(now)),
            },
            &ApproveFlags {
                option: None,
                answer: None,
                canon: None,
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("approve {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
        self.queue()
            .get(id)
            .expect("get")
            .expect("stored")
            .record
            .unwrap_or_else(|| panic!("{id} holds a record"))
            .id
    }

    /// Rejects an item at `now`.
    fn turn_down(&self, id: &str, now: &str) {
        let mut yes = |_: &str| true;
        let outcome = reject(
            &self.env(),
            &Globals::default(),
            &RejectRequest {
                id: id.to_owned(),
                reason: Some("Not for this rule.".to_owned()),
                now: now.to_owned(),
                git: self.deciding(ANN, &git_date(now)),
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("reject {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "reject {id}: {outcome:?}");
    }

    /// A card on `targets` made at [`RAISED`]: its ID.
    fn card(&self, title: &str, targets: &[&str]) -> String {
        let outcome = card_new(
            &self.env(),
            &Globals::default(),
            &CardNewRequest {
                title: title.to_owned(),
                targets: targets.iter().map(|id| (*id).to_owned()).collect(),
                now: RAISED.to_owned(),
                ..CardNewRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("card new {title}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.id.expect("a card's ID")
    }

    /// Assigns `id` to `ana` with `Ben Review` its reviewer, moves it to
    /// review and accepts it as the reviewer at `now`.
    fn accept(&self, id: &str, now: &str) {
        let globals = Globals::default();
        let assigned = card_assign(
            &self.env(),
            &globals,
            &CardAssignRequest {
                id: id.to_owned(),
                assignee: Some("ana".to_owned()),
                reviewer: Some("ben".to_owned()),
                now: RAISED.to_owned(),
                ..CardAssignRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("assign {id}: {error}"));
        assert_eq!(assigned.exit(), Exit::Answered, "{assigned:?}");
        let moved = card_move(
            &self.env(),
            &globals,
            &CardMoveRequest {
                id: id.to_owned(),
                to: Some("review".to_owned()),
                now: RAISED.to_owned(),
                ..CardMoveRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("move {id}: {error}"));
        assert_eq!(moved.exit(), Exit::Answered, "{moved:?}");
        let mut consent = |_: &str| true;
        let accepted = card_accept(
            &self.env(),
            &globals,
            &CardAcceptRequest {
                id: id.to_owned(),
                actor: "ben".to_owned(),
                now: now.to_owned(),
                briefed: true,
                ..CardAcceptRequest::default()
            },
            &mut consent,
        )
        .unwrap_or_else(|error| panic!("accept {id}: {error}"));
        assert_eq!(accepted.exit(), Exit::Answered, "{accepted:?}");
    }
}

/// `2026-01-02T10:00:00Z` as git's date: `2026-01-02T10:00:00+0000`.
fn git_date(utc: &str) -> String {
    format!("{}+0000", utc.trim_end_matches('Z'))
}

/// NH8's scene and the names the queue gave.
struct Joined {
    repo: Repo,
    a: String,
    b: String,
    x: String,
    update: String,
    applied_decision: String,
    applied_record: String,
    open_question: String,
    rejected: String,
    answered: String,
    answered_record: String,
    carried: String,
    accepted: String,
    unaccepted: String,
    elsewhere: String,
}

/// NH8's scene: A by hand; PR-0001 an update of R-12 proposed and applied
/// through the library's `approve` as [`ANN`] at [`TIE`] (B, an apply end
/// to end); cards C-0001 (R-12, accepted at [`TIE`]), C-0002 (R-12, not
/// accepted), C-0003 (R-13, accepted); a decision on R-12 applied at
/// [`TIE`] with its record, an open question, a rejected decision, a
/// question answered later with its record, and a decision applied whose
/// `Proposal:` a later commit X on R-12 carries.
fn joined(label: &str) -> Joined {
    let repo = Repo::new(label);
    write(&repo.root, RULES, rules(WINDOW_A, DOOR_A));
    let a = repo.commit(
        Stamp {
            author: BOB,
            committer: DANA,
            authored: "2025-12-20T09:00:00+0000",
            committed: "2026-01-01T10:00:00+0000",
        },
        "Add the rules\n",
    );
    repo.spec(&["index"]).code(0);

    // PR-0001: an update of R-12, applied end to end.
    let shown = repo.spec(&["--json", "show", "R-12"]);
    shown.code(0);
    let node = &shown.json()["nodes"][0];
    let base = node["span_hash"].as_str().expect("span_hash").to_owned();
    let text = node["text"]
        .as_str()
        .expect("text")
        .replace("at noon.", "at noon for ten minutes.");
    let proposed = propose(
        &repo.env(),
        &Globals::default(),
        &ProposeRequest {
            target: "R-12".to_owned(),
            base,
            text: ProposedText::Given(text.into_bytes()),
            rationale: "Tighten the window.".to_owned(),
            author_role: Some("spec-writer".to_owned()),
            author_model: Some("m".to_owned()),
            run: Some("r".to_owned()),
            login: None,
            now: RAISED.to_owned(),
            git: repo.git_env(&[]),
        },
    )
    .unwrap_or_else(|error| panic!("propose: {error}"));
    assert_eq!(proposed.exit(), Exit::Answered, "{proposed:?}");
    let update = proposed.document.id.clone().expect("an ID");
    let mut yes = |_: &str| true;
    let applied = approve(
        &repo.env(),
        &Globals::default(),
        &ApproveRequest {
            id: update.clone(),
            note: None,
            now: TIE.to_owned(),
            git: repo.deciding(ANN, &git_date(TIE)),
        },
        &mut yes,
    )
    .unwrap_or_else(|error| panic!("approve {update}: {error}"));
    assert_eq!(applied.exit(), Exit::Answered, "{applied:?}");
    let b = repo.git.git_text(&repo.root, &["rev-parse", "HEAD"]);
    assert_ne!(b, a, "the apply committed");

    // The cards.
    let accepted = repo.card("Read the window rule aloud", &["R-12"]);
    let unaccepted = repo.card("Draw the window", &["R-12"]);
    let elsewhere = repo.card("Read the door rule aloud", &["R-13"]);
    repo.accept(&accepted, TIE);
    repo.accept(&elsewhere, "2026-01-03T10:00:00Z");

    // The items.
    let applied_decision = repo.decision("R-12", "Does the window open on holidays?");
    let applied_record = repo.decide(&applied_decision, TIE);
    let open_question = repo.question("R-12", "Does the window open in the rain?");
    let rejected = repo.decision("R-12", "Does the window stay shut on Sundays?");
    repo.turn_down(&rejected, "2026-01-04T11:00:00Z");
    let answered = repo.question("R-12", "Does the window open for guests?");
    let answered_record = repo.decide(&answered, "2026-01-04T12:00:00Z");
    let carried = repo.decision("R-12", "Does the window open for ten minutes?");
    repo.decide(&carried, "2026-01-05T10:00:00Z");

    // X: a commit on R-12 carrying `carried`'s `Proposal:`.
    let head = repo.git.git_text(&repo.root, &["rev-parse", "HEAD"]);
    let current = read_text(&repo.root, RULES);
    assert!(
        current.contains("{#R-12 rev=2}"),
        "the apply raised R-12: {current}"
    );
    write(
        &repo.root,
        RULES,
        current.replace(
            "{#R-12 rev=2}\n\nThe window opens at noon for ten minutes.",
            "{#R-12 rev=3}\n\nThe window opens at noon for ten minutes, holidays too.",
        ),
    );
    let x = repo.commit(
        Stamp {
            author: CLEO,
            committer: CLEO,
            authored: "2025-12-26T09:00:00+0000",
            committed: "2026-01-06T10:00:00+0000",
        },
        &apply_message(
            &carried,
            "Carry the decision into the window rule.",
            ANN,
            &head,
            &[Revised {
                id: "R-12",
                rev: 2,
                to: Some(3),
            }],
        ),
    );
    Joined {
        repo,
        a,
        b,
        x,
        update,
        applied_decision,
        applied_record,
        open_question,
        rejected,
        answered,
        answered_record,
        carried,
        accepted,
        unaccepted,
        elsewhere,
    }
}

/// An item entry as the table builds it.
fn item_entry(at: &str, id: &str, change: &str, record: &str) -> Value {
    json!({
        "source": "item", "at": at, "id": id, "change": change, "created": false,
        "rev": null, "rev_before": null, "by": ANN.value(), "by_name": ANN.name,
        "proposal": null, "proposed_by": null, "proposer": null,
        "record": record, "title": null, "rationale": null,
    })
}

/// A card entry as the table builds it.
fn card_entry(at: &str, id: &str, title: &str) -> Value {
    json!({
        "source": "card", "at": at, "id": id, "change": "accepted", "created": false,
        "rev": null, "rev_before": null, "by": "ben", "by_name": "ben",
        "proposal": null, "proposed_by": null, "proposer": null,
        "record": null, "title": title, "rationale": null,
    })
}

/// NH8: R-12 lists X, the answered question, B, C-0001, the applied
/// decision (a tie: commit, card, item), A; none of C-0002, C-0003, the
/// open question or the rejected decision; PR-0001 once, as B; the
/// carried decision once, as X. The document lists C-0003 too. A queue
/// that cannot be read gives one note and the commits.
#[test]
fn nh8_the_queue_joins() {
    let scene = joined("nh8");
    let repo = &scene.repo;
    assert_eq!(scene.update, "PR-0001");
    let (text, document) = repo.history("R-12");
    assert_eq!(text.stderr, "", "{}", text.show());
    assert_keys(&document, "NH8");
    let list = entries(&document);
    let listed = ids(&document);
    assert_eq!(
        listed,
        [
            scene.x.as_str(),
            &scene.answered,
            &scene.b,
            &scene.accepted,
            &scene.applied_decision,
            &scene.a,
        ],
        "by at, ties commit, card, item: {document:#}"
    );
    for absent in [
        &scene.unaccepted,
        &scene.elsewhere,
        &scene.open_question,
        &scene.rejected,
        &scene.update,
        &scene.carried,
    ] {
        assert!(
            !listed.contains(absent),
            "{absent} not an entry: {listed:?}"
        );
    }
    let proposals: Vec<&Value> = list.iter().map(|entry| &entry["proposal"]).collect();
    assert_eq!(
        proposals,
        [
            &json!(scene.carried),
            &json!(null),
            &json!("PR-0001"),
            &json!(null),
            &json!(null),
            &json!(null),
        ],
        "PR-0001 once, as B; the carried decision once, as X"
    );
    assert_eq!(list[0]["change"], json!("raised"), "{}", list[0]);
    assert_eq!(list[0]["at"], json!("2026-01-06T10:00:00Z"));
    assert_eq!(
        list[1],
        item_entry(
            "2026-01-04T12:00:00Z",
            &scene.answered,
            "question",
            &scene.answered_record
        )
    );
    // B: the apply end to end, as `approve` wrote it.
    assert_eq!(list[2]["source"], json!("commit"));
    assert_eq!(list[2]["at"], json!(TIE));
    assert_eq!(list[2]["change"], json!("raised"), "{}", list[2]);
    assert_eq!(list[2]["rev"], json!(2));
    assert_eq!(list[2]["rev_before"], json!(1));
    assert_eq!(list[2]["by"], json!(ANN.value()), "{}", list[2]);
    assert_eq!(list[2]["by_name"], json!(ANN.name));
    assert_eq!(
        list[2]["proposed_by"],
        json!("agent role=spec-writer model=m run=r")
    );
    assert_eq!(list[2]["proposer"], json!("agent spec-writer"));
    assert_eq!(list[2]["rationale"], json!("Tighten the window."));
    assert_eq!(
        list[3],
        card_entry(TIE, &scene.accepted, "Read the window rule aloud")
    );
    assert_eq!(
        list[4],
        item_entry(
            TIE,
            &scene.applied_decision,
            "decision",
            &scene.applied_record
        )
    );
    assert_eq!(list[5]["created"], json!(true));

    // The text's card and item lines.
    let card_line = format!(
        "  {TIE} | accepted | {} | ben | Read the window rule aloud\n",
        scene.accepted
    );
    assert!(
        text.stdout.contains(&card_line),
        "{card_line}{}",
        text.stdout
    );
    let item_line = format!(
        "  {TIE} | decision | {} | {} | {}\n",
        scene.applied_decision,
        scene.applied_record,
        ANN.value()
    );
    assert!(
        text.stdout.contains(&item_line),
        "{item_line}{}",
        text.stdout
    );
    assert!(text.stdout.contains("\n  history 6\n"), "{}", text.stdout);

    // The document: the sections inside it, so C-0003 too.
    let (_, whole) = repo.history(RULES);
    let whole_ids = ids(&whole);
    for present in [&scene.accepted, &scene.elsewhere, &scene.applied_decision] {
        assert!(whole_ids.contains(present), "{present}: {whole_ids:?}");
    }
    for absent in [&scene.unaccepted, &scene.open_question, &scene.rejected] {
        assert!(!whole_ids.contains(absent), "{absent}: {whole_ids:?}");
    }

    // A queue that cannot be read: one note, the commits only.
    let db = repo.db();
    let version = sql(&db, "PRAGMA user_version;");
    sql(&db, "PRAGMA user_version = 99;");
    let unread_text = repo.spec(&["show", "R-12", "--history"]);
    let unread_json = repo.spec(&["--json", "show", "R-12", "--history"]);
    sql(&db, &format!("PRAGMA user_version = {};", version.trim()));
    unread_text.code(0);
    unread_json.code(0);
    let notes = history_notes(&unread_text);
    assert_eq!(notes.len(), 1, "{}", unread_text.show());
    assert!(
        notes[0].starts_with("note: history: the queue not read: ")
            && notes[0].ends_with("; commits only"),
        "{notes:?}"
    );
    assert_eq!(
        ids(&unread_json.json()),
        [scene.x.as_str(), &scene.b, &scene.a],
        "the commits only"
    );
}

/// `sqlite3 db sql`: its stdout.
fn sql(db: &Path, sql: &str) -> String {
    let output = std::process::Command::new(sqlite3())
        .arg(db)
        .arg(sql)
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8")
}

// ------------------------------------------- iteration 2: what text prints

/// ESC, which `escape_controls` prints `\u{1b}`.
const ESC: char = '\u{1b}';

/// Iteration 2 (the review's finding): each history text line is escaped
/// whole with `escape_controls` (ESC as `\u{1b}`, lower-case hex), the JSON
/// keeps the raw value: an apply's rationale and a card's title, each
/// holding ESC. M: the escape dropped.
#[test]
fn history_text_lines_are_escaped_and_the_json_is_raw() {
    let repo = Repo::new("nh-escape");
    write(&repo.root, RULES, rules(WINDOW_A, DOOR_A));
    let a = repo.commit(
        Stamp {
            author: BOB,
            committer: DANA,
            authored: "2025-12-20T09:00:00+0000",
            committed: "2026-01-01T10:00:00+0000",
        },
        "Add the rules\n",
    );
    let rationale = format!("Tighten the {ESC}[31mwindow{ESC}[0m.");
    write(&repo.root, RULES, rules(WINDOW_B, DOOR_A));
    repo.commit(
        Stamp {
            author: CLEO,
            committer: CLEO,
            authored: "2025-12-21T09:00:00+0000",
            committed: "2026-01-02T10:00:00+0000",
        },
        &apply_message(
            "PR-0001",
            &rationale,
            ANN,
            &a,
            &[Revised {
                id: "R-12",
                rev: 1,
                to: Some(2),
            }],
        ),
    );
    repo.spec(&["index"]).code(0);
    let card = repo.card("Read the window aloud", &["R-12"]);
    repo.accept(&card, "2026-01-03T10:00:00Z");
    let title = format!("Read the {ESC}[31mwindow{ESC}[0m aloud");
    sql(
        &repo.db(),
        &format!(
            "UPDATE cards SET title = 'Read the ' || char(27) || '[31mwindow' || char(27) || \
             '[0m aloud' WHERE id = '{card}';"
        ),
    );
    let (text, document) = repo.history("R-12");
    assert!(
        !text.stdout.contains(ESC),
        "no raw ESC printed: {:?}",
        text.stdout
    );
    assert!(
        text.stdout
            .contains("\n    Tighten the \\u{1b}[31mwindow\\u{1b}[0m.\n"),
        "the rationale line escaped: {}",
        text.stdout
    );
    assert!(
        text.stdout.contains(&format!(
            "\n  2026-01-03T10:00:00Z | accepted | {card} | ben | Read the \
             \\u{{1b}}[31mwindow\\u{{1b}}[0m aloud\n"
        )),
        "the card line escaped: {}",
        text.stdout
    );
    let list = entries(&document);
    assert_eq!(list[0]["id"], json!(card));
    assert_eq!(list[0]["title"], json!(title), "the JSON raw");
    assert_eq!(list[1]["rationale"], json!(rationale), "the JSON raw");
}

/// Iteration 2: an author name holding U+001F reads as any other (the log's
/// header is LF-separated fields), no note; the JSON keeps it raw, the text
/// prints `\u{1f}`. A hand commit's title is git's subject, its first
/// paragraph's lines joined with one space. M: the header split on U+001F.
#[test]
fn an_author_with_a_unit_separator_and_a_wrapped_subject() {
    let repo = Repo::new("nh-unit-separator");
    write(&repo.root, RULES, rules(WINDOW_A, DOOR_A));
    let odd = "Odd\u{1f}Name";
    let date: OsString = "2026-01-01T10:00:00+0000".into();
    let name: OsString = odd.into();
    let email: OsString = "o@example.org".into();
    let file = repo.scratch.join("odd-message.txt");
    fs::write(&file, "Add\nthe rules\n\nThe body says more.\n").expect("message");
    repo.git.add_all(&repo.root);
    repo.git.git_env(
        &repo.root,
        &[
            "commit",
            "-q",
            "--no-verify",
            "--cleanup=verbatim",
            "-F",
            file.to_str().expect("UTF-8"),
        ],
        &[
            ("GIT_AUTHOR_NAME", name.as_os_str()),
            ("GIT_AUTHOR_EMAIL", email.as_os_str()),
            ("GIT_AUTHOR_DATE", date.as_os_str()),
            ("GIT_COMMITTER_DATE", date.as_os_str()),
        ],
    );
    let oid = repo.git.git_text(&repo.root, &["rev-parse", "HEAD"]);
    assert_eq!(
        repo.git
            .git_text(&repo.root, &["log", "-1", "--format=%an"]),
        odd,
        "premise: git kept the name"
    );
    let (text, document) = repo.history("R-12");
    assert_eq!(text.stderr, "", "no note: {}", text.show());
    let list = entries(&document);
    assert_eq!(list.len(), 1, "{document:#}");
    assert_eq!(list[0]["id"], json!(oid));
    assert_eq!(list[0]["by"], json!(format!("{odd} <o@example.org>")));
    assert_eq!(list[0]["by_name"], json!(odd));
    assert_eq!(list[0]["title"], json!("Add the rules"), "git's subject");
    assert_eq!(list[0]["created"], json!(true));
    assert!(
        text.stdout.contains(&format!(
            "  2026-01-01T10:00:00Z | hand created | rev 1 | - | Odd\\u{{1f}}Name \
             <o@example.org> | - | {}\n    Add the rules\n",
            &oid[..12]
        )),
        "{}",
        text.stdout
    );
}

/// Iteration 2: a queue item with no `decided_by` prints `-` in its `by`
/// column; its JSON `by` and `by_name` are `""`.
#[test]
fn an_item_with_no_decider_prints_a_dash() {
    let repo = Repo::new("nh-no-decider");
    write(&repo.root, RULES, rules(WINDOW_A, DOOR_A));
    repo.commit(
        Stamp {
            author: BOB,
            committer: DANA,
            authored: "2025-12-20T09:00:00+0000",
            committed: "2026-01-01T10:00:00+0000",
        },
        "Add the rules\n",
    );
    repo.spec(&["index"]).code(0);
    let decision = repo.decision("R-12", "Does the window open on holidays?");
    let record = repo.decide(&decision, TIE);
    sql(
        &repo.db(),
        &format!("UPDATE proposals SET decided_by = NULL WHERE id = '{decision}';"),
    );
    let (text, document) = repo.history("R-12");
    assert_eq!(text.stderr, "", "{}", text.show());
    let list = entries(&document);
    assert_eq!(list[0]["id"], json!(decision), "{document:#}");
    assert_eq!(list[0]["by"], json!(""));
    assert_eq!(list[0]["by_name"], json!(""));
    assert!(
        text.stdout.contains(&format!(
            "\n  {TIE} | decision | {decision} | {record} | -\n"
        )),
        "{}",
        text.stdout
    );
}

// ---------------------------------------------------------------- NH9

/// One commit of a `git fast-import` stream: its message, its committer
/// time (seconds since 1970), the files it sets.
struct Seeded {
    message: String,
    time: i64,
    files: Vec<(String, String)>,
}

/// 2026-01-01T00:00:00Z.
const EPOCH_2026: i64 = 1_767_225_600;

/// A fast-import stream of `commits` on `main`, by [`BOB`], committed by
/// [`DANA`].
fn stream(commits: &[Seeded]) -> Vec<u8> {
    let mut out = Vec::new();
    for (n, commit) in commits.iter().enumerate() {
        let mark = n + 1;
        out.extend_from_slice(
            format!(
                "commit refs/heads/main\nmark :{mark}\nauthor {} {} +0000\ncommitter {} {} +0000\n\
                 data {}\n{}\n",
                BOB.value(),
                commit.time - 3600,
                DANA.value(),
                commit.time,
                commit.message.len(),
                commit.message
            )
            .as_bytes(),
        );
        if mark > 1 {
            out.extend_from_slice(format!("from :{}\n", mark - 1).as_bytes());
        }
        for (path, content) in &commit.files {
            out.extend_from_slice(
                format!(
                    "M 100644 inline {path}\ndata {}\n{content}\n",
                    content.len()
                )
                .as_bytes(),
            );
        }
        out.push(b'\n');
    }
    out
}

/// A repository of `commits` imported in one stream and checked out: the
/// OIDs, oldest first.
fn seeded(label: &str, commits: &[Seeded]) -> (Repo, Vec<String>) {
    let repo = Repo::new(label);
    repo.git
        .git_stdin(&repo.root, &["fast-import", "--quiet"], &stream(commits));
    repo.git.git(&repo.root, &["reset", "-q", "--hard", "main"]);
    let oids: Vec<String> = repo
        .git
        .git_text(&repo.root, &["rev-list", "--reverse", "main"])
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(oids.len(), commits.len());
    (repo, oids)
}

/// The first commit of a seeded repository: the config, the template and
/// `rules`.
fn first(rules_text: String) -> Seeded {
    Seeded {
        message: "Add the rules\n".to_owned(),
        time: EPOCH_2026,
        files: vec![
            ("specengine.toml".to_owned(), CONFIG.to_owned()),
            ("templates/decision.md".to_owned(), TEMPLATE.to_owned()),
            (RULES.to_owned(), rules_text),
        ],
    }
}

/// A later commit `n` (minutes after [`EPOCH_2026`]) setting `rules`.
fn later(n: usize, message: String, rules_text: String) -> Seeded {
    Seeded {
        message,
        time: EPOCH_2026 + 60 * i64::try_from(n).expect("small"),
        files: vec![(RULES.to_owned(), rules_text)],
    }
}

/// NH9 (a): 70 commits changing R-12 give 64 entries, the newest,
/// `truncated: true`.
#[test]
fn nh9_a_the_entry_cap() {
    let mut commits = vec![first(rules(WINDOW_A, DOOR_A))];
    for n in 1..70 {
        commits.push(later(
            n,
            format!("Window edit {n}\n"),
            rules(
                &format!("## Window {{#R-12}}\n\nThe window opens at minute {n}."),
                DOOR_A,
            ),
        ));
    }
    let (repo, oids) = seeded("nh9a", &commits);
    let (text, document) = repo.history("R-12");
    let history = &document["nodes"][0]["history"];
    assert_eq!(history["truncated"], json!(true), "{history}");
    let expected: Vec<String> = oids.iter().rev().take(HISTORY_MAX).cloned().collect();
    assert_eq!(ids(&document), expected, "the 64 newest");
    assert!(
        text.stdout.contains("\n  history 64, truncated\n"),
        "{}",
        text.stdout
    );
}

/// NH9 (b): R-12 created at the first commit, then 130 commits touching
/// R-13 alone: the scan reads 129 and stops: `[]`, `truncated: true`.
#[test]
fn nh9_b_the_scan_stops_at_128() {
    let mut commits = vec![first(rules(WINDOW_A, DOOR_A))];
    for n in 1..=130 {
        commits.push(later(
            n,
            format!("Door edit {n}\n"),
            rules(
                WINDOW_A,
                &format!("## Door {{#R-13}}\n\nThe door shuts at minute {n}."),
            ),
        ));
    }
    assert!(commits.len() > HISTORY_SCAN_MAX + 1);
    let (repo, _) = seeded("nh9b", &commits);
    let (text, document) = repo.history("R-12");
    assert_eq!(
        document["nodes"][0]["history"],
        json!({"entries": [], "truncated": true, "omitted": 0}),
        "{document:#}"
    );
    assert!(
        text.stdout.contains("\n  history 0, truncated\n"),
        "{}",
        text.stdout
    );
    // R-13 the same scan: 128 entries read, 64 listed.
    let (_, door) = repo.history("R-13");
    assert_eq!(entries(&door).len(), HISTORY_MAX);
    assert_eq!(door["nodes"][0]["history"]["truncated"], json!(true));
}

/// NH9 (c): R-12 created at commit 3 of 5: three entries, `truncated`
/// false; the `cat-file` session (the shim's stdin log) never names
/// commits 1 or 2, nor commit 1's blob of the file.
#[test]
fn nh9_c_the_scan_stops_at_the_creation() {
    let no_window = |door: &str| format!("# Rules\n\nThe house rules.\n\n{door}\n");
    let commits = vec![
        first(no_window(DOOR_A)),
        later(1, "Door edit\n".to_owned(), no_window(DOOR_E)),
        later(2, "Add the window\n".to_owned(), rules(WINDOW_A, DOOR_E)),
        later(
            3,
            "Window edit\n".to_owned(),
            rules("## Window {#R-12}\n\nThe window opens at one.", DOOR_E),
        ),
        later(
            4,
            "Window edit again\n".to_owned(),
            rules("## Window {#R-12}\n\nThe window opens at two.", DOOR_E),
        ),
    ];
    let (repo, oids) = seeded("nh9c", &commits);
    let (bin, args_log, stdin_log) = shim(&repo, "shim");
    let path = path_with(&repo, &bin);
    let extra: [(&str, &OsStr); 1] = [("PATH", path.as_os_str())];
    // The index first, so the read under the shim is the history's alone.
    repo.spec(&["index"]).code(0);
    let _ = fs::remove_file(&stdin_log);
    let _ = fs::remove_file(&args_log);
    let (_, document) = repo.history_at(&repo.root, &repo.home, "R-12", &extra);
    assert_eq!(
        ids(&document),
        [oids[4].as_str(), &oids[3], &oids[2]],
        "{document:#}"
    );
    assert_eq!(changes(&document), ["hand", "hand", "hand"], "{document:#}");
    assert_eq!(entries(&document)[2]["created"], json!(true));
    assert_eq!(document["nodes"][0]["history"]["truncated"], json!(false));

    let asked = fs::read_to_string(&stdin_log).unwrap_or_default();
    assert!(
        !asked.is_empty(),
        "the shim saw the cat-file session: {:?}",
        logged(&args_log)
    );
    // Commit 3's parent version (commit 2's file) is read to see the
    // creation; commit 1's version never is, and neither commit 1 nor 2 is
    // judged. The session's stdin names blobs by OID (the developer's
    // departure 1; the spec names `<commit>:<path>`): both forms are checked.
    let blob_1 = repo
        .git
        .git_text(&repo.root, &["rev-parse", &format!("{}:{RULES}", oids[0])]);
    let mut by_second = 0;
    for line in asked.lines() {
        assert!(
            !line.starts_with(oids[0].as_str()),
            "commit 1 read: {line}\n{asked}"
        );
        assert!(
            !line.starts_with(&format!("{}^", oids[1])),
            "commit 2 judged: {line}\n{asked}"
        );
        if line.starts_with(oids[1].as_str()) {
            by_second += 1;
        }
        assert!(
            !line.contains(&blob_1),
            "commit 1's blob asked: {line}\n{asked}"
        );
    }
    assert!(by_second <= 1, "commit 2 judged:\n{asked}");
}

// ---------------------------------------------------------------- NH10

/// A 300-character rationale (ASCII words), numbered `n`.
fn rationale(n: usize) -> String {
    let mut text = format!("Step {n:02}:");
    while text.len() < HISTORY_TEXT_MAX {
        text.push_str(" tighten");
    }
    text.truncate(HISTORY_TEXT_MAX);
    text
}

/// A body of `chars` ASCII characters, in lines.
fn prose(chars: usize) -> String {
    let mut text = String::new();
    let mut n = 0;
    while text.len() < chars {
        n += 1;
        text.push_str(&format!(
            "Line {n:05} of the window's long text, kept as written.\n"
        ));
    }
    text.truncate(chars);
    text.trim_end().to_owned()
}

/// A repository whose R-12 holds a body of `body` characters and 64 apply
/// commits (each a 300-character rationale), decided by `decider`.
fn capped(label: &str, body: usize, decider: &str) -> (Repo, Vec<String>) {
    let mut commits = vec![first(rules(
        &format!("## Window {{#R-12}}\n\n{}\nEdit 0.", prose(body)),
        DOOR_A,
    ))];
    for n in 1..HISTORY_MAX {
        let message = format!(
            "spec: apply PR-{n:04}\n\n{}\n\nProposal: PR-{n:04}\nDecided-by: {decider}\n\
             Proposed-by: {PROPOSED_BY}\nBase-commit: {}\n",
            rationale(n),
            "0".repeat(40)
        );
        commits.push(later(
            n,
            message,
            rules(
                &format!("## Window {{#R-12}}\n\n{}\nEdit {n}.", prose(body)),
                DOOR_A,
            ),
        ));
    }
    seeded(label, &commits)
}

/// The history block of a `spec show` text: the lines after the header up
/// to and including `  history …`.
fn block_lines(text: &str) -> Vec<&str> {
    text.lines()
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .collect()
}

/// NH10 (a): 64 entries of 300-character rationales and a long text: the
/// history is whole, the node's text is cut.
#[test]
fn nh10_the_text_is_cut_first() {
    let (repo, _) = capped("nh10a", 20_000, &ANN.value());
    let (text, document) = repo.history("R-12");
    let node = &document["nodes"][0];
    assert_eq!(entries(&document).len(), HISTORY_MAX);
    assert_eq!(node["history"]["omitted"], json!(0), "{}", node["history"]);
    assert_eq!(node["truncated"], json!(true), "the text is cut");
    for entry in entries(&document).iter().take(HISTORY_MAX - 1) {
        let rationale = entry["rationale"].as_str().expect("a rationale");
        assert_eq!(rationale.chars().count(), HISTORY_TEXT_MAX, "{entry}");
    }
    let block = block_lines(&text.stdout);
    assert_eq!(block.len(), 2 * HISTORY_MAX + 1, "every entry whole");
    assert_eq!(block.last(), Some(&"  history 64"));
    // The tail names the history part whenever it was read, 0 here, as it
    // names links and bindings (`cap.rs`'s convention for both).
    let tail = text.stdout.lines().last().expect("a tail");
    assert!(
        tail.starts_with("[truncated: docs/rules.md lines ")
            && tail.ends_with("; history not shown: 0]"),
        "{tail}"
    );
}

/// NH10 (b): entries whose lines pass the cap: the cut falls after a whole
/// entry, the text is not shown, the tail ends `; history not shown: <k>]`,
/// the JSON holds the printed entries and `omitted` k, within 40 000
/// characters plus the tail.
#[test]
fn nh10_a_history_over_the_cap_is_cut_after_a_whole_entry() {
    let decider = format!(
        "{} <ann@example.org>",
        "Ann Owner of the Long Name".repeat(18)
    );
    let (repo, oids) = capped("nh10b", 2_000, &decider);
    let (text, document) = repo.history("R-12");
    let node = &document["nodes"][0];
    let history = &node["history"];
    let printed = entries(&document).len();
    let omitted = history["omitted"].as_u64().expect("omitted") as usize;
    assert!(printed > 0 && printed < HISTORY_MAX, "{printed} printed");
    assert_eq!(printed + omitted, HISTORY_MAX, "{history}");
    let expected: Vec<String> = oids.iter().rev().take(printed).cloned().collect();
    assert_eq!(ids(&document), expected, "the newest, printed");

    let block = block_lines(&text.stdout);
    assert_eq!(
        block.len(),
        2 * printed,
        "whole entries only (a line and its rationale each), no closing line \
         past the cut:\n{}",
        block.join("\n")
    );
    for pair in block.chunks(2) {
        assert!(pair[0].starts_with("  2026-01-01T"), "{}", pair[0]);
        assert!(pair[1].starts_with("    Step "), "{}", pair[1]);
        assert_eq!(pair[1].len(), 4 + HISTORY_TEXT_MAX, "a whole rationale");
    }
    let tail = text.stdout.lines().last().expect("a tail").to_owned();
    assert!(
        tail.starts_with("[truncated: ")
            && tail.ends_with(&format!("; history not shown: {omitted}]")),
        "{tail}"
    );
    assert!(
        !text.stdout.contains("The window opens") && !text.stdout.contains("Line 00001"),
        "the node's text not shown"
    );
    let chars = text.stdout.chars().count();
    assert!(
        chars <= CAP + tail.chars().count() + 1,
        "{chars} characters"
    );
}
