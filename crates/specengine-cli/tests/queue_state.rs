//! docs/features/queue-export.md AC-01 through AC-11: `spec export state`
//! and `spec import-state`, the queue's backup and restore. Scratch git
//! repositories of `fixtures/spec-a` (`spec-b` where named) as
//! `common::proposal` makes them, a scratch `HOME` for the data directory,
//! the injected clock, consent through the callback; a fresh queue is
//! another scratch `HOME`. The oracle is `SqliteQueue::dump()` (every
//! column of both tables, `SELECT *`). The library runs every command; the
//! binary only where it is the subject (the terminal check, stdout).
//!
//! Every dump is written at run time under the test's scratch directory,
//! never under `fixtures/` (AC-11); no test reads the process `HOME`.
//!
//! Iteration 2 (review of iteration 1): the destination is checked against
//! every worktree of the root's repository and its git directory, by path
//! and by device and inode (a macOS firmlink); only "no repository" falls
//! back to the root, any other git failure refuses; a failed default
//! export removes the directories it made; import reads only a regular
//! file; an `id` outside the queue's numbers is refused; every string is
//! escaped as `serde_json` writes it.
//!
//! Iteration 3: the top ID `PR-18446744073709551615` round-trips and the
//! restored queue refuses the next `propose` as the original does; a
//! `--separate-git-dir` git directory, listed as the main entry, is named
//! "the git directory …".
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::ffi::OsStr;
use std::fs;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::proposal::{LATER, NOW, Pair, cannot, edit, json_of, with_vars};
use common::{RUN_TIMEOUT, Run, SPEC, data_dir, repository_root, snapshot};
use serde_json::Value;
use specengine_cli::{
    ApproveRequest, CliError, Env, Exit, ExportStateOutcome, ExportStateRequest, Globals,
    ImportStateOutcome, ImportStateRequest, InboxRequest, Outcome, ProposalOutcome, RejectRequest,
    ReviewRequest, STATE_FORMAT, approve, export_state, import_state, inbox, propose, reject,
    render_json, render_text, review,
};
use specengine_store::{
    EVENT_COLUMNS, GitEnv, PROPOSAL_COLUMNS, ProposalQueue as _, QUEUE_SCHEMA_VERSION, Restore,
    SqliteQueue, StoredQueue,
};

const TARGET: &str = "EDGE-SPRINT-EMPTY";
const FROM: &str = "the sprint ends;";

// ---------------------------------------------------------------- helpers

fn env_at(home: &Path, cwd: &Path) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

fn db_of(home: &Path, slug: &str) -> PathBuf {
    data_dir(home).join(format!("{slug}.db"))
}

/// Library `export state` with the data directory of `home`, in `cwd`.
fn export_at(
    pair: &Pair,
    home: &Path,
    cwd: &Path,
    out: Option<&Path>,
    now: &str,
) -> Result<ExportStateOutcome, CliError> {
    export_state(
        &env_at(home, cwd),
        &Globals::default(),
        &ExportStateRequest {
            out: out.map(Path::to_path_buf),
            now: now.to_owned(),
            git: pair.git_env(cwd),
        },
    )
}

fn export_ok(
    pair: &Pair,
    home: &Path,
    cwd: &Path,
    out: Option<&Path>,
    now: &str,
) -> ExportStateOutcome {
    export_at(pair, home, cwd, out, now).unwrap_or_else(|error| panic!("export state: {error}"))
}

/// Library `import-state FILE` into the data directory of `home`, in
/// `cwd`, consent `answer`: the outcome and the questions asked.
fn import_at(
    home: &Path,
    cwd: &Path,
    file: &Path,
    answer: bool,
) -> (Result<ImportStateOutcome, CliError>, Vec<String>) {
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        answer
    };
    let outcome = import_state(
        &env_at(home, cwd),
        &Globals::default(),
        &ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut consent,
    );
    (outcome, questions)
}

