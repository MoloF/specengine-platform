//! docs/features/spec-ids.md (RM-0073), the CLI's half, through the library
//! (and the binary where its exit and stderr are the subject): a data
//! directory holding `proposal-floor` mints every number-shape spec ID from
//! 100001, one with none below it.
//!
//! - AC-3 (d), (e): `spec approve` of a question writes a hub's record from
//!   `DEC-100001`, one more than the side's highest, and a terminal's below
//!   it whatever the tree holds from 100001; an invalid floor stores nothing,
//!   exit 2 with the floor's text. M2, M5.
//! - AC-4 (a), (b), (d), (e), (f) and (c)'s taken and reserved refusals: the
//!   propose step refuses a new ID of the other side, exit 1, naming the next
//!   free ID of its side, nothing stored, reserved or logged; the section
//!   form, `spec propose amendment` and `--dry-run` alike; every refusal that
//!   names a next free ID names its side's. (c)'s insert race is
//!   `proposal_kinds.rs` `n2_on_either_side_*`, beside n2's seam. M6, M7, M8.
//! - AC-5 (b) and the CLI's twin of (a): an apply never judges a side: a
//!   hub's open create restored on a terminal, and a terminal's open create
//!   and amendment restored on a hub, apply by `spec approve`. M9.
//!
//! Setup: scratch git repositories of `fixtures/card-press` (a print shop,
//! `STY` width 4, `STY-0001` in the tree, no `[decision_records]`) and
//! `fixtures/spec-a` (a game, `DEC` its records, `AC` feature-scoped, width
//! 2), committed, a linked worktree on `t1`, a scratch `HOME`. A hub's side
//! is a data directory where store `store_floor` wrote the file
//! (docs/features/spec-ids.md "Acceptance criteria"). Proposals are raised in
//! the linked worktree, approved from the main one.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::Path;

use common::decision::flags;
use common::proposal::{Pair, cannot, edit, refused};
use common::{data_dir, read_text, write};
use specengine_cli::{
    AmendmentRequest, ApproveFlags, CliError, CreateRequest, Exit, ExportStateRequest, Globals,
    ImportStateRequest, ProposalOutcome, ProposedText, export_state, import_state,
    propose_amendment, propose_create,
};
use specengine_core::proposal::{Author, HUB_PROPOSAL_FLOOR};
use specengine_store::{
    NewProposal, PROPOSAL_FLOOR_FILE, Place, ProposalKind, ProposalQueue as _, ProposalStatus,
    patch_hash, store_floor,
};

/// The clock of every propose.
const CLOCK: &str = "2026-11-04T12:00:00Z";
/// The clock of every approve.
const DECIDED: &str = "2026-11-04T12:30:00Z";
const RATIONALE: &str = "a second ruling";
/// The line of an invalid floor, after `<data dir>/proposal-floor: `.
const NOT_A_FLOOR: &str = "not a proposal floor (a decimal number from 1, then LF)";

const FRONT: &str = "docs/spec/sht-front.md";
const SPECIAL: &str = "docs/spec/special-edition.md";

// ---------------------------------------------------------------- helpers

/// The data directory of `pair` holds the floor a hub's start writes.
fn on_hub(pair: &Pair) {
    let dir = data_dir(&pair.home);
    fs::create_dir_all(&dir).expect("the data directory");
    assert_eq!(
        store_floor(&dir, HUB_PROPOSAL_FLOOR).expect("the floor"),
        HUB_PROPOSAL_FLOOR
    );
}

/// docs/features/spec-ids.md "Data", "Refusal texts": a terminal's.
fn off_terminal(id: &str, line: usize, next: &str) -> String {
    format!(
        "`{id}` (line {line}): numbers from 100001 are a hub's; the next free here is `{next}`; \
         nothing stored"
    )
}

/// docs/features/spec-ids.md "Data", "Refusal texts": a hub's.
fn off_hub(id: &str, line: usize, next: &str) -> String {
    format!(
        "`{id}` (line {line}): a hub numbers from 100001; the next free here is `{next}`; nothing \
         stored"
    )
}

/// A card-press style ruling whose `id:` (line 2) is `id`, as `STY-0001`
/// is written.
fn ruling(id: &str) -> String {
    format!(
        "---\nid: {id}\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n\
         canon: docs/spec/sht-front.md#the-front-sheet\nlinks:\n  constrains: [SHT-FRONT]\n\
         scope: [front-sheet]\n---\n\n# The masthead keeps one face\n\n\
         The masthead runs in one typeface every week.\n"
    )
}

