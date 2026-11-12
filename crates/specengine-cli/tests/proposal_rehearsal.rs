//! docs/features/proposal-rehearsal.md (RM-0067), the CLI's half of the dry
//! run and of the `ref-dangling` filter: `spec propose update|create
//! --dry-run` through the binary (the answer is what an agent reads), the
//! queue and the repository read directly. The walk `spec approve --open`:
//! `proposal_open_walk.rs`; `propose_change {dry_run}`:
//! `specengine-mcp/tests/mcp_rehearsal.rs`.
//!
//! - AC-01: a dry run of a new file, of an update and of new sections gives
//!   the real call's `diagnostics` (the real call runs after it, in the same
//!   state, so the two are compared, not an expectation written here); the
//!   inbox stays empty, no `proposal.created`, `git status` empty, and the
//!   real call then stores under the first ID. Text and JSON repeat byte for
//!   byte. M: the row stored; validation skipped.
//! - AC-02: a taken ID (defined, or held by a live create), a stale base,
//!   `class: generated` and the other refusals give the real reason on
//!   `update` and on both forms of `create`, exit 1; `--task` and a detached
//!   `HEAD` as the real call. M: returning before step 4.
//! - AC-03, the CLI's half: `--dry-run` on `interpretation`, `amendment` and
//!   the other kinds is a usage error, exit 2. M: the flag accepted.
//! - AC-04: `follows`, `parent` and an update adding the link, on an ID a
//!   live create or amendment holds: no `ref-dangling`, one note per holder;
//!   a reference to nothing keeps its finding; `#SECTION`, an alias and a
//!   feature-scoped ID are read canonically; a rejected holder brings the
//!   finding back in `spec check`. M: filtering on the code alone.
//! - "Rules": two agents may rehearse one ID; the second real call is
//!   refused as today.
//!
//! Every repository is a scratch copy of `fixtures/spec-a` (a game) or
//! `fixtures/spec-b` (a command-line tool, Russian prose) with a linked
//! worktree on `t1` (`common::proposal`); proposals are raised in the linked
//! worktree, as an agent does.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::path::Path;

use common::Run;
use common::proposal::{CASES, DECIDER, NOW, Pair, State, edit};
use serde_json::{Value, json};
use specengine_cli::Exit;

/// A dry run's first note ("Data").
const DRY_NOTE: &str = "dry run: nothing stored";
const RATIONALE: &str = "Rehearse the change.";
const R13: &str = "docs/records/R/R-13.md";
const R14: &str = "docs/records/R/R-14.md";
const R15: &str = "docs/records/R/R-15.md";
const R16: &str = "docs/records/R/R-16.md";
const STAMINA: &str = "docs/spec/movement/stamina.md";
const TUNING: &str = "docs/features/stamina-tuning.md";
/// A base no span hashes to.
const STALE: &str = "b3:0000000000000000000000000000000000000000000000000000000000000000";

/// A spec-a record `id`, `extra` closing its front-matter.
fn record(id: &str, extra: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n\
         {extra}---\n\n# Record {id}\n\nA body.\n"
    )
}

/// A spec-b record `id`, Russian prose, `extra` closing its front-matter.
fn record_b(id: &str, extra: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nkind: requirement\nowner: owner\nreviewed: 2026-09-20\n\
         {extra}---\n\n# \u{0421}\u{043b}\u{0438}\u{044f}\u{043d}\u{0438}\u{0435}\n\n\
         \u{0412}\u{0435}\u{0442}\u{043a}\u{0430} \u{0441}\u{043b}\u{0438}\u{0432}\u{0430}\u{0435}\u{0442}\u{0441}\u{044f}.\n"
    )
}

/// One project: free `(id, path)` pairs, an ID nothing holds, its records.
struct Corpus {
    fixture: &'static str,
    free: [(&'static str, &'static str); 4],
    missing: &'static str,
    record: fn(&str, &str) -> String,
}

/// ADR-0008: a game's corpus and a command-line tool's.
const CORPORA: [Corpus; 2] = [
    Corpus {
        fixture: "spec-a",
        free: [("R-13", R13), ("R-14", R14), ("R-15", R15), ("R-16", R16)],
        missing: "R-99",
        record,
    },
    Corpus {
        fixture: "spec-b",
        free: [
            ("REQ-900", "docs/records/REQ/REQ-900.md"),
            ("REQ-901", "docs/records/REQ/REQ-901.md"),
            ("REQ-902", "docs/records/REQ/REQ-902.md"),
            ("REQ-903", "docs/records/REQ/REQ-903.md"),
        ],
        missing: "REQ-999",
        record: record_b,
    },
];

/// One `spec propose update|create`, its text on stdin.
#[derive(Clone, Copy)]
struct Call<'a> {
    form: &'a str,
    target: &'a str,
    base: Option<&'a str>,
    text: &'a str,
    task: Option<&'a str>,
    role: &'a str,
    /// `--distinct-from X`, each (a create's; docs/features/intake-similar.md
    /// "Acceptance criteria" IS1, IS2).
    distinct: &'a [&'a str],
}