/// An import that must restore: the outcome.
fn import_ok(home: &Path, cwd: &Path, file: &Path) -> ImportStateOutcome {
    let (outcome, questions) = import_at(home, cwd, file, true);
    let outcome = outcome.unwrap_or_else(|error| panic!("import-state: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1, "{questions:?}");
    outcome
}

/// The oracle: `dump()` of an existing database.
fn queue_dump(db: &Path, slug: &str) -> String {
    assert!(db.exists(), "{} exists", db.display());
    SqliteQueue::open(db, slug)
        .expect("the queue opens")
        .dump()
        .expect("dump")
}

/// Another copy of `fixtures/spec-a` under the pair's scratch, its own git
/// repository (one commit): the same slug, another common dir.
fn repository(pair: &Pair, dir: &str) -> PathBuf {
    let repo = pair.scratch.copy("spec-a", dir);
    pair.git.init(&repo);
    pair.git.add_all(&repo);
    pair.git.commit(&repo, "the fixture");
    repo
}

/// The lines of a dump, each without its line end.
fn lines_of(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8(bytes.to_vec()).expect("a UTF-8 dump");
    let body = text.strip_suffix('\n').expect("a dump ends with LF");
    body.split('\n').map(str::to_owned).collect()
}

/// `lines` joined back into a dump.
fn dump_of(lines: &[String]) -> Vec<u8> {
    let mut out = lines.join("\n");
    out.push('\n');
    out.into_bytes()
}

/// The `(git_common_dir, worktree)` of proposal `id` in the dump `lines`.
fn dumped_place(lines: &[String], id: &str) -> (String, String) {
    for line in &lines[1..] {
        let value: Value = serde_json::from_str(line).expect("a JSON row");
        let Some(row) = value.get("proposals") else {
            continue;
        };
        if row["id"] == id {
            return (
                row["git_common_dir"].as_str().unwrap().to_owned(),
                row["worktree"].as_str().unwrap().to_owned(),
            );
        }
    }
    panic!("no row {id} in the dump");
}

/// `spec args` in `cwd` with the sandbox's variables and `HOME=home`,
/// `input` on a piped stdin (never a terminal), with a watchdog.
fn spec_in(pair: &Pair, home: &Path, cwd: &Path, args: &[&str], input: &[u8]) -> Run {
    spec_with(pair, home, cwd, args, input, &[])
}

/// [`spec_in`] with `extra` variables over the sandbox's.
fn spec_with(
    pair: &Pair,
    home: &Path,
    cwd: &Path,
    args: &[&str],
    input: &[u8],
    extra: &[(&str, &OsStr)],
) -> Run {
    let mut command = Command::new(SPEC);
    command
        .env_clear()
        .envs(pair.git.vars())
        .env("HOME", home)
        .current_dir(cwd)
        .args(args);
    for (name, value) in extra {
        command.env(name, value);
    }
    let label = args.iter().map(|arg| (*arg).to_owned()).collect();
    watched(command, label, input, RUN_TIMEOUT)
}

/// `command` run to its end with `input` on a piped stdin, killed (and
/// the test failed) after `limit`; its exit code and output.
fn watched(mut command: Command, label: Vec<String>, input: &[u8], limit: Duration) -> Run {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn {label:?}: {error}"));
    {
        let mut stdin = child.stdin.take().expect("stdin");
        let _ = stdin.write_all(input);
    }
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
        if let Some(status) = child.try_wait().expect("wait on the child") {
            break status;
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{label:?} ran longer than {limit:?}; killed");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Run {
        args: label,
        code: status.code().expect("exited, not killed by a signal"),
        stdout: String::from_utf8(out.join().expect("stdout")).expect("UTF-8 stdout"),
        stderr: String::from_utf8(err.join().expect("stderr")).expect("UTF-8 stderr"),
    }
}

/// A queue in every state, IDs in creation order:
/// `PR-0001` open (t1), `PR-0002` applied (its commit on t1), `PR-0003`
/// rejected with a reason (`decision_note`), `PR-0004` approved with its
/// commit on t1 (set back by SQL, as proposal-apply AC-16 does), `PR-0005`
/// of a second repository of the slug, `PR-0006` an orphan (its repository
/// deleted), `PR-0007` unreadable (`base_commit` `HEAD`); their events.
struct EveryState {
    pair: Pair,
    repo2: PathBuf,
    events: usize,
}

fn every_state(label: &str) -> EveryState {
    let pair = Pair::new(label, "spec-a");
    let open = pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    assert_eq!(open, "PR-0001");
    let applied = pair.propose_edit(
        &pair.linked,
        TARGET,
        "is not a reference",
        "is never a reference",
    );
    pair.approve_ok(&pair.main, &applied);
    let rejected = pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint stops;");
    let (outcome, _) = pair.reject_answer(&pair.main, &rejected, "Not wanted.", true);
    assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    let approved = pair.propose_edit(
        &pair.linked,
        "RULE-SPRINT-COST",
        "costs 12 units/s",
        "costs 13 units/s",
    );
    pair.approve_ok(&pair.main, &approved);
    pair.sql(&format!(
        "update proposals set status = 'approved', applied_commit = NULL where id = '{approved}'"
    ));
    let repo2 = repository(&pair, "repo2");
    let second = pair.propose_edit(&repo2, TARGET, FROM, "the sprint ends (repo2);");
    let repo3 = repository(&pair, "repo3");
    let orphan = pair.propose_edit(&repo3, TARGET, FROM, "the sprint ends (repo3);");
    fs::remove_dir_all(&repo3).expect("remove repo3");
    let bad = pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends (unreadable);");
    pair.sql(&format!(
        "update proposals set base_commit = 'HEAD' where id = '{bad}'"
    ));
    assert_eq!(
        [
            applied.as_str(),
            &rejected,
            &approved,
            &second,
            &orphan,
            &bad
        ],
        [
            "PR-0002", "PR-0003", "PR-0004", "PR-0005", "PR-0006", "PR-0007"
        ]
    );
    let stored = SqliteQueue::open_existing(pair.db(), &pair.slug)
        .unwrap()
        .unwrap()
        .stored_rows()
        .unwrap();
    let statuses: Vec<Option<&str>> = stored
        .proposals
        .iter()
        .map(|row| row.columns[3].as_deref())
        .collect();
    assert_eq!(
        statuses,
        [
            Some("open"),
            Some("applied"),
            Some("rejected"),
            Some("approved"),
            Some("open"),
            Some("open"),
            Some("open")
        ]
    );
    let events = stored.events.len();
    assert!(events >= 9, "{events} events");
    EveryState {
        pair,
        repo2,
        events,
    }
}

/// The text of every key `"<column>":` appears in `line` in `columns`
/// order (the dump keeps the table's column order).
fn assert_column_order(line: &str, columns: &[&str]) {
    let mut last = 0;
    for column in columns {
        let key = format!("\"{column}\":");
        let at = line[last..]
            .find(&key)
            .unwrap_or_else(|| panic!("{key} after byte {last} in {line}"));
        last += at + key.len();
    }
}

// ------------------------------------------------------------------ AC-01

/// AC-01: every state, a second repository, an orphan, an unreadable row
/// and their events: exported (from either repository: the same bytes,
/// the slug's whole queue), imported into a fresh queue, `dump()` equal;
/// the format of "Data" (header, keys in table order, `seq` a number,
/// `NULL` as `null`); the fresh queue's export is the same file. M: export
/// via `list_readable`; a filter by common dir; `decision_note` dropped.
#[test]
fn ac01_every_state_round_trips_into_a_fresh_queue() {
    let built = every_state("qe-ac01");
    let pair = &built.pair;
    let before = queue_dump(&pair.db(), &pair.slug);
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("all.jsonl");
    let outcome = export_ok(pair, &pair.home, &pair.main, Some(&file), NOW);
    assert_eq!(outcome.path, file.display().to_string());
    assert_eq!(
        (outcome.proposals, outcome.events),
        (7, built.events as u64)
    );
    assert_eq!(
        queue_dump(&pair.db(), &pair.slug),
        before,
        "export changes nothing"
    );
    let bytes = fs::read(&file).expect("the dump");

    let from_repo2 = dumps.join("repo2.jsonl");
    export_ok(pair, &pair.home, &built.repo2, Some(&from_repo2), NOW);
    assert_eq!(
        fs::read(&from_repo2).unwrap(),
        bytes,
        "the slug's whole queue"
    );

    let lines = lines_of(&bytes);
    assert_eq!(
        lines[0],
        format!(
            "{{\"format\":{STATE_FORMAT},\"queue_schema\":{QUEUE_SCHEMA_VERSION},\"project\":\"lantern-keep\",\"proposals\":7,\"events\":{}}}",
            built.events
        )
    );
    assert_eq!(lines.len(), 1 + 7 + built.events);
    for (index, line) in lines[1..8].iter().enumerate() {
        assert!(
            line.starts_with(&format!(
                "{{\"proposals\":{{\"id\":\"PR-{:04}\",\"project\":\"lantern-keep\",",
                index + 1
            )),
            "{line}"
        );
        assert_column_order(line, &PROPOSAL_COLUMNS);
        assert!(line.ends_with("}}"), "{line}");
    }
    assert!(lines[1].contains("\"decision_note\":null"), "{}", lines[1]);
    assert!(
        lines[3].contains("\"decision_note\":\"Not wanted.\""),
        "{}",
        lines[3]
    );
    assert!(
        lines[7].contains("\"base_commit\":\"HEAD\""),
        "{}",
        lines[7]
    );
    for (index, line) in lines[8..].iter().enumerate() {
        assert!(
            line.starts_with(&format!(
                "{{\"events\":{{\"seq\":{},\"project\":\"lantern-keep\",\"type\":",
                index + 1
            )),
            "{line}"
        );
        assert_column_order(line, &["seq", "project", "type", "payload", "at"]);
        let value: Value = serde_json::from_str(line).unwrap();
        assert!(value["events"]["payload"].is_string(), "{line}");
    }

    let fresh = pair.scratch.home("fresh");
    let db = db_of(&fresh, &pair.slug);
    let (imported, questions) = import_at(&fresh, &pair.main, &file, true);
    let imported = imported.unwrap_or_else(|error| panic!("import-state: {error}"));
    assert_eq!(imported.exit(), Exit::Answered);
    assert_eq!(imported.db, db.display().to_string());
    assert_eq!(
        (imported.proposals, imported.events),
        (7, built.events as u64)
    );
    assert_eq!(
        questions,
        [format!(
            "restore 7 proposal(s) and {} event(s) of lantern-keep from {} into {}? [y/N]",
            built.events,
            file.display(),
            db.display()
        )]
    );
    assert_eq!(queue_dump(&db, &pair.slug), before, "the queue as it was");

    let again = dumps.join("again.jsonl");
    export_ok(pair, &fresh, &pair.main, Some(&again), LATER);
    assert_eq!(fs::read(&again).unwrap(), bytes, "the fresh queue's dump");
}

// ------------------------------------------------------------------ AC-02

/// AC-02: equal rows inserted in another order (the opposite one, through
/// the store; and through a dump whose rows are reversed), exported later:
/// byte-identical files; a later default export of the same queue too.
/// M: `ORDER BY rowid`; the export time inside.
#[test]
fn ac02_equal_queues_give_equal_bytes() {
    let pair = Pair::new("qe-ac02", "spec-a");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let applied = pair.propose_edit(
        &pair.linked,
        TARGET,
        "is not a reference",
        "is never a reference",
    );
    pair.approve_ok(&pair.main, &applied);
    let rejected = pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint stops;");
    let (outcome, _) = pair.reject_answer(&pair.main, &rejected, "No.", true);
    assert_eq!(outcome.unwrap().exit(), Exit::Answered);
    let dumps = pair.scratch.dir("dumps");
    let first = dumps.join("first.jsonl");
    export_ok(&pair, &pair.home, &pair.main, Some(&first), NOW);
    let bytes = fs::read(&first).unwrap();

    let stored = SqliteQueue::open_existing(pair.db(), &pair.slug)
        .unwrap()
        .unwrap()
        .stored_rows()
        .unwrap();
    assert_eq!(stored.proposals.len(), 3);
    let mut reversed = stored.clone();
    reversed.proposals.reverse();
    reversed.events.reverse();
    let other = pair.scratch.home("other");
    let other_db = db_of(&other, &pair.slug);
    fs::create_dir_all(other_db.parent().unwrap()).unwrap();
    let mut queue = SqliteQueue::open(&other_db, &pair.slug).unwrap();
    assert_eq!(queue.restore(&reversed).unwrap(), Restore::Restored);
    drop(queue);
    let later = dumps.join("later.jsonl");
    export_ok(&pair, &other, &pair.main, Some(&later), LATER);
    assert_eq!(fs::read(&later).unwrap(), bytes, "another insertion order");

    // A dump with its rows reversed restores the same queue.
    let lines = lines_of(&bytes);
    let mut rows: Vec<String> = lines[1..].to_vec();
    rows.reverse();
    let mut shuffled = vec![lines[0].clone()];
    shuffled.extend(rows);
    let shuffled_file = dumps.join("shuffled.jsonl");
    fs::write(&shuffled_file, dump_of(&shuffled)).unwrap();
    let third = pair.scratch.home("third");
    import_ok(&third, &pair.main, &shuffled_file);
    let from_third = dumps.join("third.jsonl");
    export_ok(&pair, &third, &pair.main, Some(&from_third), LATER);
    assert_eq!(fs::read(&from_third).unwrap(), bytes, "a reversed dump");

    // The default destination, a later clock: another name, the same bytes.
    let default = export_ok(&pair, &pair.home, &pair.main, None, LATER);
    assert!(
        default
            .path
            .ends_with("/backups/lantern-keep-20261006T080000Z.jsonl"),
        "{}",
        default.path
    );
    assert_eq!(fs::read(&default.path).unwrap(), bytes, "a later export");
}

// ------------------------------------------------------------------ AC-03

/// AC-03: restored highest `PR-0007`, `seq` N: the next `propose` prints
/// `PR-0008`, its event `seq` N+1; the import adds no event. A dump with
/// gaps (`PR-0001`, `PR-0002`, `PR-0007`) continues at `PR-0008` too.
/// M: IDs renumbered from `PR-0001`; an event of the import's own.
#[test]
fn ac03_ids_and_seq_continue_after_the_highest_restored() {
    let built = every_state("qe-ac03");
    let pair = &built.pair;
    let n = built.events as i64;
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("all.jsonl");
    export_ok(pair, &pair.home, &pair.main, Some(&file), NOW);

    let next_after = |home: &Path, events_before: i64| {
        let db = db_of(home, &pair.slug);
        let queue = SqliteQueue::open(&db, &pair.slug).unwrap();
        let events = queue.events().unwrap();
        assert_eq!(events.len() as i64, events_before, "no event of the import");
        assert_eq!(events.last().map(|event| event.seq), Some(n));
        drop(queue);
        let (hash, text) = pair.span(&pair.linked, TARGET);
        let new_text = edit(&text, FROM, "the sprint ends after import;");
        let outcome = propose(
            &env_at(home, &pair.linked),
            &Globals::default(),
            &pair.request(&pair.linked, TARGET, &hash, &new_text),
        )
        .unwrap_or_else(|error| panic!("propose: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        let printed = render_text(&Outcome::Proposal(Box::new(outcome)));
        assert!(printed.starts_with("PR-0008\n"), "{printed}");
        let queue = SqliteQueue::open(&db, &pair.slug).unwrap();
        let last = queue.events().unwrap().pop().unwrap();
        assert_eq!(last.seq, n + 1);
        assert_eq!(last.event_type, "proposal.created");
        assert_eq!(last.payload["id"], "PR-0008");
    };

    let fresh = pair.scratch.home("fresh");
    import_ok(&fresh, &pair.main, &file);
    next_after(&fresh, n);

    // Gaps: only PR-0001, PR-0002 and PR-0007 of the proposals.
    let lines = lines_of(&fs::read(&file).unwrap());
    let header = lines[0].replace("\"proposals\":7,", "\"proposals\":3,");
    assert_ne!(header, lines[0]);
    let mut gapped = vec![header];
    for line in &lines[1..] {
        let dropped = ["PR-0003", "PR-0004", "PR-0005", "PR-0006"]
            .iter()
            .any(|id| line.starts_with(&format!("{{\"proposals\":{{\"id\":\"{id}\"")));
        if !dropped {
            gapped.push(line.clone());
        }
    }
    let gapped_file = dumps.join("gapped.jsonl");
    fs::write(&gapped_file, dump_of(&gapped)).unwrap();
    let gaps = pair.scratch.home("gaps");
    let outcome = import_ok(&gaps, &pair.main, &gapped_file);
    assert_eq!((outcome.proposals, outcome.events), (3, n as u64));
    let ids: Vec<String> = SqliteQueue::open_existing(db_of(&gaps, &pair.slug), &pair.slug)
        .unwrap()
        .unwrap()
        .stored_rows()
        .unwrap()
        .proposals
        .iter()
        .map(|row| row.id().unwrap().to_owned())
        .collect();
    assert_eq!(ids, ["PR-0001", "PR-0002", "PR-0007"], "IDs as stored");
    next_after(&gaps, n);
}

// ------------------------------------------------------------------ AC-04

/// AC-04: a non-empty queue: exit 2 naming both counts, no prompt,
/// `dump()` unchanged — the slug's own rows, another project's rows in the
/// same DB, and rows that arrive while the owner answers (the re-check in
/// the write transaction). M: insert-or-ignore; either emptiness check
/// removed.
#[test]
fn ac04_a_non_empty_queue_is_refused_and_left_as_it_was() {
    let pair = Pair::new("qe-ac04", "spec-a");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let rejected = pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint stops;");
    let (outcome, _) = pair.reject_answer(&pair.main, &rejected, "No.", true);
    assert_eq!(outcome.unwrap().exit(), Exit::Answered);
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("q.jsonl");
    let exported = export_ok(&pair, &pair.home, &pair.main, Some(&file), NOW);
    assert_eq!((exported.proposals, exported.events), (2, 3));

    // Into the same, non-empty queue.
    let db = pair.db();
    let before = queue_dump(&db, &pair.slug);
    let (outcome, questions) = import_at(&pair.home, &pair.main, &file, true);
    let message = cannot(&outcome, "import into a non-empty queue");
    assert_eq!(
        message,
        format!(
            "spec: the queue of `lantern-keep` in {db} holds 2 proposal(s), 3 event(s): \
             import-state restores only into an empty queue (a fresh data directory, or {db} \
             moved aside); nothing changed",
            db = db.display()
        )
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(queue_dump(&db, &pair.slug), before);

    // Another project's row in the DB counts too.
    let foreign_home = pair.scratch.home("foreign");
    let foreign_db = db_of(&foreign_home, &pair.slug);
    fs::create_dir_all(foreign_db.parent().unwrap()).unwrap();
    let mut row = SqliteQueue::open_existing(&db, &pair.slug)
        .unwrap()
        .unwrap()
        .stored_rows()
        .unwrap()
        .proposals
        .remove(0);
    row.columns[1] = Some("other".to_owned());
    let mut queue = SqliteQueue::open(&foreign_db, "other").unwrap();
    let state = StoredQueue {
        proposals: vec![row],
        events: Vec::new(),
    };
    assert_eq!(queue.restore(&state).unwrap(), Restore::Restored);
    drop(queue);
    let before = queue_dump(&foreign_db, &pair.slug);
    let (outcome, questions) = import_at(&foreign_home, &pair.main, &file, true);
    let message = cannot(
        &outcome,
        "import into a queue holding another project's row",
    );
    assert!(
        message.contains("holds 1 proposal(s), 0 event(s)"),
        "{message}"
    );
    assert!(questions.is_empty(), "no prompt: {questions:?}");
    assert_eq!(queue_dump(&foreign_db, &pair.slug), before);

    // A proposal raised while the owner answers: refused in the write
    // transaction, nothing inserted.
    let race = pair.scratch.home("race");
    let race_db = db_of(&race, &pair.slug);
    let (hash, text) = pair.span(&pair.linked, TARGET);
    let raced = edit(&text, FROM, "the sprint ends meanwhile;");
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        let outcome = propose(
            &env_at(&race, &pair.linked),
            &Globals::default(),
            &pair.request(&pair.linked, TARGET, &hash, &raced),
        )
        .expect("the raced propose");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        true
    };
    let outcome = import_state(
        &env_at(&race, &pair.main),
        &Globals::default(),
        &ImportStateRequest { file: file.clone() },
        &mut consent,
    );
    let message = cannot(&outcome, "a row arrived during the question");
    assert!(
        message.contains("holds 1 proposal(s), 1 event(s)"),
        "{message}"
    );
    assert_eq!(questions.len(), 1);
    let stored = SqliteQueue::open_existing(&race_db, &pair.slug)
        .unwrap()
        .unwrap()
        .stored_rows()
        .unwrap();
    assert_eq!(stored.counts().proposals, 1, "only the raced proposal");
    assert_eq!(stored.counts().events, 1);
    let new_text = PROPOSAL_COLUMNS
        .iter()
        .position(|column| *column == "new_text")
        .unwrap();
    assert_eq!(
        stored.proposals[0].columns[new_text].as_deref(),
        Some(raced.as_str())
    );
}

// ------------------------------------------------------------------ AC-05

/// The dump of a small queue (two proposals, one rejected; three events)
/// in `pair`'s scratch: its path and lines.
fn small_dump(pair: &Pair) -> (PathBuf, Vec<String>) {
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let rejected = pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint stops;");
    let (outcome, _) = pair.reject_answer(&pair.main, &rejected, "No.", true);
    assert_eq!(outcome.unwrap().exit(), Exit::Answered);
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("valid.jsonl");
    export_ok(pair, &pair.home, &pair.main, Some(&file), NOW);
    let lines = lines_of(&fs::read(&file).unwrap());
    assert_eq!(lines.len(), 1 + 2 + 3);
    (file, lines)
}

/// The import of `bytes` (written to `<dumps>/<name>.jsonl`) into a fresh
/// queue: exit 2, its message; no prompt, no data directory made.
fn refused_import(pair: &Pair, name: &str, bytes: &[u8]) -> (String, String) {
    let file = pair.scratch.dir("dumps").join(format!("{name}.jsonl"));
    fs::write(&file, bytes).unwrap();
    let fresh = pair.scratch.home(&format!("fresh-{name}"));
    let (outcome, questions) = import_at(&fresh, &pair.main, &file, true);
    let message = cannot(&outcome, name);
    assert!(questions.is_empty(), "{name}: no prompt: {questions:?}");
    assert!(
        !data_dir(&fresh).exists(),
        "{name}: nothing created under the fresh HOME"
    );
    (file.display().to_string(), message)
}

/// `line` (a JSON row) with `edit` applied to its value, compact again.
fn with_value(line: &str, edit: impl FnOnce(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(line).unwrap();
    edit(&mut value);
    value.to_string()
}

/// AC-05: one defect on a valid dump's last line — not JSON, an unknown
/// table, an unknown or a missing column, a number in a TEXT column, a
/// repeated `id`, a repeated `seq`, `project` not the header's — each exit
/// 2 `<FILE>:<line>: …` naming the line and never quoting it; cut after a
/// whole row: exit 2 naming the counts; nothing asked, the queue (and its
/// data directory) never made. M: commit per row (store `queue_state.rs`);
/// any one check dropped; the count check removed.
#[test]
fn ac05_a_defective_dump_is_refused_naming_the_line() {
    let pair = Pair::new("qe-ac05", "spec-a");
    let (_, lines) = small_dump(&pair);
    let last = lines.len();
    let head = &lines[..last - 1];
    let event = lines[last - 1].clone();
    let previous_seq = {
        let value: Value = serde_json::from_str(&lines[last - 2]).unwrap();
        value["events"]["seq"].clone()
    };
    let secret = "SECRET-MARKER";
    let cases: Vec<(&str, String, &str)> = vec![
        ("not-json", "{\"events\":{\"seq\":".to_owned(), "not JSON"),
        (
            "unknown-table",
            event.replacen("{\"events\":", &format!("{{\"{secret}\":"), 1),
            "unknown table",
        ),
        (
            "unknown-column",
            with_value(&event, |value| {
                value["events"][secret] = Value::from(secret);
            }),
            "a column the table does not have",
        ),
        (
            "missing-column",
            with_value(&event, |value| {
                value["events"].as_object_mut().unwrap().remove("at");
            }),
            "has no `at`",
        ),
        (
            "number-in-text",
            with_value(&event, |value| value["events"]["type"] = Value::from(5)),
            "`type` is not a string or null",
        ),
        ("repeated-id", lines[1].clone(), "`id` repeats"),
        (
            "repeated-seq",
            with_value(&event, |value| {
                value["events"]["seq"] = previous_seq.clone();
                value["events"]["payload"] = Value::from(secret);
            }),
            "`seq` repeats",
        ),
        (
            "project-not-header",
            with_value(&event, |value| {
                value["events"]["project"] = Value::from("zerkalo");
                value["events"]["payload"] = Value::from(secret);
            }),
            "`project` is not the header's",
        ),
    ];
    for (name, line, defect) in cases {
        let mut bad = head.to_vec();
        bad.push(line);
        let (label, message) = refused_import(&pair, name, &dump_of(&bad));
        assert!(
            message.starts_with(&format!("{label}:{last}: ")),
            "{name}: {message}"
        );
        assert!(message.contains(defect), "{name}: {message}");
        assert!(
            !message.contains(secret),
            "{name}: the line quoted: {message}"
        );
        assert!(!message.contains("PR-0001"), "{name}: {message}");
    }

    // Cut after a whole row.
    let (label, message) = refused_import(&pair, "cut", &dump_of(head));
    assert_eq!(message, format!("{label}: header counts 2, 3; found 2, 2"));
}

/// Import step 2's other defects, each exit 2 naming the line: not UTF-8,
/// an empty line, no final LF, an empty file, no header, a header with
/// another key, `seq` 0, `seq` 1.0, an `id` the queue never writes, a
/// repeated key, an array.
#[test]
fn every_other_defect_of_step_2_names_its_line() {
    let pair = Pair::new("qe-defects", "spec-a");
    let (_, lines) = small_dump(&pair);
    let last = lines.len();
    let bytes = dump_of(&lines);
    let event = lines[last - 1].clone();
    let seq = {
        let value: Value = serde_json::from_str(&event).unwrap();
        value["events"]["seq"].as_u64().unwrap()
    };
    let with_last = |line: Vec<u8>| {
        let mut out = dump_of(&lines[..last - 1]);
        out.extend(line);
        out.push(b'\n');
        out
    };
    let mut not_utf8 = event.clone().into_bytes();
    not_utf8.insert(20, 0xff);
    let header_extra = lines[0].replacen('}', ",\"host\":\"h\"}", 1);
    let mut no_header = lines[1..].to_vec();
    no_header.push(lines[0].clone());
    let bad_id = lines[1].replacen("\"id\":\"PR-0001\"", "\"id\":\"PR-1\"", 1);
    let mut bad_id_lines = lines.clone();
    bad_id_lines[1] = bad_id;
    let cases: Vec<(&str, Vec<u8>, usize, &str)> = vec![
        ("not-utf8", with_last(not_utf8), last, "not UTF-8"),
        ("empty-line", with_last(Vec::new()), last, "an empty line"),
        (
            "no-final-lf",
            bytes[..bytes.len() - 1].to_vec(),
            last,
            "no line end",
        ),
        ("empty-file", Vec::new(), 1, "empty"),
        ("no-header", dump_of(&no_header), 1, "no header"),
        (
            "header-extra-key",
            dump_of(&[vec![header_extra], lines[1..].to_vec()].concat()),
            1,
            "header's keys",
        ),
        (
            "seq-zero",
            with_last(
                event
                    .replacen(&format!("\"seq\":{seq}"), "\"seq\":0", 1)
                    .into_bytes(),
            ),
            last,
            "`seq` is not an integer",
        ),
        (
            "seq-float",
            with_last(
                event
                    .replacen(&format!("\"seq\":{seq}"), &format!("\"seq\":{seq}.0"), 1)
                    .into_bytes(),
            ),
            last,
            "`seq` is not an integer",
        ),
        (
            "bad-id",
            dump_of(&bad_id_lines),
            2,
            "`id` is not a proposal ID",
        ),
        (
            "repeated-key",
            with_last(
                event
                    .replacen(
                        &format!("\"seq\":{seq}"),
                        &format!("\"seq\":{seq},\"seq\":{seq}"),
                        1,
                    )
                    .into_bytes(),
            ),
            last,
            "repeats `seq`",
        ),
        (
            "array",
            with_last(b"[1,2]".to_vec()),
            last,
            "not a JSON object",
        ),
    ];
    for (name, bad, line, defect) in cases {
        let (label, message) = refused_import(&pair, name, &bad);
        assert!(
            message.starts_with(&format!("{label}:{line}: ")),
            "{name}: {message}"
        );
        assert!(message.contains(defect), "{name}: {message}");
    }
}

// ------------------------------------------------------------------ AC-06

/// AC-06: `format` 2 and `queue_schema` 4 (newer than this build's 3,
/// docs/features/decision-apply.md "Data"): exit 2 with `upgrade
/// SpecEngine`; a spec-b dump into spec-a: exit 2 naming both slugs; no
/// prompt, the queue never made. M: either version check removed; the slug
/// check removed.
#[test]
fn ac06_a_newer_or_another_projects_dump_is_refused() {
    let pair = Pair::new("qe-ac06", "spec-a");
    let (_, lines) = small_dump(&pair);
    for (name, from, to) in [
        ("format-2", "\"format\":1,", "\"format\":2,"),
        ("schema-4", "\"queue_schema\":3,", "\"queue_schema\":4,"),
    ] {
        let mut newer = lines.clone();
        newer[0] = newer[0].replacen(from, to, 1);
        assert_ne!(newer[0], lines[0], "{name}");
        let (label, message) = refused_import(&pair, name, &dump_of(&newer));
        assert!(
            message.starts_with(&format!("{label}:1: ")),
            "{name}: {message}"
        );
        assert!(message.contains("upgrade SpecEngine"), "{name}: {message}");
    }

    let other = Pair::new("qe-ac06-b", "spec-b");
    other.propose_edit(&other.linked, "CMD-SYNC", "`sync`", "`sync --all`");
    let other_file = other.scratch.dir("dumps").join("zerkalo.jsonl");
    export_ok(&other, &other.home, &other.main, Some(&other_file), NOW);
    let bytes = fs::read(&other_file).unwrap();
    assert!(bytes.starts_with(b"{\"format\":1,\"queue_schema\":3,\"project\":\"zerkalo\","));
    let (label, message) = refused_import(&pair, "spec-b", &bytes);
    assert!(message.starts_with(&format!("{label}:1: ")), "{message}");
    assert!(
        message.contains("`zerkalo`") && message.contains("`lantern-keep`"),
        "{message}"
    );
}

// ------------------------------------------------------------------ AC-07

/// AC-07: an `approved` proposal with its commit on its branch, exported
/// and imported fresh: both worktrees clean, `HEAD`, every ref and both
/// git indexes as they were, the proposal still `approved` with no
/// `applied_commit` (nothing completed it). M: the import runs the
/// completion lookup.
#[test]
fn ac07_an_approved_proposal_stays_approved_and_git_is_untouched() {
    let pair = Pair::new("qe-ac07", "spec-a");
    let id = pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    pair.approve_ok(&pair.main, &id);
    pair.sql("update proposals set status = 'approved', applied_commit = NULL");
    assert_eq!(pair.proposal(&id).status.as_str(), "approved");
    let file = pair.scratch.dir("dumps").join("q.jsonl");
    export_ok(&pair, &pair.home, &pair.main, Some(&file), NOW);
    let before = pair.state();
    let index_files = [
        pair.main.join(".git/index"),
        pair.main.join(".git/worktrees/t1/index"),
    ];
    let mtimes = |files: &[PathBuf]| -> Vec<_> {
        files
            .iter()
            .map(|path| fs::metadata(path).unwrap().modified().unwrap())
            .collect()
    };
    let indexes = mtimes(&index_files);
    let tip = pair.rev(&pair.main, "t1");

    let fresh = pair.scratch.home("fresh");
    import_ok(&fresh, &pair.main, &file);
    // Before any git command of the test (`git status` may refresh an
    // index itself).
    assert_eq!(mtimes(&index_files), indexes, "no git index touched");
    assert_eq!(pair.state(), before, "worktrees, refs, HEAD, status");
    assert_eq!(before.main_status, "");
    assert_eq!(before.linked_status, "");
    assert_eq!(pair.rev(&pair.main, "t1"), tip);
    let restored = SqliteQueue::open(db_of(&fresh, &pair.slug), &pair.slug)
        .unwrap()
        .list(&Default::default())
        .unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].status.as_str(), "approved");
    assert_eq!(restored[0].applied_commit, None);
}

// ------------------------------------------------------------------ AC-08

/// AC-08: `--out` in a missing subdirectory of the worktree, through a
/// symlink into it, in the linked worktree, in the repository top above a
/// root in a subdirectory, inside a root without git: exit 2 naming the
/// worktree (or root), nothing written; an existing file: exit 2, its bytes
/// untouched; a missing parent outside: exit 2. Written: 0600, `backups/`
/// 0700, no `.partial` left; stdout names the path and the counts (text,
/// `--json`, the binary). M: the worktree check removed; a truncating
/// write; the partial kept.
#[test]
fn ac08_the_destination_is_new_outside_the_worktree_and_private() {
    let pair = Pair::new("qe-ac08", "spec-a");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let main_before = snapshot(&pair.main);
    let linked_before = snapshot(&pair.linked);

    // Inside the worktree: a missing subdirectory, a symlink, the linked
    // worktree.
    let outside = pair.scratch.dir("outside");
    std::os::unix::fs::symlink(pair.main.join("docs"), outside.join("link")).unwrap();
    for (cwd, out, top) in [
        (&pair.main, PathBuf::from("missing/sub/q.jsonl"), &pair.main),
        (&pair.main, outside.join("link/q.jsonl"), &pair.main),
        (
            &pair.main,
            PathBuf::from("../outside/link/q.jsonl"),
            &pair.main,
        ),
        (&pair.linked, PathBuf::from("q.jsonl"), &pair.linked),
    ] {
        let outcome = export_at(&pair, &pair.home, cwd, Some(&out), NOW);
        let message = cannot(&outcome, &out.display().to_string());
        assert!(
            message.contains(&format!("lies inside the worktree {}", top.display())),
            "{}: {message}",
            out.display()
        );
        assert!(message.ends_with("nothing written"), "{message}");
    }
    assert_eq!(snapshot(&pair.main), main_before, "nothing written in main");
    assert_eq!(
        snapshot(&pair.linked),
        linked_before,
        "nothing written in t1"
    );
    assert!(!pair.main.join("missing").exists());

    // An existing file: refused, untouched; a missing parent outside.
    let kept = outside.join("kept.jsonl");
    fs::write(&kept, b"keep me\n").unwrap();
    let message = cannot(
        &export_at(&pair, &pair.home, &pair.main, Some(&kept), NOW),
        "an existing file",
    );
    assert!(message.contains("exists"), "{message}");
    assert_eq!(fs::read(&kept).unwrap(), b"keep me\n");
    let message = cannot(
        &export_at(
            &pair,
            &pair.home,
            &pair.main,
            Some(&outside.join("none/q.jsonl")),
            NOW,
        ),
        "a missing parent",
    );
    assert!(message.contains("does not exist"), "{message}");
    assert!(!outside.join("none").exists());

    // A new file outside: written private, no partial; the outcome printed.
    let out = outside.join("q.jsonl");
    let outcome = export_ok(&pair, &pair.home, &pair.main, Some(&out), NOW);
    assert_eq!(mode(&out), 0o600);
    assert_no_partial(&outside);
    assert_eq!(
        render_text(&Outcome::StateExport(outcome.clone())),
        format!("wrote {}: 1 proposal(s), 1 event(s)\n", out.display())
    );
    let json = json_of(&render_json(&Outcome::StateExport(outcome)));
    assert_eq!(
        json,
        serde_json::json!({"path": out.display().to_string(), "proposals": 1, "events": 1})
    );

    // The default destination: `backups/` 0700, the file 0600, no partial;
    // the same clock again: exists, refused, untouched.
    let backups = data_dir(&pair.home).join("backups");
    let default = backups.join("lantern-keep-20261005T211403Z.jsonl");
    let outcome = export_ok(&pair, &pair.home, &pair.main, None, NOW);
    assert_eq!(outcome.path, default.display().to_string());
    assert_eq!(mode(&backups), 0o700);
    assert_eq!(mode(&default), 0o600);
    assert_no_partial(&backups);
    let bytes = fs::read(&default).unwrap();
    assert_eq!(bytes, fs::read(&out).unwrap());
    let message = cannot(
        &export_at(&pair, &pair.home, &pair.main, None, NOW),
        "the same second again",
    );
    assert!(message.contains("exists"), "{message}");
    assert_eq!(fs::read(&default).unwrap(), bytes);

    // The binary: one line on stdout, never the dump.
    let by_binary = outside.join("binary.jsonl");
    let run = spec_in(
        &pair,
        &pair.home,
        &pair.main,
        &["export", "state", "--out", by_binary.to_str().unwrap()],
        b"",
    );
    run.code(0);
    assert_eq!(
        run.stdout,
        format!("wrote {}: 1 proposal(s), 1 event(s)\n", by_binary.display())
    );
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(fs::read(&by_binary).unwrap(), bytes);
    let by_json = outside.join("binary-json.jsonl");
    let run = spec_in(
        &pair,
        &pair.home,
        &pair.main,
        &[
            "--json",
            "export",
            "state",
            "--out",
            by_json.to_str().unwrap(),
        ],
        b"",
    );
    run.code(0);
    assert_eq!(
        run.json(),
        serde_json::json!({"path": by_json.display().to_string(), "proposals": 1, "events": 1})
    );
    let run = spec_in(
        &pair,
        &pair.home,
        &pair.main,
        &["export", "state", "--out", "inside.jsonl"],
        b"",
    );
    run.code(2);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr.contains("lies inside the worktree"),
        "{}",
        run.show()
    );
    assert!(!pair.main.join("inside.jsonl").exists());
}

/// The worktree top, not the root: a root in a subdirectory of its
/// repository refuses the repository's top; a root without git refuses
/// itself and writes outside.
#[test]
fn ac08_the_worktree_top_or_the_root_without_git_bounds_the_destination() {
    let pair = Pair::new("qe-ac08-top", "spec-a");
    let repo = pair.scratch.dir("nested");
    common::copy_dir(&common::fixture("spec-a"), &repo.join("sub"));
    pair.git.init(&repo);
    pair.git.add_all(&repo);
    pair.git.commit(&repo, "nested");
    let root = repo.join("sub");
    let message = cannot(
        &export_at(&pair, &pair.home, &root, Some(Path::new("../q.jsonl")), NOW),
        "the repository's top above the root",
    );
    assert!(
        message.contains(&format!("lies inside the worktree {}", repo.display())),
        "{message}"
    );
    assert!(!repo.join("q.jsonl").exists());

    let plain = pair.scratch.copy("spec-a", "plain");
    let message = cannot(
        &export_at(&pair, &pair.home, &plain, Some(Path::new("q.jsonl")), NOW),
        "inside a root without git",
    );
    assert!(
        message.contains(&format!("lies inside the worktree {}", plain.display())),
        "{message}"
    );
    assert!(!plain.join("q.jsonl").exists());
    let out = pair.scratch.dir("out").join("q.jsonl");
    let outcome = export_ok(&pair, &pair.home, &plain, Some(&out), NOW);
    assert_eq!((outcome.proposals, outcome.events), (0, 0));
    assert_eq!(
        fs::read(&out).unwrap(),
        b"{\"format\":1,\"queue_schema\":3,\"project\":\"lantern-keep\",\"proposals\":0,\"events\":0}\n"
    );
    assert!(
        !data_dir(&pair.home).exists(),
        "no queue: an empty dump, no data directory made"
    );
}

/// "Export": a row of another project in the slug's DB (a proposal, or an
/// event alone) is exit 2 naming it, nothing written. Accepted deviation
/// 3: both commands refuse a `--config` other than the root's own.
#[test]
fn export_refuses_another_projects_row_and_both_refuse_another_config() {
    let pair = Pair::new("qe-foreign", "spec-a");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let stored = SqliteQueue::open_existing(pair.db(), &pair.slug)
        .unwrap()
        .unwrap()
        .stored_rows()
        .unwrap();
    let outside = pair.scratch.dir("outside");
    let mut proposal = stored.proposals[0].clone();
    proposal.columns[1] = Some("other".to_owned());
    let mut event = stored.events[0].clone();
    event.columns[0] = Some("other".to_owned());
    for (name, state, named) in [
        (
            "proposal",
            StoredQueue {
                proposals: vec![proposal],
                events: Vec::new(),
            },
            "proposal `PR-0001`",
        ),
        (
            "event",
            StoredQueue {
                proposals: Vec::new(),
                events: vec![event],
            },
            "event 1",
        ),
    ] {
        let home = pair.scratch.home(name);
        let db = db_of(&home, &pair.slug);
        fs::create_dir_all(db.parent().unwrap()).unwrap();
        let mut queue = SqliteQueue::open(&db, "other").unwrap();
        assert_eq!(queue.restore(&state).unwrap(), Restore::Restored);
        drop(queue);
        let out = outside.join(format!("{name}.jsonl"));
        let message = cannot(&export_at(&pair, &home, &pair.main, Some(&out), NOW), name);
        assert!(
            message.contains(named) && message.contains("`other`"),
            "{name}: {message}"
        );
        assert!(message.ends_with("nothing written"), "{message}");
        assert!(!out.exists());
        assert_no_partial(&outside);
    }

    let other_config = outside.join("other.toml");
    common::copy_file(pair.main.join("specengine.toml"), &other_config);
    let globals = Globals {
        root: None,
        config: Some(other_config.clone()),
    };
    let out = outside.join("config.jsonl");
    let exported = export_state(
        &env_at(&pair.home, &pair.main),
        &globals,
        &ExportStateRequest {
            out: Some(out.clone()),
            now: NOW.to_owned(),
            git: pair.git_env(&pair.main),
        },
    );
    assert!(cannot(&exported, "export --config").contains("--config"));
    assert!(!out.exists());
    let file = outside.join("valid.jsonl");
    export_ok(&pair, &pair.home, &pair.main, Some(&file), NOW);
    let fresh = pair.scratch.home("fresh");
    let mut asked = Vec::new();
    let imported = import_state(
        &env_at(&fresh, &pair.main),
        &globals,
        &ImportStateRequest { file },
        &mut |question: &str| {
            asked.push(question.to_owned());
            true
        },
    );
    assert!(cannot(&imported, "import-state --config").contains("--config"));
    assert!(asked.is_empty());
    assert!(!data_dir(&fresh).exists());
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn assert_no_partial(dir: &Path) {
    let partials: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".partial"))
        .collect();
    assert!(partials.is_empty(), "left behind: {partials:?}");
}

// ------------------------------------------------------------------ AC-09

/// AC-09: a proposal whose recorded common dir is gone, restored as
/// stored: `inbox` notes it, `review` and `approve` exit 2 (the orphan),
/// `reject` takes it; its `git_common_dir` and `worktree` byte-equal to the
/// dump's. M: the import rewrites paths.
#[test]
fn ac09_an_orphan_is_restored_as_stored_and_only_reject_takes_it() {
    let pair = Pair::new("qe-ac09", "spec-a");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let repo3 = repository(&pair, "repo3");
    let orphan = pair.propose_edit(&repo3, TARGET, FROM, "the sprint ends (gone);");
    assert_eq!(orphan, "PR-0002");
    let file = pair.scratch.dir("dumps").join("q.jsonl");
    export_ok(&pair, &pair.home, &pair.main, Some(&file), NOW);
    fs::remove_dir_all(&repo3).unwrap();
    let lines = lines_of(&fs::read(&file).unwrap());
    let (common_dir, worktree) = dumped_place(&lines, &orphan);
    assert!(!Path::new(&common_dir).exists());

    let fresh = pair.scratch.home("fresh");
    import_ok(&fresh, &pair.main, &file);
    let stored = |id: &str| {
        let rows = SqliteQueue::open_existing(db_of(&fresh, &pair.slug), &pair.slug)
            .unwrap()
            .unwrap()
            .stored_rows()
            .unwrap();
        let row = rows
            .proposals
            .into_iter()
            .find(|row| row.id() == Some(id))
            .unwrap();
        (
            row.columns[6].clone().unwrap(),
            row.columns[7].clone().unwrap(),
        )
    };
    assert_eq!(stored(&orphan), (common_dir.clone(), worktree.clone()));

    let env = env_at(&fresh, &pair.main);
    let listed = inbox(
        &env,
        &Globals::default(),
        &InboxRequest {
            all: false,
            git: pair.git_env(&pair.main),
        },
    )
    .expect("inbox");
    assert!(
        listed.notes.iter().any(|note| note.starts_with(
            "1 proposal(s) of a repository that no longer exists not listed: PR-0002;"
        )),
        "{:?}",
        listed.notes
    );
    let ids: Vec<&str> = listed.proposals.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["PR-0001"]);
    let reviewed = review(
        &env,
        &Globals::default(),
        &ReviewRequest {
            id: orphan.clone(),
            git: pair.git_env(&pair.main),
        },
    );
    let message = cannot(&reviewed, "review of the orphan");
    assert!(message.contains("which no longer exists"), "{message}");
    let mut asked = Vec::new();
    let approved = approve(
        &env,
        &Globals::default(),
        &ApproveRequest {
            id: orphan.clone(),
            note: None,
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
        &mut |question: &str| {
            asked.push(question.to_owned());
            true
        },
    );
    let message = cannot(&approved, "approve of the orphan");
    assert!(message.contains("which no longer exists"), "{message}");
    assert!(asked.is_empty(), "{asked:?}");
    let rejected = reject(
        &env,
        &Globals::default(),
        &RejectRequest {
            id: orphan.clone(),
            reason: "Gone.".to_owned(),
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
        &mut |_: &str| true,
    )
    .expect("reject the orphan");
    assert_eq!(rejected.exit(), Exit::Answered, "{rejected:?}");
    let restored = SqliteQueue::open(db_of(&fresh, &pair.slug), &pair.slug).unwrap();
    assert_eq!(
        restored.get(&orphan).unwrap().unwrap().status.as_str(),
        "rejected"
    );
    assert_eq!(stored(&orphan), (common_dir, worktree), "paths kept");
}

// ------------------------------------------------------------------ AC-10

/// AC-10: a piped stdin: `spec import-state` exits 2 with the terminal
/// message before the file is opened (a missing FILE gives the same), no
/// prompt, nothing created; an answer other than `y`/`yes`: exit 1, `not
/// restored`, the queue never made. M: the terminal check removed or moved
/// after the read.
#[test]
fn ac10_import_needs_a_terminal_and_a_yes() {
    let pair = Pair::new("qe-ac10", "spec-a");
    let (file, _) = small_dump(&pair);
    let fresh = pair.scratch.home("fresh");
    let missing = pair.scratch.join("dumps/missing.jsonl");
    for args in [
        vec!["import-state", file.to_str().unwrap()],
        vec!["import-state", missing.to_str().unwrap()],
        vec!["--json", "import-state", file.to_str().unwrap()],
    ] {
        let run = spec_in(&pair, &fresh, &pair.main, &args, b"y\nyes\n");
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
        assert_eq!(
            run.stderr,
            "spec: `spec import-state` asks the owner for consent on a terminal, and stdin is not \
             one (a pipe, a script or an agent's shell): run it in a terminal; nothing changed\n",
            "{}",
            run.show()
        );
        assert!(!data_dir(&fresh).exists(), "{args:?}: nothing created");
    }

    let (outcome, questions) = import_at(&fresh, &pair.main, &file, false);
    let outcome = outcome.expect("a declined import is an outcome");
    assert_eq!(outcome.exit(), Exit::NotFound);
    assert_eq!(outcome.exit().code(), 1);
    assert_eq!(
        outcome.refusal.as_deref(),
        Some("not restored: the answer was not `y`; nothing changed")
    );
    assert_eq!(questions.len(), 1);
    assert!(!data_dir(&fresh).exists(), "the queue never made");
    let printed = Outcome::StateImport(outcome);
    assert_eq!(render_text(&printed), "");
    assert_eq!(
        printed.stderr_lines(),
        ["spec: not restored: the answer was not `y`; nothing changed"]
    );

    // The question escapes control characters as the queue's prompts do.
    let odd = pair.scratch.dir("dumps").join("a\u{1b}[31mb.jsonl");
    common::copy_file(&file, &odd);
    let (outcome, questions) = import_at(&fresh, &pair.main, &odd, false);
    assert_eq!(outcome.unwrap().exit(), Exit::NotFound);
    assert_eq!(questions.len(), 1);
    assert!(!questions[0].contains('\u{1b}'), "{questions:?}");
    assert!(
        questions[0].contains("a\\u{1b}[31mb.jsonl"),
        "{questions:?}"
    );
    assert!(!data_dir(&fresh).exists(), "the queue never made");
}

// ------------------------------------------------------------------ AC-11

/// AC-11: no dump under `fixtures/` (a dump is written at run time, never
/// committed), and neither test file of this slice reads the process
/// `HOME` (the run under `HOME=/nonexistent` is the dynamic half). M: a
/// committed dump; a test using the process `HOME`.
#[test]
fn ac11_no_dump_is_committed_and_no_test_reads_the_process_home() {
    let fixtures = repository_root().join("fixtures");
    let dumps: Vec<String> = snapshot(&fixtures)
        .into_iter()
        .filter(|(_, bytes)| {
            bytes.as_ref().is_some_and(|bytes| {
                let first = bytes.split(|&byte| byte == b'\n').next().unwrap_or(&[]);
                let first = String::from_utf8_lossy(first);
                first.contains("\"queue_schema\"") || first.contains("\"format\":1,")
            })
        })
        .map(|(path, _)| path)
        .collect();
    assert!(dumps.is_empty(), "a queue dump under fixtures/: {dumps:?}");

    // Assembled at run time so this file does not hold the patterns.
    let home = ["\"HO", "ME\""].concat();
    let patterns = [
        format!("var({home})"),
        format!("var_os({home})"),
        ["home", "_dir("].concat(),
    ];
    for file in [
        "crates/specengine-cli/tests/queue_state.rs",
        "crates/specengine-store/tests/queue_state.rs",
    ] {
        let text = fs::read_to_string(repository_root().join(file)).expect(file);
        for pattern in &patterns {
            assert!(
                !text.contains(pattern.as_str()),
                "{file} reads the process HOME"
            );
        }
    }
}

// ------------------------------------------------------------ iteration 2

/// `the worktree <top> of the project root`: the root's own worktree (or
/// the root, no git).
fn of_root(top: &Path) -> String {
    format!("the worktree {} of the project root", top.display())
}

/// Another worktree of the root's repository.
fn worktree_of(dir: &Path) -> String {
    format!(
        "the worktree {} of the project root's repository",
        dir.display()
    )
}

/// The root's repository's git directory (the common dir, a bare one).
fn git_dir_of(dir: &Path) -> String {
    format!(
        "the git directory {} of the project root's repository",
        dir.display()
    )
}

/// `export state --out out` from `cwd` with the git environment `git`.
fn export_git(
    home: &Path,
    cwd: &Path,
    out: &Path,
    git: GitEnv,
) -> Result<ExportStateOutcome, CliError> {
    export_state(
        &env_at(home, cwd),
        &Globals::default(),
        &ExportStateRequest {
            out: Some(out.to_path_buf()),
            now: NOW.to_owned(),
            git,
        },
    )
}

/// `--out out` from `cwd` refused: exit 2 naming `bound` (`lies inside
/// <bound>: `), nothing at `out`.
fn refused_inside(pair: &Pair, cwd: &Path, out: &Path, bound: &str) {
    let context = format!("{} from {}", out.display(), cwd.display());
    let message = cannot(&export_at(pair, &pair.home, cwd, Some(out), NOW), &context);
    assert!(
        message.starts_with(&format!(
            "spec: the destination {} lies inside {bound}: ",
            out.display()
        )),
        "{context}: {message}"
    );
    assert!(message.ends_with("nothing written"), "{message}");
    assert!(fs::symlink_metadata(out).is_err(), "{context}: written");
}

/// A linked worktree `<scratch>/<name>` on the new branch `branch` at
/// `main`, added from `from`; its path as given.
fn add_worktree(pair: &Pair, from: &Path, name: &str, branch: &str) -> PathBuf {
    let path = pair.scratch.join(name);
    pair.git.git(
        from,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            branch,
            path.to_str().unwrap(),
            "main",
        ],
    );
    path
}

/// A bare repository `<scratch>/bare.git` holding `main` (pushed by path,
/// no remote) and its linked worktree `<scratch>/bwt` on `b1`: both
/// canonical.
fn bare_with_worktree(pair: &Pair) -> (PathBuf, PathBuf) {
    let bare = pair.scratch.join("bare.git");
    pair.git.init_with(&bare, &["--bare"]);
    let bare = fs::canonicalize(&bare).unwrap();
    pair.git
        .git(&pair.main, &["push", "-q", bare.to_str().unwrap(), "main"]);
    let bwt = fs::canonicalize(add_worktree(pair, &bare, "bwt", "b1")).unwrap();
    (bare, bwt)
}

/// Every file under each of `dirs` (git directories included).
fn snapshots(dirs: &[&Path]) -> Vec<std::collections::BTreeMap<String, Option<Vec<u8>>>> {
    dirs.iter().map(|dir| snapshot(dir)).collect()
}

/// Review nit (state.rs:132-139), R2 "never inside the repository": from
/// a linked worktree root, the main worktree (a subdirectory, its `.git`,
/// `.git/worktrees/t1`), another linked worktree, one deleted and never
/// pruned (its path; the directory made again) are refused, each named;
/// from the main root, a linked worktree; from a bare repository's linked
/// worktree, the repository (its git directory); outside: written. Every
/// repository's files unchanged. M: the `git worktree list` loop dropped;
/// it and the `.git`-parent bound; it and the common-dir bound.
#[test]
fn iter2_every_worktree_and_the_git_directory_of_the_roots_repository_are_refused() {
    let pair = Pair::new("qe2-worktrees", "spec-a");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let (main, t1) = (&pair.main, &pair.linked);
    let t2 = fs::canonicalize(add_worktree(&pair, main, "t2", "t2")).unwrap();
    let gone = add_worktree(&pair, main, "gone", "gone");
    let again = add_worktree(&pair, main, "again", "again");
    fs::remove_dir_all(&gone).unwrap();
    fs::remove_dir_all(&again).unwrap();
    fs::create_dir(&again).unwrap();
    let (bare, bwt) = bare_with_worktree(&pair);
    let dirs = [main.as_path(), t1, &t2, &bare, &bwt];
    let before = snapshots(&dirs);

    for (cwd, out, bound) in [
        (t1, main.join("x.jsonl"), worktree_of(main)),
        (t1, main.join("docs/x.jsonl"), worktree_of(main)),
        (t1, main.join(".git/x.jsonl"), worktree_of(main)),
        (
            t1,
            main.join(".git/worktrees/t1/x.jsonl"),
            worktree_of(main),
        ),
        (t1, t2.join("x.jsonl"), worktree_of(&t2)),
        (t1, t2.join("missing/x.jsonl"), worktree_of(&t2)),
        (t1, gone.join("x.jsonl"), worktree_of(&gone)),
        (t1, again.join("x.jsonl"), worktree_of(&again)),
        (t1, t1.join("x.jsonl"), of_root(t1)),
        (main, t1.join("x.jsonl"), worktree_of(t1)),
        (main, t2.join("x.jsonl"), worktree_of(&t2)),
        (main, main.join(".git/x.jsonl"), of_root(main)),
        (&bwt, bare.join("x.jsonl"), git_dir_of(&bare)),
        (&bwt, bare.join("refs/x.jsonl"), git_dir_of(&bare)),
        (&bwt, bare.join("worktrees/bwt/x.jsonl"), git_dir_of(&bare)),
        (&bwt, bwt.join("x.jsonl"), of_root(&bwt)),
    ] {
        refused_inside(&pair, cwd, &out, &bound);
    }
    assert!(!gone.exists(), "nothing made at the deleted worktree");
    assert_eq!(fs::read_dir(&again).unwrap().count(), 0);

    let outside = pair.scratch.dir("outside");
    for (cwd, name) in [(t1, "t1.jsonl"), (main, "main.jsonl"), (&bwt, "bwt.jsonl")] {
        let out = outside.join(name);
        export_ok(&pair, &pair.home, cwd, Some(&out), NOW);
        assert_eq!(mode(&out), 0o600);
    }
    assert_eq!(snapshots(&dirs), before, "no repository file changed");
    for dir in [main.as_path(), t1, &t2, &bwt] {
        assert_eq!(
            pair.git_text(dir, &["status", "--porcelain"]),
            "",
            "{}",
            dir.display()
        );
    }
}

/// Review minor (state.rs:132-147): a macOS firmlink (the path under the
/// Data volume's mount point, `System`, `Volumes`, `Data` from the root;
/// assembled at run time for the anonymity test) is another path to the
/// same directory: `--out` through it into the root's worktree (a missing
/// subdirectory too), into the main worktree from a linked root, into a
/// bare repository from its linked worktree — refused, each named; outside
/// through it — written. Skipped where that mount point is absent (not
/// macOS) or the scratch is not under it. M: the device-and-inode clause
/// dropped.
#[test]
fn iter2_a_firmlink_into_the_repository_is_refused() {
    let data: PathBuf = ["/", "System", "Volumes", "Data"].iter().collect();
    let data = data.as_path();
    if !data.is_dir() {
        eprintln!("skipped: no {} (not macOS)", data.display());
        return;
    }
    let pair = Pair::new("qe2-firmlink", "spec-a");
    let via = |path: &Path| data.join(path.strip_prefix("/").unwrap());
    if !via(&pair.main).is_dir() {
        eprintln!(
            "skipped: the scratch {} is not on the Data volume",
            pair.main.display()
        );
        return;
    }
    assert_ne!(
        fs::canonicalize(via(&pair.main)).unwrap(),
        pair.main,
        "a firmlink path is not resolved to the canonical one (else this tests nothing)"
    );
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let (bare, bwt) = bare_with_worktree(&pair);
    let dirs = [pair.main.as_path(), &pair.linked, &bare, &bwt];
    let before = snapshots(&dirs);
    for (cwd, out, bound) in [
        (
            &pair.main,
            via(&pair.main).join("x.jsonl"),
            of_root(&pair.main),
        ),
        (
            &pair.main,
            via(&pair.main).join("missing/sub/x.jsonl"),
            of_root(&pair.main),
        ),
        (
            &pair.main,
            via(&pair.main.join("docs")).join("x.jsonl"),
            of_root(&pair.main),
        ),
        (
            &pair.linked,
            via(&pair.main).join("x.jsonl"),
            worktree_of(&pair.main),
        ),
        (
            &pair.linked,
            via(&pair.linked).join("x.jsonl"),
            of_root(&pair.linked),
        ),
        (&bwt, via(&bare).join("refs/x.jsonl"), git_dir_of(&bare)),
    ] {
        refused_inside(&pair, cwd, &out, &bound);
    }
    assert!(!pair.main.join("missing").exists());
    let outside = pair.scratch.dir("outside");
    let out = via(&outside).join("q.jsonl");
    export_ok(&pair, &pair.home, &pair.main, Some(&out), NOW);
    assert_eq!(mode(&outside.join("q.jsonl")), 0o600);
    assert_eq!(snapshots(&dirs), before, "no repository file changed");
}

/// Review nit (state.rs:181-185): only "no repository" falls back to the
/// root. A root in a subdirectory of its repository with another owner
/// (`GIT_TEST_ASSUME_DIFFERENT_OWNER=1`) or no `git` on `PATH`, a root
/// whose `.git` file points nowhere: exit 2 "cannot tell whether …"
/// naming the destination, the root and git's error, for a destination in
/// the repository above the root and one outside it alike; nothing
/// written; the binary too. M: the iteration-1 fallback to the root on any
/// failure; any exit 128 read as no repository.
#[test]
fn iter2_a_git_failure_other_than_no_repository_refuses_the_export() {
    let pair = Pair::new("qe2-git-errors", "spec-a");
    let repo = pair.scratch.dir("nested");
    common::copy_dir(&common::fixture("spec-a"), &repo.join("sub"));
    pair.git.init(&repo);
    pair.git.add_all(&repo);
    pair.git.commit(&repo, "nested");
    let root = repo.join("sub");
    let broken = pair.scratch.copy("spec-a", "broken");
    fs::write(
        broken.join(".git"),
        format!("gitdir: {}\n", pair.scratch.join("nowhere").display()),
    )
    .unwrap();
    let outside = pair.scratch.dir("outside");
    let no_git = pair.scratch.dir("no-git-bin");
    let owner = [("GIT_TEST_ASSUME_DIFFERENT_OWNER", OsStr::new("1"))];
    let cases = [
        (
            "another-owner",
            &root,
            with_vars(&pair.git_env(&root), &owner),
            "fatal: detected dubious ownership in repository at ",
        ),
        (
            "no-git",
            &root,
            with_vars(&pair.git_env(&root), &[("PATH", no_git.as_os_str())]),
            "`git` could not be run: no `git` program was found on PATH",
        ),
        (
            "broken-dot-git",
            &broken,
            pair.git_env(&broken),
            "fatal: not a git repository: ",
        ),
    ];
    for (name, root, git, words) in cases {
        for out in [
            repo.join(format!("{name}.jsonl")),
            outside.join(format!("{name}.jsonl")),
        ] {
            let message = cannot(&export_git(&pair.home, root, &out, git.clone()), name);
            assert!(
                message.starts_with(&format!(
                    "spec: cannot tell whether the destination {} lies inside the repository of \
                     the project root {}: ",
                    out.display(),
                    root.display()
                )),
                "{name}: {message}"
            );
            assert!(message.contains(words), "{name}: {message}");
            assert!(message.ends_with("; nothing written"), "{name}: {message}");
            assert!(!out.exists(), "{name}: {} written", out.display());
        }
    }
    let out = outside.join("binary.jsonl");
    let run = spec_with(
        &pair,
        &pair.home,
        &root,
        &["export", "state", "--out", out.to_str().unwrap()],
        b"",
        &owner,
    );
    run.code(2);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("spec: cannot tell whether the destination "),
        "{}",
        run.show()
    );
    assert!(run.stderr.contains("dubious ownership"), "{}", run.show());
    assert!(!out.exists());
    assert_no_partial(&outside);
    assert!(!data_dir(&pair.home).exists(), "nothing made");
}

