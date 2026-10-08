//! docs/features/round.md (ADR-0038; canon `docs/canon/round.md`), the
//! CLI's half: `spec round new` through the library `round_new` (and the
//! binary where its bytes or its exit are the subject), `spec round
//! answer` through the library `round_answer` with a scripted
//! [`RoundTerminal`] (whose `ask` may restage through the daemon's `stage`
//! while the question is shown), and through the binary on a
//! pseudo-terminal (`script`) where the terminal is the subject: the keys
//! are sent only once a `[y/N]` is on the screen, since macOS `script`
//! sends ^D at the end of its input before `spec` reads.
//!
//! - AC-01: open Q1 and D1 listed; a held, an orphan's, another
//!   repository's (same slug), an open update and an open create noted; a
//!   rejected and an applied one nowhere. M: list `approved` items.
//! - AC-02: H high; N1 normal, newer, waited on by a `ready` task; N2
//!   normal, older, its target only in a `done` task: H, N1, N2; the round
//!   reads the package's `open_proposals` rule the other way
//!   (`lists_proposal`, a bound item too). M: count `done` tasks.
//! - AC-03: two runs 1.2 s apart, text and JSON, library and binary:
//!   byte-equal; nothing under the root, `HEAD`, the events unchanged.
//!   `round_new` takes no clock: the runs are apart in real time, and the
//!   sheet holds no date of today. M: print the time.
//! - AC-04: spec-b's Cyrillic texts and titles verbatim, the whole sheet
//!   as the canon draws it; no `b3:`, worktree, branch, `gap_type`,
//!   author or evidence in the text (the scan half: `proposal_genre.rs`).
//!   M: print the worktree.
//! - AC-05: 25 items: 20, the note, `more` 5; the reading time; `--limit`
//!   0, 201, -3 exit 2. M: drop the note.
//! - AC-06: a planted `PR-0001: 1` in agent text: the unedited sheet
//!   stages nothing, asks nothing, exit 0 (library and terminal). M: agent
//!   text not indented.
//! - AC-07: one file of every answer form: four staged, two open, the
//!   pair and the two refused characters named; the run's JSON. M: the
//!   last duplicate wins; truncation.
//! - AC-08: a discrepancy's `ok`, text and `5` refused naming `0-2`;
//!   `[2]`, `2` stage option 2; a replaced stage named. M: `ok` takes the
//!   recommendation.
//! - AC-09: an item applied after the sheet, an update, `PR-9999`, a
//!   look-alike: named, not staged; the two valid ones staged and asked.
//!   M: one stale item refuses the file.
//! - AC-10: a pipe, `/dev/null`, a FIFO with no writer, `--staged`: exit 2
//!   at once, nothing read; on a terminal the file's defects exit 2, no
//!   stage, no event. M: the terminal checked after staging.
//! - AC-11: the real walk on a terminal, keys `y`, `n`, `y`: three
//!   questions, two records in two `spec: apply` commits, one kept. M: one
//!   question for the round.
//! - AC-12: restaged while its question is shown: step 7 refuses it, the
//!   next is asked and applied; replaced before its question: refused
//!   unasked; the end of input keeps the rest. M: the walk stops.
//! - AC-13: `--staged` asks the staged questions in round order, notes the
//!   staged update. M: the update is asked.
//! - `stage_with` (the daemon's `stage` split for the round): its checks
//!   run before the queue is opened, in the same order; `round_stage`
//!   stages with no walk.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use common::decision::{PtyRun, THREE, discrepancy, flags, hit_names};
use common::proposal::{LATER, NOW, Pair, edit, printed};
use common::{RUN_TIMEOUT, Run, SPEC, read_text};
use serde_json::{Value, json};
use specengine_cli::{
    AnswerOutcome, CliError, CreateRequest, DiscrepancyInput, Exit, Globals, IntakeOption,
    IntakeSeverity, Outcome, ProposedText, QuestionRequest, RoundAnswerRequest, RoundNewRequest,
    RoundRun, RoundSheet, RoundStageRequest, RoundTerminal, StageBody, StageRequest, WalkOutcome,
    propose_create, propose_question_with_task, render_json, render_text, round_answer, round_new,
    round_stage, stage,
};
use specengine_store::{ProposalStatus, Stage};
use task_common::{Ordered, Tasks, done};

/// spec-a's slug.
const SLUG: &str = "lantern-keep";

/// The round's injected clock.
const ROUND: &str = "2026-10-06T10:00:00Z";

/// A later clock (a second round, a restage).
const AFTER: &str = "2026-10-06T11:00:00Z";

/// The consent message of a stdin that is no terminal (canon "Run" 1).
const CONSENT: &str = "spec: `spec round answer` asks the owner for consent on a terminal, and \
                       stdin is not one (a pipe, a script or an agent's shell): run it in a \
                       terminal; nothing changed\n";

/// The 19 keys of a sheet's item, in order (canon "JSON").
const ITEM_KEYS: [&str; 19] = [
    "id",
    "kind",
    "severity",
    "gap_type",
    "summary",
    "working_answer",
    "price_of_other",
    "options",
    "recommendation",
    "evidence",
    "targets",
    "tasks",
    "worktree",
    "branch",
    "author",
    "staged",
    "staged_at",
    "created_at",
    "updated_at",
];

// ---------------------------------------------------------------- setup

/// A question of a `developer` from `cwd` (`task`: bound to it), stored:
/// when its first ask hits, asked again naming every hit.
fn store_question(
    pair: &Pair,
    cwd: &Path,
    mut request: QuestionRequest,
    task: Option<&str>,
) -> String {
    let ask = |request: &QuestionRequest| {
        propose_question_with_task(&pair.env(cwd), &Globals::default(), request, task)
            .unwrap_or_else(|error| panic!("ask: {error}"))
    };
    let first = ask(&request);
    let outcome = if first.document.created {
        first
    } else {
        request.distinct_from = hit_names(&first);
        ask(&request)
    };
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

fn question_request(
    pair: &Pair,
    cwd: &Path,
    targets: &[&str],
    text: &str,
    severity: Option<IntakeSeverity>,
    now: &str,
) -> QuestionRequest {
    QuestionRequest {
        node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
        text: text.to_owned(),
        working_answer: "Keep it as written.".to_owned(),
        price_of_other: "A new rule case.".to_owned(),
        severity,
        distinct_from: Vec::new(),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        now: now.to_owned(),
        git: pair.git_env(cwd),
    }
}

/// An open question on `targets`, severity `normal`, made at [`NOW`].
fn question(pair: &Pair, cwd: &Path, targets: &[&str], text: &str) -> String {
    let request = question_request(pair, cwd, targets, text, None, NOW);
    store_question(pair, cwd, request, None)
}

/// An open discrepancy on `targets` with the three options of
/// `common::decision::THREE` (recommendation 0).
fn gap(pair: &Pair, cwd: &Path, targets: &[&str], summary: &str) -> String {
    pair.raise_discrepancy(cwd, discrepancy(targets, summary, &THREE))
}

/// An open `create` of a new file `docs/records/R/R-13.md` (spec-a).
fn create_file(pair: &Pair, cwd: &Path) -> String {
    let r12 = read_text(&common::fixture("spec-a"), "docs/records/R/R-12.md");
    let text = edit(&r12, "id: R-12\n", "id: R-13\n");
    let outcome = propose_create(
        &pair.env(cwd),
        &Globals::default(),
        &CreateRequest {
            target: "docs/records/R/R-13.md".to_owned(),
            base: None,
            text: ProposedText::Given(text.into_bytes()),
            rationale: "A create.".to_owned(),
            author_role: Some("writer".to_owned()),
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: pair.git_env(cwd),
        },
    )
    .expect("propose create");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome.document.id.expect("an ID")
}

/// An open update of spec-a's `EDGE-SPRINT-EMPTY`.
fn update(pair: &Pair, cwd: &Path) -> String {
    pair.propose_edit(
        cwd,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    )
}

/// A git repository of a copy of spec-a at `<scratch>/<dir>` (same slug),
/// one commit.
fn other_repository(pair: &Pair, dir: &str) -> PathBuf {
    let root = pair.scratch.copy("spec-a", dir);
    pair.git.init(&root);
    pair.git.add_all(&root);
    pair.git.commit(&root, "the fixture");
    root
}

/// The daemon's `stage` of `id` from `cwd` at `now`, against its stored
/// `updated_at`; it must be stored.
fn stage_now(pair: &Pair, cwd: &Path, id: &str, body: impl FnOnce(String) -> StageBody, now: &str) {
    let updated_at = pair.proposal(id).updated_at;
    let outcome = stage(
        &pair.env(cwd),
        &Globals::default(),
        &StageRequest {
            id: id.to_owned(),
            body: body(updated_at),
            now: now.to_owned(),
            git: pair.git_env(cwd),
        },
    )
    .unwrap_or_else(|error| panic!("stage {id}: {error}"));
    assert_eq!(outcome.cause, None, "stage {id}: {outcome:?}");
}

fn approve_body(answer: Option<&str>) -> impl FnOnce(String) -> StageBody {
    let answer = answer.map(str::to_owned);
    move |updated_at| StageBody::Approve {
        option: None,
        answer,
        canon: None,
        note: None,
        updated_at,
    }
}

// ---------------------------------------------------------------- sheet

fn new_sheet(pair: &Pair, cwd: &Path, limit: Option<i64>) -> Result<RoundSheet, CliError> {
    round_new(
        &pair.env(cwd),
        &Globals::default(),
        &RoundNewRequest {
            limit,
            git: pair.git_env(cwd),
        },
    )
}

fn sheet(pair: &Pair, cwd: &Path, limit: Option<i64>) -> RoundSheet {
    new_sheet(pair, cwd, limit).unwrap_or_else(|error| panic!("round new: {error}"))
}

/// The sheet's stdout text and JSON, as `spec round new` prints them.
fn printed_sheet(sheet: &RoundSheet) -> (String, String) {
    let outcome = Outcome::RoundSheet(Box::new(sheet.clone()));
    assert_eq!(outcome.exit(), Exit::Answered);
    (render_text(&outcome), render_json(&outcome))
}

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

fn ids(items: &Value) -> Vec<String> {
    items
        .as_array()
        .expect("an array")
        .iter()
        .map(|item| item["id"].as_str().expect("an id").to_owned())
        .collect()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

/// The `## <id> - ` heading's lines through `<id>:`, LF included.
fn item_block<'a>(text: &'a str, id: &str) -> &'a str {
    let start = text
        .find(&format!("## {id} - "))
        .unwrap_or_else(|| panic!("no heading of {id} in:\n{text}"));
    let slot = format!("\n{id}:\n");
    let end = text[start..]
        .find(&slot)
        .unwrap_or_else(|| panic!("no slot of {id} in:\n{text}"))
        + start
        + slot.len();
    &text[start..end]
}

// --------------------------------------------------------------- answer

/// The owner's terminal of a library run: the keys answer the questions
/// in order (`None` or past the end: the end of input); `before` runs as
/// each question is shown, before its key.
#[derive(Default)]
struct Scripted<'a> {
    keys: Vec<Option<bool>>,
    reports: Vec<String>,
    questions: Vec<String>,
    told: Vec<String>,
    before: Option<Box<dyn FnMut(usize) + 'a>>,
}

impl<'a> Scripted<'a> {
    fn keys(keys: &[Option<bool>]) -> Self {
        Self {
            keys: keys.to_vec(),
            ..Self::default()
        }
    }

    fn report(&self) -> &str {
        assert_eq!(self.reports.len(), 1, "{:?}", self.reports);
        &self.reports[0]
    }
}

impl RoundTerminal for Scripted<'_> {
    fn report(&mut self, text: &str) {
        self.reports.push(text.to_owned());
    }

    fn ask(&mut self, question: &str) -> Option<bool> {
        let index = self.questions.len();
        self.questions.push(question.to_owned());
        if let Some(before) = self.before.as_mut() {
            before(index);
        }
        self.keys.get(index).copied().flatten()
    }

    fn tell(&mut self, line: &str) {
        self.told.push(line.to_owned());
    }
}

/// `text` written to `<scratch>/answers/<name>` (outside every worktree):
/// its absolute path.
fn answers_file(pair: &Pair, name: &str, text: impl AsRef<[u8]>) -> PathBuf {
    let path = pair.scratch.dir("answers").join(name);
    fs::write(&path, text).expect("write the answers");
    path
}

