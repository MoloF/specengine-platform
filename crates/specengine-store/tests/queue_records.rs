//! docs/features/decision-apply.md, the store's half ("Data", Queue and
//! Store): the record ID previewed read-only (`next_record`: the corpus's
//! highest or the queue's, plus one) and issued under the write lock
//! (`approve_record_from`: the compare-and-set, `Issued` when the preview
//! is stale), the five record columns set together, kept by a reopen,
//! named in the `.approved` and `.applied` events; the rows "Data" names
//! corrupt, each naming its column; `create_file` never replacing a file
//! and removing what it made; `WorktreeGit`'s intent-to-add entry, its
//! removal and the typed name-status.
//!
//! Through the public API only; every git process in the sandbox of the
//! CLI tests' `common::git`.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use common::{Scratch, write};
use git::Sandbox;
use serde_json::json;
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;
use specengine_store::{
    ApplyFailure, Choice, CreateFileError, Decision, GitEnv, Intake, NewIntake, NewProposal,
    PROPOSAL_COLUMNS, Place, ProposalKind, ProposalQueue as _, ProposalStatus, QueueError,
    RecordApproval, RecordSeries, SqliteQueue, WorktreeGit, create_file, patch_hash,
};

const T0: &str = "2026-10-05T09:00:00Z";
const T1: &str = "2026-10-05T12:00:00Z";
const T2: &str = "2026-10-05T13:00:00Z";
const PROJECT: &str = "demo";

fn place() -> Place {
    Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r".to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn author() -> Author {
    Author::new(Some("developer".to_owned()), None, None).expect("an agent")
}

fn discrepancy(summary: &str) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes"),
        price: "1 item".to_owned(),
    };
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: "docs/x.md".to_owned(),
        place: place(),
        author: author(),
        intake: Intake {
            target_ids: vec!["A-1".to_owned()],
            severity: IntakeSeverity::High,
            gap_type: Some(GapType::Contradicts),
            summary: summary.to_owned(),
            working_answer: None,
            price_of_other: None,
            evidence: vec![Evidence {
                file: "src/a.rs".to_owned(),
                qpath: None,
                lines: Some("3-9".to_owned()),
                observed: "walks".to_owned(),
                documented: "rests".to_owned(),
            }],
            options: vec![option("code"), option("spec")],
            recommendation: Some(1),
            distinct_from: Vec::new(),
        },
    }
}

fn update() -> NewProposal {
    let base_hash = format!("b3:{}", common::blake3_hex(b"base"));
    NewProposal {
        kind: ProposalKind::Update,
        target_id: "A-1".to_owned(),
        target_path: "docs/x.md".to_owned(),
        place: place(),
        patch_hash: patch_hash("A-1", &base_hash, "new"),
        base_hash: Some(base_hash),
        base_text: Some("base".to_owned()),
        new_text: "new".to_owned(),
        rationale: "Why.".to_owned(),
        author: author(),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
    }
}

fn series(corpus_max: u64) -> RecordSeries {
    RecordSeries {
        prefix: "DEC".to_owned(),
        width: 4,
        corpus_max,
    }
}

fn decision() -> Decision {
    Decision {
        decided_by: "Owner <owner@example.invalid>".to_owned(),
        note: Some("Because.".to_owned()),
    }
}

fn approval(preview: &str, corpus_max: u64, choice: Choice) -> RecordApproval {
    RecordApproval {
        series: series(corpus_max),
        preview: preview.to_owned(),
        path: format!("docs/records/DEC/{preview}.md"),
        title: "Spec".to_owned(),
        text: format!("---\nid: {preview}\n---\n"),
        choice,
    }
}

/// A queue with one discrepancy (`PR-0001`).
fn queue_with_item(scratch: &Scratch) -> SqliteQueue {
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    queue
        .create_intake(&discrepancy("It departs."), &[], None, T0)
        .expect("intake")
        .created
        .expect("stored");
    queue
}