/// Review nit (state.rs:181-185), the fallback itself: from a root
/// without git, "no repository" is read from git's own words in any
/// locale — `LC_ALL=de_DE.UTF-8` with the real git and with a git that
/// translates (macOS's ships none) — and the root bounds the destination:
/// inside refused naming it, outside written. M: `LC_ALL=C` dropped (the
/// translated words read as a failure).
#[test]
fn iter2_no_repository_falls_back_to_the_root_in_a_german_locale() {
    let pair = Pair::new("qe2-german", "spec-a");
    let plain = pair.scratch.copy("spec-a", "plain");
    let outside = pair.scratch.dir("outside");
    let german = [
        ("LC_ALL", OsStr::new("de_DE.UTF-8")),
        ("LANG", OsStr::new("de_DE.UTF-8")),
        ("LANGUAGE", OsStr::new("de")),
    ];
    let real = with_vars(&pair.git_env(&plain), &german);
    let shim = pair.git.translating_git(&pair.scratch.join("shim"));
    let path = pair.git.path_with(&shim);
    let translating = with_vars(&real, &[("PATH", path.as_os_str())]);
    let mut probe_env = german.to_vec();
    probe_env.push(("PATH", path.as_os_str()));
    let probe = pair
        .git
        .command("git", &plain, &probe_env)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap();
    assert_eq!(probe.status.code(), Some(128));
    assert!(
        String::from_utf8_lossy(&probe.stderr).starts_with("fatal: Kein Git-Repository"),
        "the git on PATH translates: {}",
        String::from_utf8_lossy(&probe.stderr)
    );
    for (name, git) in [("real", real), ("translating", translating)] {
        let inside = plain.join(format!("{name}.jsonl"));
        let message = cannot(&export_git(&pair.home, &plain, &inside, git.clone()), name);
        assert!(
            message.starts_with(&format!(
                "spec: the destination {} lies inside {}: ",
                inside.display(),
                of_root(&plain)
            )),
            "{name}: {message}"
        );
        assert!(!inside.exists());
        let out = outside.join(format!("{name}.jsonl"));
        let outcome = export_git(&pair.home, &plain, &out, git)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!((outcome.proposals, outcome.events), (0, 0), "{name}");
        assert_eq!(mode(&out), 0o600);
    }
}