fn answer(
    pair: &Pair,
    cwd: &Path,
    file: Option<&Path>,
    now: &str,
    terminal: &mut Scripted,
) -> Result<RoundRun, CliError> {
    round_answer(
        &pair.env(cwd),
        &Globals::default(),
        &RoundAnswerRequest {
            file: file.map(Path::to_path_buf),
            now: now.to_owned(),
            git: pair.git_env(cwd),
        },
        terminal,
    )
}

fn answered(pair: &Pair, cwd: &Path, file: Option<&Path>, terminal: &mut Scripted) -> RoundRun {
    answer(pair, cwd, file, ROUND, terminal).unwrap_or_else(|error| panic!("round answer: {error}"))
}

/// The run's stdout text and JSON, as `spec round answer` prints them.
fn printed_run(run: &RoundRun) -> (String, String) {
    let outcome = Outcome::Round(Box::new(run.clone()));
    (render_text(&outcome), render_json(&outcome))
}

/// The IDs of every `proposal.staged` event, in order.
fn staged_events(pair: &Pair) -> Vec<String> {
    pair.events()
        .iter()
        .filter(|event| event.event_type == "proposal.staged")
        .map(|event| event.payload["id"].as_str().expect("an id").to_owned())
        .collect()
}

fn stored_stage(pair: &Pair, id: &str) -> Option<Stage> {
    pair.proposal(id).staged.map(|staged| staged.stage)
}

fn working_answer() -> Stage {
    Stage::Approve {
        option: None,
        answer: None,
        canon: None,
        note: None,
        span_hash: None,
    }
}

fn option_stage(option: u64) -> Stage {
    Stage::Approve {
        option: Some(option),
        answer: None,
        canon: None,
        note: None,
        span_hash: None,
    }
}

fn answer_stage(answer: &str) -> Stage {
    Stage::Approve {
        option: None,
        answer: Some(answer.to_owned()),
        canon: None,
        note: None,
        span_hash: None,
    }
}

/// `spec args` in `cwd` on a pseudo-terminal (`script`): each `[y/N]` on
/// the screen answered by the next of `keys`; past them the terminal's
/// input ends (`script` sends ^D). The terminal stays open until `spec`
/// exits; a watchdog kills a run past [`RUN_TIMEOUT`].
fn pty_keys(pair: &Pair, cwd: &Path, args: &[&str], keys: &[&str]) -> PtyRun {
    let script = ["/usr/bin/script", "/bin/script"]
        .into_iter()
        .find(|path| Path::new(path).exists())
        .expect("a `script` program gives `spec` a terminal");
    let mut command = Command::new(script);
    if cfg!(target_os = "macos") {
        command.arg("-q").arg("/dev/null").arg(SPEC).args(args);
    } else {
        let quoted: Vec<String> = std::iter::once(SPEC)
            .chain(args.iter().copied())
            .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
            .collect();
        command.arg("-qec").arg(quoted.join(" ")).arg("/dev/null");
    }
    command
        .env_clear()
        .envs(pair.git.vars())
        .env("HOME", &pair.home)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().expect("spawn script");
    let mut input = child.stdin.take();
    let mut stdout = child.stdout.take().expect("stdout");
    let (sender, chunks) = mpsc::channel::<Vec<u8>>();
    let reader = std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match stdout.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if sender.send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut output = Vec::new();
    let mut answered = 0;
    let started = Instant::now();
    let status = loop {
        while let Ok(chunk) = chunks.try_recv() {
            output.extend_from_slice(&chunk);
        }
        let prompts = String::from_utf8_lossy(&output).matches("[y/N]").count();
        while answered < prompts {
            match (keys.get(answered), input.as_mut()) {
                (Some(key), Some(stdin)) => {
                    let _ = stdin.write_all(format!("{key}\n").as_bytes());
                    let _ = stdin.flush();
                }
                _ => input = None,
            }
            answered += 1;
        }
        if let Some(status) = child.try_wait().expect("wait on script") {
            break status;
        }
        if started.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("spec {args:?} on a terminal ran longer than {RUN_TIMEOUT:?}; killed");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    drop(input);
    reader.join().expect("the reader");
    while let Ok(chunk) = chunks.try_recv() {
        output.extend_from_slice(&chunk);
    }
    let output = String::from_utf8(output)
        .expect("UTF-8 terminal output")
        .replace("\r\n", "\n");
    PtyRun {
        code: status
            .code()
            .expect("script exited, not killed by a signal"),
        asked: output.contains("[y/N]"),
        output,
    }
}

/// The lines of a terminal's output that hold a `[y/N]` prompt.
fn prompts(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains("[y/N]"))
        .collect()
}

/// `spec args` with stdin `/dev/null`, the sandbox's variables and this
/// `HOME`.
fn spec_null(pair: &Pair, cwd: &Path, args: &[&str]) -> Run {
    let vars = pair.git.vars();
    let mut env: Vec<(&str, &std::ffi::OsStr)> = vars
        .iter()
        .map(|(key, value)| (key.to_str().expect("a UTF-8 name"), value.as_os_str()))
        .collect();
    env.push(("HOME", pair.home.as_os_str()));
    common::spec_with(cwd, args, &env)
}

// ------------------------------------------------------------------ AC-01

#[test]
fn ac01_the_sheet_lists_open_questions_and_discrepancies_of_this_repository_only() {
    let pair = Pair::new("rd-ac01", "spec-a");
    let main = pair.main.clone();
    let q1 = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let d1 = gap(
        &pair,
        &main,
        &["RULE-SPRINT-COST"],
        "The sprint cost is charged twice",
    );
    let held = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    let rejected = question(
        &pair,
        &main,
        &["EDGE-STAM-ZERO"],
        "Does an empty bar stop the run?",
    );
    let applied = question(
        &pair,
        &main,
        &["RULE-CORE-LOOP"],
        "Does the loop end at dawn?",
    );
    let (outcome, _) = pair.reject_answer(&main, &rejected, "A duplicate.", true);
    assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    pair.decide_ok(&main, &applied, &flags(None, None, None), None);
    assert_eq!(pair.proposal(&applied).status, ProposalStatus::Applied);
    // An apply in progress or stopped after step 7: the row `approved`
    // with its record set (the applied one's decision and record columns,
    // another record ID).
    pair.sql(&format!(
        "UPDATE proposals SET (status, decided_by, decided_at, record_title, record_text, \
         choice) = (SELECT 'approved', decided_by, decided_at, record_title, record_text, choice \
         FROM proposals WHERE id = '{applied}'), record_id = 'DEC-0099', record_path = \
         'docs/records/DEC/DEC-0099.md' WHERE id = '{held}';"
    ));
    assert_eq!(pair.proposal(&held).status, ProposalStatus::Approved);
    let other = other_repository(&pair, "other");
    let elsewhere = question(
        &pair,
        &other,
        &["RULE-STAM-REGEN"],
        "Does the copy regenerate too?",
    );
    let gone = other_repository(&pair, "gone");
    let orphan = question(
        &pair,
        &gone,
        &["RULE-SPRINT-COST"],
        "Is the copy's cost doubled?",
    );
    fs::remove_dir_all(&gone).expect("remove the repository");
    let changed = update(&pair, &main);
    let created = create_file(&pair, &main);

    let (text, json_text) = printed_sheet(&sheet(&pair, &main, None));
    let json = parsed(&json_text);
    assert_eq!(ids(&json["items"]), [q1.clone(), d1.clone()], "{json_text}");
    let notes = vec![
        format!(
            "1 held item(s) not on this sheet (approved: an apply in progress or stopped): \
             {held}; spec approve <ID> completes each"
        ),
        format!(
            "2 open change proposal(s) not on this sheet: {changed}, {created}; spec inbox \
             lists them"
        ),
        format!(
            "1 item(s) of another repository of the project `{SLUG}` not on this sheet: \
             {elsewhere}; run `spec round new` there"
        ),
        format!(
            "1 item(s) of a repository that no longer exists not on this sheet: {orphan}; spec \
             reject <ID> --reason ... takes one out"
        ),
    ];
    assert_eq!(strings(&json["notes"]), notes, "{json_text}");
    let tail: String = notes.iter().map(|note| format!("note: {note}\n")).collect();
    assert!(text.ends_with(&format!("\n{tail}")), "{text}");
    for settled in [&rejected, &applied] {
        assert!(!text.contains(settled.as_str()), "{settled} in:\n{text}");
        assert!(
            !json_text.contains(settled.as_str()),
            "{settled} in {json_text}"
        );
    }
    assert_eq!(json["more"], json!(0));
    assert_eq!(json["project"], json!(SLUG));
    // Keys in order: the document's five, an item's 19.
    assert_eq!(
        task_common::keys(&json_text),
        ["project", "items", "more", "minutes", "notes"]
    );
    let ordered = Ordered::of(&json_text);
    for index in 0..2 {
        assert_eq!(ordered.get("items").at(index).keys(), ITEM_KEYS);
    }
    // The other worktree of the repository lists the same items.
    let linked = parsed(&printed_sheet(&sheet(&pair, &pair.linked, None)).1);
    assert_eq!(ids(&linked["items"]), [q1, d1]);
    // The other repository lists its own item and notes this one's.
    let theirs = parsed(&printed_sheet(&sheet(&pair, &other, None)).1);
    assert_eq!(ids(&theirs["items"]), [elsewhere]);
}

// ------------------------------------------------------------------ AC-02

#[test]
fn ac02_severity_then_waiting_tasks_then_age_order_the_sheet() {
    let pair = Pair::new("rd-ac02", "spec-a");
    let main = pair.main.clone();
    let ready = pair.new_ok(&main, &["RULE-SPRINT-COST"]);
    pair.plan_ok(&main, &ready, &[], &[]);
    pair.approve_task(&main, &ready);
    let closed = pair.new_ok(&main, &["RULE-CORE-LOOP"]);
    pair.plan_ok(&main, &closed, &[], &[]);
    pair.approve_task(&main, &closed);
    pair.claim_ok(&main, &closed, &main);
    pair.report_ok(&main, &closed);
    let completed = done(pair.complete(&main, &closed), "complete");
    assert_eq!(completed.status.expect("a status").as_str(), "done");

    let request = |targets: &[&str], text: &str, severity, now: &str| {
        question_request(&pair, &main, targets, text, Some(severity), now)
    };
    let n2 = store_question(
        &pair,
        &main,
        request(
            &["RULE-CORE-LOOP"],
            "Does the loop end at dawn?",
            IntakeSeverity::Normal,
            NOW,
        ),
        None,
    );
    let n1 = store_question(
        &pair,
        &main,
        request(
            &["RULE-SPRINT-COST"],
            "Is the sprint cost per second?",
            IntakeSeverity::Normal,
            LATER,
        ),
        None,
    );
    let h = store_question(
        &pair,
        &main,
        request(
            &["RULE-MOVE-SPEEDS"],
            "Is the walk speed fixed?",
            IntakeSeverity::High,
            LATER,
        ),
        None,
    );
    // Bound to the ready task, its target outside the snapshot: waited on.
    let bound = store_question(
        &pair,
        &main,
        request(
            &["EDGE-STAM-ZERO"],
            "Does an empty bar stop the run?",
            IntakeSeverity::Low,
            NOW,
        ),
        Some(&ready),
    );

    let (text, json_text) = printed_sheet(&sheet(&pair, &main, None));
    let json = parsed(&json_text);
    assert_eq!(
        ids(&json["items"]),
        [h.clone(), n1.clone(), n2.clone(), bound.clone()],
        "{text}"
    );
    let headings: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("## "))
        .collect();
    assert_eq!(
        headings,
        [
            format!("## {h} - question, high"),
            format!("## {n1} - question, normal"),
            format!("## {n2} - question, normal"),
            format!("## {bound} - question, low"),
        ]
    );
    let waiting = json!([{"id": ready, "status": "ready", "title": "Tune the regeneration"}]);
    assert_eq!(json["items"][0]["tasks"], json!([]));
    assert_eq!(json["items"][1]["tasks"], waiting);
    assert_eq!(json["items"][2]["tasks"], json!([]));
    assert_eq!(json["items"][3]["tasks"], waiting);
    let line = format!("Waiting: Tune the regeneration ({ready})\n");
    assert!(item_block(&text, &n1).contains(&line), "{text}");
    assert!(item_block(&text, &bound).contains(&line), "{text}");
    assert!(!item_block(&text, &n2).contains("Waiting:"), "{text}");
    assert!(!item_block(&text, &h).contains("Waiting:"), "{text}");

    // `lists_proposal` read both ways: the ready task's package lists what
    // the round counts it waiting on.
    let package = pair.package(&main, &ready);
    let listed: BTreeSet<String> = ids(&package["open_proposals"]).into_iter().collect();
    assert_eq!(listed, BTreeSet::from([n1, bound]), "{package}");
}