/// `(type, payload)` of every event of `id`.
fn events_of(queue: &SqliteQueue, id: &str) -> Vec<(String, serde_json::Value)> {
    queue
        .events()
        .expect("events")
        .into_iter()
        .filter(|event| event.payload["id"] == id)
        .map(|event| (event.event_type, event.payload))
        .collect()
}

// ------------------------------------------------------------ the ID

/// AC-05, the store half: `next_record` is one more than the greater of
/// the corpus's highest and the queue's issued numbers of the prefix,
/// padded to its width, and writes nothing; `approve_record_from` issues
/// only the ID still next (else `Issued` naming the next, nothing
/// written), sets the five columns together, logs `proposal.approved`
/// with `record`. M: issuing from the worktree (the corpus) only.
#[test]
fn ac05_the_queue_issues_the_next_record_id_under_its_lock() {
    let scratch = Scratch::new("qr-issue");
    let mut queue = queue_with_item(&scratch);
    let dump = queue.dump().expect("dump");
    assert_eq!(queue.next_record(&series(23)).unwrap(), "DEC-0024");
    assert_eq!(queue.next_record(&series(0)).unwrap(), "DEC-0001");
    assert_eq!(queue.dump().expect("dump"), dump, "read only");

    let item = queue.get("PR-0001").unwrap().unwrap();
    let stale = queue.approve_record_from(
        "PR-0001",
        &item.seen(),
        &approval("DEC-0023", 23, Choice::Option(1)),
        &decision(),
        T1,
    );
    match stale {
        Err(QueueError::Issued { next }) => assert_eq!(next, "DEC-0024"),
        other => panic!("expected Issued, got {other:?}"),
    }
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");

    let approved = queue
        .approve_record_from(
            "PR-0001",
            &item.seen(),
            &approval("DEC-0024", 23, Choice::Option(1)),
            &decision(),
            T1,
        )
        .expect("issued");
    assert_eq!(approved.status, ProposalStatus::Approved);
    let record = approved.record.clone().expect("the record");
    assert_eq!(record.id, "DEC-0024");
    assert_eq!(record.path, "docs/records/DEC/DEC-0024.md");
    assert_eq!(record.title, "Spec");
    assert_eq!(record.text, "---\nid: DEC-0024\n---\n");
    assert_eq!(record.choice, Choice::Option(1));
    assert_eq!(approved.decided_at.as_deref(), Some(T1));
    assert_eq!(approved.decision_note.as_deref(), Some("Because."));
    assert_eq!(
        events_of(&queue, "PR-0001"),
        [
            ("proposal.created".to_owned(), json!({"id": "PR-0001"})),
            (
                "proposal.approved".to_owned(),
                json!({"id": "PR-0001", "record": "DEC-0024"})
            ),
        ]
    );
    // The queue's issued number counts over a lower corpus.
    assert_eq!(queue.next_record(&series(23)).unwrap(), "DEC-0025");
    assert_eq!(queue.next_record(&series(0)).unwrap(), "DEC-0025");
    assert_eq!(queue.next_record(&series(40)).unwrap(), "DEC-0041");
    let other = RecordSeries {
        prefix: "ADR".to_owned(),
        width: 4,
        corpus_max: 2,
    };
    assert_eq!(
        queue.next_record(&other).unwrap(),
        "ADR-0003",
        "another prefix"
    );

    // A second item previewing the issued ID: refused, the next named.
    let second = queue
        .create_intake(&discrepancy("It departs again."), &[], None, T0)
        .expect("intake")
        .created
        .expect("stored");
    let dump = queue.dump().expect("dump");
    match queue.approve_record_from(
        &second.id,
        &second.seen(),
        &approval("DEC-0024", 23, Choice::Option(0)),
        &decision(),
        T1,
    ) {
        Err(QueueError::Issued { next }) => assert_eq!(next, "DEC-0025"),
        other => panic!("expected Issued, got {other:?}"),
    }
    assert_eq!(queue.dump().expect("dump"), dump);
}

