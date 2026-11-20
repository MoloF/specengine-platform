//! docs/features/mixed-load-memory.md (RM-0134) MX1, MX2: one queue holding
//! every kind in every state the store writes, written through the store's
//! API at fixed clocks into a template database, a few columns then set as
//! a crash, an older build or a hand edit leaves them, and every row
//! restored as given ([`SqliteQueue::restore`]) into the queue under test.
//! A `#[cfg(test)]` module of the store's oracle tests (its parent,
//! `mixed_load_oracle`, invented places), which the CLI's goldens
//! (`inbox_rows.rs`, real repositories) include by `#[path]`; each includer
//! names the store's crate as its sibling module `store`. Like any store
//! source it names no kind by a literal or by comparison: a kind is picked
//! by what it does (`amends`, `holds_why`).
//!
//! The rows ([`seed`]), all the project's, by ID number:
//! - ours: each kind that applies (an update, a file-form create, a
//!   section-form create, an interpretation, an amendment) and each kind
//!   that decides (a question, a discrepancy, a decision) `open`,
//!   `approved`, `applied` and `rejected` (a deciding one approved with its
//!   record: the recorded items); a drift item in each state (its states
//!   but `open` set by hand: no store call approves a drift item alone); an
//!   update `open` with a reject staged, a question `open` with an approve
//!   staged;
//! - another repository's (it exists): an update and a question `open`, an
//!   update `applied`;
//! - a gone repository's: 12 rows, ten `open`, two `approved`;
//! - ours, set by hand: a corrupt `author` (`open`), a range-like
//!   `branch` (`open`), a `base_commit` that is no object ID (`open`), a
//!   NULL `status`, an unknown `status` (`parked`), and a corrupt `author`
//!   on a `rejected` row (skipped silently unless every state is asked).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::path::Path;

use specengine_core::drift::{DriftEvidence, DriftLock, DriftMarker, DriftRow};
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;

use super::store::{
    Choice, Decision, DriftResult, Intake, Interpretation, NewDrift, NewIntake, NewProposal,
    PROPOSAL_COLUMNS, Place, ProposalKind, ProposalQueue as _, RecordApproval, RecordSeries,
    Restore, SqliteQueue, Stage, StoredQueue, patch_hash,
};

/// The rows [`seed`] writes.
pub const ROWS: usize = 56;

/// The section the updates edit: `fixtures/spec-a`'s, so a walk of the
/// CLI reads a node that exists.
pub const TARGET: &str = "EDGE-SPRINT-EMPTY";
/// Its file.
pub const TARGET_PATH: &str = "docs/spec/movement/sprint.md";
/// Another section of the same file.
pub const OTHER_TARGET: &str = "RULE-SPRINT-COST";

/// The places the rows are raised at.
pub struct Places {
    /// The current repository's (the queue's reader's).
    pub ours: Place,
    /// Another repository of the project, which exists.
    pub other: Place,
    /// A repository that no longer exists.
    pub gone: Place,
}

impl Places {
    /// Invented places, no file behind them (the store reads no file).
    pub fn invented() -> Self {
        let place = |dir: &str, branch: &str| Place {
            git_common_dir: format!("/{dir}/.git"),
            worktree: format!("/{dir}"),
            root_rel: String::new(),
            branch: branch.to_owned(),
            base_commit: "1".repeat(40),
        };
        Self {
            ours: place("r", "t1"),
            other: place("o", "main"),
            gone: place("g", "main"),
        }
    }
}

/// What [`seed`] wrote: each row's role and ID, by ID number.
pub struct Seeded {
    pub roles: Vec<(String, String)>,
}

impl Seeded {
    /// The ID of `role`.
    pub fn id(&self, role: &str) -> &str {
        self.roles
            .iter()
            .find(|(name, _)| name == role)
            .map_or_else(|| panic!("no role {role}"), |(_, id)| id.as_str())
    }
}

/// Distinct, rising UTC stamps, one a minute from 08:01.
struct Clock(u32);

impl Clock {
    fn tick(&mut self) -> String {
        self.0 += 1;
        format!("2026-11-20T{:02}:{:02}:00Z", 8 + self.0 / 60, self.0 % 60)
    }
}

fn agent(login: Option<&str>) -> Author {
    let mut author = Author::new(
        Some("developer".to_owned()),
        Some("m-1".to_owned()),
        Some("r-7".to_owned()),
    )
    .expect("an agent");
    author.login = login.map(str::to_owned);
    author
}

fn owner(note: &str) -> Decision {
    Decision {
        decided_by: "Ann Owner <ann@example.invalid>".to_owned(),
        note: Some(note.to_owned()),
        staged_at: None,
    }
}