// ------------------------------------------------------------------ AC-03

#[test]
fn ac03_the_sheet_is_byte_deterministic_and_writes_nothing() {
    let pair = Pair::new("rd-ac03", "spec-a");
    let main = pair.main.clone();
    let ready = pair.new_ok(&main, &["RULE-STAM-REGEN"]);
    pair.plan_ok(&main, &ready, &[], &[]);
    pair.approve_task(&main, &ready);
    let request = question_request(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
        Some(IntakeSeverity::High),
        NOW,
    );
    let q = store_question(&pair, &main, request, None);
    let d = gap(
        &pair,
        &main,
        &["RULE-SPRINT-COST"],
        "The sprint cost is charged twice",
    );
    stage_now(
        &pair,
        &main,
        &q,
        approve_body(Some("no, \"at once\"")),
        LATER,
    );
    let before = pair.state();
    let events = pair.events().len();

    let first = printed_sheet(&sheet(&pair, &main, None));
    let text = &first.0;
    assert!(
        text.contains(&format!(
            "Staged {LATER}: spec approve {q} --answer \"no, \\\"at once\\\"\"\n"
        )),
        "{text}"
    );
    assert!(item_block(text, &d).contains("Options:\n"), "{text}");
    std::thread::sleep(Duration::from_millis(1200));
    let second = printed_sheet(&sheet(&pair, &main, None));
    assert_eq!(first, second);

    let binary = |args: &[&str]| {
        let run = pair.spec_piped(&main, args, b"");
        run.code(0);
        run.stdout
    };
    let text_run = binary(&["round", "new"]);
    let json_run = binary(&["--json", "round", "new"]);
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(binary(&["round", "new"]), text_run);
    assert_eq!(binary(&["--json", "round", "new"]), json_run);
    assert_eq!(text_run, first.0);
    assert_eq!(json_run, first.1);

    // No clock: today's date is nowhere (the fixture's dates are earlier).
    let today = &specengine_cli::utc_now()[..10];
    if !["2026-10-05", "2026-10-06", "2026-10-07"].contains(&today) {
        assert!(!first.0.contains(today), "{}", first.0);
        assert!(!first.1.contains(today), "{}", first.1);
    }
    assert_eq!(
        pair.state(),
        before,
        "the sheet wrote under a root or the queue"
    );
    assert_eq!(pair.events().len(), events, "the sheet logged an event");
}

// ------------------------------------------------------------------ AC-04

/// spec-b's texts (Russian), as Unicode escapes.
const Q_RU: &str = "\u{414}\u{43e}\u{43b}\u{436}\u{435}\u{43d} \u{43b}\u{438} sync \u{43f}\u{440}\u{43e}\u{43f}\u{443}\u{441}\u{43a}\u{430}\u{442}\u{44c} \u{433}\u{440}\u{44f}\u{437}\u{43d}\u{44b}\u{435} \u{434}\u{435}\u{440}\u{435}\u{432}\u{44c}\u{44f}?";
const WA_RU: &str = "\u{414}\u{430}, \u{441} \u{43f}\u{440}\u{435}\u{434}\u{443}\u{43f}\u{440}\u{435}\u{436}\u{434}\u{435}\u{43d}\u{438}\u{435}\u{43c}";
const PRICE_RU: &str = "\u{41d}\u{43e}\u{432}\u{44b}\u{439} \u{444}\u{43b}\u{430}\u{433}";
const D_RU: &str = "status \u{43f}\u{43e}\u{43a}\u{430}\u{437}\u{44b}\u{432}\u{430}\u{435}\u{442} \u{43b}\u{438}\u{448}\u{43d}\u{44e}\u{44e} \u{432}\u{435}\u{442}\u{43a}\u{443}";
const OPTIONS_RU: [(&str, &str, &str); 3] = [
    (
        "\u{41e}\u{441}\u{442}\u{430}\u{432}\u{438}\u{442}\u{44c} \u{441}\u{43f}\u{435}\u{446}\u{438}\u{444}\u{438}\u{43a}\u{430}\u{446}\u{438}\u{44e}",
        "\u{418}\u{441}\u{43f}\u{440}\u{430}\u{432}\u{438}\u{442}\u{44c} \u{43a}\u{43e}\u{434}",
        "\u{43e}\u{434}\u{43d}\u{430} \u{437}\u{430}\u{434}\u{430}\u{447}\u{430}",
    ),
    (
        "\u{418}\u{437}\u{43c}\u{435}\u{43d}\u{438}\u{442}\u{44c} \u{441}\u{43f}\u{435}\u{446}\u{438}\u{444}\u{438}\u{43a}\u{430}\u{446}\u{438}\u{44e}",
        "\u{41d}\u{43e}\u{432}\u{43e}\u{435} \u{43f}\u{440}\u{430}\u{432}\u{438}\u{43b}\u{43e}",
        "\u{442}\u{435}\u{441}\u{442}",
    ),
    (
        "\u{41e}\u{442}\u{43b}\u{43e}\u{436}\u{438}\u{442}\u{44c}",
        "\u{41d}\u{438}\u{447}\u{435}\u{433}\u{43e}",
        "\u{440}\u{430}\u{437}\u{440}\u{44b}\u{432} \u{43e}\u{441}\u{442}\u{430}\u{451}\u{442}\u{441}\u{44f}",
    ),
];
const TITLE_SYNC: &str = "\u{41a}\u{43e}\u{43c}\u{430}\u{43d}\u{434}\u{430} sync";
const TITLE_STATUS: &str = "\u{41a}\u{43e}\u{43c}\u{430}\u{43d}\u{434}\u{430} status";

#[test]
fn ac04_project_text_travels_verbatim_and_the_sheet_shows_no_place() {
    let pair = Pair::new("rd-ac04", "spec-b");
    let linked = pair.linked.clone();
    let mut request = question_request(&pair, &linked, &["CMD-SYNC"], Q_RU, None, NOW);
    request.working_answer = WA_RU.to_owned();
    request.price_of_other = PRICE_RU.to_owned();
    let q = store_question(&pair, &linked, request, None);
    let mut input: DiscrepancyInput = discrepancy(&["CMD-STATUS"], D_RU, &OPTIONS_RU);
    input.working_answer = Some(OPTIONS_RU[0].0.to_owned());
    let d = pair.raise_discrepancy(&linked, input);
    assert_eq!((q.as_str(), d.as_str()), ("PR-0001", "PR-0002"));

    let (text, json_text) = printed_sheet(&sheet(&pair, &pair.main, None));
    let paragraph = "2 items, about 1 min of reading. Answer each on a line of its own, at the start \
                     of the line: `PR-0001: ok`. ok takes the working answer, a number takes that \
                     option, `reject: <reason>` rejects, ? or nothing leaves it open, other text is \
                     your own answer to a question. Keep the project line. Then, in a terminal: \
                     spec round answer <file>; each answer is asked again there, one at a time.";
    let options: String = OPTIONS_RU
        .iter()
        .enumerate()
        .map(|(index, (label, effect, price))| {
            let recommended = if index == 0 { " (recommended)" } else { "" };
            format!("  - [{index}] {label} | {effect} | {price}{recommended}\n")
        })
        .collect();
    let expected = format!(
        "# Open questions: zerkalo\n\nproject: zerkalo\n\n{paragraph}\n\n\
         ## PR-0001 - question, normal\n\n  {Q_RU}\n\nWorking answer:\n  {WA_RU}\n\n\
         Another answer costs:\n  {PRICE_RU}\n\nConcerns: {TITLE_SYNC} (CMD-SYNC)\n\nPR-0001:\n\n\
         ## PR-0002 - discrepancy, normal\n\n  {D_RU}\n\nWorking answer:\n  {}\n\n\
         Options:\n{options}\nConcerns: {TITLE_STATUS} (CMD-STATUS)\n\nPR-0002:\n",
        OPTIONS_RU[0].0
    );
    assert_eq!(text, expected);
    for absent in [
        "b3:",
        pair.main.to_str().expect("UTF-8"),
        linked.to_str().expect("UTF-8"),
        "t1",
        "contradicts",
        "developer",
        "claude-opus",
        "src/stamina.rs",
        "regenerates",
        "10-20",
    ] {
        assert!(!text.contains(absent), "{absent:?} in:\n{text}");
    }
    // The JSON keeps every field raw.
    let json = parsed(&json_text);
    let (qi, di) = (&json["items"][0], &json["items"][1]);
    assert_eq!(qi["summary"], json!(Q_RU));
    assert_eq!(qi["working_answer"], json!(WA_RU));
    assert_eq!(qi["price_of_other"], json!(PRICE_RU));
    assert_eq!(
        qi["targets"],
        json!([{"id": "CMD-SYNC", "path": "docs/spec/cli.md", "title": TITLE_SYNC}])
    );
    assert_eq!(qi["worktree"], json!(linked.to_str().expect("UTF-8")));
    assert_eq!(qi["branch"], json!("t1"));
    assert_eq!(qi["author"]["role"], json!("developer"));
    assert_eq!(di["gap_type"], json!("contradicts"));
    assert_eq!(di["options"][2]["label"], json!(OPTIONS_RU[2].0));
    assert_eq!(di["recommendation"], json!(0));
    assert_eq!(di["evidence"][0]["file"], json!("src/stamina.rs"));
    assert_eq!(json["minutes"], json!(1));
}

// ------------------------------------------------------------------ AC-05

#[test]
fn ac05_the_limit_cuts_the_sheet_with_a_note_and_bounds_itself() {
    let pair = Pair::new("rd-ac05", "spec-a");
    let main = pair.main.clone();
    let targets = [
        "RULE-STAM-REGEN",
        "RULE-SPRINT-COST",
        "RULE-MOVE-SPEEDS",
        "EDGE-STAM-ZERO",
        "RULE-CORE-LOOP",
    ];
    let mut all = Vec::new();
    for index in 0..25 {
        all.push(question(
            &pair,
            &main,
            &[targets[index % targets.len()]],
            &format!("Round question {index:02}: is case {index} covered by the rule?"),
        ));
    }
    let note = "5 more item(s) not on this sheet: spec round new --limit 25";
    let (text, json_text) = printed_sheet(&sheet(&pair, &main, None));
    let json = parsed(&json_text);
    assert_eq!(ids(&json["items"]), all[..20]);
    assert_eq!(json["more"], json!(5));
    assert_eq!(strings(&json["notes"]), [note]);
    assert!(text.ends_with(&format!("\nnote: {note}\n")), "{text}");
    assert!(text.contains("\n20 items, about "), "{text}");
    // The reading time: the item blocks' characters, rounded up.
    let chars: usize = all[..20]
        .iter()
        .map(|id| item_block(&text, id).chars().count())
        .sum();
    assert_eq!(json["minutes"], json!(chars.div_ceil(1000)), "{chars}");
    assert!(text.contains(&format!(", about {} min of reading.", chars.div_ceil(1000))));

    let whole = parsed(&printed_sheet(&sheet(&pair, &main, Some(25))).1);
    assert_eq!(ids(&whole["items"]), all);
    assert_eq!(whole["more"], json!(0));
    assert_eq!(whole["notes"], json!([]));
    let one = parsed(&printed_sheet(&sheet(&pair, &main, Some(1))).1);
    assert_eq!(ids(&one["items"]), all[..1]);
    assert_eq!(
        strings(&one["notes"]),
        ["24 more item(s) not on this sheet: spec round new --limit 25"]
    );
    assert_eq!(
        ids(&parsed(&printed_sheet(&sheet(&pair, &main, Some(200))).1)["items"]),
        all
    );

    for limit in [0, 201, -3] {
        let message = common::proposal::cannot(&new_sheet(&pair, &main, Some(limit)), "limit");
        assert_eq!(
            message,
            format!("spec: --limit {limit}: the limit is 1 to 200")
        );
        let limit = limit.to_string();
        let run = pair.spec_piped(&main, &["round", "new", "--limit", &limit], b"");
        run.code(2);
        assert_eq!(
            run.stderr,
            format!("spec: --limit {limit}: the limit is 1 to 200\n")
        );
        assert_eq!(run.stdout, "");
    }
    pair.spec_piped(&main, &["round", "new", "--limit", "x"], b"")
        .code(2);
}