/// "Data", Queue: the record is kept by its proposal across a reopen
/// (`reopen_from`), so a later step 7 issues the same ID, never another;
/// `applied_with` logs `proposal.applied` with `record`; a stale `seen`,
/// an update, a path not clean, a choice not of the kind or out of range
/// are refused, nothing written.
#[test]
fn the_record_is_kept_across_a_reopen_and_named_by_the_events() {
    let scratch = Scratch::new("qr-keep");
    let mut queue = queue_with_item(&scratch);
    let item = queue.get("PR-0001").unwrap().unwrap();
    let dump = queue.dump().expect("dump");
    for (label, bad) in [
        ("a path leaving the root", {
            let mut bad = approval("DEC-0024", 23, Choice::Option(1));
            bad.path = "../DEC-0024.md".to_owned();
            bad
        }),
        ("an absolute path", {
            let mut bad = approval("DEC-0024", 23, Choice::Option(1));
            bad.path = "/DEC-0024.md".to_owned();
            bad
        }),
        (
            "an option out of range",
            approval("DEC-0024", 23, Choice::Option(2)),
        ),
        (
            "a question's choice",
            approval("DEC-0024", 23, Choice::WorkingAnswer),
        ),
        (
            "an answer",
            approval("DEC-0024", 23, Choice::Answer("x".to_owned())),
        ),
    ] {
        match queue.approve_record_from("PR-0001", &item.seen(), &bad, &decision(), T1) {
            Err(QueueError::Invalid(_)) => {}
            other => panic!("{label}: expected Invalid, got {other:?}"),
        }
        assert_eq!(queue.dump().expect("dump"), dump, "{label}");
    }
    let held = queue
        .approve_record_from(
            "PR-0001",
            &item.seen(),
            &approval("DEC-0024", 23, Choice::Option(1)),
            &decision(),
            T1,
        )
        .expect("issued");
    // The state read before step 7 is stale now.
    match queue.approve_record_from(
        "PR-0001",
        &item.seen(),
        &approval("DEC-0024", 23, Choice::Option(1)),
        &decision(),
        T2,
    ) {
        Err(QueueError::Changed { .. }) => {}
        other => panic!("expected Changed, got {other:?}"),
    }
    let reopened = queue
        .reopen_from(
            "PR-0001",
            &held.seen(),
            &ApplyFailure {
                step: 9,
                reason: "the commit failed".to_owned(),
            },
            T2,
        )
        .expect("reopened");
    assert_eq!(reopened.status, ProposalStatus::Open);
    assert_eq!(reopened.record, held.record, "the record kept");
    assert_eq!(queue.next_record(&series(23)).unwrap(), "DEC-0025");
    // Its next step 7: the ID it keeps, whatever the series says.
    match queue.approve_record_from(
        "PR-0001",
        &reopened.seen(),
        &approval("DEC-0031", 30, Choice::Option(1)),
        &decision(),
        "2026-10-05T14:00:00Z",
    ) {
        Err(QueueError::Issued { next }) => assert_eq!(next, "DEC-0024"),
        other => panic!("expected Issued, got {other:?}"),
    }
    let again = queue
        .approve_record_from(
            "PR-0001",
            &reopened.seen(),
            &approval("DEC-0024", 30, Choice::Option(0)),
            &decision(),
            "2026-10-05T14:00:00Z",
        )
        .expect("approved again");
    assert_eq!(
        again.record.as_ref().unwrap().choice,
        Choice::Option(0),
        "replaced"
    );
    let applied = queue
        .applied_with(
            "PR-0001",
            &"2".repeat(40),
            &decision(),
            "2026-10-05T15:00:00Z",
        )
        .expect("applied");
    assert_eq!(applied.status, ProposalStatus::Applied);
    assert_eq!(applied.record.as_ref().unwrap().id, "DEC-0024");
    let events = events_of(&queue, "PR-0001");
    assert_eq!(
        events.last().unwrap(),
        &(
            "proposal.applied".to_owned(),
            json!({"id": "PR-0001", "commit": "2".repeat(40), "record": "DEC-0024"})
        )
    );
    assert!(
        events
            .iter()
            .filter(|(kind, _)| kind == "proposal.approved")
            .all(|(_, payload)| payload["record"] == json!("DEC-0024")),
        "{events:?}"
    );

    // An update writes no record.
    let created = queue.create(&update(), T0).expect("an update");
    match queue.approve_record_from(
        &created.id,
        &created.seen(),
        &approval("DEC-0030", 23, Choice::Option(0)),
        &decision(),
        T1,
    ) {
        Err(QueueError::Invalid(message)) => assert!(message.contains("update"), "{message}"),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

// ------------------------------------------------------------ corrupt rows

/// One corrupt-row case: its label, the rows it edits, the edits
/// (`column`, value), the column the refusal names.
type CorruptCase<'a> = (
    &'a str,
    &'a specengine_store::StoredQueue,
    Vec<(&'a str, Option<&'a str>)>,
    &'a str,
);

/// "Data", Queue: corrupt, named: any record column on an `update`; the
/// five set partly; a `choice` of another shape, kind or out of range; a
/// `record_path` not clean; a `record_id` of no ID's shape; an `approved`
/// or `applied` deciding row without them. Each broken row restored alone
/// into a fresh queue: `get` fails naming `PR-0001` and the column,
/// `list_readable` skips it naming it. M: a column missing.
#[test]
fn a_corrupt_record_row_names_its_column() {
    let scratch = Scratch::new("qr-corrupt");
    let mut queue = queue_with_item(&scratch);
    let item = queue.get("PR-0001").unwrap().unwrap();
    queue
        .approve_record_from(
            "PR-0001",
            &item.seen(),
            &approval("DEC-0024", 23, Choice::Option(1)),
            &decision(),
            T1,
        )
        .expect("issued");
    let decided = queue.stored_rows().expect("rows");
    drop(queue);
    let scratch_update = scratch.db("update");
    let mut updates = SqliteQueue::open(&scratch_update, PROJECT).expect("open");
    updates.create(&update(), T0).expect("an update");
    let undecided = updates.stored_rows().expect("rows");
    drop(updates);
    let at = |name: &str| {
        PROPOSAL_COLUMNS
            .iter()
            .position(|column| *column == name)
            .unwrap_or_else(|| panic!("no column {name}"))
    };
    assert_eq!(
        &PROPOSAL_COLUMNS[35..40],
        [
            "record_id",
            "record_path",
            "record_title",
            "record_text",
            "choice"
        ]
    );
    assert_eq!(&PROPOSAL_COLUMNS[40..], ["task_id"]);
    let cases: Vec<CorruptCase<'_>> = vec![
        (
            "an update with a record",
            &undecided,
            vec![("record_id", Some("DEC-0001"))],
            "record_id",
        ),
        (
            "an update with a choice",
            &undecided,
            vec![("choice", Some("{\"option\":0}"))],
            "choice",
        ),
        (
            "set partly",
            &decided,
            vec![("record_title", None)],
            "record_title",
        ),
        (
            "text NULL",
            &decided,
            vec![("record_text", None)],
            "record_text",
        ),
        (
            "approved without them",
            &decided,
            vec![
                ("record_id", None),
                ("record_path", None),
                ("record_title", None),
                ("record_text", None),
                ("choice", None),
            ],
            "record_id",
        ),
        (
            "a choice out of range",
            &decided,
            vec![("choice", Some("{\"option\":2}"))],
            "choice",
        ),
        (
            "a question's choice",
            &decided,
            vec![("choice", Some("{\"working_answer\":true}"))],
            "choice",
        ),
        (
            "an answer to a discrepancy",
            &decided,
            vec![("choice", Some("{\"answer\":\"x\"}"))],
            "choice",
        ),
        (
            "two keys",
            &decided,
            vec![("choice", Some("{\"option\":1,\"answer\":\"x\"}"))],
            "choice",
        ),
        (
            "not JSON",
            &decided,
            vec![("choice", Some("option 1"))],
            "choice",
        ),
        (
            "a path leaving the root",
            &decided,
            vec![("record_path", Some("../DEC-0024.md"))],
            "record_path",
        ),
        (
            "an absolute path",
            &decided,
            vec![("record_path", Some("/DEC-0024.md"))],
            "record_path",
        ),
        (
            "no ID",
            &decided,
            vec![("record_id", Some("dec-24"))],
            "record_id",
        ),
    ];
    for (index, (label, base, edits, column)) in cases.into_iter().enumerate() {
        let mut state = base.clone();
        for (name, value) in edits {
            state.proposals[0].columns[at(name)] = value.map(str::to_owned);
        }
        let db = scratch.db(&format!("broken-{index}"));
        let mut broken = SqliteQueue::open(&db, PROJECT).expect("open");
        broken.restore(&state).expect("restore takes rows raw");
        let message = match broken.get("PR-0001") {
            Err(error) => error.to_string(),
            Ok(found) => panic!("{label}: read as {found:?}"),
        };
        assert!(
            message.contains("PR-0001") && message.contains(&format!("`{column}`")),
            "{label}: {message}"
        );
        let listed = broken
            .list_readable(&specengine_store::ProposalFilter::default())
            .expect("list_readable");
        assert!(listed.proposals.is_empty(), "{label}");
        assert_eq!(listed.unreadable.len(), 1, "{label}");
        assert_eq!(listed.unreadable[0].column, column, "{label}");
    }
    // The decided row itself reads.
    let db = scratch.db("good");
    let mut good = SqliteQueue::open(&db, PROJECT).expect("open");
    good.restore(&decided).expect("restore");
    assert_eq!(
        good.get("PR-0001").unwrap().unwrap().record.unwrap().id,
        "DEC-0024"
    );
}