/// `ruling(id)`'s path.
fn ruling_path(id: &str) -> String {
    format!("docs/records/STY/{id}.md")
}

/// A new spec file amending `SHT-FRONT`, defining the section `{#id}`.
fn amending(id: &str) -> String {
    format!(
        "---\nclass: spec\nstatus: draft\nscope: [front-sheet]\nlinks:\n  amends: [SHT-FRONT]\n\
         ---\n\n# A special edition\n\n## Its masthead {{#{id}}}\n\n\
         A special edition may run its own masthead.\n"
    )
}

/// A refusal text of "Data", by the new ID, its line and the next free ID.
type Refusal = fn(&str, usize, &str) -> String;

/// The 1-based line of `text` holding `needle`, which occurs once.
fn line_of(text: &str, needle: &str) -> usize {
    assert_eq!(text.matches(needle).count(), 1, "{needle:?} once");
    text.lines()
        .position(|line| line.contains(needle))
        .expect("the line")
        + 1
}

fn request(pair: &Pair, cwd: &Path, target: &str, base: Option<&str>, text: &str) -> CreateRequest {
    CreateRequest {
        target: target.to_owned(),
        base: base.map(str::to_owned),
        text: ProposedText::Given(text.as_bytes().to_vec()),
        rationale: RATIONALE.to_owned(),
        author_role: Some("writer".to_owned()),
        author_model: None,
        run: None,
        login: None,
        now: CLOCK.to_owned(),
        git: pair.git_env(cwd),
    }
}

fn create(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    base: Option<&str>,
    text: &str,
) -> Result<ProposalOutcome, CliError> {
    propose_create(
        &pair.env(cwd),
        &Globals::default(),
        &request(pair, cwd, target, base, text),
    )
}

/// A create that must be stored: its ID.
fn created(pair: &Pair, cwd: &Path, target: &str, base: Option<&str>, text: &str) -> String {
    let outcome = create(pair, cwd, target, base, text)
        .unwrap_or_else(|error| panic!("propose create {target}: {error}"));
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "propose create {target}: {outcome:?}"
    );
    outcome.document.id.clone().expect("an ID")
}

fn amend(pair: &Pair, cwd: &Path, target: &str, text: &str) -> Result<ProposalOutcome, CliError> {
    propose_amendment(
        &pair.env(cwd),
        &Globals::default(),
        &AmendmentRequest {
            target: target.to_owned(),
            text: ProposedText::Given(text.as_bytes().to_vec()),
            rationale: RATIONALE.to_owned(),
            author_role: Some("writer".to_owned()),
            author_model: None,
            run: None,
            login: None,
            now: CLOCK.to_owned(),
            git: pair.git_env(cwd),
        },
        None,
    )
}

/// `(id, by, status)` of every reservation; none without a queue.
fn reservations(pair: &Pair) -> Vec<(String, String, String)> {
    if !pair.db().exists() {
        return Vec::new();
    }
    pair.queue()
        .reserved()
        .expect("reserved")
        .into_iter()
        .map(|held| (held.id, held.by, held.status.as_str().to_owned()))
        .collect()
}

/// A refused propose (exit 1) of `call`: its reason; nothing stored,
/// written, reserved or logged.
fn refused_by(
    pair: &Pair,
    context: &str,
    call: impl FnOnce() -> Result<ProposalOutcome, CliError>,
) -> String {
    let before = (pair.state(), reservations(pair), pair.events().len());
    let outcome = call();
    let reason = refused(&outcome, context);
    assert!(
        outcome
            .as_ref()
            .is_ok_and(|outcome| outcome.document.id.is_none()),
        "{context}: {outcome:?}"
    );
    assert_eq!(
        (pair.state(), reservations(pair), pair.events().len()),
        before,
        "{context}: nothing stored, written, reserved or logged"
    );
    reason
}

fn refused_create(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    base: Option<&str>,
    text: &str,
    context: &str,
) -> String {
    refused_by(pair, context, || create(pair, cwd, target, base, text))
}

/// `path` holding `text` committed on `t1` in the linked worktree.
fn committed(pair: &Pair, path: &str, text: &str) {
    write(&pair.linked, path, text);
    pair.commit_all(&pair.linked, &format!("{path} by hand"));
}