/// 64 hex digits of `n`.
fn hex(n: u32) -> String {
    format!("{n:064x}")
}

fn applying(place: &Place, kind: ProposalKind, target: &str, n: u32) -> NewProposal {
    let base_text = format!("The rule as it stood ({n}).\n");
    let base_hash = format!("b3:{}", hex(n));
    let new_text = format!("The rule, made plain ({n}).\n");
    NewProposal {
        kind,
        target_id: target.to_owned(),
        target_path: TARGET_PATH.to_owned(),
        place: place.clone(),
        patch_hash: patch_hash(target, &base_hash, &new_text),
        base_hash: Some(base_hash),
        base_text: Some(base_text),
        new_text,
        rationale: format!(
            "Edit {n}: the sprint's empty case, said once and in the rule's own words.\nIts \
             second line is never shown."
        ),
        author: agent(Some("ann")),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
        interpretation: None,
    }
}

/// An interpretation of [`TARGET`]: an applying proposal carrying its
/// decision, how and why.
fn interpreting(place: &Place, n: u32) -> NewProposal {
    NewProposal {
        interpretation: Some(Interpretation {
            decision: "DEC-0023".to_owned(),
            how: format!("Rest is any speed under 40 % of walking ({n})."),
            why: "DEC-0023 says rest; the rule names no speed.".to_owned(),
        }),
        ..applying(place, ProposalKind::Interpretation, TARGET, n)
    }
}

fn update(place: &Place, n: u32) -> NewProposal {
    applying(place, ProposalKind::Update, TARGET, n)
}

/// A file-form create of `MX-NEW-<n>`.
fn new_file(place: &Place, n: u32) -> NewProposal {
    let id = format!("MX-NEW-{n}");
    let text = format!("---\nid: {id}\n---\n\n# New record {n}\n");
    NewProposal {
        kind: ProposalKind::Create,
        patch_hash: patch_hash(&id, "", &text),
        target_id: id.clone(),
        target_path: format!("docs/records/{id}.md"),
        place: place.clone(),
        base_hash: None,
        base_text: None,
        new_text: text,
        rationale: format!("A new record ({n})."),
        author: agent(None),
        diagnostics: Vec::new(),
        new_ids: vec![id],
        interpretation: None,
    }
}

/// A section-form create of `MX-SEC-<n>` under [`OTHER_TARGET`].
fn sections(place: &Place, n: u32) -> NewProposal {
    let mut proposal = applying(place, ProposalKind::Create, OTHER_TARGET, n);
    proposal.new_text = format!("## Cost {{#{OTHER_TARGET}}}\n\n### New {{#MX-SEC-{n}}}\n");
    proposal.patch_hash = patch_hash(
        OTHER_TARGET,
        proposal.base_hash.as_deref().unwrap_or_default(),
        &proposal.new_text,
    );
    proposal.new_ids = vec![format!("MX-SEC-{n}")];
    proposal
}

/// An amendment: the new spec `docs/features/mx-amend-<n>.md`.
fn amendment(place: &Place, n: u32) -> NewProposal {
    let path = format!("docs/features/mx-amend-{n}.md");
    let text = format!(
        "---\nclass: spec\nstatus: draft\nlinks:\n  amends: [{TARGET}]\n---\n\n# Amendment \
         {n}\n\n## Delay {{#AC-01}}\n\nThe sprint waits {n} s.\n"
    );
    NewProposal {
        kind: ProposalKind::ALL
            .into_iter()
            .find(|kind| kind.amends())
            .expect("the kind that amends"),
        patch_hash: patch_hash(&path, "", &text),
        target_id: path.clone(),
        target_path: path,
        place: place.clone(),
        base_hash: None,
        base_text: None,
        new_text: text,
        rationale: format!("The sprint's delay, amended ({n})."),
        author: agent(None),
        diagnostics: Vec::new(),
        new_ids: vec![format!("mx-amend-{n}/AC-01")],
        interpretation: None,
    }
}