// ------------------------------------------------------------ the file

/// AC-09, the store half, and step 8: `create_file` makes a new file
/// (temporary name, hard link) and the missing directories, never through
/// a symlink or over anything at the path (a dangling symlink too): the
/// bytes there intact, nothing left, the directories it made removed;
/// `remove` takes back the file and its directories. M: rename over the
/// path.
#[test]
fn ac09_create_file_never_replaces_and_takes_back_what_it_made() {
    let scratch = Scratch::new("qr-create");
    let root = scratch.join("root");
    write(&root, "docs/records/DEC/DEC-0001.md", "old");
    let root = fs::canonicalize(&root).unwrap();
    let listing = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };

    match create_file(&root, "docs/records/DEC/DEC-0001.md", b"new") {
        Err(CreateFileError::Exists) => {}
        other => panic!("expected Exists, got {other:?}"),
    }
    assert_eq!(
        fs::read(root.join("docs/records/DEC/DEC-0001.md")).unwrap(),
        b"old"
    );
    assert_eq!(
        listing(&root.join("docs/records/DEC")),
        ["DEC-0001.md"],
        "no temporary"
    );

    symlink(
        root.join("nowhere"),
        root.join("docs/records/DEC/DEC-0002.md"),
    )
    .unwrap();
    match create_file(&root, "docs/records/DEC/DEC-0002.md", b"new") {
        Err(CreateFileError::Exists) => {}
        other => panic!("a dangling symlink: expected Exists, got {other:?}"),
    }
    assert!(!root.join("nowhere").exists(), "nothing written through it");

    let created = create_file(&root, "docs/records/DEC/new/deeper/DEC-0003.md", b"three")
        .expect("created with its directories");
    assert_eq!(
        fs::read(root.join("docs/records/DEC/new/deeper/DEC-0003.md")).unwrap(),
        b"three"
    );
    assert_eq!(
        created.made_dirs,
        [
            root.join("docs/records/DEC/new"),
            root.join("docs/records/DEC/new/deeper")
        ]
    );
    assert_eq!(
        listing(&root.join("docs/records/DEC/new/deeper")),
        ["DEC-0003.md"]
    );
    assert_eq!(created.remove(), Vec::<String>::new());
    assert!(
        !root.join("docs/records/DEC/new").exists(),
        "its directories removed"
    );

    fs::create_dir_all(scratch.join("elsewhere")).unwrap();
    symlink(scratch.join("elsewhere"), root.join("docs/records/link")).unwrap();
    match create_file(&root, "docs/records/link/DEC-0004.md", b"x") {
        Err(CreateFileError::Symlink(component)) => assert_eq!(component, "docs/records/link"),
        other => panic!("expected Symlink, got {other:?}"),
    }
    assert!(listing(&scratch.join("elsewhere")).is_empty());
    match create_file(&root, "docs/records/DEC/DEC-0001.md/x.md", b"x") {
        Err(CreateFileError::NotDirectory(component)) => {
            assert_eq!(component, "docs/records/DEC/DEC-0001.md");
        }
        other => panic!("expected NotDirectory, got {other:?}"),
    }
    match create_file(&root, "docs/fresh/one/../x.md", b"x") {
        Err(CreateFileError::BadPath) => {}
        other => panic!("expected BadPath, got {other:?}"),
    }
    assert!(
        !root.join("docs/fresh").exists(),
        "no directory left by a refusal"
    );
}