/// A store-level live create holding `ids`, at `path` of the linked
/// worktree: another proposal's reservation.
fn held(pair: &Pair, path: &str, ids: &[&str]) -> String {
    let text = ruling(ids[0]);
    let proposal = NewProposal {
        kind: ProposalKind::Create,
        target_id: ids[0].to_owned(),
        target_path: path.to_owned(),
        place: Place {
            git_common_dir: pair.main.join(".git").display().to_string(),
            worktree: pair.linked.display().to_string(),
            root_rel: String::new(),
            branch: "t1".to_owned(),
            base_commit: pair.rev(&pair.linked, "HEAD"),
        },
        base_hash: None,
        base_text: None,
        patch_hash: patch_hash(ids[0], "", &text),
        new_text: text,
        rationale: "The other one.".to_owned(),
        author: Author::human(),
        diagnostics: Vec::new(),
        new_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
        interpretation: None,
    };
    pair.queue()
        .create(&proposal, CLOCK)
        .expect("the other create")
        .id
}

/// An approve from the main worktree that must apply.
fn applied(pair: &Pair, id: &str) {
    let (outcome, questions) = pair.decide_at(
        &pair.main,
        id,
        &ApproveFlags::default(),
        None,
        true,
        DECIDED,
    );
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
    assert_eq!(questions.len(), 1, "approve {id}: {questions:?}");
    assert_eq!(pair.proposal(id).status, ProposalStatus::Applied, "{id}");
}