/// A question or a decision on [`TARGET`], its text `Q<n>`'s own: the kind
/// that holds a why (a decision) its why, the other (a question) a gist on
/// every third.
fn answered(place: &Place, kind: ProposalKind, n: u32) -> NewIntake {
    NewIntake {
        kind,
        target_path: TARGET_PATH.to_owned(),
        place: place.clone(),
        author: agent(Some("ann")),
        intake: Intake {
            target_ids: vec![TARGET.to_owned(), OTHER_TARGET.to_owned()],
            severity: if n.is_multiple_of(2) {
                IntakeSeverity::Normal
            } else {
                IntakeSeverity::High
            },
            gap_type: None,
            summary: format!("Does an empty sprint end at once in case {n}?\nIts detail."),
            working_answer: Some(format!("Yes, at once ({n}).")),
            price_of_other: Some("A rebalance of the stamina table.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
            why: kind.holds_why().then(|| format!("Work needed it ({n}).")),
            gist: (!kind.holds_why() && n.is_multiple_of(3))
                .then(|| format!("Empty sprint, case {n}")),
            matters: None,
        },
        actor: None,
        card_id: None,
    }
}

fn discrepancy(place: &Place, n: u32) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes ({n})"),
        price: "1 item".to_owned(),
    };
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: TARGET_PATH.to_owned(),
        place: place.clone(),
        author: agent(None),
        intake: Intake {
            target_ids: vec![TARGET.to_owned()],
            severity: IntakeSeverity::High,
            gap_type: Some(GapType::Contradicts),
            summary: format!("The code ends an empty sprint late in case {n}."),
            working_answer: None,
            price_of_other: None,
            evidence: vec![Evidence {
                file: "game/sprint.rs".to_owned(),
                qpath: None,
                lines: Some("3-9".to_owned()),
                observed: "ends after a frame".to_owned(),
                documented: "ends at once".to_owned(),
            }],
            options: vec![option("code"), option("spec")],
            recommendation: Some(1),
            distinct_from: Vec::new(),
            why: None,
            gist: None,
            matters: None,
        },
        actor: None,
        card_id: None,
    }
}

fn drift(place: &Place, n: u32) -> NewDrift {
    let node = format!("RULE-DRIFT-{n}");
    let row = DriftRow {
        node: node.clone(),
        relation: "configures".to_owned(),
        file: "crates/tally/counter.rs".to_owned(),
        kind: Some("item".to_owned()),
        symbol: Some("crates/tally::counter::CAP".to_owned()),
        item_kind: Some("const_item".to_owned()),
        state: "code_ahead".to_owned(),
        category: Some("code_changed".to_owned()),
        detail: None,
        class: Some("check".to_owned()),
        pre_existing: true,
        markers: vec![DriftMarker {
            line: 8,
            rev: Some(2),
        }],
        node_rev: Some(2),
        lock: Some(DriftLock {
            rev: 2,
            commit: "8".repeat(40),
            verified_at: "2026-10-08".to_owned(),
        }),
        compared: Some("ast".to_owned()),
    };
    NewDrift {
        target_id: node,
        target_path: "spec.lock".to_owned(),
        place: place.clone(),
        author: agent(None),
        task_id: None,
        severity: IntakeSeverity::Normal,
        summary: format!("code_ahead code_changed ({n})"),
        evidence: serde_json::to_string(&DriftEvidence {
            rows: vec![row],
            more: 0,
        })
        .expect("JSON"),
        hash: format!("b3:{}", hex(n)),
    }
}

/// The template queue and its clock, naming each row's role.
struct Writer {
    queue: SqliteQueue,
    clock: Clock,
    roles: Vec<(String, String)>,
    record: u32,
}

/// The states a row is taken to through the store.
#[derive(Clone, Copy, PartialEq, Eq)]
enum To {
    Open,
    Approved,
    Applied,
    Rejected,
}

const STATES: [(To, &str); 4] = [
    (To::Open, "open"),
    (To::Approved, "approved"),
    (To::Applied, "applied"),
    (To::Rejected, "rejected"),
];

impl Writer {
    fn named(&mut self, role: String, id: String) -> String {
        self.roles.push((role, id.clone()));
        id
    }

    fn applying(&mut self, role: String, proposal: &NewProposal, to: To) -> String {
        let now = self.clock.tick();
        let id = self.queue.create(proposal, &now).expect("created").id;
        match to {
            To::Open => {}
            To::Approved | To::Applied => {
                let now = self.clock.tick();
                self.queue
                    .approve(&id, &owner("Because."), &now)
                    .expect("approved");
                if to == To::Applied {
                    let now = self.clock.tick();
                    self.queue
                        .applied(&id, &"c".repeat(40), &now)
                        .expect("applied");
                }
            }
            To::Rejected => {
                let now = self.clock.tick();
                self.queue
                    .reject(&id, &owner("Not wanted."), &now)
                    .expect("rejected");
            }
        }
        self.named(role, id)
    }