/// The names in `dir`, sorted (none when it is absent).
fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Review nit (state.rs:166-169): a default export whose write fails
/// (`ulimit -f 0`, `SIGXFSZ` ignored: the partial's write gets `EFBIG`)
/// exits 2 naming the partial and leaves none of the directories it made:
/// under a fresh `HOME` every level of the data directory; beside an
/// existing queue only `backups/`, the queue untouched. A first level the
/// run cannot write into: "cannot create …", nothing made. M: the removal
/// of the made directories dropped.
#[test]
fn iter2_a_failed_default_export_leaves_no_directory_it_made() {
    let pair = Pair::new("qe2-cleanup", "spec-a");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let limited = |home: &Path| {
        let mut command = Command::new("/bin/sh");
        command
            .env_clear()
            .envs(pair.git.vars())
            .env("HOME", home)
            .current_dir(&pair.main)
            .args([
                "-c",
                "trap '' XFSZ; ulimit -f 0; exec \"$0\" export state",
                SPEC,
            ]);
        let label = vec!["sh -c ulimit -f 0; spec export state".to_owned()];
        watched(command, label, b"", RUN_TIMEOUT)
    };
    let failed_write = |run: &Run| {
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
        let line = run.stderr.trim_end();
        assert!(
            line.starts_with("spec: cannot write ")
                && line.contains(".partial: ")
                && line.ends_with("; nothing written")
                && !line.contains("could not be removed"),
            "{}",
            run.show()
        );
    };

    let fresh = pair.scratch.home("fresh");
    let run = limited(&fresh);
    failed_write(&run);
    assert_eq!(
        names_in(&fresh),
        Vec::<String>::new(),
        "every level the run made removed"
    );

    // SQLite sizes a missing `-shm` on open, which `ulimit -f 0` refuses
    // (`disk I/O error`, before `backups/` is made): a connection held
    // open keeps the companions in place, as a running daemon would.
    let data = data_dir(&pair.home);
    let held = pair.queue();
    let queue = held.dump().unwrap();
    let names = names_in(&data);
    let run = limited(&pair.home);
    failed_write(&run);
    assert!(!data.join("backups").exists(), "backups/ removed");
    assert_eq!(names_in(&data), names, "the data directory as it was");
    drop(held);
    assert_eq!(queue_dump(&pair.db(), &pair.slug), queue);

    let locked = pair.scratch.home("locked");
    let first = locked.join(
        data_dir(&locked)
            .strip_prefix(&locked)
            .unwrap()
            .components()
            .next()
            .unwrap(),
    );
    fs::create_dir(&first).unwrap();
    fs::set_permissions(&first, fs::Permissions::from_mode(0o500)).unwrap();
    let run = spec_in(&pair, &locked, &pair.main, &["export", "state"], b"");
    run.code(2);
    assert!(
        run.stderr.starts_with("spec: cannot create ")
            && run.stderr.trim_end().ends_with("; nothing written"),
        "{}",
        run.show()
    );
    assert_eq!(names_in(&first), Vec::<String>::new(), "nothing made");
    fs::set_permissions(&first, fs::Permissions::from_mode(0o700)).unwrap();
}