/// `spec export state` of the queue to `out`.
fn exported(pair: &Pair, out: &Path) {
    export_state(
        &pair.env(&pair.main),
        &Globals::default(),
        &ExportStateRequest {
            out: Some(out.to_path_buf()),
            now: DECIDED.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"));
}

/// `spec import-state file` into the data directory, made empty first, then
/// holding the floor when `hub`.
fn imported(pair: &Pair, file: &Path, hub: bool) {
    let dir = data_dir(&pair.home);
    fs::remove_dir_all(&dir).expect("the data directory removed");
    if hub {
        on_hub(pair);
    }
    let mut consent = |_: &str| true;
    let outcome = import_state(
        &pair.env(&pair.main),
        &Globals::default(),
        &ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut consent,
    )
    .unwrap_or_else(|error| panic!("import state: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        data_dir(&pair.home).join(PROPOSAL_FLOOR_FILE).exists(),
        hub,
        "the floor as set"
    );
}

/// spec-a's `DEC-0023` as a record numbered `id`, its own H1, no
/// `supersedes`.
fn high_record(id: &str) -> String {
    format!(
        "---\nid: {id}\nclass: decision\nstatus: accepted\ndate: 2026-09-01\n\
         canon: docs/spec/movement/stamina.md#regeneration\nscope: [stamina]\n---\n\n\
         # Regeneration waits a full second\n\nStamina regenerates after a full second at rest.\n"
    )
}

fn dec_path(id: &str) -> String {
    format!("docs/records/DEC/{id}.md")
}

// ------------------------------------------------------------ AC-3 (d)

/// docs/features/spec-ids.md "Acceptance criteria" AC-3 (d): on spec-a with
/// a floor, `spec approve` of a question writes `DEC-100001`; with
/// `DEC-100003` added to the tree, the next writes `DEC-100004`. The host
/// terminal under the hub's `HOME` is on the hub's side with no daemon (I2).
/// M5: the side read from the daemon's flag (`DEC-0024`).
#[test]
fn ac3d_a_hubs_data_directory_writes_records_from_the_start() {
    let pair = Pair::new("si-3d-hub", "spec-a");
    on_hub(&pair);
    let first = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        "Does regeneration wait for full rest?",
        "Yes, a full second.",
        "a new case in the rule",
    );
    assert_eq!(first, "PR-100001", "the floor's PR- range");
    let (_, question) = pair.decide_ok(&pair.main, &first, &flags(None, None, None), None);
    assert!(question.contains("DEC-100001"), "{question}");
    let record = read_text(&pair.linked, &dec_path("DEC-100001"));
    assert!(record.starts_with("---\nid: DEC-100001\n"), "{record}");
    assert!(!pair.linked.join(dec_path("DEC-0024")).exists());
    assert_eq!(pair.proposal(&first).record.expect("kept").id, "DEC-100001");

    committed(&pair, &dec_path("DEC-100003"), &high_record("DEC-100003"));
    let second = pair.raise_question(
        &pair.linked,
        &["EDGE-STAM-ZERO"],
        "Is Exhausted applied at once?",
        "yes",
        "a delay rule",
    );
    let (_, question) = pair.decide_ok(&pair.main, &second, &flags(None, None, None), None);
    assert!(question.contains("DEC-100004"), "{question}");
    assert!(read_text(&pair.linked, &dec_path("DEC-100004")).starts_with("---\nid: DEC-100004\n"));
    assert!(!pair.linked.join(dec_path("DEC-100002")).exists());
}

/// docs/features/spec-ids.md "Acceptance criteria" AC-3 (d), the terminal:
/// no floor and `DEC-100003` in the tree, the record is `DEC-0024` (spec-a's
/// highest below the start is `DEC-0023`). M2: the corpus maximum counts
/// numbers from 100001 (`DEC-100004`).
#[test]
fn ac3d_a_terminal_writes_records_below_the_start_whatever_the_tree_holds() {
    let pair = Pair::new("si-3d-terminal", "spec-a");
    committed(&pair, &dec_path("DEC-100003"), &high_record("DEC-100003"));
    let id = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        "Does regeneration wait for full rest?",
        "Yes, a full second.",
        "a new case in the rule",
    );
    assert_eq!(id, "PR-0001");
    let (_, question) = pair.decide_ok(&pair.main, &id, &flags(None, None, None), None);
    assert!(question.contains("DEC-0024"), "{question}");
    assert!(read_text(&pair.linked, &dec_path("DEC-0024")).starts_with("---\nid: DEC-0024\n"));
    assert!(!pair.linked.join(dec_path("DEC-100004")).exists());
}

// ------------------------------------------------------------ AC-3 (e)

/// `(type, step)` pairs, typed.
fn typed(pairs: &[(&str, Option<u64>)]) -> Vec<(String, Option<u64>)> {
    pairs
        .iter()
        .map(|(kind, step)| ((*kind).to_owned(), *step))
        .collect()
}

/// A spec-a requirement record `R-13`, the propose step's subject.
const R13_TEXT: &str = "---\nid: R-13\nclass: canon\nstatus: accepted\nowner: owner\n\
                        reviewed: 2026-09-20\n---\n\n# A new record\n\nA body.\n";

/// docs/features/spec-ids.md "Acceptance criteria" AC-3 (e) and
/// "Data", "The constant and the side": an invalid `proposal-floor` stops the preview,
/// exit 2 with the floor's text and no question asked: no record and no
/// change of the proposal, its one `proposal.apply_failed` logged under the
/// existing rule of a refusal at apply's steps 2-10 (the coordinator's
/// working answer; `docs/canon/hub-confirm.md` "Order" words it for a hub's
/// POST); the propose step of a create stores nothing, no event.
#[test]
fn ac3e_an_invalid_floor_stores_nothing_exit_2() {
    let pair = Pair::new("si-3e", "spec-a");
    let id = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        "Does regeneration wait for full rest?",
        "Yes, a full second.",
        "a new case in the rule",
    );
    let floor = data_dir(&pair.home).join(PROPOSAL_FLOOR_FILE);
    fs::write(&floor, b"x\n").expect("an invalid floor");
    let before = pair.state();
    let events = pair.events().len();
    let (outcome, questions) = pair.decide(&pair.main, &id, &flags(None, None, None), None, true);
    let message = cannot(&outcome, "an invalid floor");
    assert!(
        message.contains(&format!("{PROPOSAL_FLOOR_FILE}: {NOT_A_FLOOR}")),
        "{message}"
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state(), before, "no file, ref or row changed");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Open);
    assert!(stored.record.is_none(), "no record: {:?}", stored.record);
    let logged: Vec<String> = pair.events()[events..]
        .iter()
        .map(|event| event.event_type.clone())
        .collect();
    assert_eq!(logged, ["proposal.apply_failed"], "its one event");
    assert!(!pair.linked.join(dec_path("DEC-0024")).exists());

    let before = (pair.state(), pair.events().len());
    let message = cannot(
        &create(
            &pair,
            &pair.linked,
            "docs/records/R/R-13.md",
            None,
            R13_TEXT,
        ),
        "a create",
    );
    assert!(message.contains(NOT_A_FLOOR), "{message}");
    assert_eq!(
        (pair.state(), pair.events().len()),
        before,
        "nothing stored, no event"
    );
    assert_eq!(
        fs::read(&floor).expect("kept"),
        b"x\n",
        "the file as it was"
    );
}