// ------------------------------------------------------------ git

/// Steps 9 and 10's git: `intent_to_add` gives the new path an index entry
/// (`git ls-files --stage` shows it), `remove_cached` takes it back and
/// leaves the file; `name_status` is typed, `adds` true only of an `A` of
/// that path. M: the intent-to-add entry left.
#[test]
fn the_intent_to_add_entry_is_made_and_taken_back() {
    let scratch = Scratch::new("qr-git");
    let sandbox = Sandbox::new(scratch.path());
    let repo = scratch.join("repo");
    write(&repo, "docs/a.md", "# A\n");
    sandbox.init(&repo);
    let repo = fs::canonicalize(&repo).unwrap();
    sandbox.add_all(&repo);
    sandbox.commit(&repo, "first");
    let env = GitEnv::new(&repo, sandbox.vars());
    let git = WorktreeGit::new(&repo, &env).expect("git");
    write(&repo, "docs/records/DEC-0001.md", "# Record\n");
    assert!(!git.is_tracked("docs/records/DEC-0001.md").unwrap());
    git.intent_to_add("docs/records/DEC-0001.md")
        .expect("intent to add");
    assert!(git.is_tracked("docs/records/DEC-0001.md").unwrap());
    let staged = sandbox.git_text(
        &repo,
        &["ls-files", "--stage", "--", "docs/records/DEC-0001.md"],
    );
    assert!(staged.ends_with("\tdocs/records/DEC-0001.md"), "{staged}");
    git.remove_cached("docs/records/DEC-0001.md")
        .expect("removed");
    assert_eq!(
        sandbox.git_text(
            &repo,
            &["ls-files", "--stage", "--", "docs/records/DEC-0001.md"]
        ),
        ""
    );
    assert!(
        repo.join("docs/records/DEC-0001.md").is_file(),
        "the file stays"
    );
    git.remove_cached("docs/records/none.md")
        .expect("none is no failure");

    let first = sandbox.git_text(&repo, &["rev-parse", "HEAD"]);
    sandbox.git(&repo, &["add", "--", "docs/records/DEC-0001.md"]);
    write(&repo, "docs/a.md", "# A\n\nChanged.\n");
    sandbox.git(&repo, &["add", "--", "docs/a.md"]);
    sandbox.commit(&repo, "second");
    let second = sandbox.git_text(&repo, &["rev-parse", "HEAD"]);
    let changed = git.name_status(&first, &second).expect("name-status");
    let shown: Vec<String> = changed.iter().map(ToString::to_string).collect();
    assert_eq!(shown, ["M docs/a.md", "A docs/records/DEC-0001.md"]);
    assert!(changed[1].adds("docs/records/DEC-0001.md"));
    assert!(!changed[0].adds("docs/a.md"), "a modification adds nothing");
    assert!(!changed[1].adds("docs/a.md"));
}