    fn deciding(&mut self, role: String, intake: &NewIntake, choice: Choice, to: To) -> String {
        let now = self.clock.tick();
        let id = self
            .queue
            .create_intake(intake, &[], None, &now)
            .expect("an intake")
            .created
            .expect("stored: no hit")
            .id;
        match to {
            To::Open => {}
            To::Approved | To::Applied => {
                let series = RecordSeries {
                    prefix: "DEC".to_owned(),
                    width: 4,
                    corpus_max: 0,
                };
                let preview = self.queue.next_record(&series).expect("the next record");
                self.record += 1;
                let seen = self.queue.get(&id).expect("get").expect("stored").seen();
                let approval = RecordApproval {
                    series,
                    preview: preview.clone(),
                    path: format!("docs/decisions/{preview}.md"),
                    title: format!("Record {}", self.record),
                    text: format!("---\nid: {preview}\n---\n"),
                    choice,
                };
                let now = self.clock.tick();
                self.queue
                    .approve_record_from(&id, &seen, &approval, &owner("Because."), &now)
                    .expect("approved with its record");
                if to == To::Applied {
                    let now = self.clock.tick();
                    self.queue
                        .applied(&id, &"d".repeat(40), &now)
                        .expect("applied");
                }
            }
            To::Rejected => {
                let now = self.clock.tick();
                self.queue
                    .reject(&id, &owner("Asked before."), &now)
                    .expect("rejected");
            }
        }
        self.named(role, id)
    }

    fn drift(&mut self, role: String, item: NewDrift) -> String {
        let now = self.clock.tick();
        let made = self.queue.create_drift(&[item], &now).expect("a raise");
        let [DriftResult::Created { proposal, .. }] = made.as_slice() else {
            panic!("a drift item stored: {made:?}");
        };
        let id = proposal.id.clone();
        self.named(role, id)
    }

    fn staged(&mut self, id: &str, stage: &Stage) {
        let seen = self.queue.get(id).expect("get").expect("stored").seen();
        let now = self.clock.tick();
        self.queue
            .stage_from(id, &seen, stage, &now)
            .expect("staged");
    }
}

/// `row`'s column `name` set to `value`.
fn set(rows: &mut StoredQueue, id: &str, name: &str, value: Option<&str>) {
    let at = PROPOSAL_COLUMNS
        .iter()
        .position(|column| *column == name)
        .unwrap_or_else(|| panic!("no column {name}"));
    let row = rows
        .proposals
        .iter_mut()
        .find(|row| row.id() == Some(id))
        .unwrap_or_else(|| panic!("no row {id}"));
    row.columns[at] = value.map(str::to_owned);
}