// ------------------------------------------------------------------ AC-06

#[test]
fn ac06_a_planted_answer_line_in_agent_text_answers_nothing() {
    let pair = Pair::new("rd-ac06", "spec-a");
    let main = pair.main.clone();
    let d = gap(
        &pair,
        &main,
        &["RULE-SPRINT-COST"],
        "The sprint cost is charged twice",
    );
    let text = format!("Does regeneration wait for rest?\n{d}: 1\n> {d}: ok");
    let q = question(&pair, &main, &["RULE-STAM-REGEN"], &text);
    let (sheet_text, _) = printed_sheet(&sheet(&pair, &main, None));
    let events = pair.events().len();
    let file = answers_file(&pair, "sheet.md", &sheet_text);

    // Fed back unedited: nothing staged, nothing asked.
    let mut terminal = Scripted::keys(&[Some(true), Some(true)]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    assert_eq!(staged_events(&pair), Vec::<String>::new(), "{sheet_text}");
    assert!(terminal.questions.is_empty(), "{:?}", terminal.questions);
    // The planted lines are the question's text, indented.
    assert!(
        sheet_text.contains(&format!("\n  {d}: 1\n  > {d}: ok\n")),
        "{sheet_text}"
    );
    assert_eq!(run.exit(), Exit::Answered, "{run:?}");
    let slot = |id: &str| {
        sheet_text
            .lines()
            .position(|line| line == format!("{id}:"))
            .expect("a slot")
            + 1
    };
    assert_eq!(
        terminal.report(),
        format!(
            "{d} line {}: left open\n{q} line {}: left open\nanswers: 0 staged, 2 left open, 0 \
             refused\n",
            slot(&d),
            slot(&q)
        )
    );
    assert!(run.walk.is_empty());
    assert_eq!(
        printed_run(&run).0,
        "round: 0 applied, 0 rejected, 0 kept staged, 0 refused\n"
    );
    assert_eq!(pair.events().len(), events, "{:?}", staged_events(&pair));
    assert_eq!(
        (stored_stage(&pair, &d), stored_stage(&pair, &q)),
        (None, None)
    );

    // The binary on a terminal: the same, no question.
    let file = file.to_str().expect("UTF-8");
    let run = pair.spec_pty(&main, &["round", "answer", file], None);
    assert_eq!(run.code, 0, "{}", run.output);
    assert!(!run.asked, "{}", run.output);
    assert!(
        run.output
            .contains("answers: 0 staged, 2 left open, 0 refused\n"),
        "{}",
        run.output
    );
    assert_eq!(pair.events().len(), events);
}

// ------------------------------------------------------------------ AC-07

#[test]
fn ac07_every_answer_form_in_one_file() {
    let pair = Pair::new("rd-ac07", "spec-a");
    let main = pair.main.clone();
    let a = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let b = gap(
        &pair,
        &main,
        &["RULE-SPRINT-COST"],
        "The sprint cost is charged twice",
    );
    let c = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "How long is the regeneration delay?",
    );
    let d = question(
        &pair,
        &main,
        &["EDGE-STAM-ZERO"],
        "Does an empty bar stop the run?",
    );
    let e = question(
        &pair,
        &main,
        &["RULE-CORE-LOOP"],
        "Does the loop end at dawn?",
    );
    let f = question(
        &pair,
        &main,
        &["EDGE-SPRINT-EMPTY"],
        "Does the empty tank slow the walk?",
    );
    let g = question(
        &pair,
        &main,
        &["MEC-STAMINA"],
        "Is stamina shared by the party?",
    );
    let h = question(&pair, &main, &["MEC-SPRINT"], "Can a sprint be cancelled?");
    let i = question(&pair, &main, &["DOM-MOVEMENT"], "Is movement grid based?");
    let j = question(&pair, &main, &["DOM-GAME"], "Is there a night cycle?");
    let long = "a".repeat(2049);
    let own = "Regeneration waits 2 s, not 1.5";
    let file = answers_file(
        &pair,
        "reply.md",
        format!(
            "project: {SLUG}\n{a}: ok\n{b}: 1\n{c}: {own}\n{d}: reject: dup\n{e}: ?\n{f}:\n  {g}: \
             ok\n> {g}: ok\n{h}: ok\n{h}: no\n{i}: {long}\n{j}: a\u{202e}b\n"
        ),
    );
    let events = pair.events().len();
    let mut terminal = Scripted::keys(&[Some(false); 4]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);

    let pair_refusal =
        "refused: answered on lines 10 and 11: give one answer per item; nothing staged";
    assert_eq!(
        terminal.report(),
        format!(
            "{a} line 2: staged: approve, the working answer\n\
             {b} line 3: staged: approve, option 1: Change the spec\n\
             {c} line 4: staged: approve, your answer\n\
             {d} line 5: staged: reject\n\
             {e} line 6: left open\n\
             {f} line 7: left open\n\
             {h} line 10: {pair_refusal}\n\
             {h} line 11: {pair_refusal}\n\
             {i} line 12: refused: the answer: 2049 bytes; at most 2048; nothing staged\n\
             {j} line 13: refused: holds U+202E: an answer never carries it; nothing staged\n\
             answers: 4 staged, 2 left open, 4 refused\n"
        )
    );
    // Exactly four staged, one event each, nothing else written.
    assert_eq!(
        staged_events(&pair),
        [a.clone(), b.clone(), c.clone(), d.clone()]
    );
    assert_eq!(pair.events().len(), events + 4);
    assert_eq!(stored_stage(&pair, &a), Some(working_answer()));
    assert_eq!(stored_stage(&pair, &b), Some(option_stage(1)));
    assert_eq!(stored_stage(&pair, &c), Some(answer_stage(own)));
    assert_eq!(
        stored_stage(&pair, &d),
        Some(Stage::Reject {
            reason: "dup".to_owned()
        })
    );
    for id in [&e, &f, &g, &h, &i, &j] {
        assert_eq!(stored_stage(&pair, id), None, "{id}");
        assert_eq!(pair.proposal(id).status, ProposalStatus::Open, "{id}");
    }
    for id in [&a, &b, &c, &d] {
        let staged = pair.proposal(id).staged.expect("staged");
        assert_eq!(staged.at, ROUND, "{id}");
    }
    // `spec review` after declining all: each stage shown.
    let review = |id: &str| parsed(&printed(&pair.review_ok(&main, id)).1);
    assert_eq!(review(&a)["staged"]["decision"], json!("approve"));
    assert_eq!(review(&a)["staged"]["answer"], json!(null));
    assert_eq!(review(&b)["staged"]["option"], json!(1));
    assert_eq!(review(&c)["staged"]["answer"], json!(own));
    assert_eq!(
        review(&d)["staged"],
        json!({"decision": "reject", "reason": "dup"})
    );
    assert_eq!(review(&g)["staged"], json!(null));

    // The walk: the four in round order, each its own question, all kept.
    assert_eq!(terminal.questions.len(), 4, "{:?}", terminal.questions);
    for (question, id) in terminal.questions.iter().zip([&a, &b, &c]) {
        assert!(question.contains(&format!("apply {id} as ")), "{question}");
        assert!(question.contains(", staged) on main in "), "{question}");
        assert!(question.ends_with("? [y/N]"), "{question}");
    }
    let last = &terminal.questions[3];
    assert!(last.contains(&format!("reject {d} (")), "{last}");
    assert!(last.ends_with(", staged)? [y/N]"), "{last}");
    assert!(
        terminal.questions[0].contains(&format!("staged {ROUND}: spec approve {a}\n")),
        "{}",
        terminal.questions[0]
    );
    let kept: Vec<(String, WalkOutcome, Option<String>)> = run
        .walk
        .iter()
        .map(|step| (step.id.clone(), step.outcome, step.reason.clone()))
        .collect();
    let keep = |id: &str, command: &str| {
        (
            id.to_owned(),
            WalkOutcome::Kept,
            Some(format!(
                "kept staged: {command} {id} or spec round answer --staged confirms it"
            )),
        )
    };
    assert_eq!(
        kept,
        [
            keep(&a, "spec approve"),
            keep(&b, "spec approve"),
            keep(&c, "spec approve"),
            keep(&d, "spec reject"),
        ]
    );
    assert_eq!(run.exit(), Exit::NotFound);
    let (text, json_text) = printed_run(&run);
    assert_eq!(
        text,
        "round: 0 applied, 0 rejected, 4 kept staged, 0 refused\n"
    );

    // The run's JSON: keys in order, raw answers, the refused ones' reasons.
    let ordered = Ordered::of(&json_text);
    assert_eq!(ordered.keys(), ["project", "answers", "walk", "notes"]);
    assert_eq!(
        ordered.get("answers").at(0).keys(),
        ["id", "line", "answer", "outcome", "stage", "reason"]
    );
    assert_eq!(
        ordered.get("walk").at(0).keys(),
        ["id", "outcome", "commit", "record", "reason"]
    );
    let json = parsed(&json_text);
    let answers = json["answers"].as_array().expect("answers");
    assert_eq!(answers.len(), 10);
    assert_eq!(answers[0]["outcome"], json!("staged"));
    assert_eq!(answers[1]["stage"]["option"], json!(1));
    assert_eq!(answers[4]["outcome"], json!("open"));
    assert_eq!(answers[8]["answer"], json!(long), "never cut");
    assert_eq!(answers[8]["stage"], json!(null));
    assert_eq!(
        answers[8]["reason"],
        json!("the answer: 2049 bytes; at most 2048")
    );
    assert_eq!(
        run.answers
            .iter()
            .map(|answer| answer.outcome)
            .collect::<Vec<_>>(),
        [
            AnswerOutcome::Staged,
            AnswerOutcome::Staged,
            AnswerOutcome::Staged,
            AnswerOutcome::Staged,
            AnswerOutcome::Open,
            AnswerOutcome::Open,
            AnswerOutcome::Refused,
            AnswerOutcome::Refused,
            AnswerOutcome::Refused,
            AnswerOutcome::Refused,
        ]
    );
    assert_eq!(json["walk"][3]["outcome"], json!("kept"));
}

// ------------------------------------------------------------------ AC-08

#[test]
fn ac08_a_discrepancy_takes_an_option_by_number_only() {
    let pair = Pair::new("rd-ac08", "spec-a");
    let main = pair.main.clone();
    let targets = [
        "RULE-STAM-REGEN",
        "RULE-SPRINT-COST",
        "RULE-MOVE-SPEEDS",
        "EDGE-STAM-ZERO",
        "RULE-CORE-LOOP",
    ];
    let gaps: Vec<String> = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            gap(
                &pair,
                &main,
                &[target],
                &format!("Gap {index} contradicts the rule"),
            )
        })
        .collect();
    let q = question(&pair, &main, &["MEC-SPRINT"], "Can a sprint be cancelled?");
    let file = answers_file(
        &pair,
        "a.md",
        format!(
            "project: {SLUG}\n{}: ok\n{}: Keep both waits\n{}: 5\n{}: [2]\n{}: 2\n{q}: 1\n",
            gaps[0], gaps[1], gaps[2], gaps[3], gaps[4]
        ),
    );
    let mut terminal = Scripted::keys(&[Some(false), Some(false)]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    let by_number = |id: &str| {
        format!(
            "refused: `{id}` takes one of its options by number, 0-2, or `reject: <reason>`; nothing staged"
        )
    };
    assert_eq!(
        terminal.report(),
        format!(
            "{} line 2: {}\n{} line 3: {}\n{} line 4: refused: option 5: `{}` has options 0-2; \
             nothing staged\n{} line 5: staged: approve, option 2: Defer\n{} line 6: staged: \
             approve, option 2: Defer\n{q} line 7: refused: `{q}` has no options: `ok` takes its \
             working answer, other text is your answer; nothing staged\nanswers: 2 staged, 0 left \
             open, 4 refused\n",
            gaps[0],
            by_number(&gaps[0]),
            gaps[1],
            by_number(&gaps[1]),
            gaps[2],
            gaps[2],
            gaps[3],
            gaps[4]
        )
    );
    for id in &gaps[..3] {
        assert_eq!(stored_stage(&pair, id), None, "{id}");
    }
    assert_eq!(stored_stage(&pair, &q), None);
    assert_eq!(stored_stage(&pair, &gaps[3]), Some(option_stage(2)));
    assert_eq!(stored_stage(&pair, &gaps[4]), Some(option_stage(2)));
    assert_eq!(staged_events(&pair), [gaps[3].clone(), gaps[4].clone()]);
    assert_eq!(terminal.questions.len(), 2);
    assert!(
        terminal.questions[0].contains("  [2] Defer | Nothing changes now | the gap stays open\n")
    );

    // A second answer replaces the stage, named in the report and above
    // its question.
    let file = answers_file(&pair, "b.md", format!("project: {SLUG}\n{}: 1\n", gaps[3]));
    let mut terminal = Scripted::keys(&[Some(false)]);
    let run_two = answer(&pair, &main, Some(&file), AFTER, &mut terminal).expect("round answer");
    assert_eq!(
        terminal.report(),
        format!(
            "{} line 2: staged: approve, option 1: Change the spec; replaces the choice staged \
             {ROUND}\nanswers: 1 staged, 0 left open, 0 refused\n",
            gaps[3]
        )
    );
    assert!(
        terminal.questions[0].starts_with(&format!(
            "note: this answer replaces the choice staged {ROUND}\n"
        )),
        "{}",
        terminal.questions[0]
    );
    assert_eq!(stored_stage(&pair, &gaps[3]), Some(option_stage(1)));
    assert_eq!(run.exit(), Exit::NotFound);
    assert_eq!(run_two.exit(), Exit::NotFound, "kept");
}