/// `spec propose update TARGET --base BASE`.
fn update<'a>(target: &'a str, base: &'a str, text: &'a str) -> Call<'a> {
    Call {
        form: "update",
        target,
        base: Some(base),
        text,
        task: None,
        role: "spec-writer",
        distinct: &[],
    }
}

/// `spec propose create TARGET`: a new file.
fn create<'a>(target: &'a str, text: &'a str) -> Call<'a> {
    Call {
        form: "create",
        target,
        base: None,
        text,
        task: None,
        role: "spec-writer",
        distinct: &[],
    }
}

/// `spec propose create TARGET --base BASE`: new sections in its span.
fn sections<'a>(target: &'a str, base: &'a str, text: &'a str) -> Call<'a> {
    Call {
        form: "create",
        target,
        base: Some(base),
        text,
        task: None,
        role: "spec-writer",
        distinct: &[],
    }
}

impl<'a> Call<'a> {
    /// `--task T`.
    fn task(self, task: &'a str) -> Self {
        Self {
            task: Some(task),
            ..self
        }
    }

    /// `--author-role`: another agent's call.
    fn by(self, role: &'a str) -> Self {
        Self { role, ..self }
    }

    /// `--distinct-from` each of `distinct`: a create titled as a live one.
    fn naming(self, distinct: &'a [&'a str]) -> Self {
        Self { distinct, ..self }
    }

    fn run(&self, pair: &Pair, cwd: &Path, json: bool, dry: bool) -> Run {
        let mut args = Vec::new();
        if json {
            args.push("--json");
        }
        args.extend([
            "propose",
            self.form,
            self.target,
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--author-role",
            self.role,
        ]);
        if let Some(base) = self.base {
            args.extend(["--base", base]);
        }
        if let Some(task) = self.task {
            args.extend(["--task", task]);
        }
        for name in self.distinct {
            args.extend(["--distinct-from", name]);
        }
        if dry {
            args.push("--dry-run");
        }
        pair.spec_piped(cwd, &args, self.text.as_bytes())
    }

    /// The real call, which must store: its document.
    fn stored(&self, pair: &Pair, cwd: &Path) -> Value {
        let run = self.run(pair, cwd, true, false);
        run.code(0);
        run.json()
    }

    /// The dry run, which must answer: its document.
    fn rehearsed(&self, pair: &Pair, cwd: &Path) -> Value {
        let run = self.run(pair, cwd, true, true);
        run.code(0);
        run.json()
    }
}

fn id_of(document: &Value) -> String {
    document["id"].as_str().expect("an ID").to_owned()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("an array: {value}"))
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