/// Writes [`ROWS`] rows at `places` into `db`, a queue of `project` with no
/// row yet, through a template queue at `template` (a path no database is
/// at). Every stamp is fixed: two seeds of one `places` hold equal rows.
pub fn seed(template: &Path, db: &Path, project: &str, places: &Places) -> Seeded {
    assert!(
        !template.exists(),
        "{}: a fresh template",
        template.display()
    );
    let mut writer = Writer {
        queue: SqliteQueue::open(template, project).expect("the template queue"),
        clock: Clock(0),
        roles: Vec::new(),
        record: 0,
    };
    let w = &mut writer;
    let ours = &places.ours;
    let mut n = 0;
    let mut next = || {
        n += 1;
        n
    };
    // A reject staged first: a walk of the open items meets it before any
    // item it asks about.
    let staged = w.applying(
        "ours update open, reject staged".to_owned(),
        &update(ours, next()),
        To::Open,
    );
    w.staged(
        &staged,
        &Stage::Reject {
            reason: "Out of scope for the sprint.".to_owned(),
        },
    );
    for (to, state) in STATES {
        w.applying(format!("ours update {state}"), &update(ours, next()), to);
    }
    for (to, state) in STATES {
        w.applying(
            format!("ours create file {state}"),
            &new_file(ours, next()),
            to,
        );
    }
    w.applying(
        "ours create sections open".to_owned(),
        &sections(ours, next()),
        To::Open,
    );
    for (to, state) in STATES {
        let proposal = interpreting(ours, next());
        w.applying(format!("ours interpretation {state}"), &proposal, to);
    }
    for (to, state) in STATES {
        w.applying(
            format!("ours amendment {state}"),
            &amendment(ours, next()),
            to,
        );
    }
    for (to, state) in STATES {
        let intake = answered(ours, ProposalKind::Question, next());
        w.deciding(
            format!("ours question {state}"),
            &intake,
            Choice::WorkingAnswer,
            to,
        );
    }
    for (to, state) in STATES {
        let intake = discrepancy(ours, next());
        w.deciding(
            format!("ours discrepancy {state}"),
            &intake,
            Choice::Option(0),
            to,
        );
    }
    for (to, state) in STATES {
        let intake = answered(ours, ProposalKind::Decision, next());
        w.deciding(
            format!("ours decision {state}"),
            &intake,
            Choice::WorkingAnswer,
            to,
        );
    }
    for (_, state) in STATES {
        w.drift(format!("ours drift {state}"), drift(ours, next()));
    }
    let asked = answered(ours, ProposalKind::Question, next());
    let question = w.deciding(
        "ours question open, approve staged".to_owned(),
        &asked,
        Choice::WorkingAnswer,
        To::Open,
    );
    w.staged(
        &question,
        &Stage::Approve {
            option: None,
            answer: Some("No: it ends after a frame.".to_owned()),
            canon: None,
            editorial: false,
            note: Some("Checked with design.".to_owned()),
            span_hash: None,
        },
    );
    let other = &places.other;
    w.applying(
        "other update open".to_owned(),
        &update(other, next()),
        To::Open,
    );
    let intake = answered(other, ProposalKind::Question, next());
    w.deciding(
        "other question open".to_owned(),
        &intake,
        Choice::WorkingAnswer,
        To::Open,
    );
    w.applying(
        "other update applied".to_owned(),
        &update(other, next()),
        To::Applied,
    );
    let gone = &places.gone;
    for k in 1..=12 {
        let to = if k > 10 { To::Approved } else { To::Open };
        if k % 2 == 0 && to == To::Open {
            let intake = answered(gone, ProposalKind::Question, next());
            w.deciding(
                format!("gone {k} question"),
                &intake,
                Choice::WorkingAnswer,
                to,
            );
        } else {
            w.applying(format!("gone {k} update"), &update(gone, next()), to);
        }
    }
    let intake = answered(ours, ProposalKind::Question, next());
    let bad_author = w.deciding(
        "ours corrupt author".to_owned(),
        &intake,
        Choice::WorkingAnswer,
        To::Open,
    );
    let bad_branch = w.applying(
        "ours corrupt branch".to_owned(),
        &update(ours, next()),
        To::Open,
    );
    let bad_base = w.applying(
        "ours corrupt base_commit".to_owned(),
        &update(ours, next()),
        To::Open,
    );
    let intake = answered(ours, ProposalKind::Question, next());
    let null_state = w.deciding(
        "ours NULL status".to_owned(),
        &intake,
        Choice::WorkingAnswer,
        To::Open,
    );
    let parked = w.applying(
        "ours unknown status".to_owned(),
        &update(ours, next()),
        To::Open,
    );
    let intake = discrepancy(ours, next());
    let bad_rejected = w.deciding(
        "ours corrupt author rejected".to_owned(),
        &intake,
        Choice::Option(0),
        To::Rejected,
    );
    let mut rows = writer.queue.stored_rows().expect("the template's rows");
    let id = |role: &str| {
        writer
            .roles
            .iter()
            .find(|(name, _)| name == role)
            .map(|(_, id)| id.clone())
            .unwrap_or_else(|| panic!("no role {role}"))
    };
    // A drift item is approved by its node's lock accept, applied by its
    // commit: no store call takes it there alone.
    for state in ["approved", "applied", "rejected"] {
        let drift = id(&format!("ours drift {state}"));
        set(&mut rows, &drift, "status", Some(state));
        set(
            &mut rows,
            &drift,
            "decided_by",
            Some("Ann Owner <ann@example.invalid>"),
        );
        set(
            &mut rows,
            &drift,
            "decided_at",
            Some("2026-11-20T11:00:00Z"),
        );
        if state == "applied" {
            set(&mut rows, &drift, "applied_commit", Some(&"e".repeat(40)));
        }
    }
    set(
        &mut rows,
        &bad_author,
        "author",
        Some("{\"type\":\"agent\",\"role\":"),
    );
    set(&mut rows, &bad_branch, "branch", Some("t1..main"));
    set(&mut rows, &bad_base, "base_commit", Some("HEAD"));
    set(&mut rows, &null_state, "status", None);
    set(&mut rows, &parked, "status", Some("parked"));
    set(
        &mut rows,
        &bad_rejected,
        "author",
        Some("[\"not an author\"]"),
    );
    assert_eq!(rows.proposals.len(), ROWS, "the seeded rows");
    let mut queue = SqliteQueue::open(db, project).expect("the queue under test");
    assert!(
        matches!(queue.restore(&rows).expect("restored"), Restore::Restored),
        "{}: a queue with no row",
        db.display()
    );
    Seeded {
        roles: writer.roles,
    }
}