/// The coordinator's working answer on docs/features/spec-ids.md
/// "Rules and edge cases" I5 (ADR-0012): an invalid `proposal-floor` never stops an
/// apply of a create, which judges no side (step 6's hint, text only, falls
/// back to the words before the slice): a create stored on a terminal,
/// then the file corrupted, applies by `spec approve` with the events of
/// any applied create and no other; meanwhile a propose and a record's
/// decide still exit 2 with the floor's text.
#[test]
fn a_corrupted_floor_stops_no_apply_of_a_create() {
    let pair = Pair::new("si-corrupt", "spec-a");
    let id = created(
        &pair,
        &pair.linked,
        "docs/records/R/R-13.md",
        None,
        R13_TEXT,
    );
    let question = pair.raise_question(
        &pair.linked,
        &["Q-031"],
        "Does regeneration wait for full rest?",
        "Yes, a full second.",
        "a new case in the rule",
    );
    let floor = data_dir(&pair.home).join(PROPOSAL_FLOOR_FILE);
    fs::write(&floor, b"x\n").expect("an invalid floor");

    let before = (pair.state(), pair.events().len());
    let message = cannot(
        &create(
            &pair,
            &pair.linked,
            "docs/records/R/R-14.md",
            None,
            &R13_TEXT.replace("R-13", "R-14"),
        ),
        "a propose",
    );
    assert!(message.contains(NOT_A_FLOOR), "{message}");
    assert_eq!(
        (pair.state(), pair.events().len()),
        before,
        "nothing stored"
    );
    let (outcome, _) = pair.decide(&pair.main, &question, &flags(None, None, None), None, true);
    let message = cannot(&outcome, "a decide");
    assert!(message.contains(NOT_A_FLOOR), "{message}");

    applied(&pair, &id);
    assert_eq!(
        pair.events_of(&id),
        typed(&[
            ("proposal.created", None),
            ("proposal.approved", None),
            ("proposal.applied", None)
        ])
    );
    assert_eq!(read_text(&pair.linked, "docs/records/R/R-13.md"), R13_TEXT);
    assert_eq!(
        fs::read(&floor).expect("kept"),
        b"x\n",
        "the file as it was"
    );
}

// ------------------------------------------------------------ AC-4 (a), (b)

/// docs/features/spec-ids.md "Acceptance criteria" AC-4 (a): no floor, a
/// file-form create holding `id: STY-100002` exits 1 with the terminal's
/// text naming `STY-0002`; the queue empty, the event count unchanged. Its
/// own side stores. M6: the step removed.
#[test]
fn ac4a_a_terminal_refuses_a_hubs_number() {
    let pair = Pair::new("si-4a", "card-press");
    let reason = refused_create(
        &pair,
        &pair.linked,
        &ruling_path("STY-100002"),
        None,
        &ruling("STY-100002"),
        "a hub's number",
    );
    assert_eq!(reason, off_terminal("STY-100002", 2, "STY-0002"));
    assert!(pair.proposals().is_empty());
    assert!(pair.events().is_empty());
    assert!(reservations(&pair).is_empty());

    let id = created(
        &pair,
        &pair.linked,
        &ruling_path("STY-0002"),
        None,
        &ruling("STY-0002"),
    );
    assert_eq!(id, "PR-0001");
    assert_eq!(pair.proposal(&id).new_ids, ["STY-0002"]);
}

/// docs/features/spec-ids.md "Data", "Created IDs: the propose step": an
/// all-digit number past `u64` is no terminal's number (the developer's
/// iteration 2 counts it on the hub's side), so a terminal refuses
/// `STY-18446744073709551616` naming `STY-0002`, nothing stored; one below
/// the overflow, `u64::MAX`, alike.
#[test]
fn a_terminal_refuses_an_overflowing_number() {
    let pair = Pair::new("si-overflow", "card-press");
    for id in ["STY-18446744073709551616", "STY-18446744073709551615"] {
        let reason = refused_create(&pair, &pair.linked, &ruling_path(id), None, &ruling(id), id);
        assert_eq!(reason, off_terminal(id, 2, "STY-0002"));
    }
    assert!(pair.proposals().is_empty());
    assert!(pair.events().is_empty());
}

/// docs/features/spec-ids.md "Acceptance criteria" AC-4 (b): a floor, a
/// create of `STY-0002` exits 1 naming `STY-100001`; `STY-100001` stores,
/// and a name-shape ID is never judged. M6, M7 (the hub's next free over
/// every number: `STY-0002`).
#[test]
fn ac4b_a_hub_refuses_a_terminals_number() {
    let pair = Pair::new("si-4b", "card-press");
    on_hub(&pair);
    let reason = refused_create(
        &pair,
        &pair.linked,
        &ruling_path("STY-0002"),
        None,
        &ruling("STY-0002"),
        "a terminal's number",
    );
    assert_eq!(reason, off_hub("STY-0002", 2, "STY-100001"));
    assert!(pair.events().is_empty());

    let id = created(
        &pair,
        &pair.linked,
        &ruling_path("STY-100001"),
        None,
        &ruling("STY-100001"),
    );
    assert_eq!(id, "PR-100001");
    assert_eq!(
        reservations(&pair),
        [(
            "STY-100001".to_owned(),
            "PR-100001".to_owned(),
            "open".to_owned()
        )]
    );
    let sheet = "---\nid: SHT-SPECIAL\nclass: canon\ntitle: A special sheet\nparent: PRN-WEEKLY\n\
                 owner: owner\nreviewed: 2026-10-01\n---\n\n# A special sheet\n\nA body.\n";
    let named = created(&pair, &pair.linked, "docs/spec/sht-special.md", None, sheet);
    assert_eq!(pair.proposal(&named).new_ids, ["SHT-SPECIAL"]);
}