/// `spec args` in `cwd` with stdin, stdout and stderr on a terminal
/// (BSD `script`), killed by `perl`'s alarm after `alarm` seconds (and
/// `script` by the watchdog 30 seconds later): the terminal's output
/// (stdout and stderr merged, CRLF as LF) as `stdout`.
fn on_terminal(pair: &Pair, home: &Path, cwd: &Path, args: &[&str], alarm: u64) -> Run {
    let mut command = Command::new("/usr/bin/script");
    command
        .env_clear()
        .envs(pair.git.vars())
        .env("HOME", home)
        .current_dir(cwd)
        .args([
            "-q",
            "/dev/null",
            "/usr/bin/perl",
            "-e",
            "alarm shift; exec @ARGV",
        ])
        .arg(alarm.to_string())
        .arg(SPEC)
        .args(args);
    let label = args.iter().map(|arg| (*arg).to_owned()).collect();
    let mut run = watched(command, label, b"", Duration::from_secs(alarm + 30));
    run.stdout = run.stdout.replace("\r\n", "\n");
    run
}

/// Review nit (state.rs:356): import reads only a regular file. Through
/// the binary on a terminal, each under a kill timer so a regression
/// cannot hang the suite: a FIFO (whose open would block; 10 s), a
/// symlink to it, `/dev/zero` (never ends; last, 3 s, so a regression
/// fails on the FIFO first and never fills memory) — and a directory
/// through the library: each exit 2 "cannot read <FILE>: not a regular
/// file (a dump is one file)" at once, nothing asked, nothing made; a
/// symlink to a regular dump restores. The terminal half runs on macOS
/// (BSD `script`). M: `fs::read` instead of the regular-file check.
#[test]
fn iter2_import_reads_only_a_regular_file() {
    let pair = Pair::new("qe2-regular", "spec-a");
    let (file, _) = small_dump(&pair);
    let dumps = file.parent().unwrap().to_path_buf();
    let fresh = pair.scratch.home("fresh");
    let not_regular = |path: &Path| {
        format!(
            "spec: cannot read {}: not a regular file (a dump is one file)",
            path.display()
        )
    };

    if cfg!(target_os = "macos") && Path::new("/usr/bin/script").exists() {
        let fifo = dumps.join("fifo.jsonl");
        let made = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
        assert!(made.success());
        let to_fifo = dumps.join("to-fifo.jsonl");
        std::os::unix::fs::symlink(&fifo, &to_fifo).unwrap();
        for (target, alarm) in [
            (fifo.clone(), 10),
            (to_fifo, 10),
            (PathBuf::from("/dev/zero"), 3),
        ] {
            let started = Instant::now();
            let run = on_terminal(
                &pair,
                &fresh,
                &pair.main,
                &["import-state", target.to_str().unwrap()],
                alarm,
            );
            let elapsed = started.elapsed();
            run.code(2);
            assert!(
                run.stdout.contains(&format!("{}\n", not_regular(&target))),
                "{}: {}",
                target.display(),
                run.show()
            );
            assert!(
                !run.stdout.contains("[y/N]"),
                "nothing asked: {}",
                run.show()
            );
            assert!(
                elapsed < Duration::from_secs(alarm),
                "{}: at once, not after {elapsed:?}",
                target.display()
            );
            assert!(!data_dir(&fresh).exists());
        }
        assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    } else {
        eprintln!("the terminal half skipped: no BSD script (macOS) here");
    }

    let dir = pair.scratch.dir("dumps/a-directory.jsonl");
    let (outcome, questions) = import_at(&fresh, &pair.main, &dir, true);
    assert_eq!(cannot(&outcome, "a directory"), not_regular(&dir));
    assert!(questions.is_empty(), "{questions:?}");
    assert!(!data_dir(&fresh).exists());

    let link = dumps.join("link.jsonl");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    let outcome = import_ok(&fresh, &pair.main, &link);
    assert_eq!((outcome.proposals, outcome.events), (2, 3));
}