/// The subjects of a document's findings of `code`.
fn subjects(document: &Value, code: &str) -> Vec<String> {
    document["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("diagnostics: {document}"))
        .iter()
        .filter(|finding| finding["code"] == code)
        .map(|finding| finding["subject"].as_str().expect("a subject").to_owned())
        .collect()
}

fn keys(document: &Value) -> Vec<String> {
    document
        .as_object()
        .expect("a document")
        .keys()
        .cloned()
        .collect()
}

/// "Data": the note of an ID a live proposal holds.
fn held_note(id: &str, by: &str, status: &str, kind: &str) -> String {
    format!("`{id}` is reserved by `{by}` ({status}), a live {kind}: no ref-dangling stored")
}

/// How many of `document`'s notes say `note`.
fn said(document: &Value, note: &str) -> usize {
    strings(&document["notes"])
        .iter()
        .filter(|written| written.as_str() == note)
        .count()
}

/// "Data", the answer: the real call's document with no ID and no state.
fn assert_rehearsed(document: &Value, kind: &str, target: &str, path: &str, context: &str) {
    assert_eq!(document["id"], Value::Null, "{context}: {document}");
    assert_eq!(document["status"], Value::Null, "{context}: {document}");
    assert_eq!(document["kind"], json!(kind), "{context}");
    assert_eq!(document["target_id"], json!(target), "{context}");
    assert_eq!(document["target_path"], json!(path), "{context}");
    assert_eq!(
        document["notes"][0],
        json!(DRY_NOTE),
        "{context}: {document}"
    );
    assert!(document["diagnostics"].is_array(), "{context}: {document}");
    for key in ["base_commit", "preview", "revisions", "created_at"] {
        assert_eq!(document[key], Value::Null, "{context}: {key}: {document}");
    }
}

/// Nothing in the queue, no event, the repository as it was.
fn assert_nothing_stored(pair: &Pair, before: &State, context: &str) {
    for args in [vec!["--json", "inbox"], vec!["--json", "inbox", "--all"]] {
        let run = pair.spec_piped(&pair.main, &args, b"");
        run.code(0);
        assert_eq!(run.json()["proposals"], json!([]), "{context}: {args:?}");
    }
    assert!(pair.proposals().is_empty(), "{context}: a row stored");
    assert!(
        pair.events()
            .iter()
            .all(|event| event.event_type != "proposal.created"),
        "{context}: {:?}",
        pair.events()
    );
    let after = pair.state();
    assert_eq!(after.main_status, "", "{context}");
    assert_eq!(after.linked_status, "", "{context}");
    assert_eq!(&after, before, "{context}");
}

// ------------------------------------------------------------------ AC-01

/// AC-01 on a game and on a command-line tool: a new file with a finding.
/// M: the row stored; validation skipped.
#[test]
fn ac01_a_dry_run_of_a_new_file_gives_the_real_diagnostics_and_stores_nothing() {
    for corpus in &CORPORA {
        let context = corpus.fixture;
        let pair = Pair::new("pr-ac01", context);
        let (id, path) = corpus.free[0];
        let text = (corpus.record)(
            id,
            &format!("links: {{derived_from: [{}]}}\n", corpus.missing),
        );
        let call = create(path, &text);
        let before = pair.state();

        let rehearsed = call.rehearsed(&pair, &pair.linked);
        assert_rehearsed(&rehearsed, "create", id, path, context);
        assert_eq!(
            subjects(&rehearsed, "ref-dangling"),
            [corpus.missing],
            "{context}: {rehearsed}"
        );
        assert_nothing_stored(&pair, &before, context);

        let stored = call.stored(&pair, &pair.linked);
        assert_eq!(stored["id"], json!("PR-0001"), "{context}: no ID burnt");
        assert_eq!(stored["status"], json!("open"), "{context}");
        assert_eq!(
            stored["diagnostics"], rehearsed["diagnostics"],
            "{context}: the real call's diagnostics"
        );
        for key in ["kind", "target_id", "target_path", "target_ids"] {
            assert_eq!(stored[key], rehearsed[key], "{context}: {key}");
        }
        assert_eq!(keys(&stored), keys(&rehearsed), "{context}: its document");
        assert_eq!(
            pair.proposal("PR-0001").diagnostics.len(),
            1,
            "{context}: stored as rehearsed"
        );
        let created = pair
            .events()
            .iter()
            .filter(|event| event.event_type == "proposal.created")
            .count();
        assert_eq!(created, 1, "{context}");
    }
}

/// AC-01: exit 0 without findings too, `diagnostics` empty, not `null`.
#[test]
fn ac01_a_clean_dry_run_exits_0() {
    for corpus in &CORPORA {
        let pair = Pair::new("pr-ac01-clean", corpus.fixture);
        let (id, path) = corpus.free[0];
        let text = (corpus.record)(id, "");
        let before = pair.state();
        let rehearsed = create(path, &text).rehearsed(&pair, &pair.linked);
        assert_rehearsed(&rehearsed, "create", id, path, corpus.fixture);
        assert_eq!(rehearsed["diagnostics"], json!([]), "{}", corpus.fixture);
        assert_nothing_stored(&pair, &before, corpus.fixture);
    }
}

/// AC-01, an update: both fixtures clean, then a body citing nothing. The
/// real call stores what was rehearsed. M: validation skipped.
#[test]
fn ac01_a_dry_run_of_an_update_gives_the_real_diagnostics() {
    for case in &CASES {
        let pair = Pair::of("pr-ac01-update", case);
        let (hash, span) = pair.span(&pair.linked, case.target);
        let text = edit(&span, case.first.0, case.first.1);
        let call = update(case.target, &hash, &text);
        let before = pair.state();
        let rehearsed = call.rehearsed(&pair, &pair.linked);
        assert_rehearsed(&rehearsed, "update", case.target, case.path, case.fixture);
        assert_eq!(rehearsed["diagnostics"], json!([]), "{}", case.fixture);
        assert_nothing_stored(&pair, &before, case.fixture);
        let stored = call.stored(&pair, &pair.linked);
        assert_eq!(stored["id"], json!("PR-0001"), "{}", case.fixture);
    }

    let pair = Pair::new("pr-ac01-citing", "spec-a");
    let target = "EDGE-SPRINT-EMPTY";
    let (hash, span) = pair.span(&pair.linked, target);
    let citing = edit(
        &span,
        "the sprint ends;",
        "the sprint ends (see RULE-UNDECLARED);",
    );
    let call = update(target, &hash, &citing);
    let before = pair.state();
    let rehearsed = call.rehearsed(&pair, &pair.linked);
    assert_eq!(
        rehearsed["diagnostics"][0]["subject"],
        json!("RULE-UNDECLARED"),
        "{rehearsed}"
    );
    assert_nothing_stored(&pair, &before, "citing");
    let stored = call.stored(&pair, &pair.linked);
    assert_eq!(stored["diagnostics"], rehearsed["diagnostics"]);
}

/// AC-01, new sections: the new ID is reported, not held. M: the row stored.
#[test]
fn ac01_a_dry_run_of_new_sections_holds_no_id() {
    let pair = Pair::new("pr-ac01-sections", "spec-a");
    let target = "RULE-STAM-REGEN";
    let (hash, span) = pair.span(&pair.linked, target);
    let text = format!("{span}\n\n### Rest delay {{#EDGE-STAM-REST}}\n- Waits 1.5 s.");
    let call = sections(target, &hash, &text);
    let before = pair.state();
    let rehearsed = call.rehearsed(&pair, &pair.linked);
    assert_rehearsed(&rehearsed, "create", target, STAMINA, "sections");
    assert_nothing_stored(&pair, &before, "sections");
    let stored = call.stored(&pair, &pair.linked);
    assert_eq!(id_of(&stored), "PR-0001");
    assert_eq!(stored["target_ids"], rehearsed["target_ids"]);
    assert_eq!(stored["diagnostics"], rehearsed["diagnostics"]);
    assert!(
        pair.proposal("PR-0001")
            .target_ids()
            .contains(&"EDGE-STAM-REST".to_owned()),
        "the real call holds it"
    );
}

/// One state, one answer: a repeated dry run is byte for byte the same,
/// text and JSON (no ID, no time, no path of a run in it).
#[test]
fn a_repeated_dry_run_is_byte_identical() {
    for corpus in &CORPORA {
        let pair = Pair::new("pr-repeat", corpus.fixture);
        let (id, path) = corpus.free[0];
        let text = (corpus.record)(
            id,
            &format!("links: {{derived_from: [{}]}}\n", corpus.missing),
        );
        let call = create(path, &text);
        for json in [false, true] {
            let first = call.run(&pair, &pair.linked, json, true);
            first.code(0);
            let second = call.run(&pair, &pair.linked, json, true);
            second.code(0);
            assert_eq!(first.stdout, second.stdout, "{}", corpus.fixture);
            assert_eq!(first.stderr, second.stderr, "{}", corpus.fixture);
        }
    }
}

/// The text form: no ID, the note said, the real call's lines after its ID.
#[test]
fn a_dry_runs_text_names_no_id_and_says_nothing_was_stored() {
    let pair = Pair::new("pr-text", "spec-a");
    let text = record("R-13", "links: {derived_from: [R-99]}\n");
    let call = create(R13, &text);
    let dry = call.run(&pair, &pair.linked, false, true);
    dry.code(0);
    assert!(!dry.stdout.contains("PR-"), "{}", dry.show());
    assert!(
        format!("{}{}", dry.stdout, dry.stderr).contains(DRY_NOTE),
        "{}",
        dry.show()
    );
    let real = call.run(&pair, &pair.linked, false, false);
    real.code(0);
    let (head, lines) = real.stdout.split_once('\n').expect("the ID's line");
    assert_eq!(head, "PR-0001", "{}", real.show());
    assert!(lines.starts_with("introduced: 1\n"), "{}", real.show());
    assert!(
        dry.stdout.ends_with(lines),
        "{}\n{}",
        dry.show(),
        real.show()
    );
}

// ------------------------------------------------------------------ AC-02

/// `call` refused alike, real and rehearsed: exit `code`, nothing on stdout,
/// `fragments` in the reason, nothing stored, written or logged by either.
fn assert_real_reason(pair: &Pair, cwd: &Path, call: Call<'_>, code: i32, fragments: &[&str]) {
    let before = pair.state();
    let events = pair.events();
    let real = call.run(pair, cwd, false, false);
    real.code(code);
    let reason = real
        .stderr
        .lines()
        .last()
        .unwrap_or_else(|| panic!("a reason: {}", real.show()))
        .to_owned();
    assert!(reason.starts_with("spec: "), "{}", real.show());
    for fragment in fragments {
        assert!(reason.contains(fragment), "{fragment}: {reason}");
    }
    let dry = call.run(pair, cwd, false, true);
    dry.code(code);
    assert_eq!(dry.stdout, "", "{}", dry.show());
    assert!(
        dry.stderr.lines().any(|line| line == reason),
        "the real reason {reason:?}: {}",
        dry.show()
    );
    if code == 1 {
        // `ProposalOutcome::refused`: the reason is the last note.
        let run = call.run(pair, cwd, true, true);
        run.code(1);
        let document = run.json();
        assert_eq!(document["id"], Value::Null, "{document}");
        assert_eq!(document["status"], Value::Null, "{document}");
        assert_eq!(
            strings(&document["notes"]).last().map(String::as_str),
            reason.strip_prefix("spec: "),
            "{document}"
        );
    }
    assert_eq!(pair.state(), before, "{reason}");
    assert_eq!(pair.events(), events, "{reason}");
}

/// AC-02 on `update` and both forms of `create`. M: returning before step 4
/// (a create's step 4 is the new IDs' check; a taken ID then answers 0).
#[test]
fn ac02_a_taken_id_a_stale_base_and_a_generated_text_give_the_real_reason() {
    let pair = Pair::new("pr-ac02", "spec-a");
    let cwd = pair.linked.clone();
    let target = "RULE-STAM-REGEN";
    let (hash, span) = pair.span(&cwd, target);
    let rate = edit(&span, "Base rate 10 units/s,", "Base rate 11 units/s,");
    let rest = format!("{span}\n\n### Rest delay {{#EDGE-STAM-REST}}\n- Waits 1.5 s.");

    // A taken ID: a new file's, a new section's.
    let taken = record("R-12", "");
    assert_real_reason(
        &pair,
        &cwd,
        create("docs/records/R/R-50.md", &taken),
        1,
        &["R-12", "docs/records/R/R-12.md"],
    );
    let taken = format!("{span}\n\n### Twice {{#EDGE-SPRINT-EMPTY}}\n- Again.");
    assert_real_reason(
        &pair,
        &cwd,
        sections(target, &hash, &taken),
        1,
        &["EDGE-SPRINT-EMPTY"],
    );

    // A stale base: the current hash named.
    assert_real_reason(&pair, &cwd, update(target, STALE, &rate), 1, &[&hash]);
    assert_real_reason(&pair, &cwd, sections(target, STALE, &rest), 1, &[&hash]);

    // `class: generated` in the text.
    let generated = edit(&record("R-13", ""), "class: canon\n", "class: generated\n");
    assert_real_reason(
        &pair,
        &cwd,
        create(R13, &generated),
        1,
        &["class: generated"],
    );

    // Every refusal of the real call, not only the three.
    assert_real_reason(&pair, &cwd, update(target, &hash, &span), 1, &["no change"]);
    let amends = record("R-13", "links: {amends: [RULE-STAM-REGEN]}\n");
    assert_real_reason(&pair, &cwd, create(R13, &amends), 1, &["propose amendment"]);

    // "Rules": `--task` as in the real call.
    let plain = record("R-13", "");
    assert_real_reason(
        &pair,
        &cwd,
        create(R13, &plain).task("T-9999"),
        1,
        &["T-9999"],
    );
    assert_real_reason(
        &pair,
        &cwd,
        update(target, &hash, &rate).task("T-9999"),
        1,
        &["T-9999"],
    );
    assert!(pair.proposals().is_empty(), "nothing stored");

    // Held by a live create: the holder named, a rehearsal refused alike.
    let holder = id_of(&create(R13, &plain).stored(&pair, &cwd));
    assert_real_reason(
        &pair,
        &cwd,
        create("docs/records/R/other.md", &plain),
        1,
        &["R-13", &holder],
    );
}

/// AC-02, `class: generated` as a target: on a command-line tool's generated
/// glossary entry, an update and new sections.
#[test]
fn ac02_a_generated_document_refuses_an_update_and_new_sections() {
    let pair = Pair::new("pr-ac02-generated", "spec-b");
    let cwd = pair.linked.clone();
    let target = "GLS-task-branch";
    let (hash, span) = pair.span(&cwd, target);
    let more = format!("{span}\nMore.\n");
    assert_real_reason(&pair, &cwd, update(target, &hash, &more), 1, &["generated"]);
    let section = format!("{span}\n\n## More {{#GLS-more}}\n\nMore.\n");
    assert_real_reason(
        &pair,
        &cwd,
        sections(target, &hash, &section),
        1,
        &["generated"],
    );
}

/// AC-02, step 5 of an update: what the real call cannot bind, a dry run
/// cannot either (exit 2, the same line). M: returning before the binding.
#[test]
fn ac02_a_detached_head_exits_2_as_the_real_call() {
    let pair = Pair::new("pr-ac02-detached", "spec-a");
    let cwd = pair.linked.clone();
    let target = "RULE-STAM-REGEN";
    let (hash, span) = pair.span(&cwd, target);
    let rate = edit(&span, "Base rate 10 units/s,", "Base rate 11 units/s,");
    let plain = record("R-13", "");
    pair.git.git(&cwd, &["checkout", "-q", "--detach"]);
    assert_real_reason(
        &pair,
        &cwd,
        update(target, &hash, &rate),
        2,
        &["cannot be bound"],
    );
    assert_real_reason(&pair, &cwd, create(R13, &plain), 2, &["cannot be bound"]);
}

/// "Rules": a dry run takes no ID: two agents rehearse one ID at two paths,
/// the first real call stores, the second is refused as today, and so is a
/// rehearsal from then on.
#[test]
fn two_agents_may_rehearse_one_id_and_the_second_real_call_is_refused() {
    let pair = Pair::new("pr-two-agents", "spec-a");
    let cwd = pair.linked.clone();
    let text = record("R-13", "");
    let other = "docs/records/R/other.md";
    let first = create(R13, &text).by("spec-writer");
    let second = create(other, &text).by("architect");
    let before = pair.state();
    for _ in 0..2 {
        first.rehearsed(&pair, &cwd);
        second.rehearsed(&pair, &cwd);
    }
    assert_nothing_stored(&pair, &before, "rehearsals");

    let stored = first.stored(&pair, &cwd);
    assert_eq!(id_of(&stored), "PR-0001", "no ID burnt");
    assert_real_reason(&pair, &cwd, second, 1, &["R-13", "PR-0001"]);
}

// ------------------------------------------------------------------ AC-03

/// AC-03, the CLI's half: D9, `--dry-run` on `update` and `create` only: a
/// clap usage error naming the flag, exit 2, on any other kind. M: the flag
/// accepted and ignored.
#[test]
fn ac03_the_other_kinds_take_no_dry_run_flag() {
    let pair = Pair::new("pr-ac03", "spec-a");
    let before = pair.state();
    for args in [
        vec![
            "propose",
            "interpretation",
            "RULE-STAM-REGEN",
            "--base",
            STALE,
            "--decision",
            "DEC-0023",
            "--how",
            "h",
            "--why",
            "w",
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--dry-run",
        ],
        vec![
            "propose",
            "amendment",
            "docs/features/regen-delay.md",
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--dry-run",
        ],
        vec![
            "propose",
            "question",
            "RULE-STAM-REGEN",
            "--text",
            "t",
            "--working-answer",
            "w",
            "--price-of-other",
            "p",
            "--dry-run",
        ],
        vec!["propose", "drift", "RULE-STAM-REGEN", "--dry-run"],
    ] {
        let run = pair.spec_piped(&pair.linked, &args, b"text\n");
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
        assert!(run.stderr.contains("--dry-run"), "{}", run.show());
        assert_nothing_stored(&pair, &before, args[1]);
    }
}

// ------------------------------------------------------------------ AC-04

/// AC-04 on a game and on a command-line tool: `follows` and `parent` on an
/// ID a live create holds, raised by another agent (D10), real and
/// rehearsed; a reference to nothing keeps its finding beside it. M:
/// filtering on the code alone.
#[test]
fn ac04_follows_and_parent_on_a_live_create_store_no_ref_dangling() {
    const LIVE: [&str; 3] = ["PR-0001", "PR-0002", "PR-0003"];
    for corpus in &CORPORA {
        let context = corpus.fixture;
        let pair = Pair::new("pr-ac04", context);
        let cwd = pair.linked.clone();
        let [(held, path), follower, child, both] = corpus.free;
        let text = (corpus.record)(held, "");
        let holder = id_of(&create(path, &text).by("architect").stored(&pair, &cwd));
        let note = held_note(held, &holder, "open", "create");

        for (label, (id, path), extra) in [
            (
                "follows",
                follower,
                format!("links: {{follows: [{held}]}}\n"),
            ),
            ("parent", child, format!("parent: {held}\n")),
        ] {
            let text = (corpus.record)(id, &extra);
            // spec-b's records share one title: each names the live ones
            // (docs/features/intake-similar.md "Acceptance criteria" IS2;
            // a name no entry has is ignored).
            let call = create(path, &text).naming(&LIVE);
            let rehearsed = call.rehearsed(&pair, &cwd);
            assert_eq!(
                subjects(&rehearsed, "ref-dangling"),
                Vec::<String>::new(),
                "{context}: {label}: {rehearsed}"
            );
            assert_eq!(rehearsed["notes"][0], json!(DRY_NOTE), "{context}");
            assert_eq!(said(&rehearsed, &note), 1, "{context}: {rehearsed}");

            let stored = call.stored(&pair, &cwd);
            assert_eq!(
                subjects(&stored, "ref-dangling"),
                Vec::<String>::new(),
                "{context}: {label}: {stored}"
            );
            assert_eq!(said(&stored, &note), 1, "{context}: {label}: {stored}");
            let row = pair.proposal(&id_of(&stored));
            assert!(
                row.diagnostics
                    .iter()
                    .all(|finding| finding.code != "ref-dangling"),
                "{context}: {label}: stored {:?}",
                row.diagnostics
            );
            assert_eq!(row.status.as_str(), "open", "{context}");
        }

        // The finding of a reference to nothing stays, stored.
        let (id, path) = both;
        let text = (corpus.record)(
            id,
            &format!("links: {{follows: [{held}, {}]}}\n", corpus.missing),
        );
        let stored = create(path, &text).naming(&LIVE).stored(&pair, &cwd);
        assert_eq!(
            subjects(&stored, "ref-dangling"),
            [corpus.missing],
            "{context}: {stored}"
        );
        assert_eq!(said(&stored, &note), 1, "{context}: {stored}");
        let row = pair.proposal(&id_of(&stored));
        assert_eq!(row.diagnostics.len(), 1, "{context}: {:?}", row.diagnostics);
        assert_eq!(row.diagnostics[0].subject, corpus.missing, "{context}");
    }
}

/// AC-04, the text form: `introduced: 0` and the note.
#[test]
fn ac04_the_text_form_says_the_note() {
    let pair = Pair::new("pr-ac04-text", "spec-a");
    let cwd = pair.linked.clone();
    let holder = id_of(&create(R13, &record("R-13", "")).stored(&pair, &cwd));
    let text = record("R-14", "links: {follows: [R-13]}\n");
    let run = create(R14, &text).run(&pair, &cwd, false, false);
    run.code(0);
    assert!(run.stdout.contains("introduced: 0\n"), "{}", run.show());
    let note = held_note("R-13", &holder, "open", "create");
    assert!(
        format!("{}{}", run.stdout, run.stderr).contains(&note),
        "{}",
        run.show()
    );
}

/// AC-04, the update's site: an update adding the link to a document, real
/// and rehearsed; beside a reference to nothing. M: the update's step 4 not
/// filtered.
#[test]
fn ac04_an_update_adding_the_link_stores_no_ref_dangling() {
    let pair = Pair::new("pr-ac04-update", "spec-a");
    let cwd = pair.linked.clone();
    let holder = id_of(&create(R13, &record("R-13", "")).stored(&pair, &cwd));
    let note = held_note("R-13", &holder, "open", "create");
    let target = "MEC-SPRINT";
    let (hash, span) = pair.span(&cwd, target);
    let links = "  depends_on: [MEC-STAMINA]\n";

    let text = edit(
        &span,
        links,
        "  depends_on: [MEC-STAMINA]\n  follows: [R-13]\n",
    );
    let call = update(target, &hash, &text);
    let rehearsed = call.rehearsed(&pair, &cwd);
    assert_eq!(
        subjects(&rehearsed, "ref-dangling"),
        Vec::<String>::new(),
        "{rehearsed}"
    );
    assert_eq!(said(&rehearsed, &note), 1, "{rehearsed}");
    let stored = call.stored(&pair, &cwd);
    assert_eq!(
        subjects(&stored, "ref-dangling"),
        Vec::<String>::new(),
        "{stored}"
    );
    assert_eq!(said(&stored, &note), 1, "{stored}");

    let text = edit(
        &span,
        links,
        "  depends_on: [MEC-STAMINA]\n  follows: [R-13, R-99]\n",
    );
    let stored = update(target, &hash, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), ["R-99"], "{stored}");
    assert_eq!(said(&stored, &note), 1, "{stored}");
}