/// docs/features/spec-ids.md "Rules and edge cases", several new IDs on
/// mixed sides: the first of the other side in text order is named, at its
/// line; the next free ID counts the text's other new IDs of the side.
#[test]
fn several_new_ids_name_the_first_of_the_other_side() {
    let text = |doc: &str, a: &str, b: &str| {
        format!(
            "---\nid: {doc}\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n---\n\n\
             # Rulings\n\n## One {{#{a}}}\n\nA body.\n\n## Two {{#{b}}}\n\nA body.\n"
        )
    };
    let pair = Pair::new("si-mixed-terminal", "card-press");
    let mixed = text("STY-0005", "STY-100002", "STY-0007");
    let reason = refused_create(
        &pair,
        &pair.linked,
        &ruling_path("STY-0005"),
        None,
        &mixed,
        "a terminal's mixed text",
    );
    assert_eq!(
        reason,
        off_terminal("STY-100002", line_of(&mixed, "{#STY-100002}"), "STY-0008")
    );

    let pair = Pair::new("si-mixed-hub", "card-press");
    on_hub(&pair);
    let mixed = text("STY-100005", "STY-0002", "STY-100009");
    let reason = refused_create(
        &pair,
        &pair.linked,
        &ruling_path("STY-100005"),
        None,
        &mixed,
        "a hub's mixed text",
    );
    assert_eq!(
        reason,
        off_hub("STY-0002", line_of(&mixed, "{#STY-0002}"), "STY-100010")
    );
}

// ------------------------------------------------------------ AC-4 (c)

/// docs/features/spec-ids.md "Acceptance criteria" AC-4 (c), the taken and
/// the reserved refusals, a terminal: with `STY-0001` and `STY-100001` in the
/// tree each names `STY-0002`, the other side's numbers in the index and in
/// a live create counted by neither ("Data", "Next free"). The existing
/// checks run first: a taken ID of the other side reports as taken. M7.
#[test]
fn ac4c_a_terminals_taken_and_reserved_refusals_name_its_next_free() {
    let pair = Pair::new("si-4c-terminal", "card-press");
    committed(&pair, &ruling_path("STY-100001"), &ruling("STY-100001"));
    for (taken, path) in [("STY-100001", "STY-100001-b"), ("STY-0001", "STY-0001-b")] {
        let reason = refused_create(
            &pair,
            &pair.linked,
            &format!("docs/records/STY/{path}.md"),
            None,
            &ruling(taken),
            taken,
        );
        assert_eq!(
            reason,
            format!(
                "`{taken}` is defined in `{}` (`id: {taken}`); the next free is `STY-0002`; \
                 nothing stored",
                ruling_path(taken)
            )
        );
    }
    let holder = held(&pair, "docs/records/STY/held.md", &["STY-100002"]);
    assert_eq!(holder, "PR-0001");
    let reason = refused_create(
        &pair,
        &pair.linked,
        &ruling_path("STY-100002"),
        None,
        &ruling("STY-100002"),
        "reserved",
    );
    assert_eq!(
        reason,
        "`STY-100002` is reserved by `PR-0001` (open), a live create; the next free is \
         `STY-0002`; nothing stored"
    );
}