// ------------------------------------------------------------------ AC-09

#[test]
fn ac09_each_stale_unknown_or_look_alike_answer_is_refused_alone() {
    let pair = Pair::new("rd-ac09", "spec-a");
    let main = pair.main.clone();
    let stale = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let changed = update(&pair, &main);
    let target = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    let v1 = question(
        &pair,
        &main,
        &["EDGE-STAM-ZERO"],
        "Does an empty bar stop the run?",
    );
    let v2 = question(
        &pair,
        &main,
        &["RULE-CORE-LOOP"],
        "Does the loop end at dawn?",
    );
    let (sheet_text, _) = printed_sheet(&sheet(&pair, &main, None));
    assert!(sheet_text.contains(&format!("\n{stale}:\n")));
    // Applied after the sheet was printed.
    pair.decide_ok(&main, &stale, &flags(None, None, None), None);
    let commit = pair.proposal(&stale).applied_commit.expect("a commit");
    let look_alike = target.replacen('P', "\u{0420}", 1);
    let file = answers_file(
        &pair,
        "reply.md",
        format!(
            "project: {SLUG}\n{stale}: ok\n{changed}: ok\nPR-9999: ok\n{look_alike}: ok\n{v1}: \
             ok\n{v2}: Only after a full stop\n"
        ),
    );
    let mut terminal = Scripted::keys(&[Some(true), Some(false)]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    assert_eq!(
        terminal.report(),
        format!(
            "{stale} line 2: refused: `{stale}` is applied: commit {commit}; nothing staged\n\
             {changed} line 3: refused: `{changed}` is a change proposal (update): `spec approve \
             {changed}` decides it; nothing staged\n\
             PR-9999 line 4: refused: no proposal `PR-9999` in this project's queue; nothing \
             staged\n\
             {target} line 5: refused: `{look_alike}:` uses look-alike letters or digits; proposal \
             IDs are Latin only: write `{target}:` at the start of the line; nothing staged\n\
             {v1} line 6: staged: approve, the working answer\n\
             {v2} line 7: staged: approve, your answer\n\
             answers: 2 staged, 0 left open, 4 refused\n"
        )
    );
    assert_eq!(staged_events(&pair), [v1.clone(), v2.clone()]);
    assert_eq!(terminal.questions.len(), 2);
    assert!(terminal.questions[0].contains(&format!("apply {v1} as ")));
    assert!(terminal.questions[1].contains(&format!("apply {v2} as ")));
    assert_eq!(run.walk.len(), 2);
    assert_eq!(run.walk[0].outcome, WalkOutcome::Applied);
    assert!(
        run.walk[0].commit.is_some() && run.walk[0].record.is_some(),
        "{run:?}"
    );
    assert_eq!(run.walk[1].outcome, WalkOutcome::Kept);
    assert_eq!(pair.proposal(&v1).status, ProposalStatus::Applied);
    assert_eq!(
        stored_stage(&pair, &v2),
        Some(answer_stage("Only after a full stop"))
    );
    for id in [&changed, &target] {
        assert_eq!(stored_stage(&pair, id), None);
        assert_eq!(pair.proposal(id).status, ProposalStatus::Open);
    }
    assert_eq!(
        printed_run(&run).0,
        "round: 1 applied, 0 rejected, 1 kept staged, 0 refused\n"
    );
    assert_eq!(run.exit(), Exit::NotFound);
}

// ------------------------------------------------------------------ AC-10

#[test]
fn ac10_no_terminal_exits_2_before_the_file_or_the_queue_is_read() {
    let pair = Pair::new("rd-ac10-pipe", "spec-a");
    let main = pair.main.clone();
    let q = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let file = answers_file(&pair, "a.md", format!("project: {SLUG}\n{q}: ok\n"));
    let file = file.to_str().expect("UTF-8").to_owned();
    let events = pair.events().len();
    let refused = |run: &Run| {
        run.code(2);
        assert_eq!(run.stderr, CONSENT, "{}", run.show());
        assert_eq!(run.stdout, "", "{}", run.show());
        assert_eq!(pair.events().len(), events, "{:?}", staged_events(&pair));
        assert_eq!(stored_stage(&pair, &q), None);
    };
    refused(&pair.spec_piped(&main, &["round", "answer", &file], b"y\ny\n"));
    refused(&pair.spec_piped(&main, &["--json", "round", "answer", &file], b"y\n"));
    refused(&pair.spec_piped(&main, &["round", "answer", "--staged"], b"y\n"));
    refused(&spec_null(&pair, &main, &["round", "answer", &file]));
    // A FIFO with no writer is never opened.
    let fifo = pair.scratch.dir("answers").join("fifo");
    let made = Command::new("/usr/bin/mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo runs");
    assert!(made.success());
    let fifo = fifo.to_str().expect("UTF-8");
    let started = Instant::now();
    refused(&pair.spec_piped(&main, &["round", "answer", fifo], b""));
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
    // Both or neither of FILE and --staged: a usage error.
    for args in [
        &["round", "answer", &file, "--staged"][..],
        &["round", "answer"][..],
    ] {
        let run = pair.spec_pty(&main, args, None);
        assert_eq!(run.code, 2, "{args:?}: {}", run.output);
        assert!(!run.asked);
    }
    assert_eq!(pair.events().len(), events);
}

#[test]
fn ac10_on_a_terminal_the_file_and_the_project_line_exit_2_with_nothing_staged() {
    let pair = Pair::new("rd-ac10-tty", "spec-a");
    let main = pair.main.clone();
    let q = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let events = pair.events().len();
    let run_on = |path: &Path| {
        let path = path.to_str().expect("UTF-8");
        let run = pair.spec_pty(&main, &["round", "answer", path], None);
        assert_eq!(run.code, 2, "{path}: {}", run.output);
        assert!(!run.asked, "{}", run.output);
        assert_eq!(pair.events().len(), events);
        assert_eq!(stored_stage(&pair, &q), None);
        run.output
    };
    let line = |label: &Path, rest: &str| format!("spec: {}{rest}\n", label.display());

    let sparse = pair.scratch.dir("answers").join("big.md");
    fs::File::create(&sparse)
        .and_then(|file| file.set_len(8_388_609))
        .expect("a sparse file");
    assert_eq!(
        run_on(&sparse),
        line(
            &sparse,
            ": 8388609 bytes; at most 8388608: return only the answer lines"
        )
    );
    // At the cap the file is read: eight MiB of NUL hold no project line.
    let at_cap = pair.scratch.dir("answers").join("cap.md");
    fs::File::create(&at_cap)
        .and_then(|file| file.set_len(8_388_608))
        .expect("a sparse file");
    assert_eq!(
        run_on(&at_cap),
        line(
            &at_cap,
            &format!(
                ": the answers name no project: keep the line `project: {SLUG}`; nothing changed"
            )
        )
    );
    let missing = answers_file(&pair, "missing.md", format!("{q}: ok\n"));
    assert_eq!(
        run_on(&missing),
        line(
            &missing,
            &format!(
                ": the answers name no project: keep the line `project: {SLUG}`; nothing changed"
            )
        )
    );
    let other = answers_file(&pair, "other.md", format!("project: other-game\n{q}: ok\n"));
    assert_eq!(
        run_on(&other),
        line(
            &other,
            &format!(":1: the answers are for `other-game`, this is `{SLUG}`; nothing changed")
        )
    );
    let twice = answers_file(
        &pair,
        "twice.md",
        format!("project: {SLUG}\n{q}: ok\nproject: {SLUG}\n"),
    );
    assert_eq!(
        run_on(&twice),
        line(
            &twice,
            &format!(
                ": the answers name a project on lines 1 and 3: keep one line `project: {SLUG}`; nothing changed"
            )
        )
    );
    let bad = answers_file(&pair, "bad.md", b"project: lantern-keep\nPR-0001: \xff\n");
    assert_eq!(run_on(&bad), line(&bad, ":2: not UTF-8"));
    let fifo = pair.scratch.dir("answers").join("fifo");
    assert!(
        Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success()
    );
    assert_eq!(
        run_on(&fifo),
        line(
            &fifo,
            ": not a regular file: give the filled sheet or a reply as a file"
        )
    );
    let absent = pair.scratch.dir("answers").join("absent.md");
    assert!(
        run_on(&absent).starts_with(&line(&absent, ": cannot read it: ").trim_end().to_owned())
    );
}

// ------------------------------------------------------------------ AC-11

#[test]
fn ac11_one_key_per_answer_on_a_terminal() {
    let pair = Pair::new("rd-ac11", "spec-a");
    let main = pair.main.clone();
    let q1 = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let q2 = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    let d3 = gap(
        &pair,
        &main,
        &["RULE-SPRINT-COST"],
        "The sprint cost is charged twice",
    );
    let file = answers_file(
        &pair,
        "reply.md",
        format!("project: {SLUG}\n{q1}: ok\n{q2}: My own answer\n{d3}: [1]\n"),
    );
    let head = pair.rev(&main, "HEAD");
    let file = file.to_str().expect("UTF-8");
    let run = pty_keys(&pair, &main, &["round", "answer", file], &["y", "n", "y"]);
    assert_eq!(run.code, 1, "{}", run.output);
    let asked = prompts(&run.output);
    assert_eq!(asked.len(), 3, "{}", run.output);
    for (prompt, id) in asked.iter().zip([&q1, &q2, &d3]) {
        assert!(prompt.contains(&format!("apply {id} as ")), "{prompt}");
        assert!(prompt.contains(", staged) on main in "), "{prompt}");
        assert!(prompt.contains("? [y/N]"), "{prompt}");
    }
    let report = run
        .output
        .find("answers: 3 staged, 0 left open, 0 refused\n")
        .expect("the report");
    assert!(
        report < run.output.find("[y/N]").expect("a prompt"),
        "{}",
        run.output
    );
    assert!(
        run.output.contains(&format!(
            "kept staged: spec approve {q2} or spec round answer --staged confirms it\n"
        )),
        "{}",
        run.output
    );
    assert!(
        run.output
            .ends_with("round: 2 applied, 0 rejected, 1 kept staged, 0 refused\n"),
        "{}",
        run.output
    );
    // Two records, each in its own `spec: apply` commit, each added.
    let subjects = pair.git_text(&main, &["log", "--format=%s", &format!("{head}..HEAD")]);
    assert_eq!(subjects.lines().count(), 2, "{subjects}");
    assert!(
        subjects
            .lines()
            .all(|subject| subject.starts_with("spec: apply ")),
        "{subjects}"
    );
    for id in [&q1, &d3] {
        let proposal = pair.proposal(id);
        assert_eq!(proposal.status, ProposalStatus::Applied, "{id}");
        let commit = proposal.applied_commit.expect("a commit");
        let changes = pair.git_text(&main, &["show", "--name-status", "--format=", &commit]);
        let lines: Vec<&str> = changes.lines().collect();
        assert_eq!(lines.len(), 1, "{changes}");
        assert!(
            lines[0].starts_with("A\tdocs/records/DEC/DEC-"),
            "{changes}"
        );
        assert!(lines[0].ends_with(".md"), "{changes}");
    }
    assert_eq!(pair.proposal(&q2).status, ProposalStatus::Open);
    assert_eq!(
        stored_stage(&pair, &q2),
        Some(answer_stage("My own answer"))
    );
}

// ------------------------------------------------------------------ AC-12

#[test]
fn ac12_a_stage_replaced_while_asked_is_refused_and_the_walk_goes_on() {
    let pair = Pair::new("rd-ac12", "spec-a");
    let main = pair.main.clone();
    let q1 = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let q2 = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    let file = answers_file(
        &pair,
        "reply.md",
        format!("project: {SLUG}\n{q1}: ok\n{q2}: ok\n"),
    );
    let mut terminal = Scripted::keys(&[Some(true), Some(true)]);
    terminal.before = Some(Box::new(|index| {
        if index == 0 {
            stage_now(
                &pair,
                &main,
                &q1,
                approve_body(Some("Another answer")),
                AFTER,
            );
        }
    }));
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    let (questions, told) = (terminal.questions.clone(), terminal.told.clone());
    drop(terminal);
    assert_eq!(questions.len(), 2, "{questions:?}");
    assert!(questions[1].contains(&format!("apply {q2} as ")));
    assert_eq!(run.walk.len(), 2);
    assert_eq!(run.walk[0].outcome, WalkOutcome::Refused);
    let reason = run.walk[0].reason.clone().expect("a reason");
    assert!(reason.contains("(step 7)"), "{reason}");
    assert!(
        reason.contains(&format!(
            "{q1} changed since the question: its staged choice was replaced or removed"
        )),
        "{reason}"
    );
    assert!(
        told.contains(&format!("{q1}: refused: {reason}")),
        "{told:?}"
    );
    assert_eq!(run.walk[1].outcome, WalkOutcome::Applied);
    assert_eq!(pair.proposal(&q2).status, ProposalStatus::Applied);
    assert_eq!(pair.proposal(&q1).status, ProposalStatus::Open);
    assert_eq!(
        stored_stage(&pair, &q1),
        Some(answer_stage("Another answer"))
    );
    assert_eq!(
        printed_run(&run).0,
        "round: 1 applied, 0 rejected, 0 kept staged, 1 refused\n"
    );
    assert_eq!(run.exit(), Exit::NotFound);
}

#[test]
fn a_stage_replaced_before_its_question_is_refused_unasked() {
    let pair = Pair::new("rd-moved", "spec-a");
    let main = pair.main.clone();
    let q1 = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let q2 = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    let file = answers_file(
        &pair,
        "reply.md",
        format!("project: {SLUG}\n{q1}: ok\n{q2}: ok\n"),
    );
    let mut terminal = Scripted::keys(&[Some(false), Some(true)]);
    terminal.before = Some(Box::new(|index| {
        if index == 0 {
            stage_now(
                &pair,
                &main,
                &q2,
                approve_body(Some("Another answer")),
                AFTER,
            );
        }
    }));
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    let questions = terminal.questions.clone();
    drop(terminal);
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert_eq!(run.walk[0].outcome, WalkOutcome::Kept);
    assert_eq!(run.walk[1].outcome, WalkOutcome::Refused);
    assert_eq!(
        run.walk[1].reason.as_deref(),
        Some(
            format!(
                "`{q2}` changed since this round staged it: its staged choice was replaced or \
                 removed; nothing changed"
            )
            .as_str()
        )
    );
    assert_eq!(
        stored_stage(&pair, &q2),
        Some(answer_stage("Another answer"))
    );
    assert_eq!(pair.proposal(&q2).status, ProposalStatus::Open);
}

#[test]
fn the_end_of_input_keeps_the_rest_staged() {
    let pair = Pair::new("rd-eof", "spec-a");
    let main = pair.main.clone();
    let q1 = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let q2 = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    let file = answers_file(
        &pair,
        "reply.md",
        format!("project: {SLUG}\n{q1}: ok\n{q2}: reject: dup\n"),
    );
    let mut terminal = Scripted::keys(&[]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    assert_eq!(terminal.questions.len(), 1);
    let reasons: Vec<(WalkOutcome, Option<String>)> = run
        .walk
        .iter()
        .map(|step| (step.outcome, step.reason.clone()))
        .collect();
    assert_eq!(
        reasons,
        [
            (
                WalkOutcome::Kept,
                Some(format!(
                    "end of input: kept staged: spec approve {q1} or spec round answer --staged confirms it"
                ))
            ),
            (
                WalkOutcome::Kept,
                Some(format!(
                    "end of input: kept staged: spec reject {q2} or spec round answer --staged confirms it"
                ))
            ),
        ]
    );
    assert_eq!(stored_stage(&pair, &q1), Some(working_answer()));
    assert_eq!(
        stored_stage(&pair, &q2),
        Some(Stage::Reject {
            reason: "dup".to_owned()
        })
    );

    // The binary: ^D at the first question keeps both.
    let pair = Pair::new("rd-eof-tty", "spec-a");
    let main = pair.main.clone();
    let q1 = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let q2 = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    let file = answers_file(
        &pair,
        "reply.md",
        format!("project: {SLUG}\n{q1}: ok\n{q2}: ok\n"),
    );
    let run = pty_keys(
        &pair,
        &main,
        &["round", "answer", file.to_str().expect("UTF-8")],
        &[],
    );
    assert_eq!(run.code, 1, "{}", run.output);
    assert_eq!(prompts(&run.output).len(), 1, "{}", run.output);
    assert!(
        run.output
            .ends_with("round: 0 applied, 0 rejected, 2 kept staged, 0 refused\n"),
        "{}",
        run.output
    );
    assert_eq!(stored_stage(&pair, &q2), Some(working_answer()));
}

// ------------------------------------------------------------------ AC-13

#[test]
fn ac13_staged_asks_the_staged_questions_in_round_order_and_notes_an_update() {
    let pair = Pair::new("rd-ac13", "spec-a");
    let main = pair.main.clone();
    let second = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let request = question_request(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
        Some(IntakeSeverity::High),
        LATER,
    );
    let first = store_question(&pair, &main, request, None);
    let unstaged = question(
        &pair,
        &main,
        &["EDGE-STAM-ZERO"],
        "Does an empty bar stop the run?",
    );
    let changed = update(&pair, &main);
    // Staged in reverse round order, the update too.
    stage_now(&pair, &main, &second, approve_body(None), LATER);
    stage_now(
        &pair,
        &main,
        &first,
        |updated_at| StageBody::Reject {
            reason: "dup".to_owned(),
            updated_at,
        },
        AFTER,
    );
    stage_now(&pair, &main, &changed, approve_body(None), AFTER);
    let events = pair.events().len();

    let mut terminal = Scripted::keys(&[Some(false), Some(false)]);
    let run = answered(&pair, &main, None, &mut terminal);
    assert!(terminal.reports.is_empty(), "{:?}", terminal.reports);
    assert_eq!(terminal.questions.len(), 2, "{:?}", terminal.questions);
    assert!(
        terminal.questions[0].contains(&format!("reject {first} (")),
        "{}",
        terminal.questions[0]
    );
    assert!(
        terminal.questions[1].contains(&format!("apply {second} as ")),
        "{}",
        terminal.questions[1]
    );
    assert_eq!(
        run.notes,
        [format!(
            "1 staged change proposal(s) not asked: {changed}; spec approve <ID> confirms each"
        )]
    );
    assert!(run.answers.is_empty());
    assert_eq!(
        run.walk
            .iter()
            .map(|step| (step.id.clone(), step.outcome))
            .collect::<Vec<_>>(),
        [
            (first.clone(), WalkOutcome::Kept),
            (second.clone(), WalkOutcome::Kept)
        ]
    );
    assert_eq!(
        printed_run(&run).0,
        "round: 0 applied, 0 rejected, 2 kept staged, 0 refused\n"
    );
    assert_eq!(pair.events().len(), events, "--staged writes no stage");
    assert_eq!(stored_stage(&pair, &unstaged), None);
    assert_eq!(run.exit(), Exit::NotFound);

    // The binary on a terminal: the same questions, the note on stderr.
    let run = pty_keys(&pair, &main, &["round", "answer", "--staged"], &["n", "n"]);
    assert_eq!(run.code, 1, "{}", run.output);
    let asked = prompts(&run.output);
    assert_eq!(asked.len(), 2, "{}", run.output);
    assert!(asked[0].contains(&format!("reject {first} (")));
    assert!(asked[1].contains(&format!("apply {second} as ")));
    assert!(
        run.output.contains(&format!(
            "note: 1 staged change proposal(s) not asked: {changed}; spec approve <ID> confirms each\n"
        )),
        "{}",
        run.output
    );
    assert_eq!(pair.events().len(), events);
}

// ------------------------------------------------------- stage_with, round_stage

#[test]
fn stage_checks_its_clock_and_id_before_the_queue_is_opened() {
    let pair = Pair::new("rd-stage-order", "spec-a");
    let main = pair.main.clone();
    let request = |id: &str, now: &str| StageRequest {
        id: id.to_owned(),
        body: StageBody::Reject {
            reason: "dup".to_owned(),
            updated_at: NOW.to_owned(),
        },
        now: now.to_owned(),
        git: pair.git_env(&main),
    };
    let env = pair.env(&main);
    let bad_clock = stage(&env, &Globals::default(), &request("PR-0001", "yesterday"));
    let message = common::proposal::cannot(&bad_clock, "a bad clock");
    assert!(message.contains("yesterday"), "{message}");
    assert!(
        !pair.db().exists(),
        "the queue opened before the clock was checked"
    );
    let look_alike = stage(&env, &Globals::default(), &request("\u{0420}R-0001", ROUND))
        .expect("a usage refusal");
    assert_eq!(look_alike.exit(), Exit::CannotRun, "{look_alike:?}");
    assert!(
        !pair.db().exists(),
        "the queue opened before the ID was checked"
    );

    // With the queue: `updated_at` before the flags, as the daemon's route.
    let q = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let option = |updated_at: &str| StageRequest {
        id: q.clone(),
        body: StageBody::Approve {
            option: Some(1),
            answer: None,
            canon: None,
            note: None,
            updated_at: updated_at.to_owned(),
        },
        now: ROUND.to_owned(),
        git: pair.git_env(&main),
    };
    let stale = stage(&env, &Globals::default(), &option("2000-01-01T00:00:00Z")).expect("stage");
    let reason = stale.proposal.refusal.clone().expect("refused");
    assert!(reason.contains("changed since it was read"), "{reason}");
    let current = pair.proposal(&q).updated_at;
    let flagged = stage(&env, &Globals::default(), &option(&current)).expect("stage");
    assert_ne!(flagged.cause, None, "{flagged:?}");
    assert!(
        !flagged
            .proposal
            .refusal
            .clone()
            .unwrap_or_default()
            .contains("changed since")
    );
    assert_eq!(stored_stage(&pair, &q), None);
}

#[test]
fn round_stage_checks_and_stages_with_no_walk() {
    let pair = Pair::new("rd-round-stage", "spec-a");
    let main = pair.main.clone();
    let q = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let d = gap(
        &pair,
        &main,
        &["RULE-SPRINT-COST"],
        "The sprint cost is charged twice",
    );
    let run_on = |text: String| {
        round_stage(
            &pair.env(&main),
            &Globals::default(),
            &RoundStageRequest {
                text,
                now: ROUND.to_owned(),
                git: pair.git_env(&main),
            },
        )
    };
    let missing = run_on(format!("{q}: ok\n"));
    assert_eq!(
        common::proposal::cannot(&missing, "no project"),
        format!(
            "spec: the answers name no project: keep the line `project: {SLUG}`; nothing changed"
        )
    );
    let other = run_on(format!("project: x\n{q}: ok\n"));
    assert_eq!(
        common::proposal::cannot(&other, "another project"),
        format!("spec: line 1: the answers are for `x`, this is `{SLUG}`; nothing changed")
    );
    assert!(staged_events(&pair).is_empty());
    let run = run_on(format!("project: {SLUG}\n{q}: ok\n{d}: ok\n")).expect("round stage");
    assert!(run.walk.is_empty());
    assert_eq!(staged_events(&pair), std::slice::from_ref(&q));
    assert_eq!(stored_stage(&pair, &q), Some(working_answer()));
    assert_eq!(
        printed_run(&run).0,
        format!(
            "{q} line 2: staged: approve, the working answer\n{d} line 3: refused: `{d}` takes \
             one of its options by number, 0-2, or `reject: <reason>`; nothing staged\nanswers: 1 \
             staged, 0 left open, 1 refused\n"
        )
    );
    assert_eq!(run.exit(), Exit::NotFound);
}

// A discrepancy's option labels with an LF stay on one line of the sheet.
#[test]
fn one_line_slots_escape_lf_and_cr() {
    let pair = Pair::new("rd-slots", "spec-a");
    let main = pair.main.clone();
    let mut input = discrepancy(
        &["RULE-SPRINT-COST"],
        "The sprint cost is charged twice",
        &THREE,
    );
    input.options[1] = IntakeOption {
        label: "Change\nPR-0009: 1".to_owned(),
        effect: "one\rtwo".to_owned(),
        price: "\u{2028}cheap".to_owned(),
    };
    let stored = pair.raise_discrepancy(&main, input);
    let (text, _) = printed_sheet(&sheet(&pair, &main, None));
    assert!(
        text.contains("  - [1] Change\\nPR-0009: 1 | one\\rtwo | \\u{2028}cheap\n"),
        "{text}"
    );
    assert!(
        !text.lines().any(|line| line.starts_with("PR-0009")),
        "{text}"
    );
    assert!(text.contains(&format!("\n{stored}:\n")));
}

// ------------------------------------------------- "Answers": checked against the queue now

#[test]
fn approves_meet_apply_steps_2_and_3_read_only_and_a_reject_still_settles() {
    let pair = Pair::new("rd-ready", "spec-a");
    let main = pair.main.clone();
    let linked = pair.linked.clone();
    let q1 = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let q2 = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    let q3 = question(
        &pair,
        &linked,
        &["EDGE-STAM-ZERO"],
        "Does an empty bar stop the run?",
    );
    let q4 = question(
        &pair,
        &linked,
        &["RULE-CORE-LOOP"],
        "Does the loop end at dawn?",
    );
    let path_only = question(
        &pair,
        &main,
        &["docs/features/stamina-tuning.md"],
        "Is the tuning plan complete?",
    );
    pair.drop_records_table(&main);
    pair.git.git(
        &main,
        &[
            "worktree",
            "remove",
            "--force",
            linked.to_str().expect("UTF-8"),
        ],
    );
    assert!(!linked.exists());
    let before = pair.state();
    let file = answers_file(
        &pair,
        "reply.md",
        format!(
            "project: {SLUG}\n{q1}: ok\n{q2}: reject: no table\n{q3}: Some answer\n{q4}: reject: \
             gone\n{path_only}: ok\n"
        ),
    );
    let mut terminal = Scripted::keys(&[Some(true), Some(true)]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    let report: Vec<&str> = terminal.report().lines().collect();
    assert_eq!(report.len(), 6, "{report:?}");
    let settles = "; `reject: <reason>` still settles it; nothing staged";
    assert!(
        report[0].starts_with(&format!("{q1} line 2: refused: not applied (step 3): ")),
        "{}",
        report[0]
    );
    assert!(
        report[0].contains("has no `[decision_records]`"),
        "{}",
        report[0]
    );
    assert!(report[0].ends_with(settles), "{}", report[0]);
    assert_eq!(report[1], format!("{q2} line 3: staged: reject"));
    assert!(
        report[2].starts_with(&format!("{q3} line 4: refused: not applied (step 2): ")),
        "{}",
        report[2]
    );
    assert!(
        report[2].contains(linked.to_str().expect("UTF-8")),
        "{}",
        report[2]
    );
    assert!(report[2].ends_with(settles), "{}", report[2]);
    assert_eq!(report[3], format!("{q4} line 5: staged: reject"));
    assert_eq!(
        report[4],
        format!(
            "{path_only} line 6: refused: `{path_only}` names no ID, only a path: its record needs \
             --canon: `spec approve {path_only} --canon REF`; nothing staged"
        )
    );
    assert_eq!(report[5], "answers: 2 staged, 0 left open, 3 refused");
    assert_eq!(staged_events(&pair), [q2.clone(), q4.clone()]);
    assert_eq!(
        run.walk
            .iter()
            .map(|step| (step.id.clone(), step.outcome))
            .collect::<Vec<_>>(),
        [
            (q2.clone(), WalkOutcome::Rejected),
            (q4.clone(), WalkOutcome::Rejected)
        ]
    );
    assert_eq!(
        printed_run(&run).0,
        "round: 0 applied, 2 rejected, 0 kept staged, 0 refused\n"
    );
    for id in [&q2, &q4] {
        assert_eq!(pair.proposal(id).status, ProposalStatus::Rejected, "{id}");
    }
    // Steps 2 and 3 read only: no file, ref or worktree changed.
    let after = pair.state();
    assert_eq!(
        (
            &after.main_files,
            &after.refs,
            &after.main_head,
            &after.main_status
        ),
        (
            &before.main_files,
            &before.refs,
            &before.main_head,
            &before.main_status
        )
    );
}

#[test]
fn another_repository_s_item_is_refused_and_an_orphan_takes_only_a_reject() {
    let pair = Pair::new("rd-places", "spec-a");
    let main = pair.main.clone();
    let other = other_repository(&pair, "other");
    let theirs = question(
        &pair,
        &other,
        &["RULE-STAM-REGEN"],
        "Does the copy regenerate too?",
    );
    let gone = other_repository(&pair, "gone");
    let orphan = question(
        &pair,
        &gone,
        &["RULE-SPRINT-COST"],
        "Is the copy's cost doubled?",
    );
    let orphan_rejected = question(
        &pair,
        &gone,
        &["RULE-MOVE-SPEEDS"],
        "Is the copy's walk slower?",
    );
    fs::remove_dir_all(&gone).expect("remove the repository");
    let file = answers_file(
        &pair,
        "reply.md",
        format!("project: {SLUG}\n{theirs}: ok\n{orphan}: ok\n{orphan_rejected}: reject: gone\n"),
    );
    let mut terminal = Scripted::keys(&[Some(true)]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    let report: Vec<&str> = terminal.report().lines().collect();
    assert!(
        report[0].starts_with(&format!(
            "{theirs} line 2: refused: `{theirs}` belongs to another repository of the project `{SLUG}`: "
        )),
        "{}",
        report[0]
    );
    assert!(
        report[0].ends_with(": run the command there; nothing staged"),
        "{}",
        report[0]
    );
    assert!(
        report[1].starts_with(&format!(
            "{orphan} line 3: refused: `{orphan}` belongs to the repository "
        )),
        "{}",
        report[1]
    );
    assert!(
        report[1].contains("which no longer exists"),
        "{}",
        report[1]
    );
    assert_eq!(
        report[2],
        format!("{orphan_rejected} line 4: staged: reject")
    );
    assert_eq!(report[3], "answers: 1 staged, 0 left open, 2 refused");
    assert_eq!(run.walk.len(), 1);
    assert_eq!(run.walk[0].outcome, WalkOutcome::Rejected, "{run:?}");
    assert_eq!(
        pair.proposal(&orphan_rejected).status,
        ProposalStatus::Rejected
    );
    assert_eq!(pair.proposal(&theirs).status, ProposalStatus::Open);
    assert_eq!(stored_stage(&pair, &theirs), None);
}

#[test]
fn a_corrupt_row_is_a_note_and_refuses_only_its_answer() {
    let pair = Pair::new("rd-corrupt", "spec-a");
    let main = pair.main.clone();
    let q1 = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let q2 = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    // An approved question with no record: a row that does not decode.
    pair.sql(&format!(
        "UPDATE proposals SET status = 'approved' WHERE id = '{q2}';"
    ));
    let (text, json_text) = printed_sheet(&sheet(&pair, &main, None));
    let json = parsed(&json_text);
    assert_eq!(ids(&json["items"]), std::slice::from_ref(&q1));
    let notes = strings(&json["notes"]);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].contains(&q2) && notes[0].ends_with("; not listed"),
        "{notes:?}"
    );
    assert!(text.ends_with(&format!("note: {}\n", notes[0])), "{text}");
    let file = answers_file(
        &pair,
        "reply.md",
        format!("project: {SLUG}\n{q1}: ok\n{q2}: ok\n"),
    );
    let mut terminal = Scripted::keys(&[Some(false)]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    let report: Vec<&str> = terminal.report().lines().collect();
    assert_eq!(
        report[0],
        format!("{q1} line 2: staged: approve, the working answer")
    );
    assert!(
        report[1].starts_with(&format!("{q2} line 3: refused: ")),
        "{}",
        report[1]
    );
    assert!(report[1].contains("record_id"), "{}", report[1]);
    assert_eq!(staged_events(&pair), std::slice::from_ref(&q1));
    assert_eq!(run.walk.len(), 1);
}

// ------------------------------------------------------- "Sheet": edges

#[test]
fn an_empty_queue_prints_the_frame_and_its_notes_and_exits_0() {
    let pair = Pair::new("rd-empty", "spec-a");
    let main = pair.main.clone();
    let (text, json_text) = printed_sheet(&sheet(&pair, &main, None));
    assert_eq!(
        text,
        format!(
            "# Open questions: {SLUG}\n\nproject: {SLUG}\n\nNo open questions or discrepancies.\n"
        )
    );
    assert_eq!(
        parsed(&json_text),
        json!({"project": SLUG, "items": [], "more": 0, "minutes": 0, "notes": []})
    );
    let run = pair.spec_piped(&main, &["round", "new"], b"");
    run.code(0);
    assert_eq!(run.stdout, text);
    // Only a change proposal: no item, its note.
    let changed = update(&pair, &main);
    let (text, _) = printed_sheet(&sheet(&pair, &main, None));
    assert_eq!(
        text,
        format!(
            "# Open questions: {SLUG}\n\nproject: {SLUG}\n\nNo open questions or \
             discrepancies.\n\nnote: 1 open change proposal(s) not on this sheet: {changed}; spec \
             inbox lists them\n"
        )
    );
    // An unedited empty sheet answers nothing.
    let file = answers_file(&pair, "empty.md", &text);
    let mut terminal = Scripted::keys(&[]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    assert_eq!(
        terminal.report(),
        "answers: 0 staged, 0 left open, 0 refused\n"
    );
    assert_eq!(run.exit(), Exit::Answered);
}

#[test]
fn a_target_gone_from_the_tree_is_named_not_found_now() {
    let pair = Pair::new("rd-gone-target", "spec-a");
    let main = pair.main.clone();
    let q = question(
        &pair,
        &main,
        &["EDGE-STAM-ZERO", "RULE-STAM-REGEN"],
        "Does an empty bar stop the run?",
    );
    common::replace(
        &main,
        "docs/spec/movement/stamina.md",
        "## Depletion {#EDGE-STAM-ZERO}\n",
        "## Depletion\n",
    );
    let (text, json_text) = printed_sheet(&sheet(&pair, &main, None));
    assert!(
        item_block(&text, &q)
            .contains("Concerns: EDGE-STAM-ZERO (not found now); Regeneration (RULE-STAM-REGEN)\n"),
        "{text}"
    );
    let json = parsed(&json_text);
    assert_eq!(
        json["items"][0]["targets"],
        json!([
            {"id": "EDGE-STAM-ZERO", "path": "docs/spec/movement/stamina.md", "title": null},
            {"id": "RULE-STAM-REGEN", "path": "docs/spec/movement/stamina.md", "title": "Regeneration"},
        ])
    );
}

#[test]
fn a_directory_with_no_project_cannot_run_a_round() {
    let pair = Pair::new("rd-no-project", "spec-a");
    let bare = pair.scratch.dir("bare");
    let run = pair.spec_piped(&bare, &["round", "new"], b"");
    run.code(2);
    assert!(run.stderr.contains("specengine.toml"), "{}", run.show());
    assert_eq!(run.stdout, "");
    let run = pair.spec_pty(&bare, &["round", "answer", "--staged"], None);
    assert_eq!(run.code, 2, "{}", run.output);
    assert!(run.output.contains("specengine.toml"), "{}", run.output);
}

// ------------------------------------------------------- iteration 2

/// The report of `count` answers for `q` on lines 2.. of one file.
fn repeated(pair: &Pair, q: &str, count: usize) -> (RoundRun, String) {
    let mut text = format!("project: {SLUG}\n");
    for index in 0..count {
        text.push_str(&format!("{q}: answer {index}\n"));
    }
    let file = answers_file(pair, &format!("repeated-{count}.md"), &text);
    let mut terminal = Scripted::keys(&[]);
    let run = answered(pair, &pair.main, Some(&file), &mut terminal);
    assert!(terminal.questions.is_empty());
    let report = terminal.report().to_owned();
    (run, report)
}

#[test]
fn a_refusal_names_at_most_ten_lines_and_counts_the_rest() {
    let pair = Pair::new("rd-ten", "spec-a");
    let main = pair.main.clone();
    let q = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    for (count, named) in [
        (10, "2, 3, 4, 5, 6, 7, 8, 9, 10 and 11"),
        (11, "2, 3, 4, 5, 6, 7, 8, 9, 10, 11 and 1 more"),
        (12, "2, 3, 4, 5, 6, 7, 8, 9, 10, 11 and 2 more"),
    ] {
        let (run, report) = repeated(&pair, &q, count);
        let reason = format!("answered on lines {named}: give one answer per item");
        let mut expected: String = (2..count + 2)
            .map(|line| format!("{q} line {line}: refused: {reason}; nothing staged\n"))
            .collect();
        expected.push_str(&format!(
            "answers: 0 staged, 0 left open, {count} refused\n"
        ));
        assert_eq!(report, expected);
        assert!(
            run.answers
                .iter()
                .all(|answer| answer.reason.as_deref() == Some(reason.as_str())),
            "{run:?}"
        );
    }
    // Repeated project lines: the same cap.
    let mut text = String::new();
    for _ in 0..12 {
        text.push_str(&format!("project: {SLUG}\n"));
    }
    text.push_str(&format!("{q}: ok\n"));
    let file = answers_file(&pair, "projects.md", &text);
    let mut terminal = Scripted::keys(&[]);
    let refused = answer(&pair, &main, Some(&file), ROUND, &mut terminal);
    assert_eq!(
        common::proposal::cannot(&refused, "twelve project lines"),
        format!(
            "spec: {}: the answers name a project on lines 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 and 2 \
             more: keep one line `project: {SLUG}`; nothing changed",
            file.display()
        )
    );
    assert!(staged_events(&pair).is_empty());
}

#[test]
fn four_thousand_answers_for_one_item_give_a_linear_report() {
    let pair = Pair::new("rd-linear", "spec-a");
    let main = pair.main.clone();
    let q = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let count = 4000;
    let started = Instant::now();
    let (run, report) = repeated(&pair, &q, count);
    let took = started.elapsed();
    assert_eq!(report.lines().count(), count + 1);
    assert!(
        report.ends_with(&format!(
            "answers: 0 staged, 0 left open, {count} refused\n"
        )),
        "{}",
        &report[report.len() - 200..]
    );
    // About 133 bytes a line: each refusal names ten lines and counts 3990.
    let per_line = report.len() / count;
    assert!((110..=160).contains(&per_line), "{per_line} bytes a line");
    assert!(report.contains(" and 3990 more: give one answer per item"));
    let (_, json) = printed_run(&run);
    assert!(json.len() < count * 400, "{} bytes of JSON", json.len());
    eprintln!(
        "{count} lines: {} bytes of report ({per_line} a line), {took:?}",
        report.len()
    );
    assert!(took < Duration::from_secs(10), "{took:?} for {count} lines");
    assert!(staged_events(&pair).is_empty());
}

#[test]
fn reject_without_its_colon_is_refused_in_the_report() {
    let pair = Pair::new("rd-reject-colon", "spec-a");
    let main = pair.main.clone();
    let q1 = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    let q2 = question(
        &pair,
        &main,
        &["RULE-MOVE-SPEEDS"],
        "Is the walk speed fixed?",
    );
    let q3 = question(
        &pair,
        &main,
        &["EDGE-STAM-ZERO"],
        "Does an empty bar stop the run?",
    );
    let file = answers_file(
        &pair,
        "reply.md",
        format!("project: {SLUG}\n{q1}: reject : dup\n{q2}: Reject dup\n{q3}: rejected twice\n"),
    );
    let mut terminal = Scripted::keys(&[Some(false)]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    let colon = "refused: `reject` takes its reason after a colon right behind it: write `reject: \
                 <reason>`; nothing staged";
    assert_eq!(
        terminal.report(),
        format!(
            "{q1} line 2: {colon}\n{q2} line 3: {colon}\n{q3} line 4: staged: approve, your \
             answer\nanswers: 1 staged, 0 left open, 2 refused\n"
        )
    );
    assert_eq!(staged_events(&pair), std::slice::from_ref(&q3));
    assert_eq!(
        (stored_stage(&pair, &q1), stored_stage(&pair, &q2)),
        (None, None)
    );
    assert_eq!(run.exit(), Exit::NotFound);
}

#[test]
fn an_orphan_s_kept_reject_names_spec_reject_alone() {
    let pair = Pair::new("rd-orphan-kept", "spec-a");
    let main = pair.main.clone();
    let gone = other_repository(&pair, "gone");
    let orphan = question(
        &pair,
        &gone,
        &["RULE-SPRINT-COST"],
        "Is the copy's cost doubled?",
    );
    fs::remove_dir_all(&gone).expect("remove the repository");
    let file = answers_file(
        &pair,
        "reply.md",
        format!("project: {SLUG}\n{orphan}: reject: gone\n"),
    );
    let mut terminal = Scripted::keys(&[Some(false)]);
    let run = answered(&pair, &main, Some(&file), &mut terminal);
    assert_eq!(
        terminal.report(),
        format!("{orphan} line 2: staged: reject\nanswers: 1 staged, 0 left open, 0 refused\n")
    );
    let kept = format!("kept staged: spec reject {orphan} confirms it");
    assert_eq!(run.walk.len(), 1);
    assert_eq!(run.walk[0].outcome, WalkOutcome::Kept);
    assert_eq!(run.walk[0].reason.as_deref(), Some(kept.as_str()));
    assert!(terminal.told.contains(&kept), "{:?}", terminal.told);
    assert_eq!(
        stored_stage(&pair, &orphan),
        Some(Stage::Reject {
            reason: "gone".to_owned()
        })
    );
    // `--staged` never asks it: a note.
    let mut terminal = Scripted::keys(&[Some(true)]);
    let run = answered(&pair, &main, None, &mut terminal);
    assert!(terminal.questions.is_empty(), "{:?}", terminal.questions);
    assert_eq!(
        run.notes,
        [format!(
            "1 staged item(s) of a repository that no longer exists not asked: {orphan}; spec \
             reject <ID> confirms each"
        )]
    );
    assert_eq!(pair.proposal(&orphan).status, ProposalStatus::Open);
}

#[test]
fn staged_notes_another_repository_s_and_a_gone_one_s_staged_items_unasked() {
    let pair = Pair::new("rd-staged-notes", "spec-a");
    let main = pair.main.clone();
    let ours = question(
        &pair,
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for rest?",
    );
    stage_now(&pair, &main, &ours, approve_body(None), LATER);
    let other = other_repository(&pair, "other");
    let theirs = question(
        &pair,
        &other,
        &["RULE-MOVE-SPEEDS"],
        "Is the copy's walk slower?",
    );
    stage_now(&pair, &other, &theirs, approve_body(None), LATER);
    let gone = other_repository(&pair, "gone");
    let orphan = question(
        &pair,
        &gone,
        &["RULE-SPRINT-COST"],
        "Is the copy's cost doubled?",
    );
    stage_now(
        &pair,
        &gone,
        &orphan,
        |updated_at| StageBody::Reject {
            reason: "gone".to_owned(),
            updated_at,
        },
        LATER,
    );
    fs::remove_dir_all(&gone).expect("remove the repository");
    let events = pair.events().len();
    let orphan_note = format!(
        "1 staged item(s) of a repository that no longer exists not asked: {orphan}; spec reject \
         <ID> confirms each"
    );
    let other_note = |id: &str| {
        format!(
            "1 staged item(s) of another repository of the project `{SLUG}` not asked: {id}; run \
             `spec round answer --staged` there"
        )
    };

    let mut terminal = Scripted::keys(&[Some(false)]);
    let run = answered(&pair, &main, None, &mut terminal);
    assert_eq!(terminal.questions.len(), 1, "{:?}", terminal.questions);
    assert!(terminal.questions[0].contains(&format!("apply {ours} as ")));
    assert_eq!(run.notes, [other_note(&theirs), orphan_note.clone()]);
    assert_eq!(run.walk.len(), 1);

    // From the other repository: its own asked, ours noted.
    let mut terminal = Scripted::keys(&[Some(false)]);
    let run = answered(&pair, &other, None, &mut terminal);
    assert_eq!(terminal.questions.len(), 1, "{:?}", terminal.questions);
    assert!(terminal.questions[0].contains(&format!("apply {theirs} as ")));
    assert_eq!(run.notes, [other_note(&ours), orphan_note]);
    assert_eq!(pair.events().len(), events, "--staged writes nothing");
}

/// `O_NONBLOCK` of `open(2)` on this system (macOS and the BSDs, Linux).
const NONBLOCK: i32 = if cfg!(target_os = "linux") {
    0o4000
} else {
    0x0004
};

#[test]
fn a_fifo_swapped_in_after_the_size_check_never_holds_the_open() {
    use std::os::unix::fs::OpenOptionsExt as _;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let pair = Pair::new("rd-fifo-swap", "spec-a");
    let dir = pair.scratch.dir("swap");
    // Not UTF-8: a regular file is refused right after its read, before
    // the queue is opened, so each round is short.
    let regular = dir.join("regular");
    fs::write(&regular, b"project: x\n\xff\n").expect("write");
    let fifo = dir.join("fifo");
    assert!(
        Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success()
    );
    let path = dir.join("answers.md");
    fs::hard_link(&regular, &path).expect("link");

    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (stop, regular, fifo, path, dir) = (
            Arc::clone(&stop),
            regular.clone(),
            fifo.clone(),
            path.clone(),
            dir.clone(),
        );
        std::thread::spawn(move || {
            let mut swaps = 0_u64;
            while !stop.load(Ordering::SeqCst) {
                for (source, tmp) in [(&regular, "tmp-regular"), (&fifo, "tmp-fifo")] {
                    let tmp = dir.join(tmp);
                    let _ = fs::remove_file(&tmp);
                    fs::hard_link(source, &tmp).expect("link");
                    fs::rename(&tmp, &path).expect("rename");
                    swaps += 1;
                }
            }
            swaps
        })
    };
    let (sender, done) = mpsc::channel();
    let worker = {
        let (env, git, path, stop) = (
            pair.env(&pair.main),
            pair.git_env(&pair.main),
            path.clone(),
            Arc::clone(&stop),
        );
        std::thread::spawn(move || {
            let (mut fifos, mut regulars, mut others) = (0, 0, Vec::new());
            let started = Instant::now();
            for _ in 0..3000 {
                if stop.load(Ordering::SeqCst) || started.elapsed() > Duration::from_secs(8) {
                    break;
                }
                let mut terminal = Scripted::keys(&[]);
                let outcome = round_answer(
                    &env,
                    &Globals::default(),
                    &RoundAnswerRequest {
                        file: Some(path.clone()),
                        now: ROUND.to_owned(),
                        git: git.clone(),
                    },
                    &mut terminal,
                );
                match outcome {
                    Err(error) if error.message.contains(": not a regular file: ") => fifos += 1,
                    Err(error) if error.message.contains(":2: not UTF-8") => regulars += 1,
                    other => others.push(format!("{other:?}")),
                }
            }
            let _ = sender.send((fifos, regulars, others));
        })
    };
    let finished = done.recv_timeout(Duration::from_secs(60));
    stop.store(true, Ordering::SeqCst);
    let swaps = swapper.join().expect("the swapper");
    let (fifos, regulars, others) = match finished {
        Ok(counts) => counts,
        Err(_) => {
            // Held in `open` on the FIFO: a writer's open (never waiting)
            // lets it go, so the test ends.
            let writer = fs::OpenOptions::new()
                .write(true)
                .custom_flags(NONBLOCK)
                .open(&fifo);
            drop(writer);
            let _ = done.recv_timeout(Duration::from_secs(30));
            panic!("`round answer` held the open of a FIFO swapped in after its size check");
        }
    };
    worker.join().expect("the worker");
    eprintln!("{fifos} FIFO, {regulars} regular, {swaps} swaps");
    assert!(others.is_empty(), "{others:?}");
    assert!(
        fifos > 0 && regulars > 0,
        "{fifos} FIFO, {regulars} regular, {swaps} swaps"
    );
}