/// Step 4's `WorktreeGit::is_intent_to_add`: true only of an index entry
/// `git add --intent-to-add` made (porcelain v2 `.A`), whatever the file
/// holds (an empty one too), in a subdirectory, a name with a space; false
/// of a real `git add` (`A.`, an empty file's too, whose blob is git's
/// empty one), a tracked file clean or modified, an untracked file, a path
/// nowhere, and a file whose name only begins with an intent-to-add one's.
/// In a repository with no commit yet as in one with history. M: `A.`
/// accepted.
#[test]
fn only_an_intent_to_add_entry_is_one() {
    let scratch = Scratch::new("qr-ita");
    let sandbox = Sandbox::new(scratch.path());
    for born in [true, false] {
        let repo = scratch.join(if born { "born" } else { "unborn" });
        write(&repo, "docs/a.md", "# A\n");
        sandbox.init(&repo);
        let repo = fs::canonicalize(&repo).unwrap();
        if born {
            sandbox.add_all(&repo);
            sandbox.commit(&repo, "first");
        }
        let env = GitEnv::new(&repo, sandbox.vars());
        let git = WorktreeGit::new(&repo, &env).expect("git");
        for (path, bytes) in [
            ("docs/records/DEC-0001.md", "# Record\n"),
            ("docs/records/empty.md", ""),
            ("docs/records/with space.md", "# Spaced\n"),
            ("staged.md", "# Staged\n"),
            ("staged-empty.md", ""),
            ("untracked.md", "# Untracked\n"),
            ("docs/records/DEC-0001.md.bak", "# Backup\n"),
        ] {
            write(&repo, path, bytes);
        }
        for path in [
            "docs/records/DEC-0001.md",
            "docs/records/empty.md",
            "docs/records/with space.md",
        ] {
            git.intent_to_add(path).expect("intent to add");
        }
        sandbox.git(&repo, &["add", "--", "staged.md", "staged-empty.md"]);
        let short = sandbox.git_text(
            &repo,
            &[
                "status",
                "--porcelain=v2",
                "--untracked-files=no",
                "--",
                "staged-empty.md",
            ],
        );
        assert!(short.starts_with("1 A. "), "{short}");
        assert!(
            short.contains(" e69de29bb2d1d6434b8b29ae775ad8c2e48c5391 "),
            "git's empty blob: {short}"
        );
        if born {
            write(&repo, "docs/a.md", "# A\n\nChanged.\n");
        }
        for (path, want) in [
            ("docs/records/DEC-0001.md", true),
            ("docs/records/empty.md", true),
            ("docs/records/with space.md", true),
            ("staged.md", false),
            ("staged-empty.md", false),
            ("docs/a.md", false),
            ("untracked.md", false),
            ("docs/records/DEC-0001.md.bak", false),
            ("docs/records/DEC-0001", false),
            ("docs/records", false),
            ("nowhere.md", false),
        ] {
            assert_eq!(
                git.is_intent_to_add(path).expect("git reads its index"),
                want,
                "born {born}: {path}"
            );
        }
        // A real add over the intent-to-add entry is no longer one.
        sandbox.git(&repo, &["add", "--", "docs/records/DEC-0001.md"]);
        assert!(!git.is_intent_to_add("docs/records/DEC-0001.md").unwrap());
        // Taken back: no entry.
        git.remove_cached("docs/records/empty.md").expect("removed");
        assert!(!git.is_intent_to_add("docs/records/empty.md").unwrap());
    }
}