/// docs/features/spec-ids.md "Acceptance criteria" AC-4 (c), a hub: with
/// `STY-0001` alone the taken refusal names `STY-100001`; with `STY-100001`
/// added, the taken and the reserved refusals name `STY-100002`, the
/// terminal's numbers in a live create counted by neither. M7.
#[test]
fn ac4c_a_hubs_taken_and_reserved_refusals_name_its_next_free() {
    let pair = Pair::new("si-4c-hub", "card-press");
    on_hub(&pair);
    let taken = |id: &str, next: &str| {
        let reason = refused_create(
            &pair,
            &pair.linked,
            &format!("docs/records/STY/{id}-b.md"),
            None,
            &ruling(id),
            id,
        );
        assert_eq!(
            reason,
            format!(
                "`{id}` is defined in `{}` (`id: {id}`); the next free is `{next}`; nothing \
                 stored",
                ruling_path(id)
            )
        );
    };
    taken("STY-0001", "STY-100001");
    committed(&pair, &ruling_path("STY-100001"), &ruling("STY-100001"));
    taken("STY-0001", "STY-100002");
    taken("STY-100001", "STY-100002");
    let holder = held(&pair, "docs/records/STY/held.md", &["STY-0002"]);
    assert_eq!(holder, "PR-100001");
    let reason = refused_create(
        &pair,
        &pair.linked,
        &ruling_path("STY-0002"),
        None,
        &ruling("STY-0002"),
        "reserved",
    );
    assert_eq!(
        reason,
        "`STY-0002` is reserved by `PR-100001` (open), a live create; the next free is \
         `STY-100002`; nothing stored"
    );
}

// ------------------------------------------------------------ AC-4 (d)

/// docs/features/spec-ids.md "Acceptance criteria" AC-4 (d): the section form
/// (`--base`) and `spec propose amendment` refuse alike on either side, the
/// line the site's (the file's after the insert, the new file's). M6.
#[test]
fn ac4d_the_section_form_and_an_amendment_refuse_alike() {
    for hub in [false, true] {
        let pair = Pair::new(&format!("si-4d-{hub}"), "card-press");
        if hub {
            on_hub(&pair);
        }
        let (other, refusal): (&str, Refusal) = if hub {
            ("STY-0002", off_hub)
        } else {
            ("STY-100002", off_terminal)
        };
        let next = if hub { "STY-100001" } else { "STY-0002" };
        let (hash, span) = pair.span(&pair.linked, "SHT-FRONT-HEAD");
        let text = format!("{span}\n\n### A ruling {{#{other}}}\n\nThe masthead keeps one face.");
        let after = edit(&read_text(&pair.linked, FRONT), &span, &text);
        let site = format!("{{#{other}}}");
        let reason = refused_create(
            &pair,
            &pair.linked,
            "SHT-FRONT-HEAD",
            Some(&hash),
            &text,
            "the section form",
        );
        assert_eq!(
            reason,
            refusal(other, line_of(&after, &site), next),
            "{hub}"
        );

        let amendment = amending(other);
        let reason = refused_by(&pair, "an amendment", || {
            amend(&pair, &pair.linked, SPECIAL, &amendment)
        });
        assert_eq!(
            reason,
            refusal(other, line_of(&amendment, &site), next),
            "{hub}"
        );
        assert!(pair.proposals().is_empty(), "{hub}");
    }
}

// ------------------------------------------------------------ AC-4 (e)

/// docs/features/spec-ids.md "Acceptance criteria" AC-4 (e): spec-a's
/// `stamina-tuning.md` gaining `{#AC-100001}` (feature-scoped, width 2) is
/// refused on a terminal naming `stamina-tuning/AC-08`, and stored with a
/// floor, `target_ids` holding `stamina-tuning/AC-100001`.
#[test]
fn ac4e_a_feature_scoped_id_is_judged_in_its_scope() {
    let pair = Pair::new("si-4e", "spec-a");
    let target = "stamina-tuning/AC-07";
    let (hash, span) = pair.span(&pair.linked, target);
    let text = format!(
        "{span}\n\n#### A second tick {{#AC-100001}}\n\nThe second tick follows the first."
    );
    let after = edit(
        &read_text(&pair.linked, "docs/features/stamina-tuning.md"),
        &span,
        &text,
    );
    let reason = refused_create(
        &pair,
        &pair.linked,
        target,
        Some(&hash),
        &text,
        "a terminal",
    );
    assert_eq!(
        reason,
        off_terminal(
            "stamina-tuning/AC-100001",
            line_of(&after, "{#AC-100001}"),
            "stamina-tuning/AC-08"
        )
    );

    on_hub(&pair);
    let id = created(&pair, &pair.linked, target, Some(&hash), &text);
    assert_eq!(id, "PR-100001");
    let stored = pair.proposal(&id);
    assert_eq!(stored.new_ids, ["stamina-tuning/AC-100001"]);
    assert!(
        stored
            .target_ids()
            .contains(&"stamina-tuning/AC-100001".to_owned()),
        "{:?}",
        stored.target_ids()
    );
}

// ------------------------------------------------------------ AC-4 (f)