/// AC-04: A rejected, B applied: the finding returns in `spec check`, and a
/// later dependant stores it: nothing holds the ID any more ("Rules").
#[test]
fn ac04_a_rejected_holder_brings_the_finding_back() {
    let pair = Pair::new("pr-ac04-rejected", "spec-a");
    let cwd = pair.linked.clone();
    let holder = id_of(&create(R13, &record("R-13", "")).stored(&pair, &cwd));
    let text = record("R-14", "links: {follows: [R-13]}\n");
    let follower = id_of(&create(R14, &text).stored(&pair, &cwd));
    assert!(pair.proposal(&follower).diagnostics.is_empty());

    let (outcome, _) = pair.reject_answer(&pair.main, &holder, "Not needed.", true);
    assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    pair.approve_ok(&pair.main, &follower);

    let run = pair.spec_piped(&cwd, &["check"], b"");
    run.code(1);
    let report = format!("{}{}", run.stdout, run.stderr);
    assert!(
        report.lines().any(|line| line.contains("ref-dangling")
            && line.contains("R-13")
            && line.contains(R14)),
        "{}",
        run.show()
    );

    let text = record("R-15", "links: {follows: [R-13]}\n");
    let stored = create(R15, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), ["R-13"], "{stored}");
    assert!(
        !strings(&stored["notes"])
            .iter()
            .any(|note| note.contains("no ref-dangling stored")),
        "{stored}"
    );
}