/// Review nit (state_file.rs:318), iteration 3: an `id` the queue never
/// writes — `PR-0000`, a leading zero (`PR-00001`, `u64::MAX` as
/// `PR-018446744073709551615`), past `u64::MAX` — refused at its line
/// naming the range 1 to `u64::MAX`; `PR-18446744073709551615` (the top ID
/// the queue writes) restored as given. M: the range check removed;
/// `>= 1` back to `(1..u64::MAX)`.
#[test]
fn iter2_an_id_outside_the_queues_numbers_is_refused_at_its_line() {
    let pair = Pair::new("qe2-ids", "spec-a");
    let (_, lines) = small_dump(&pair);
    let with_id = |id: &str| {
        let mut changed = lines.clone();
        changed[1] = changed[1].replacen(
            "{\"proposals\":{\"id\":\"PR-0001\"",
            &format!("{{\"proposals\":{{\"id\":\"{id}\""),
            1,
        );
        assert_ne!(changed[1], lines[1], "the first row is PR-0001");
        dump_of(&changed)
    };
    for id in [
        "PR-0000",
        "PR-00001",
        "PR-018446744073709551615",
        "PR-18446744073709551616",
        "PR-99999999999999999999",
    ] {
        let (label, message) = refused_import(&pair, id, &with_id(id));
        assert_eq!(
            message,
            format!(
                "{label}:2: `id` is not a proposal ID as the queue writes it (`PR-` and 4 or \
                 more digits, numbered from 1 to 18446744073709551615)"
            ),
            "{id}"
        );
    }
    let highest = "PR-18446744073709551615";
    let file = pair.scratch.dir("dumps").join("highest.jsonl");
    fs::write(&file, with_id(highest)).unwrap();
    let fresh = pair.scratch.home("fresh-highest");
    import_ok(&fresh, &pair.main, &file);
    let stored = SqliteQueue::open_existing(db_of(&fresh, &pair.slug), &pair.slug)
        .unwrap()
        .unwrap()
        .stored_rows()
        .unwrap();
    let ids: Vec<&str> = stored.proposals.iter().filter_map(|row| row.id()).collect();
    assert_eq!(ids, ["PR-0002", highest]);
}