/// docs/features/spec-ids.md "Acceptance criteria" AC-4 (f): `--dry-run` of
/// (a) and (b) through the binary: the same exit (1) and reason on stderr as
/// the real call, nothing on stdout, nothing stored by either. M8: the dry
/// run skips the step.
#[test]
fn ac4f_the_dry_run_refuses_alike() {
    for hub in [false, true] {
        let pair = Pair::new(&format!("si-4f-{hub}"), "card-press");
        if hub {
            on_hub(&pair);
        }
        let (other, want) = if hub {
            ("STY-0002", off_hub("STY-0002", 2, "STY-100001"))
        } else {
            ("STY-100002", off_terminal("STY-100002", 2, "STY-0002"))
        };
        let path = ruling_path(other);
        let text = ruling(other);
        let run = |dry: bool| {
            let mut args = vec![
                "propose",
                "create",
                path.as_str(),
                "--text-file",
                "-",
                "--rationale",
                RATIONALE,
                "--author-role",
                "writer",
            ];
            if dry {
                args.push("--dry-run");
            }
            pair.spec_piped(&pair.linked, &args, text.as_bytes())
        };
        let before = (pair.state(), reservations(&pair), pair.events().len());
        let real = run(false);
        real.code(1);
        assert_eq!(real.stdout, "", "{}", real.show());
        let reason = format!("spec: {want}");
        assert_eq!(
            real.stderr.lines().last(),
            Some(reason.as_str()),
            "{}",
            real.show()
        );
        let dry = run(true);
        dry.code(1);
        assert_eq!(dry.stdout, "", "{}", dry.show());
        assert!(
            dry.stderr.lines().any(|line| line == reason),
            "the real reason {reason:?}: {}",
            dry.show()
        );
        assert_eq!(
            (pair.state(), reservations(&pair), pair.events().len()),
            before,
            "{hub}: nothing stored by either"
        );
    }
}

// ------------------------------------------------------------ AC-5

/// docs/features/spec-ids.md "Acceptance criteria" AC-5 (b): a hub's open
/// create of `STY-100001`, restored with no floor, applies by `spec approve`
/// as stored (I5, ADR-0012). M9: the side judged inside `NewIds::checked`
/// (apply step 6 refuses, exit 1).
#[test]
fn ac5b_a_hubs_create_restored_on_a_terminal_applies() {
    let pair = Pair::new("si-5b", "card-press");
    on_hub(&pair);
    let id = created(
        &pair,
        &pair.linked,
        &ruling_path("STY-100001"),
        None,
        &ruling("STY-100001"),
    );
    assert_eq!(id, "PR-100001");
    let dump = pair.scratch.dir("dumps").join("hub.jsonl");
    exported(&pair, &dump);
    imported(&pair, &dump, false);
    applied(&pair, &id);
    assert_eq!(
        read_text(&pair.linked, &ruling_path("STY-100001")),
        ruling("STY-100001")
    );
    assert_eq!(
        pair.git_text(&pair.linked, &["status", "--porcelain"]),
        "",
        "committed"
    );
}

/// docs/features/spec-ids.md "Acceptance criteria" AC-5 (a), the terminal's
/// twin of the hub's confirm (`specengine-http` `tests/spec_ids_hub.rs`): a
/// terminal's open create of `STY-0002` and open amendment defining
/// `STY-0003`, restored into a data directory holding the floor, apply by
/// `spec approve`. M9.
#[test]
fn ac5a_a_terminals_create_and_amendment_restored_on_a_hub_apply() {
    let pair = Pair::new("si-5a", "card-press");
    let create_id = created(
        &pair,
        &pair.linked,
        &ruling_path("STY-0002"),
        None,
        &ruling("STY-0002"),
    );
    let amendment = amending("STY-0003");
    let outcome = amend(&pair, &pair.linked, SPECIAL, &amendment)
        .unwrap_or_else(|error| panic!("propose amendment: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let amend_id = outcome.document.id.clone().expect("an ID");
    assert_eq!(
        (create_id.as_str(), amend_id.as_str()),
        ("PR-0001", "PR-0002")
    );
    let dump = pair.scratch.dir("dumps").join("terminal.jsonl");
    exported(&pair, &dump);
    imported(&pair, &dump, true);
    applied(&pair, &create_id);
    applied(&pair, &amend_id);
    assert_eq!(
        read_text(&pair.linked, &ruling_path("STY-0002")),
        ruling("STY-0002")
    );
    assert_eq!(read_text(&pair.linked, SPECIAL), amendment);
}