/// AC-04, what `reserved()` holds (D10): an `approved` create and a live
/// amendment hold alike, each note in its own words. A proposal's own kind
/// is no matter: an amendment that follows a held ID stores no finding
/// either (`validated`, the site it shares with a create).
#[test]
fn ac04_an_approved_create_and_a_live_amendment_hold_alike() {
    let pair = Pair::new("pr-ac04-holders", "spec-a");
    let cwd = pair.linked.clone();
    let holder = id_of(&create(R13, &record("R-13", "")).stored(&pair, &cwd));
    pair.sql(&format!(
        "update proposals set status = 'approved', decided_by = '{DECIDER}', \
         decided_at = '{NOW}', decision_note = 'by hand', updated_at = '{NOW}' \
         where id = '{holder}'"
    ));
    let text = record("R-14", "links: {follows: [R-13]}\n");
    let stored = create(R14, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());
    let note = held_note("R-13", &holder, "approved", "create");
    assert_eq!(said(&stored, &note), 1, "{stored}");

    let amendment = "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
                     [RULE-STAM-REGEN]\n  follows: [R-14]\n---\n\n# Regeneration waits\n\n## \
                     Delay {#AC-01}\n\nRegeneration starts 1.5 s after the last sprint.\n";
    let run = pair.spec_piped(
        &cwd,
        &[
            "--json",
            "propose",
            "amendment",
            "docs/features/regen-delay.md",
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--author-role",
            "spec-author",
        ],
        amendment.as_bytes(),
    );
    run.code(0);
    let amended = run.json();
    assert_eq!(amended["kind"], json!("amendment"), "{amended}");
    assert_eq!(
        subjects(&amended, "ref-dangling"),
        Vec::<String>::new(),
        "{amended}"
    );
    let follower = id_of(&stored);
    let note = held_note("R-14", &follower, "open", "create");
    assert_eq!(said(&amended, &note), 1, "{amended}");

    let by = id_of(&amended);
    let text = record("R-15", "links: {verifies: [regen-delay/AC-01]}\n");
    let stored = create(R15, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());
    let note = held_note("regen-delay/AC-01", &by, "open", "amendment");
    assert_eq!(said(&stored, &note), 1, "{stored}");
}

/// AC-04, "Data": `subject`, cut at `#` and made canonical, is what is
/// compared: a section of a held document, an alias of a held ID, a
/// feature-scoped ID by its slug and bare in its own document. M: the
/// subject compared as written.
#[test]
fn ac04_the_subject_is_cut_at_its_section_and_made_canonical() {
    let pair = Pair::new("pr-ac04-canonical", "spec-a");
    let cwd = pair.linked.clone();

    let holder = id_of(&create(R13, &record("R-13", "")).stored(&pair, &cwd));
    let text = record("R-14", "links: {follows: [R-13#EDGE-PART]}\n");
    let stored = create(R14, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());
    let note = held_note("R-13", &holder, "open", "create");
    assert_eq!(said(&stored, &note), 1, "{stored}");

    // `QST` is an alias prefix of `Q`.
    let question = record("Q-040", "");
    let holder = id_of(&create("docs/records/Q/Q-040.md", &question).stored(&pair, &cwd));
    let text = record("R-15", "links: {depends_on: [QST-040]}\n");
    let stored = create(R15, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());
    let note = held_note("Q-040", &holder, "open", "create");
    assert_eq!(said(&stored, &note), 1, "{stored}");

    // A section of a feature document: held as `slug/ID`.
    let target = "stamina-tuning/AC-07";
    let (hash, span) = pair.span(&cwd, target);
    let text = format!("{span}\n\n#### Rest {{#AC-08}}\n\nA body.");
    let holder = id_of(&sections(target, &hash, &text).stored(&pair, &cwd));
    let note = held_note("stamina-tuning/AC-08", &holder, "open", "create");
    let text = record("R-16", "links: {verifies: [stamina-tuning/AC-08]}\n");
    let stored = create(R16, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());
    assert_eq!(said(&stored, &note), 1, "{stored}");

    // Bare, in its own document.
    let (hash, span) = pair.span(&cwd, TUNING);
    let text = edit(
        &span,
        "priority: high\n",
        "priority: high\nlinks: {depends_on: [AC-08]}\n",
    );
    let stored = update(TUNING, &hash, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());
    assert_eq!(said(&stored, &note), 1, "{stored}");
}

/// AC-04, "Data": one note per holder: one proposal holding two IDs, each
/// referenced, twice over: one note; two proposals: two.
#[test]
fn ac04_one_note_per_holder() {
    let pair = Pair::new("pr-ac04-notes", "spec-a");
    let cwd = pair.linked.clone();
    let text = format!(
        "{}\n## Case {{#EDGE-HELD}}\n\nA case.\n",
        record("R-13", "")
    );
    let first = id_of(&create(R13, &text).stored(&pair, &cwd));
    let second = id_of(&create(R14, &record("R-14", "")).stored(&pair, &cwd));
    let of = |document: &Value, holder: &str| -> Vec<String> {
        strings(&document["notes"])
            .into_iter()
            .filter(|note| note.contains(&format!("`{holder}`")))
            .collect()
    };

    let text = record(
        "R-15",
        "links: {follows: [R-13], depends_on: [R-13, EDGE-HELD]}\n",
    );
    let stored = create(R15, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());
    let notes = of(&stored, &first);
    assert_eq!(notes.len(), 1, "{stored}");
    assert!(notes[0].contains("no ref-dangling stored"), "{stored}");

    let text = record("R-16", "links: {follows: [R-13, R-14]}\n");
    let stored = create(R16, &text).stored(&pair, &cwd);
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());
    assert_eq!(of(&stored, &first).len(), 1, "{stored}");
    assert_eq!(of(&stored, &second).len(), 1, "{stored}");
}