/// Review nit (state_file.rs:54,75-77): every string of the dump is
/// written exactly as `serde_json::to_string` writes it — control
/// characters (`\u0001`, ESC, NUL, tab, CR, LF), DEL, a bidi override
/// (U+202E), U+2028, a quote, a backslash, a slash, a non-BMP character —
/// the whole file equal to the lines built so from the stored rows; it
/// restores to an equal queue. M: a text written raw between quotes.
#[test]
fn iter2_every_text_is_escaped_as_serde_json_writes_it() {
    let pair = Pair::new("qe2-escape", "spec-a");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let mut state = pair.queue().stored_rows().unwrap();
    let odd = "a\u{1}b\u{1b}[31mc\u{7f}d\u{0}e\tf\rg\nh\u{202e}i\u{2028}j\"k\\l/m\u{1f600}n";
    let at = |name: &str| PROPOSAL_COLUMNS.iter().position(|c| *c == name).unwrap();
    for column in [
        "target_path",
        "base_text",
        "new_text",
        "rationale",
        "decision_note",
    ] {
        state.proposals[0].columns[at(column)] = Some(format!("{column}:{odd}"));
    }
    let payload = EVENT_COLUMNS[1..]
        .iter()
        .position(|c| *c == "payload")
        .unwrap();
    state.events[0].columns[payload] = Some(format!("payload:{odd}"));

    let home = pair.scratch.home("odd");
    let db = db_of(&home, &pair.slug);
    fs::create_dir_all(db.parent().unwrap()).unwrap();
    let mut queue = SqliteQueue::open(&db, &pair.slug).unwrap();
    assert_eq!(queue.restore(&state).unwrap(), Restore::Restored);
    drop(queue);
    let stored = SqliteQueue::open_existing(&db, &pair.slug)
        .unwrap()
        .unwrap()
        .stored_rows()
        .unwrap();
    assert_eq!(stored, state, "stored as given");

    let out = pair.scratch.dir("outside").join("odd.jsonl");
    export_ok(&pair, &home, &pair.main, Some(&out), NOW);
    let json = |text: &str| serde_json::to_string(text).unwrap();
    let field = |column: &str, value: &Option<String>| {
        let value = value.as_deref().map_or_else(|| "null".to_owned(), json);
        format!("{}:{value}", json(column))
    };
    let mut expected = format!(
        "{{\"format\":{STATE_FORMAT},\"queue_schema\":{QUEUE_SCHEMA_VERSION},\"project\":{},\
         \"proposals\":{},\"events\":{}}}\n",
        json(&pair.slug),
        stored.proposals.len(),
        stored.events.len()
    );
    for row in &stored.proposals {
        let fields: Vec<String> = PROPOSAL_COLUMNS
            .iter()
            .zip(&row.columns)
            .map(|(column, value)| field(column, value))
            .collect();
        expected.push_str(&format!("{{\"proposals\":{{{}}}}}\n", fields.join(",")));
    }
    for row in &stored.events {
        let fields: Vec<String> = EVENT_COLUMNS[1..]
            .iter()
            .zip(&row.columns)
            .map(|(column, value)| field(column, value))
            .collect();
        expected.push_str(&format!(
            "{{\"events\":{{\"seq\":{},{}}}}}\n",
            row.seq,
            fields.join(",")
        ));
    }
    let bytes = fs::read(&out).unwrap();
    let text = String::from_utf8(bytes.clone()).expect("a UTF-8 dump");
    assert_eq!(text, expected);
    assert!(
        !bytes.iter().any(|&byte| byte < 0x20 && byte != b'\n'),
        "no raw control character"
    );
    for escaped in [
        "\\u0001",
        "\\u001b[31m",
        "\\u0000",
        "\\t",
        "\\r",
        "\\n",
        "\\\"",
        "\\\\",
    ] {
        assert!(text.contains(escaped), "{escaped} in the dump");
    }
    assert_eq!(
        text.lines().count(),
        1 + stored.proposals.len() + stored.events.len()
    );

    let fresh = pair.scratch.home("fresh");
    import_ok(&fresh, &pair.main, &out);
    assert_eq!(
        queue_dump(&db_of(&fresh, &pair.slug), &pair.slug),
        queue_dump(&db, &pair.slug)
    );
}

// ------------------------------------------------------------ iteration 3

/// The top ID the queue writes: `u64::MAX`.
const TOP: &str = "PR-18446744073709551615";

/// `propose` of an edit of `TARGET` (read in the linked worktree) into the
/// data directory of `home`.
fn propose_at(pair: &Pair, home: &Path, to: &str) -> Result<ProposalOutcome, CliError> {
    let (hash, text) = pair.span(&pair.linked, TARGET);
    propose(
        &env_at(home, &pair.linked),
        &Globals::default(),
        &pair.request(&pair.linked, TARGET, &hash, &edit(&text, FROM, to)),
    )
}

/// AC-01 at the top ID: a queue restored with `PR-18446744073709551614`
/// gives the next `propose` `PR-18446744073709551615`; that queue exports,
/// imports into a fresh data directory (`dump()` equal) and re-exports
/// byte-identically (a later clock too); after it both the original and
/// the restored queue refuse `propose` with the same exit 2, the queue
/// unchanged — through the library and the binary. M: `>= 1` back to
/// `(1..u64::MAX)` (the import of the top ID refused).
#[test]
fn iter3_the_top_id_round_trips_and_nothing_is_proposed_after_it() {
    let pair = Pair::new("qe3-top", "spec-a");
    let dumps = pair.scratch.dir("dumps");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint stops;");
    let one = dumps.join("one.jsonl");
    export_ok(&pair, &pair.home, &pair.main, Some(&one), NOW);
    let text = fs::read_to_string(&one).unwrap();
    let below = "PR-18446744073709551614";
    let renamed = text.replace("PR-0001", below);
    assert_eq!(renamed.matches(below).count(), 2, "the row and its event");
    let below_file = dumps.join("below.jsonl");
    fs::write(&below_file, &renamed).unwrap();

    let original = pair.scratch.home("original");
    let outcome = import_ok(&original, &pair.main, &below_file);
    assert_eq!((outcome.proposals, outcome.events), (1, 1));
    let created = propose_at(&pair, &original, "the sprint halts;")
        .unwrap_or_else(|error| panic!("propose after {below}: {error}"));
    assert_eq!(created.exit(), Exit::Answered, "{created:?}");
    let printed = render_text(&Outcome::Proposal(Box::new(created)));
    assert!(printed.starts_with(&format!("{TOP}\n")), "{printed}");
    let db = db_of(&original, &pair.slug);
    let last = SqliteQueue::open(&db, &pair.slug)
        .unwrap()
        .events()
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!((last.seq, last.payload["id"].as_str()), (2, Some(TOP)));

    let top = dumps.join("top.jsonl");
    export_ok(&pair, &original, &pair.main, Some(&top), NOW);
    let exported = fs::read(&top).unwrap();
    let lines = lines_of(&exported);
    assert!(
        lines[0].ends_with(",\"proposals\":2,\"events\":2}"),
        "{}",
        lines[0]
    );
    let ids: Vec<String> = lines[1..]
        .iter()
        .filter_map(|line| {
            let value: Value = serde_json::from_str(line).unwrap();
            value.get("proposals").map(|row| row["id"].to_string())
        })
        .collect();
    assert_eq!(ids, [format!("\"{below}\""), format!("\"{TOP}\"")]);

    let fresh = pair.scratch.home("fresh");
    let outcome = import_ok(&fresh, &pair.main, &top);
    assert_eq!((outcome.proposals, outcome.events), (2, 2));
    let fresh_db = db_of(&fresh, &pair.slug);
    assert_eq!(
        queue_dump(&fresh_db, &pair.slug),
        queue_dump(&db, &pair.slug)
    );
    let again = dumps.join("again.jsonl");
    export_ok(&pair, &fresh, &pair.main, Some(&again), LATER);
    assert!(
        fs::read(&again).unwrap() == exported,
        "the re-export is byte-identical"
    );

    let refusal =
        format!("spec: index database: the queue holds `{TOP}`: no proposal ID follows it");
    for home in [&original, &fresh] {
        let before = queue_dump(&db_of(home, &pair.slug), &pair.slug);
        let message = cannot(
            &propose_at(&pair, home, "the sprint ceases;"),
            &home.display().to_string(),
        );
        assert_eq!(message, refusal, "{}", home.display());
        assert_eq!(queue_dump(&db_of(home, &pair.slug), &pair.slug), before);
    }

    let (hash, span) = pair.span(&pair.linked, TARGET);
    let text_file = dumps.join("text.md");
    fs::write(&text_file, edit(&span, FROM, "the sprint pauses;")).unwrap();
    let before = queue_dump(&fresh_db, &pair.slug);
    let run = spec_in(
        &pair,
        &fresh,
        &pair.linked,
        &[
            "propose",
            "update",
            TARGET,
            "--base",
            &hash,
            "--text-file",
            text_file.to_str().unwrap(),
            "--rationale",
            "After the top ID.",
            "--author-role",
            "spec-writer",
            "--author-model",
            "claude-opus-5-5",
        ],
        b"",
    );
    assert_eq!(run.code, 2, "{run:?}");
    assert_eq!(run.stdout, "", "{run:?}");
    assert_eq!(run.stderr, format!("{refusal}\n"), "{run:?}");
    assert_eq!(queue_dump(&fresh_db, &pair.slug), before);
}

/// `--separate-git-dir`, from its linked worktree and from the main
/// checkout: `git worktree list` prints the git directory itself as the
/// main entry, so `--out` into it (its top, `refs/`, `worktrees/<name>/`) is
/// refused naming "the git directory …", never "the worktree …"; the
/// linked worktree from the main checkout is "the worktree …", each root
/// itself "of the project root"; outside: written. No repository file
/// changed. (The main checkout from the linked worktree is a known limit of
/// the canon, not asserted.) M: `|| bound.is(&common)` dropped (named "the
/// worktree <git dir> …").
#[test]
fn iter3_a_separate_git_directory_is_named_the_git_directory() {
    let pair = Pair::new("qe3-separate", "spec-a");
    pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint ends at once;");
    let sep = pair.scratch.copy("spec-a", "sep");
    let git_dir = pair.scratch.join("sep.git");
    pair.git
        .init_with(&sep, &["--separate-git-dir", git_dir.to_str().unwrap()]);
    pair.git.add_all(&sep);
    pair.git.commit(&sep, "the fixture");
    let sep = fs::canonicalize(&sep).unwrap();
    let git_dir = fs::canonicalize(&git_dir).unwrap();
    assert!(sep.join(".git").is_file(), "a `.git` file, not a directory");
    let wt = fs::canonicalize(add_worktree(&pair, &sep, "sepwt", "s1")).unwrap();
    let main_entry = pair.git_text(&wt, &["worktree", "list", "--porcelain"]);
    assert!(
        main_entry.starts_with(&format!("worktree {}\n", git_dir.display())),
        "git lists the git directory as the main entry: {main_entry}"
    );
    let dirs = [sep.as_path(), &git_dir, &wt];
    let before = snapshots(&dirs);

    let in_git = |rest: &str| git_dir.join(rest);
    for (cwd, out, bound) in [
        (&wt, in_git("x.jsonl"), git_dir_of(&git_dir)),
        (&wt, in_git("refs/x.jsonl"), git_dir_of(&git_dir)),
        (&wt, in_git("worktrees/sepwt/x.jsonl"), git_dir_of(&git_dir)),
        (&wt, in_git("missing/x.jsonl"), git_dir_of(&git_dir)),
        (&wt, wt.join("x.jsonl"), of_root(&wt)),
        (&sep, in_git("x.jsonl"), git_dir_of(&git_dir)),
        (&sep, in_git("refs/x.jsonl"), git_dir_of(&git_dir)),
        (&sep, wt.join("x.jsonl"), worktree_of(&wt)),
        (&sep, sep.join("x.jsonl"), of_root(&sep)),
    ] {
        refused_inside(&pair, cwd, &out, &bound);
    }
    assert!(!git_dir.join("missing").exists(), "nothing made in it");

    let outside = pair.scratch.dir("outside");
    for (cwd, name) in [(&wt, "wt.jsonl"), (&sep, "sep.jsonl")] {
        let out = outside.join(name);
        export_ok(&pair, &pair.home, cwd, Some(&out), NOW);
        assert_eq!(mode(&out), 0o600);
    }
    assert_eq!(snapshots(&dirs), before, "no repository file changed");
    for dir in [&sep, &wt] {
        assert_eq!(pair.git_text(dir, &["status", "--porcelain"]), "");
    }
}
