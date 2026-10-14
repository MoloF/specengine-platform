//! docs/features/kind-amendment.md, AC-T1 (the CLI half of AC-R1 to
//! AC-R10 and AC-R13) over git copies of `fixtures/spec-a`
//! (`RULE-STAM-REGEN` in canon `stamina.md`, `DEC-0007` superseded, `R-12`
//! `immutable_text`, `QST` aliasing `Q`, the live spec `stamina-tuning.md`
//! with `AC-07`), spec-b (a command-line tool, Russian prose, a legacy
//! Cyrillic alias, a `class: generated` record) and the non-Rust spec-c.
//!
//! The queue's commands run through the library with the sandbox's git
//! environment and an injected clock; the binary where its exit code,
//! stdout or stderr is the subject (`spec propose amendment`, `--base`,
//! the decision flags on a terminal, `spec check`). Proposals are raised in
//! the linked worktree `t1` unless said, approved from the main one.
//! "Refused": exit 1 (or the exit named), nothing stored, written or
//! reserved.
//!
//! Mutations (docs/features/kind-amendment.md "Acceptance criteria"): the
//! amended ID in `target_ids` (AC-R2), the class rule dropped (AC-R3), the
//! live test dropped (AC-R4), the apply writing the amended file (AC-R5),
//! no step-4 re-check (AC-R6), completion needing another trailer (AC-R7),
//! `new_file()` left `== ProposalKind::Create` (AC-R8), `lists_proposal`
//! by `target_ids()` alone (AC-R9), the amends-on-create refusal dropped,
//! `amended_refs` ignoring links, the round treating it as deciding.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::path::Path;

use common::decision::flags;
use common::proposal::{
    DECIDER, LATER, NOW, Pair, cannot, edit, json_of, printed, printed_inbox, refused,
};
use common::{read, read_text, replace, write};
use serde_json::{Value, json};
use specengine_cli::{
    AMENDMENT_KIND, AmendmentRequest, CliError, CreateRequest, Exit, ExportStateRequest, Globals,
    ImportStateRequest, Outcome, ProposalOutcome, ProposedText, ReviewRequest, RoundAnswerRequest,
    RoundNewRequest, RoundRun, RoundTerminal, StageBody, StageOutcome, StageRequest, export_state,
    import_state, propose_amendment, propose_amendment_brief, propose_create, render_json,
    render_text, review_brief, round_answer, round_new, stage,
};
use specengine_core::ProjectConfig;
use specengine_model::{AMENDS, LINK_TYPES};
use specengine_store::{ProposalKind, ProposalQueue as _, ProposalStatus, Stage};
use task_common::Tasks;

/// "Data"'s path, its one new ID, the amended node's file.
const PATH: &str = "docs/features/regen-delay.md";
const NEW_ID: &str = "regen-delay/AC-01";
const STAMINA: &str = "docs/spec/movement/stamina.md";
const TUNING: &str = "docs/features/stamina-tuning.md";
const RATIONALE: &str = "Regeneration waits after a sprint.\n\nThe delay counts from the sprint.";
const ROLE: &str = "spec-author";
const MODEL: &str = "claude-opus-5-5";

/// "Data"'s text, whole.
const TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
                    [RULE-STAM-REGEN]\n---\n\n# Regeneration waits after a sprint\n\n## Delay \
                    {#AC-01}\n\nRegeneration starts 1.5 s after the last sprint, not after any \
                    stamina use.\n";

/// "Data"'s text with its `amends` list written `list`.
fn amending(list: &str) -> String {
    edit(
        TEXT,
        "amends: [RULE-STAM-REGEN]",
        &format!("amends: [{list}]"),
    )
}

/// An agent's `spec propose amendment PATH --text-file - --rationale R`
/// from `cwd` at [`NOW`].
fn request(pair: &Pair, cwd: &Path, target: &str, text: &str) -> AmendmentRequest {
    AmendmentRequest {
        target: target.to_owned(),
        text: ProposedText::Given(text.as_bytes().to_vec()),
        rationale: RATIONALE.to_owned(),
        author_role: Some(ROLE.to_owned()),
        author_model: Some(MODEL.to_owned()),
        run: None,
        now: NOW.to_owned(),
        git: pair.git_env(cwd),
    }
}

fn amend(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    text: &str,
    task: Option<&str>,
) -> Result<ProposalOutcome, CliError> {
    propose_amendment(
        &pair.env(cwd),
        &Globals::default(),
        &request(pair, cwd, target, text),
        task,
    )
}

/// An amendment that must be stored: its ID.
fn amended(pair: &Pair, cwd: &Path, target: &str, text: &str) -> String {
    let outcome = amend(pair, cwd, target, text, None)
        .unwrap_or_else(|error| panic!("propose amendment {target}: {error}"));
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "propose amendment {target}: {outcome:?}"
    );
    outcome.document.id.clone().expect("an ID")
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

/// A refused propose (exit 1): its reason; nothing stored, written or
/// reserved, no ID taken.
fn refused_amend(pair: &Pair, cwd: &Path, target: &str, text: &str, context: &str) -> String {
    let before = (pair.state(), reservations(pair), pair.events().len());
    let outcome = amend(pair, cwd, target, text, None);
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
        "{context}: nothing stored, written or reserved"
    );
    reason
}

/// Like [`refused_amend`] for an exit 2: its message.
fn cannot_amend(pair: &Pair, cwd: &Path, target: &str, text: &str, context: &str) -> String {
    let before = (pair.state(), reservations(pair), pair.events().len());
    let message = cannot(&amend(pair, cwd, target, text, None), context);
    assert_eq!(
        (pair.state(), reservations(pair), pair.events().len()),
        before,
        "{context}: nothing stored, written or reserved"
    );
    message
}

/// `git diff-tree --name-status` of `commit`: its lines.
fn name_status(pair: &Pair, commit: &str) -> Vec<String> {
    pair.git_text(
        &pair.main,
        &["diff-tree", "-r", "--no-commit-id", "--name-status", commit],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// The blob of `path` at `commit`, its bytes.
fn blob(pair: &Pair, commit: &str, path: &str) -> Vec<u8> {
    pair.git.git(
        &pair.main,
        &["cat-file", "blob", &format!("{commit}:{path}")],
    )
}

/// Both worktrees' `git status --porcelain`, untracked files listed.
fn assert_clean(pair: &Pair, context: &str) {
    assert_eq!(pair.porcelain(&pair.main), "", "{context}: main");
    assert_eq!(pair.porcelain(&pair.linked), "", "{context}: t1");
}

/// The review document of `id` as `spec review --json` prints it, raw.
fn review_raw(pair: &Pair, id: &str) -> String {
    printed(&pair.review_ok(&pair.main, id)).1
}

/// `(type, step)` of `id`'s `proposal.apply_failed` events.
fn failures(pair: &Pair, id: &str) -> Vec<(String, Option<u64>)> {
    pair.events_of(id)
        .into_iter()
        .filter(|(kind, _)| kind == "proposal.apply_failed")
        .collect()
}

// ------------------------------------------------------------------ AC-R1

/// AC-R1, "Names": the model's `AMENDS` is `amends`, one of `LINK_TYPES`;
/// core's `amended_refs` reads a document's front-matter `amends` values
/// as written, in order (a scope, a section, a revision kept), none when
/// the document declares none or links otherwise; the store's kind is
/// core's constant. M: `amended_refs` ignoring links.
#[test]
fn ac_r1_amended_refs_reads_the_front_matter_as_written_in_order() {
    assert_eq!(AMENDS, "amends");
    assert_eq!(LINK_TYPES.iter().filter(|link| **link == AMENDS).count(), 1);
    assert_eq!(AMENDMENT_KIND, "amendment");
    assert_eq!(ProposalKind::Amendment.as_str(), AMENDMENT_KIND);
    let config =
        ProjectConfig::from_toml(&read_text(&common::fixture("spec-a"), "specengine.toml"))
            .expect("config");
    let written = |text: &str| -> Vec<String> {
        let parsed = specengine_core::parse(PATH, text.as_bytes(), &config.scheme);
        specengine_core::create::amended_refs(&parsed)
            .iter()
            .map(|reference| {
                let mut out = String::new();
                if let Some(scope) = &reference.scope {
                    out.push_str(scope);
                    out.push('/');
                }
                out.push_str(&reference.id);
                if let Some(section) = &reference.section {
                    out.push('#');
                    out.push_str(section);
                }
                if let Some(rev) = reference.rev {
                    out.push_str(&format!("@{rev}"));
                }
                out
            })
            .collect()
    };
    assert_eq!(written(TEXT), ["RULE-STAM-REGEN"]);
    assert_eq!(
        written(&amending(
            "stamina-tuning/AC-07, RULE-STAM-REGEN, MEC-STAMINA#RULE-STAM-REGEN, R-12@2"
        )),
        [
            "stamina-tuning/AC-07",
            "RULE-STAM-REGEN",
            "MEC-STAMINA#RULE-STAM-REGEN",
            "R-12@2"
        ]
    );
    // Another link type, no links, no front-matter: none.
    assert!(written(&edit(TEXT, "amends:", "derived_from:")).is_empty());
    assert!(written(&edit(TEXT, "links:\n  amends: [RULE-STAM-REGEN]\n", "")).is_empty());
    assert!(written("# Title\n\nBody.\n").is_empty());
    // Both links: only the amended.
    assert_eq!(
        written(&edit(
            TEXT,
            "  amends: [RULE-STAM-REGEN]\n",
            "  derived_from: [R-12]\n  amends: [EDGE-STAM-ZERO]\n"
        )),
        ["EDGE-STAM-ZERO"]
    );
}

// ------------------------------------------------------------------ AC-R2

/// AC-R2: the binary stores "Data"'s amendment (exit 0, `PR-0001`,
/// `introduced: 0`, nothing on stderr); the inbox line and the JSON
/// `target_ids` of "Data" (the file and its own new ID, never the amended
/// `RULE-STAM-REGEN`); `reserved()` exactly `regen-delay/AC-01` by
/// `PR-0001`; a create of that ID (the same path, no `amends`) then refused
/// as reserved; `git status --porcelain` empty in both worktrees, every
/// file as before. M (C-R2): the amended ID in `target_ids`.
#[test]
fn ac_r2_propose_stores_pr_0001_holding_only_its_own_new_id() {
    let pair = Pair::new("ka-r2", "spec-a");
    let before = pair.state();
    let run = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "amendment",
            PATH,
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--author-role",
            ROLE,
            "--author-model",
            MODEL,
        ],
        TEXT.as_bytes(),
    );
    assert_eq!(run.code, 0, "{}", run.show());
    assert_eq!(run.stdout, "PR-0001\nintroduced: 0\n");
    assert_eq!(run.stderr, "");
    let after = pair.state();
    assert_eq!(after.main_files, before.main_files);
    assert_eq!(after.linked_files, before.linked_files);
    assert_eq!(after.refs, before.refs);
    assert_clean(&pair, "after propose");

    let stored = pair.proposal("PR-0001");
    assert_eq!(stored.kind, ProposalKind::Amendment);
    assert_eq!(stored.status, ProposalStatus::Open);
    assert_eq!(stored.target_id, PATH);
    assert_eq!(stored.target_path, PATH);
    assert_eq!(
        (stored.base_hash.as_str(), stored.base_text.as_str()),
        ("", "")
    );
    assert_eq!(stored.new_text, TEXT);
    assert_eq!(stored.new_ids, [NEW_ID]);
    assert_eq!(stored.target_ids(), [PATH, NEW_ID]);
    assert_eq!(stored.place.branch, "t1");
    assert_eq!(
        reservations(&pair),
        [(NEW_ID.to_owned(), "PR-0001".to_owned(), "open".to_owned())],
        "only its own new ID, never the amended one"
    );

    let inbox = pair.inbox(&pair.main, false).expect("inbox");
    let (text, json_text) = printed_inbox(&inbox);
    assert_eq!(
        text,
        format!(
            "PR-0001 | amendment | open | {PATH} | t1 | {} | Regeneration waits after a sprint.\n",
            stored.created_at
        )
    );
    let json = json_of(&json_text);
    assert_eq!(json["proposals"][0]["kind"], json!("amendment"), "{json}");
    let review = json_of(&review_raw(&pair, "PR-0001"));
    assert_eq!(review["kind"], json!(AMENDMENT_KIND));
    assert_eq!(review["target_ids"], json!([PATH, NEW_ID]), "{review}");
    assert_eq!(review["base_hash"], Value::Null);
    assert_eq!(review["base_text"], Value::Null);

    // A create of the reserved ID: refused naming its holder.
    let plain = edit(TEXT, "links:\n  amends: [RULE-STAM-REGEN]\n", "");
    let before = (pair.state(), reservations(&pair));
    let outcome = propose_create(
        &pair.env(&pair.main),
        &Globals::default(),
        &CreateRequest {
            target: PATH.to_owned(),
            base: None,
            text: ProposedText::Given(plain.into_bytes()),
            rationale: "A plain spec.".to_owned(),
            author_role: Some(ROLE.to_owned()),
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: pair.git_env(&pair.main),
        },
    );
    let reason = refused(&outcome, "a create of the reserved ID");
    assert!(
        reason.contains(&format!(
            "`{NEW_ID}` is reserved by `PR-0001` (open), a live amendment"
        )),
        "{reason}"
    );
    assert_eq!(
        (pair.state(), reservations(&pair)),
        before,
        "nothing stored"
    );
    assert_clean(&pair, "after the refused create");
}

/// AC-R2 on spec-b and spec-c (genre independence, ADR-0008): spec-b's
/// amendment of `CMD-SYNC` (Russian prose, its criterion prefix `CRIT`)
/// and spec-c's of `FEE-DAY-CAP` (its own prefixes, Python code) are
/// stored with `target_ids` the file and its own new ID, nothing written;
/// each is applied as one commit `A <path>`, the amended file untouched.
#[test]
fn ac_r2_r5_spec_b_and_spec_c_amend_their_own_nodes() {
    let b_text = "---\nclass: spec\nstatus: draft\nscope: [cli]\nlinks:\n  amends: [CMD-SYNC]\n---\n\n\
                  # \u{0421}\u{0438}\u{043d}\u{0445}\u{0440}\u{043e}\u{043d}\u{0438}\u{0437}\u{0430}\u{0446}\u{0438}\u{044f}\n\n\
                  ## \u{041a}\u{0440}\u{0438}\u{0442}\u{0435}\u{0440}\u{0438}\u{0439} {#CRIT-01}\n\n\
                  `sync --all` \u{0441}\u{0432}\u{0435}\u{0440}\u{044f}\u{0435}\u{0442} \u{0432}\u{0441}\u{0435} \u{043a}\u{043e}\u{043f}\u{0438}\u{0438}.\n";
    let c_text = "---\nclass: spec\nstatus: draft\nscope: [fees]\nlinks:\n  amends: [FEE-DAY-CAP]\n---\n\n\
                  # Day cap per dock\n\n## A cap per dock {#CHK-01}\n\nThe day cap counts per dock, not per rider.\n";
    for (fixture, path, text, new_id, node, holder) in [
        (
            "spec-b",
            "docs/features/sync-all.md",
            b_text,
            "sync-all/CRIT-01",
            "CMD-SYNC",
            "docs/spec/cli.md",
        ),
        (
            "spec-c",
            "docs/features/dock-cap.md",
            c_text,
            "dock-cap/CHK-01",
            "FEE-DAY-CAP",
            "docs/spec/fees.md",
        ),
    ] {
        let pair = Pair::new(&format!("ka-r2-{fixture}"), fixture);
        let before = pair.state();
        let id = amended(&pair, &pair.linked, path, text);
        assert_eq!(id, "PR-0001", "{fixture}");
        let after = pair.state();
        assert_eq!(after.main_files, before.main_files, "{fixture}");
        assert_eq!(after.linked_files, before.linked_files, "{fixture}");
        assert_clean(&pair, fixture);
        let stored = pair.proposal(&id);
        assert_eq!(stored.kind, ProposalKind::Amendment, "{fixture}");
        assert_eq!(stored.target_ids(), [path, new_id], "{fixture}");
        assert_eq!(
            reservations(&pair),
            [(new_id.to_owned(), id.clone(), "open".to_owned())],
            "{fixture}"
        );
        let holder_bytes = read(&pair.linked, holder);
        let (outcome, questions) =
            pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
        let outcome = outcome.unwrap_or_else(|error| panic!("{fixture}: approve: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{fixture}: {outcome:?}");
        assert_eq!(
            questions,
            [format!(
                "amends {node} ({holder})\napply {id} to {path} on t1 in {} (new file)? [y/N]",
                pair.linked.display()
            )],
            "{fixture}"
        );
        let head = pair.rev(&pair.linked, "t1");
        assert_eq!(
            name_status(&pair, &head),
            [format!("A\t{path}")],
            "{fixture}"
        );
        assert_eq!(blob(&pair, &head, path), text.as_bytes(), "{fixture}");
        assert_eq!(read(&pair.linked, holder), holder_bytes, "{fixture}");
        assert!(reservations(&pair).is_empty(), "{fixture}");
        assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    }
}

// ------------------------------------------------------------------ AC-R3

/// AC-R3, propose 2 and 3: exit 1, the queue empty: no `amends` (and a
/// list other than `amends`), `class: canon`, no `class:`, `class:
/// generated`, `status: shipped` (and `abandoned`), a decision not
/// `accepted`; an ID target; an indexed file; a path that is no `.md`;
/// `spec propose create` of the "Data" text names `spec propose
/// amendment`. M: the class rule dropped; the amends-on-create refusal
/// dropped.
#[test]
fn ac_r3_text_and_target_refusals_store_nothing() {
    let pair = Pair::new("ka-r3", "spec-a");
    let cwd = pair.linked.clone();
    let p = |name: &str| format!("docs/features/{name}.md");
    let cases: Vec<(String, String, String)> = vec![
        (
            p("no-amends"),
            edit(TEXT, "links:\n  amends: [RULE-STAM-REGEN]\n", ""),
            format!(
                "`{}` declares no `amends` link: name the node it amends in `links: {{amends: \
                 [ID]}}`; a new file amending nothing is `spec propose create`; nothing stored",
                p("no-amends")
            ),
        ),
        (
            p("derived"),
            edit(TEXT, "amends:", "derived_from:"),
            format!(
                "`{}` declares no `amends` link: name the node it amends in `links: {{amends: \
                 [ID]}}`; a new file amending nothing is `spec propose create`; nothing stored",
                p("derived")
            ),
        ),
        (
            p("canon"),
            edit(TEXT, "class: spec", "class: canon"),
            format!(
                "`{}` is `class: canon`: an amendment is applied when its document goes to Tier \
                 3 by status, which only a spec or a decision can; nothing stored",
                p("canon")
            ),
        ),
        (
            p("classless"),
            edit(TEXT, "class: spec\n", ""),
            format!(
                "`{}` declares no `class:`: an amendment is applied when its document goes to \
                 Tier 3 by status, which only a spec or a decision can; nothing stored",
                p("classless")
            ),
        ),
        (
            p("shipped"),
            edit(TEXT, "status: draft", "status: shipped"),
            format!(
                "`{}` is archived (`status: shipped`): an amendment is written live; nothing \
                 stored",
                p("shipped")
            ),
        ),
        (
            p("abandoned"),
            edit(TEXT, "status: draft", "status: abandoned"),
            format!(
                "`{}` is archived (`status: abandoned`): an amendment is written live; nothing \
                 stored",
                p("abandoned")
            ),
        ),
        (
            p("proposed-decision"),
            edit(
                TEXT,
                "class: spec\nstatus: draft",
                "class: decision\nstatus: proposed",
            ),
            // Iteration 2: a decision is live only while `accepted`.
            format!(
                "`{}` is `class: decision` with `status: proposed`: a decision document is live \
                 only while `accepted`, and an amendment is written live; nothing stored",
                p("proposed-decision")
            ),
        ),
        (
            "RULE-STAM-REGEN".to_owned(),
            TEXT.to_owned(),
            "`RULE-STAM-REGEN`: an amendment is a new file: name its .md path; nothing stored"
                .to_owned(),
        ),
        (
            TUNING.to_owned(),
            TEXT.to_owned(),
            format!("`{TUNING}` exists: an amendment is a new file; nothing stored"),
        ),
        (
            "docs/features/regen-delay.txt".to_owned(),
            TEXT.to_owned(),
            "`docs/features/regen-delay.txt`: an amendment is a new file: name its .md path; \
             nothing stored"
                .to_owned(),
        ),
    ];
    for (target, text, want) in &cases {
        let reason = refused_amend(&pair, &cwd, target, text, target);
        assert_eq!(&reason, want, "{target}");
    }
    // `class: generated`: a create's file-form rule answers first.
    let reason = refused_amend(
        &pair,
        &cwd,
        &p("generated"),
        &edit(TEXT, "class: spec", "class: generated"),
        "generated",
    );
    assert!(reason.contains("class: generated"), "{reason}");
    assert!(pair.proposals().is_empty(), "the queue is empty");

    // A create of the "Data" text: refused naming the amendment's command.
    let before = (pair.state(), reservations(&pair));
    let outcome = propose_create(
        &pair.env(&cwd),
        &Globals::default(),
        &CreateRequest {
            target: PATH.to_owned(),
            base: None,
            text: ProposedText::Given(TEXT.as_bytes().to_vec()),
            rationale: RATIONALE.to_owned(),
            author_role: Some(ROLE.to_owned()),
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: pair.git_env(&cwd),
        },
    );
    assert_eq!(
        refused(&outcome, "a create declaring amends"),
        format!(
            "`{PATH}` declares `amends`: raise it as `spec propose amendment {PATH}`; nothing \
             stored"
        )
    );
    assert_eq!((pair.state(), reservations(&pair)), before);

    // The same text as an amendment stores; a decision `accepted` too.
    assert_eq!(amended(&pair, &cwd, PATH, TEXT), "PR-0001", "no ID used");
    let decision = edit(
        &amending("EDGE-STAM-ZERO"),
        "class: spec\nstatus: draft",
        "class: decision\nstatus: accepted",
    );
    let decision = edit(&decision, "{#AC-01}", "{#AC-02}");
    assert_eq!(
        amended(&pair, &cwd, "docs/features/zero-stop.md", &decision),
        "PR-0002"
    );
    assert_clean(&pair, "after the refusals");
}

/// AC-R3, the form (exit 2, clap): `--base` is no flag of `spec propose
/// amendment`; an author outside the grammar is exit 2 too; nothing
/// stored.
#[test]
fn ac_r3_base_and_a_bad_author_are_exit_2() {
    let pair = Pair::new("ka-r3-form", "spec-a");
    let run = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "amendment",
            PATH,
            "--base",
            "b3:00",
            "--text-file",
            "-",
            "--rationale",
            "R",
            "--author-role",
            ROLE,
        ],
        TEXT.as_bytes(),
    );
    assert_eq!(run.code, 2, "{}", run.show());
    assert!(run.stderr.contains("'--base'"), "{}", run.stderr);
    assert_eq!(run.stdout, "");
    let run = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "amendment",
            PATH,
            "--text-file",
            "-",
            "--rationale",
            "R",
            "--author-role",
            "a b",
        ],
        TEXT.as_bytes(),
    );
    assert_eq!(run.code, 2, "{}", run.show());
    assert!(pair.proposals().is_empty());
    assert_clean(&pair, "form");
}

/// "Data"'s text with its `amends` list left unclosed: YAML that does not
/// parse on line 6.
fn broken_text() -> String {
    edit(
        TEXT,
        "amends: [RULE-STAM-REGEN]",
        "amends: [RULE-STAM-REGEN",
    )
}

/// Iteration 2's refusal of a text whose front-matter does not read: the
/// parser's finding, its file and line named.
fn yaml_refusal(path: &str) -> String {
    format!(
        "{path}:6: frontmatter-yaml: front-matter is not valid YAML: unclosed bracket '['; an \
         amendment names the nodes it amends in its front-matter, which does not read"
    )
}

/// Edge data (the test guide), iteration 2: an amendment whose front-matter
/// is broken YAML (an unclosed list in `links:`) is refused (exit 1,
/// nothing stored), naming the file, the line and the parser's finding,
/// not that the text "declares no `amends` link"; the binary prints it as
/// `spec: …` on stderr. M: the front-matter rule dropped.
#[test]
fn edge_a_broken_front_matter_is_refused_naming_the_file_and_line() {
    let pair = Pair::new("ka-edge-yaml", "spec-a");
    let reason = refused_amend(&pair, &pair.linked, PATH, &broken_text(), "broken YAML");
    assert_eq!(reason, format!("{}; nothing stored", yaml_refusal(PATH)));
    let run = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "amendment",
            PATH,
            "--text-file",
            "-",
            "--rationale",
            "R",
            "--author-role",
            ROLE,
        ],
        broken_text().as_bytes(),
    );
    assert_eq!(run.code, 1, "{}", run.show());
    assert_eq!(run.stdout, "");
    assert_eq!(
        run.stderr,
        format!("spec: {}; nothing stored\n", yaml_refusal(PATH))
    );
    // Not-a-mapping and an unclosed block are refused the same way.
    let not_mapping = "---\n- class\n- spec\n---\n\n# Regeneration\n\nBody.\n";
    let reason = refused_amend(&pair, &pair.linked, PATH, not_mapping, "not a mapping");
    assert!(
        reason.starts_with(&format!("{PATH}:")) && reason.contains("frontmatter-not-mapping"),
        "{reason}"
    );
    let unclosed = "---\nclass: spec\nstatus: draft\nlinks:\n  amends: [RULE-STAM-REGEN]\n\n# R\n";
    let reason = refused_amend(&pair, &pair.linked, PATH, unclosed, "unclosed block");
    assert!(
        reason.starts_with(&format!("{PATH}:")) && reason.contains("frontmatter-unclosed"),
        "{reason}"
    );
    assert!(pair.proposals().is_empty());
}

/// Iteration 2, apply step 6 (reachable only through a stored row whose
/// text no longer reads: here `new_text` and its `patch_hash` rewritten in
/// the queue after propose): the review's preview `unavailable` naming step
/// 6 and the parser's finding; approve exits 1, never asks, one
/// `apply_failed` (step 6), `open`, nothing written. M: the front-matter
/// rule dropped (step 6 then names "declares no `amends` link").
#[test]
fn edge_a_stored_text_whose_front_matter_does_not_read_fails_step_6() {
    let pair = Pair::new("ka-edge-step6", "spec-a");
    let id = amended(&pair, &pair.linked, PATH, TEXT);
    let broken = broken_text();
    let hash = specengine_store::patch_hash(PATH, "", &broken);
    pair.sql(&format!(
        "UPDATE proposals SET new_text = '{}', patch_hash = '{hash}' WHERE id = '{id}';",
        broken.replace('\'', "''")
    ));
    assert_eq!(pair.proposal(&id).new_text, broken, "the row reads");
    let why = format!("{}; nothing changed", yaml_refusal(PATH));
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(
        review.document.preview.map(|preview| preview.as_str()),
        Some("unavailable")
    );
    assert!(
        review
            .document
            .notes
            .contains(&format!("not applicable now (step 6): {why}")),
        "{:?}",
        review.document.notes
    );
    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let reason = refused(&outcome, "step 6");
    assert!(
        reason.contains("(step 6)") && reason.contains(&why),
        "{reason}"
    );
    assert!(questions.is_empty(), "never asked: {questions:?}");
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
    assert_eq!(
        failures(&pair, &id),
        [("proposal.apply_failed".to_owned(), Some(6))]
    );
    assert!(!pair.linked.join(PATH).exists());
}

/// Iteration 2 (the create's verdict, ADR-0012, pinned as today): `spec
/// propose create` of a new file whose front-matter is broken YAML is
/// stored with its finding (`introduced: 1`, `frontmatter-yaml` at line 6),
/// a findings never refusing; so is one whose broken front-matter writes
/// `amends:`, which does not read and so declares nothing.
#[test]
fn edge_a_create_of_a_broken_front_matter_is_stored_with_its_finding() {
    let pair = Pair::new("ka-edge-create", "spec-a");
    let derived = edit(
        TEXT,
        "  amends: [RULE-STAM-REGEN]\n",
        "  derived_from: [RULE-STAM-REGEN\n",
    );
    for (number, (path, text)) in [
        ("docs/features/broken-derived.md", derived),
        ("docs/features/broken-amends.md", broken_text()),
    ]
    .into_iter()
    .enumerate()
    {
        let run = pair.spec_piped(
            &pair.linked,
            &[
                "propose",
                "create",
                path,
                "--text-file",
                "-",
                "--rationale",
                "R",
                "--author-role",
                ROLE,
            ],
            text.as_bytes(),
        );
        assert_eq!(run.code, 0, "{path}: {}", run.show());
        let id = format!("PR-{:04}", number + 1);
        assert!(
            run.stdout.starts_with(&format!("{id}\nintroduced: 1\n")),
            "{path}: {}",
            run.show()
        );
        let finding = format!(
            "error  {path}:6: frontmatter-yaml: front-matter is not valid YAML: unclosed bracket \
             '['"
        );
        assert!(
            run.stdout.contains(&finding) || run.stderr.contains(&finding),
            "{path}: {}",
            run.show()
        );
        let stored = pair.proposal(&id);
        assert_eq!(stored.kind, ProposalKind::Create, "{path}");
        assert_eq!(
            stored
                .diagnostics
                .iter()
                .map(|finding| (finding.code.as_str(), finding.line))
                .collect::<Vec<_>>(),
            [("frontmatter-yaml", 6)],
            "{path}"
        );
    }
    assert_clean(&pair, "nothing written");
}

/// Iteration 2: a `class: decision` amendment whose `status:` is not
/// `accepted` (`proposed`, `superseded-by DEC-0023`) is refused naming the
/// decision's liveness; a spec `shipped` or `abandoned` keeps `is
/// archived` (`ac_r3_text_and_target_refusals_store_nothing`); a decision
/// without `status:` is not Tier 3 and is stored. M: the old text.
#[test]
fn iteration2_a_decision_amendment_is_live_only_while_accepted() {
    let pair = Pair::new("ka-decision-live", "spec-a");
    let cwd = pair.linked.clone();
    for (name, status) in [
        ("proposed-note", "proposed"),
        ("superseded-note", "superseded-by DEC-0023"),
    ] {
        let path = format!("docs/features/{name}.md");
        let text = edit(
            TEXT,
            "class: spec\nstatus: draft",
            &format!("class: decision\nstatus: {status}"),
        );
        let reason = refused_amend(&pair, &cwd, &path, &text, status);
        assert_eq!(
            reason,
            format!(
                "`{path}` is `class: decision` with `status: {status}`: a decision document is \
                 live only while `accepted`, and an amendment is written live; nothing stored"
            ),
            "{status}"
        );
    }
    let bare = edit(TEXT, "class: spec\nstatus: draft", "class: decision");
    assert_eq!(
        amended(&pair, &cwd, "docs/features/bare-decision.md", &bare),
        "PR-0001"
    );
}

/// Iteration 2: a reservation refusal names its holder's kind: a create of
/// an amendment's new ID is refused `…, a live amendment`, an amendment of
/// a create's new ID `…, a live create`, each with the holder's state
/// (`approved` too); the store's `Reserved` says the same. M: the kind
/// ignored (CLI `reserved_by`; the store's `Display`; the reservation's
/// kind).
#[test]
fn iteration2_reservation_refusals_name_the_holders_kind() {
    let pair = Pair::new("ka-held-kind", "spec-a");
    let cwd = pair.linked.clone();
    let plain = |section: &str| {
        edit(
            &edit(TEXT, "links:\n  amends: [RULE-STAM-REGEN]\n", ""),
            "{#AC-01}",
            &format!("{{#{section}}}"),
        )
    };
    let create = |path: &str, text: &str| {
        propose_create(
            &pair.env(&cwd),
            &Globals::default(),
            &CreateRequest {
                target: path.to_owned(),
                base: None,
                text: ProposedText::Given(text.as_bytes().to_vec()),
                rationale: "A plain spec.".to_owned(),
                author_role: Some(ROLE.to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: pair.git_env(&cwd),
            },
        )
    };
    // PR-0001, an amendment, holds `regen-delay/AC-01`.
    assert_eq!(amended(&pair, &cwd, PATH, TEXT), "PR-0001");
    let reason = refused(
        &create(PATH, &plain("AC-01")),
        "a create of the amendment's ID",
    );
    assert!(
        reason.starts_with(&format!(
            "`{NEW_ID}` is reserved by `PR-0001` (open), a live amendment"
        )),
        "{reason}"
    );
    // PR-0002, a create, holds `held/AC-01`.
    let outcome = create("docs/features/held.md", &plain("AC-01")).expect("create");
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0002"),
        "{outcome:?}"
    );
    let reason = refused_amend(
        &pair,
        &cwd,
        "docs/features/held.md",
        TEXT,
        "an amendment of the create's ID",
    );
    assert!(
        reason.starts_with("`held/AC-01` is reserved by `PR-0002` (open), a live create"),
        "{reason}"
    );
    // Approved, the holder's state follows.
    pair.queue()
        .approve(
            "PR-0001",
            &specengine_store::Decision {
                decided_by: DECIDER.to_owned(),
                note: None,
                staged_at: None,
            },
            LATER,
        )
        .expect("approved");
    let reason = refused(&create(PATH, &plain("AC-01")), "approved holder");
    assert!(
        reason.starts_with(&format!(
            "`{NEW_ID}` is reserved by `PR-0001` (approved), a live amendment"
        )),
        "{reason}"
    );
    let kinds: Vec<(String, ProposalKind)> = pair
        .queue()
        .reserved()
        .expect("reserved")
        .into_iter()
        .map(|held| (held.id, held.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            (NEW_ID.to_owned(), ProposalKind::Amendment),
            ("held/AC-01".to_owned(), ProposalKind::Create)
        ]
    );
    // The store's error names the kind it is given.
    for (kind, word) in [
        (ProposalKind::Create, "create"),
        (ProposalKind::Amendment, "amendment"),
    ] {
        let error = specengine_store::QueueError::Reserved {
            id: "x/AC-01".to_owned(),
            by: "PR-0009".to_owned(),
            kind,
        };
        assert_eq!(
            error.to_string(),
            format!("`x/AC-01` is reserved by `PR-0009`, a live {word}")
        );
    }
}

/// Iteration 2 (`Listing`: the amended IDs read once, after the bound task
/// and the targets): the packages and the round list as before. T-0001
/// wants `RULE-STAM-REGEN`, T-0002 `MEC-SPRINT`; PR-0001 amends
/// `RULE-STAM-REGEN` (unbound), PR-0002 amends `MEC-SPRINT` bound to
/// T-0001, PR-0003 a question on `RULE-STAM-REGEN`, PR-0004 an update of
/// `EDGE-STAM-ZERO`: T-0001 lists PR-0001 (amended), PR-0002 (bound),
/// PR-0003 (target); T-0002 lists PR-0002 (amended); the round's one item,
/// PR-0003, is waited on by T-0001 alone; the three change proposals are
/// noted. Twice, the same.
#[test]
fn iteration2_listings_are_unchanged_by_the_lazy_read() {
    let pair = Pair::new("ka-listing", "spec-a");
    let main = pair.main.clone();
    let regen = pair.new_ok(&main, &["RULE-STAM-REGEN"]);
    pair.plan_ok(&main, &regen, &[], &[]);
    pair.approve_task(&main, &regen);
    let sprint = pair.new_ok(&main, &["MEC-SPRINT"]);
    pair.plan_ok(&main, &sprint, &[], &[]);
    pair.approve_task(&main, &sprint);
    assert_eq!(amended(&pair, &main, PATH, TEXT), "PR-0001");
    let bound = propose_amendment(
        &pair.env(&main),
        &Globals::default(),
        &request(
            &pair,
            &main,
            "docs/features/sprint-note.md",
            &edit(&amending("MEC-SPRINT"), "{#AC-01}", "{#AC-02}"),
        ),
        Some(&regen),
    )
    .expect("bound");
    assert_eq!(bound.document.id.as_deref(), Some("PR-0002"), "{bound:?}");
    let question = pair.raise_question(
        &main,
        &["RULE-STAM-REGEN"],
        "Does regeneration wait for the end of a sprint?",
        "Yes: 1.5 s after it.",
        "A rebalance of the delay.",
    );
    assert_eq!(question, "PR-0003");
    let update = pair.propose_edit(
        &main,
        "EDGE-STAM-ZERO",
        "is applied immediately.",
        "is applied at once.",
    );
    assert_eq!(update, "PR-0004");
    let listed = |task: &str| -> Vec<String> {
        pair.package(&main, task)["open_proposals"]
            .as_array()
            .expect("open_proposals")
            .iter()
            .map(|entry| entry["id"].as_str().unwrap_or_default().to_owned())
            .collect()
    };
    let sheet = || {
        let sheet = round_new(
            &pair.env(&main),
            &Globals::default(),
            &RoundNewRequest {
                limit: None,
                git: pair.git_env(&main),
            },
        )
        .expect("round new");
        let outcome = Outcome::RoundSheet(Box::new(sheet));
        (render_text(&outcome), json_of(&render_json(&outcome)))
    };
    for pass in 0..2 {
        assert_eq!(listed(&regen), ["PR-0001", "PR-0002", "PR-0003"], "{pass}");
        assert_eq!(listed(&sprint), ["PR-0002"], "{pass}");
        let (text, json) = sheet();
        let items = json["items"].as_array().expect("items");
        assert_eq!(items.len(), 1, "{json}");
        assert_eq!(items[0]["id"], json!("PR-0003"));
        let waiting: Vec<&str> = items[0]["tasks"]
            .as_array()
            .expect("tasks")
            .iter()
            .map(|task| task["id"].as_str().expect("an id"))
            .collect();
        assert_eq!(waiting, [regen.as_str()], "{pass}");
        assert!(
            text.contains(
                "note: 3 open change proposal(s) not on this sheet: PR-0001, PR-0002, PR-0004; \
                 spec inbox lists them"
            ),
            "{text}"
        );
    }
}

/// "Elsewhere", Create: the section form and `update` do not judge
/// `amends`: an update of a spec document (its path, the whole file)
/// adding `links: {amends: [...]}` is stored as an update.
#[test]
fn an_update_adding_amends_is_not_judged() {
    let pair = Pair::new("ka-update", "spec-a");
    let (hash, text) = pair.span(&pair.linked, TUNING);
    let new_text = edit(
        &text,
        "refs: [R-12, QST-031]\n",
        "refs: [R-12, QST-031]\nlinks:\n  amends: [RULE-STAM-REGEN]\n",
    );
    let outcome = pair
        .propose(&pair.linked, TUNING, &hash, &new_text)
        .expect("propose update");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(pair.proposal("PR-0001").kind, ProposalKind::Update);
}

// ------------------------------------------------------------------ AC-R4

/// AC-R4, propose 5: each amended ID judged in written order over the
/// index refreshed now; nothing stored: an alias, `ID#SECTION`, a bare
/// feature-scoped ID, `@rev` (exit 1 naming the canonical form); a
/// Cyrillic look-alike, `project:` (exit 2 with the Latin fix); archived,
/// unknown, a reserved ID, defined by the text, named twice (exit 1, ``amends
/// `X`: <why>; nothing stored``); `R-12` (`immutable_text`) and the live
/// `stamina-tuning/AC-07` stored. M: the live test dropped.
#[test]
fn ac_r4_amended_ids_are_canonical_live_and_named_once() {
    let pair = Pair::new("ka-r4", "spec-a");
    let cwd = pair.linked.clone();
    // A live create holding `other/AC-01` (a reserved ID is no node).
    let outcome = propose_create(
        &pair.env(&cwd),
        &Globals::default(),
        &CreateRequest {
            target: "docs/features/other.md".to_owned(),
            base: None,
            text: ProposedText::Given(
                b"---\nclass: spec\nstatus: draft\n---\n\n# Other\n\n## One {#AC-01}\n\nA.\n"
                    .to_vec(),
            ),
            rationale: "Other.".to_owned(),
            author_role: Some(ROLE.to_owned()),
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: pair.git_env(&cwd),
        },
    )
    .expect("create");
    assert_eq!(outcome.document.id.as_deref(), Some("PR-0001"));
    let lookalike = "RULE-ST\u{0410}M-REGEN";
    let refusals: Vec<(String, String)> = vec![
        (
            "QST-031".to_owned(),
            "amends: `QST-031` is a legacy alias: a proposal names the canonical ID `Q-031`; \
             nothing stored"
                .to_owned(),
        ),
        (
            "MEC-STAMINA#RULE-STAM-REGEN".to_owned(),
            "amends: `MEC-STAMINA#RULE-STAM-REGEN` names a section through its document: a \
             proposal names the section's own ID `RULE-STAM-REGEN`; nothing stored"
                .to_owned(),
        ),
        (
            "AC-07".to_owned(),
            "amends `AC-07`: an amendment names the canonical ID `stamina-tuning/AC-07`; nothing \
             stored"
                .to_owned(),
        ),
        (
            "RULE-STAM-REGEN@3".to_owned(),
            "amends: `RULE-STAM-REGEN@3` carries a revision: a proposal names the bare ID \
             `RULE-STAM-REGEN` (`--base` pins the text); nothing stored"
                .to_owned(),
        ),
        (
            "DEC-0007".to_owned(),
            "amends `DEC-0007`: DEC-0007 is archived (docs/records/DEC/DEC-0007.md, status: \
             superseded-by DEC-0023); nothing stored"
                .to_owned(),
        ),
        (
            "RULE-NOPE".to_owned(),
            "amends `RULE-NOPE`: no node RULE-NOPE; nothing stored".to_owned(),
        ),
        (
            "other/AC-01".to_owned(),
            "amends `other/AC-01`: no node other/AC-01; nothing stored".to_owned(),
        ),
        (
            NEW_ID.to_owned(),
            format!("amends `{NEW_ID}`: {NEW_ID} is defined by this text; nothing stored"),
        ),
        (
            "AC-01".to_owned(),
            "amends `AC-01`: AC-01 is defined by this text; nothing stored".to_owned(),
        ),
        (
            "RULE-STAM-REGEN, RULE-STAM-REGEN".to_owned(),
            "amends `RULE-STAM-REGEN`: RULE-STAM-REGEN is named twice; nothing stored".to_owned(),
        ),
        // Written order: the first refusal answers.
        (
            "RULE-STAM-REGEN, RULE-NOPE, QST-031".to_owned(),
            "amends `RULE-NOPE`: no node RULE-NOPE; nothing stored".to_owned(),
        ),
    ];
    for (list, want) in &refusals {
        let reason = refused_amend(&pair, &cwd, PATH, &amending(list), list);
        assert_eq!(&reason, want, "amends: [{list}]");
    }
    for (list, fix) in [
        (lookalike, "write `RULE-STAM-REGEN`"),
        (
            "lantern-keep:RULE-STAM-REGEN",
            "drop the qualifier to read this project's node",
        ),
    ] {
        let message = cannot_amend(&pair, &cwd, PATH, &amending(list), list);
        assert!(
            message.starts_with("spec: amends: ") && message.contains(fix),
            "{list:?}: {message}"
        );
    }
    assert_eq!(pair.proposals().len(), 1, "only the create is stored");
    // `R-12` (`immutable_text`: nothing writes it) and a live spec's
    // criterion are stored.
    assert_eq!(amended(&pair, &cwd, PATH, &amending("R-12")), "PR-0002");
    assert_eq!(
        amended(
            &pair,
            &cwd,
            "docs/features/tuning-note.md",
            &edit(
                &amending("stamina-tuning/AC-07, Q-031"),
                "{#AC-01}",
                "{#AC-03}"
            ),
        ),
        "PR-0003"
    );
    assert_clean(&pair, "after the refusals");
}

/// AC-R4 on spec-b: a legacy Cyrillic alias (`\u{0422}\u{0420}\u{0411}-002`,
/// all one script) is refused naming the canonical `REQ-002`; a record of
/// `class: generated` (`GLS-task-branch`) is refused `is generated`; the
/// live `REQ-002` is stored.
#[test]
fn ac_r4_spec_b_names_the_canonical_id_and_refuses_a_generated_node() {
    let pair = Pair::new("ka-r4-b", "spec-b");
    let cwd = pair.linked.clone();
    let path = "docs/features/sync-note.md";
    let text = |list: &str| {
        format!(
            "---\nclass: spec\nstatus: draft\nscope: [cli]\nlinks:\n  amends: [{list}]\n---\n\n\
             # \u{0417}\u{0430}\u{043c}\u{0435}\u{0442}\u{043a}\u{0430}\n\n\u{0422}\u{0435}\u{043a}\u{0441}\u{0442}.\n"
        )
    };
    let alias = "\u{0422}\u{0420}\u{0411}-002";
    let reason = refused_amend(&pair, &cwd, path, &text(alias), "alias");
    assert!(
        reason.starts_with("amends") && reason.contains("`REQ-002`"),
        "{reason}"
    );
    let reason = refused_amend(&pair, &cwd, path, &text("GLS-task-branch"), "generated");
    assert_eq!(
        reason,
        "amends `GLS-task-branch`: GLS-task-branch is generated \
         (docs/records/GLS/GLS-task-branch.md); nothing stored"
    );
    let id = amended(&pair, &cwd, path, &text("REQ-002"));
    assert_eq!(id, "PR-0001");
    // A file without new IDs is named by its path, holding none.
    assert_eq!(pair.proposal(&id).target_ids(), [path]);
    assert!(reservations(&pair).is_empty());
}

// ------------------------------------------------------------------ AC-R5

/// AC-R5: approve asks "Data"'s consent (`amends RULE-STAM-REGEN
/// (docs/spec/movement/stamina.md)`, then the new file's question); `y`
/// prints `applied PR-0001 as <sha>`; the commit is one, one parent,
/// exactly `A docs/features/regen-delay.md` (`git show --name-status`),
/// its blob `new_text`, an update's four trailers and no other;
/// `stamina.md` byte-identical in both worktrees and in the commit's
/// tree; `reserved()` empty; `spec check` names no `ref-dangling` in the
/// new file. M (C-R1): the apply writes the amended file too.
#[test]
fn ac_r5_approve_adds_only_the_new_file() {
    let pair = Pair::new("ka-r5", "spec-a");
    let id = amended(&pair, &pair.linked, PATH, TEXT);
    let stamina = (read(&pair.main, STAMINA), read(&pair.linked, STAMINA));
    let base = pair.rev(&pair.linked, "HEAD");
    let base_commit = pair.proposal(&id).place.base_commit;
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("approve: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "amends RULE-STAM-REGEN ({STAMINA})\napply PR-0001 to {PATH} on t1 in {} (new file)? \
             [y/N]",
            pair.linked.display()
        )]
    );
    let head = pair.rev(&pair.linked, "t1");
    // "Data"'s `applied PR-0001 as <sha>`, the branch after it as every
    // apply prints it.
    assert_eq!(
        printed(&outcome).0,
        format!("applied PR-0001 as {head} on t1\n")
    );
    assert_eq!(
        pair.git_text(&pair.main, &["rev-list", "--parents", "-n", "1", &head]),
        format!("{head} {base}"),
        "one commit, one parent"
    );
    assert_eq!(
        pair.git_text(&pair.main, &["show", "--name-status", "--format=", &head]),
        format!("A\t{PATH}")
    );
    assert_eq!(name_status(&pair, &head), [format!("A\t{PATH}")]);
    assert_eq!(
        blob(&pair, &head, PATH),
        TEXT.as_bytes(),
        "its blob is new_text"
    );
    assert_eq!(
        pair.git_text(&pair.main, &["log", "-1", "--format=%B", &head]),
        format!(
            "spec: apply PR-0001\n\n{RATIONALE}\n\nProposal: PR-0001\nDecided-by: {DECIDER}\n\
             Proposed-by: agent role={ROLE} model={MODEL} run=unknown\nBase-commit: {base_commit}"
        ),
        "an update's four trailers, no other"
    );
    assert_eq!(
        (read(&pair.main, STAMINA), read(&pair.linked, STAMINA)),
        stamina,
        "the amended file is never written"
    );
    assert_eq!(blob(&pair, &head, STAMINA), stamina.1, "nor committed");
    assert_eq!(read(&pair.linked, PATH), TEXT.as_bytes());
    assert_clean(&pair, "after the apply");
    assert!(reservations(&pair).is_empty(), "reserved() empty");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    // The new node resolves; `spec check` names no dangling reference in it.
    let shown = pair.node(&pair.linked, NEW_ID);
    assert_eq!(shown.path, PATH);
    let run = pair.spec_piped(&pair.linked, &["check"], b"");
    for line in run.stdout.lines().chain(run.stderr.lines()) {
        assert!(
            !(line.contains(PATH) && line.contains("ref-dangling")),
            "{line}"
        );
    }
}

/// AC-R5, "Consent": two amended nodes, each named with its holder in the
/// text's order before the question; both files untouched. `n` writes
/// nothing and keeps it open.
#[test]
fn ac_r5_the_consent_names_each_amended_node_in_order() {
    let pair = Pair::new("ka-r5-two", "spec-a");
    let text = amending("stamina-tuning/AC-07, RULE-STAM-REGEN");
    let id = amended(&pair, &pair.linked, PATH, &text);
    let question = format!(
        "amends stamina-tuning/AC-07 ({TUNING})\namends RULE-STAM-REGEN ({STAMINA})\napply \
         PR-0001 to {PATH} on t1 in {} (new file)? [y/N]",
        pair.linked.display()
    );
    let before = pair.state();
    let (outcome, questions) =
        pair.approve_answer(&pair.main, &id, false, pair.git_env(&pair.main));
    assert_eq!(questions, std::slice::from_ref(&question));
    let outcome = outcome.expect("declined");
    assert_ne!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(pair.state(), before, "declined: nothing written");
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
    let held = (read(&pair.linked, TUNING), read(&pair.linked, STAMINA));
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    assert_eq!(outcome.expect("approve").exit(), Exit::Answered);
    assert_eq!(questions, [question]);
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(name_status(&pair, &head), [format!("A\t{PATH}")]);
    assert_eq!(
        (read(&pair.linked, TUNING), read(&pair.linked, STAMINA)),
        held
    );
}

// ------------------------------------------------------------------ AC-R6

/// AC-R6, apply step 4: the amended `stamina-tuning/AC-07`'s document
/// committed `status: shipped` (or the criterion removed) after propose,
/// in the recorded worktree: the review's preview `unavailable` naming
/// step 4; approve exits 1, never asks, one `apply_failed` (step 4),
/// `open`, nothing written. M: no re-check.
#[test]
fn ac_r6_an_amended_node_no_longer_live_fails_step_4() {
    for (label, change, why) in [
        (
            "shipped",
            ("status: draft\n", "status: shipped\n"),
            format!(
                "amends `stamina-tuning/AC-07`: stamina-tuning/AC-07 is archived ({TUNING}, \
                 status: shipped)"
            ),
        ),
        (
            "removed",
            ("{#AC-07}", "{#AC-08}"),
            "amends `stamina-tuning/AC-07`: no node stamina-tuning/AC-07".to_owned(),
        ),
    ] {
        let pair = Pair::new(&format!("ka-r6-{label}"), "spec-a");
        let id = amended(&pair, &pair.linked, PATH, &amending("stamina-tuning/AC-07"));
        replace(&pair.linked, TUNING, change.0, change.1);
        pair.commit_all(&pair.linked, "The tuning moves on.");
        let review = pair.review_ok(&pair.main, &id);
        assert_eq!(
            review.document.preview.map(|preview| preview.as_str()),
            Some("unavailable"),
            "{label}"
        );
        assert!(
            review
                .document
                .notes
                .iter()
                .any(|note| note.starts_with(&format!("not applicable now (step 4): {why}"))),
            "{label}: {:?}",
            review.document.notes
        );
        let before = pair.state();
        let (outcome, questions) =
            pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
        let reason = refused(&outcome, label);
        assert!(
            reason.contains("(step 4)") && reason.contains(&why),
            "{label}: {reason}"
        );
        assert!(questions.is_empty(), "{label}: never asked: {questions:?}");
        assert_eq!(pair.state(), before, "{label}: nothing written");
        assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
        assert_eq!(
            failures(&pair, &id),
            [("proposal.apply_failed".to_owned(), Some(4))],
            "{label}"
        );
        assert!(!pair.linked.join(PATH).exists(), "{label}");
    }
}

// ------------------------------------------------------------------ AC-R7

/// AC-R7: `--option 0`, `--answer x`, `--canon y` on an amendment: exit 2
/// naming the kind, never asked, no event, nothing changed; the binary on
/// a terminal alike.
#[test]
fn ac_r7_a_decision_flag_on_an_amendment_is_exit_2() {
    let pair = Pair::new("ka-r7-flags", "spec-a");
    let id = amended(&pair, &pair.linked, PATH, TEXT);
    let before = (pair.state(), pair.events().len());
    for (flag, given) in [
        ("--option", flags(Some(0), None, None)),
        ("--answer", flags(None, Some("x"), None)),
        ("--canon", flags(None, None, Some("y"))),
    ] {
        let (outcome, questions) = pair.decide(&pair.main, &id, &given, None, true);
        let message = cannot(&outcome, flag);
        assert_eq!(
            message,
            format!(
                "spec: `{flag}` decides a `question`, `discrepancy` or `decision`; `{id}` is an \
                 amendment: `spec approve {id}` applies it as proposed; nothing changed"
            )
        );
        assert!(questions.is_empty(), "{flag}");
        assert_eq!(
            (pair.state(), pair.events().len()),
            before,
            "{flag}: no event"
        );
    }
    for args in [
        vec!["approve", id.as_str(), "--option", "0"],
        vec!["approve", id.as_str(), "--answer", "x"],
        vec!["approve", id.as_str(), "--canon", "y"],
    ] {
        let run = pair.spec_pty(&pair.main, &args, None);
        assert_eq!(run.code, 2, "{args:?}: {}", run.output);
        assert!(!run.asked, "{args:?}: never asked: {}", run.output);
        assert!(
            run.output.contains(&format!("`{id}` is an amendment")),
            "{args:?}: {}",
            run.output
        );
    }
    assert_eq!((pair.state(), pair.events().len()), before);
}

/// AC-R7, completion: the new file committed by hand with `--trailer
/// 'Proposal: PR-0001'` alone: `spec approve` completes it with that
/// commit, no second commit, no failure; the amended nodes are not checked
/// again. M: completion needing another trailer.
#[test]
fn ac_r7_a_hand_commit_with_proposal_completes_it() {
    let pair = Pair::new("ka-r7-complete", "spec-a");
    let id = amended(&pair, &pair.linked, PATH, TEXT);
    write(&pair.linked, PATH, TEXT);
    pair.git.git(&pair.linked, &["add", "--", PATH]);
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "-m",
            "By hand.",
            "--trailer",
            &format!("Proposal: {id}"),
        ],
    );
    let hand = pair.rev(&pair.linked, "HEAD");
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap_or_else(|error| panic!("approve: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "complete {id} by its commit {hand} on t1 in {}? [y/N]",
            pair.linked.display()
        )],
        "an open proposal's completion asks once"
    );
    assert!(
        outcome
            .messages
            .iter()
            .map(|message| message.line())
            .any(|line| line
                == format!("note: `{id}` completed by its commit {hand} on `t1`; no new commit")),
        "{:?}",
        outcome.messages
    );
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied, "{outcome:?}");
    assert_eq!(stored.applied_commit.as_deref(), Some(hand.as_str()));
    assert_eq!(pair.rev(&pair.linked, "t1"), hand, "no second commit");
    assert!(failures(&pair, &id).is_empty(), "{:?}", pair.events_of(&id));
    assert!(reservations(&pair).is_empty());
}

/// AC-R7: `spec reject` settles an amendment and frees
/// `regen-delay/AC-01`: a create of it is then stored.
#[test]
fn ac_r7_reject_frees_the_reservation() {
    let pair = Pair::new("ka-r7-reject", "spec-a");
    let id = amended(&pair, &pair.linked, PATH, TEXT);
    assert_eq!(reservations(&pair).len(), 1);
    let (outcome, _) = pair.reject_answer(&pair.main, &id, "Absorb it in the rule.", true);
    assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Rejected);
    assert!(reservations(&pair).is_empty(), "freed");
    let plain = edit(TEXT, "links:\n  amends: [RULE-STAM-REGEN]\n", "");
    let created = propose_create(
        &pair.env(&pair.linked),
        &Globals::default(),
        &CreateRequest {
            target: PATH.to_owned(),
            base: None,
            text: ProposedText::Given(plain.into_bytes()),
            rationale: "A plain spec.".to_owned(),
            author_role: Some(ROLE.to_owned()),
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: pair.git_env(&pair.linked),
        },
    )
    .expect("create");
    assert_eq!(
        created.document.id.as_deref(),
        Some("PR-0002"),
        "{created:?}"
    );
    assert_eq!(
        reservations(&pair),
        [(NEW_ID.to_owned(), "PR-0002".to_owned(), "open".to_owned())]
    );
}

// ------------------------------------------------------------------ AC-R8

/// The owner's terminal of a library round: the keys answer in order.
struct Keys {
    keys: Vec<Option<bool>>,
    asked: Vec<String>,
    reports: Vec<String>,
}

impl RoundTerminal for Keys {
    fn report(&mut self, text: &str) {
        self.reports.push(text.to_owned());
    }

    fn ask(&mut self, question: &str) -> Option<bool> {
        let index = self.asked.len();
        self.asked.push(question.to_owned());
        self.keys.get(index).copied().flatten()
    }

    fn tell(&mut self, _line: &str) {}
}

/// `spec round answer [FILE]` (none: `--staged`) from the main worktree.
fn round_with(pair: &Pair, text: Option<&str>) -> (RoundRun, Keys) {
    let file = text.map(|text| {
        let path = pair.scratch.dir("answers").join("a.md");
        std::fs::write(&path, text).expect("write the answers");
        path
    });
    let mut terminal = Keys {
        keys: Vec::new(),
        asked: Vec::new(),
        reports: Vec::new(),
    };
    let run = round_answer(
        &pair.env(&pair.main),
        &Globals::default(),
        &RoundAnswerRequest {
            file,
            now: "2026-10-06T10:00:00Z".to_owned(),
            git: pair.git_env(&pair.main),
        },
        &mut terminal,
    )
    .unwrap_or_else(|error| panic!("round answer: {error}"));
    (run, terminal)
}

/// The library's stage of `id` from the main worktree.
fn stage_of(
    pair: &Pair,
    id: &str,
    option: Option<u64>,
    answer: Option<&str>,
    canon: Option<&str>,
) -> StageOutcome {
    stage(
        &pair.env(&pair.main),
        &Globals::default(),
        &StageRequest {
            id: id.to_owned(),
            body: StageBody::Approve {
                option,
                answer: answer.map(str::to_owned),
                canon: canon.map(str::to_owned),
                editorial: None,
                note: None,
                updated_at: pair.proposal(id).updated_at,
            },
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("stage {id}: {error}"))
}

/// AC-R8: the stage's `option`, `answer`, `canon` are usage refusals
/// (exit 2), nothing staged; a staged approve holds `span_hash` null; the
/// round never lists it (`spec round new` notes `1 open change
/// proposal(s) not on this sheet: PR-0001`), its answer line is refused
/// `is a change proposal (amendment)`, `--staged` notes it unasked. M:
/// `new_file()` left `== ProposalKind::Create`; the round treating it as
/// deciding.
#[test]
fn ac_r8_the_stage_and_the_round_take_it_as_a_change_proposal() {
    let pair = Pair::new("ka-r8", "spec-a");
    let id = amended(&pair, &pair.main, PATH, TEXT);
    for (label, option, answer, canon) in [
        ("option", Some(0), None, None),
        ("answer", None, Some("x"), None),
        ("canon", None, None, Some("MEC-STAMINA")),
    ] {
        let outcome = stage_of(&pair, &id, option, answer, canon);
        assert_eq!(outcome.exit(), Exit::CannotRun, "{label}: {outcome:?}");
        assert!(
            pair.proposal(&id).staged.is_none(),
            "{label}: nothing staged"
        );
    }

    let sheet = round_new(
        &pair.env(&pair.main),
        &Globals::default(),
        &RoundNewRequest {
            limit: None,
            git: pair.git_env(&pair.main),
        },
    )
    .expect("round new");
    let outcome = Outcome::RoundSheet(Box::new(sheet));
    let text = render_text(&outcome);
    let json = json_of(&render_json(&outcome));
    assert_eq!(json["items"], json!([]), "never on the sheet: {json}");
    assert!(!text.contains(&format!("## {id} - ")), "{text}");
    assert!(
        text.ends_with(&format!(
            "note: 1 open change proposal(s) not on this sheet: {id}; spec inbox lists them\n"
        )),
        "{text}"
    );

    let (run, terminal) = round_with(&pair, Some(&format!("project: lantern-keep\n{id}: ok\n")));
    assert_eq!(
        terminal.reports,
        [format!(
            "{id} line 2: refused: `{id}` is a change proposal (amendment): `spec approve {id}` \
             decides it; nothing staged\nanswers: 0 staged, 0 left open, 1 refused\n"
        )]
    );
    assert!(terminal.asked.is_empty());
    assert_eq!(run.exit(), Exit::NotFound);
    assert!(pair.proposal(&id).staged.is_none());

    let staged = stage_of(&pair, &id, None, None, None);
    assert_eq!(staged.exit(), Exit::Answered, "{staged:?}");
    assert_eq!(
        pair.proposal(&id).staged.map(|staged| staged.stage),
        Some(Stage::Approve {
            option: None,
            answer: None,
            canon: None,
            editorial: false,
            note: None,
            span_hash: None,
        }),
        "a new file's stage holds no span hash"
    );
    let (run, terminal) = round_with(&pair, None);
    assert_eq!(
        run.notes,
        [format!(
            "1 staged change proposal(s) not asked: {id}; spec approve <ID> confirms each"
        )]
    );
    assert!(terminal.asked.is_empty(), "{:?}", terminal.asked);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
}

// ------------------------------------------------------------------ AC-R9

/// AC-R9, W33: bound (`--task T-0002`) it is in that task's
/// `open_proposals`, no assumption; unbound, it is listed by the task
/// wanting the node it amends (`RULE-STAM-REGEN`), not by another; applied
/// in the compared place, no `task.refreshed`. M: `lists_proposal` by
/// `target_ids()` alone.
#[test]
fn ac_r9_tasks_list_it_bound_or_by_the_nodes_it_amends() {
    let pair = Pair::new("ka-r9", "spec-a");
    let main = pair.main.clone();
    let wanting = pair.new_ok(&main, &["RULE-STAM-REGEN"]);
    pair.plan_ok(&main, &wanting, &[], &[]);
    pair.approve_task(&main, &wanting);
    let other = pair.new_ok(&main, &["MEC-SPRINT"]);
    pair.plan_ok(&main, &other, &[], &[]);
    pair.approve_task(&main, &other);
    let listed = |task: &str| -> Vec<(String, String)> {
        let package = pair.package(&main, task);
        assert!(
            package["assumptions"]
                .as_array()
                .is_none_or(|assumptions| assumptions.is_empty()),
            "no assumption: {package}"
        );
        package["open_proposals"]
            .as_array()
            .expect("open_proposals")
            .iter()
            .map(|entry| {
                (
                    entry["id"].as_str().unwrap_or_default().to_owned(),
                    entry["kind"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect()
    };
    let unbound = amended(&pair, &main, PATH, TEXT);
    let kind = AMENDMENT_KIND.to_owned();
    assert_eq!(listed(&wanting), [(unbound.clone(), kind.clone())]);
    assert!(listed(&other).is_empty(), "not by another task");

    let bound = propose_amendment(
        &pair.env(&main),
        &Globals::default(),
        &request(
            &pair,
            &main,
            "docs/features/zero-stop.md",
            &edit(&amending("EDGE-STAM-ZERO"), "{#AC-01}", "{#AC-02}"),
        ),
        Some(&other),
    )
    .expect("propose --task");
    assert_eq!(bound.exit(), Exit::Answered, "{bound:?}");
    let bound = bound.document.id.expect("an ID");
    assert_eq!(
        pair.proposal(&bound).task_id.as_deref(),
        Some(other.as_str())
    );
    assert_eq!(listed(&other), [(bound.clone(), kind.clone())]);
    assert_eq!(listed(&wanting), [(unbound.clone(), kind)]);

    pair.approve_ok(&main, &unbound);
    assert_eq!(pair.proposal(&unbound).status, ProposalStatus::Applied);
    let refreshed: Vec<(String, Value)> = pair
        .task_events()
        .into_iter()
        .filter(|(event, _)| event == "task.refreshed")
        .collect();
    assert!(refreshed.is_empty(), "{refreshed:?}");
    assert!(listed(&wanting).is_empty(), "applied: listed no more");
}

// ----------------------------------------------------------------- AC-R10

/// AC-R10: the review document is a new file's: 47 keys (46 until
/// docs/features/rev-discipline.md added `revisions`, `null` here; the
/// name keeps the count AC-R10 cites it by), the fixture's `Proposal` set
/// in order, `diff` `@@ -0,0 +1,13 @@`, `preview` `applies`; `--brief`
/// keeps the keys (texts and diff dropped); the inbox entry the fixture's
/// `InboxEntry` keys; `fixtures/daemon-keys.json` holds 45 sets (41 until
/// docs/features/health-left.md added its three, AC-T2, and
/// docs/features/rev-discipline.md its `RevisionRow`, TE-3; the amendment
/// adds none). M: a key added.
#[test]
fn ac_r10_the_review_is_a_new_files_46_keys() {
    let pair = Pair::new("ka-r10", "spec-a");
    let id = amended(&pair, &pair.linked, PATH, TEXT);
    let raw = review_raw(&pair, &id);
    let keys = task_common::keys(&raw);
    // 47 since docs/features/rev-discipline.md added `revisions`.
    assert_eq!(keys.len(), 47, "{keys:?}");
    let fixture: Value = serde_json::from_str(&read_text(
        &common::repository_root(),
        "fixtures/daemon-keys.json",
    ))
    .expect("daemon-keys.json");
    let sets = fixture.as_object().expect("sets");
    assert_eq!(
        sets.len(),
        45,
        "no set added by the amendment: 41, health-left's three and \
         rev-discipline's `RevisionRow`"
    );
    let set = |name: &str| -> Vec<String> {
        sets[name]
            .as_array()
            .unwrap_or_else(|| panic!("no set {name}"))
            .iter()
            .map(|key| key.as_str().expect("a key").to_owned())
            .collect()
    };
    assert_eq!(keys, set("Proposal"));
    let review = json_of(&raw);
    assert_eq!(review["kind"], json!(AMENDMENT_KIND));
    assert_eq!(review["preview"], json!("applies"));
    let diff = review["diff"].as_str().expect("a diff");
    assert!(
        diff.starts_with(&format!(
            "--- base {PATH}\n+++ proposed {PATH}\n@@ -0,0 +1,13 @@\n+---\n"
        )),
        "{diff}"
    );
    assert_eq!(
        diff.lines().filter(|line| line.starts_with('+')).count(),
        14
    );
    assert_eq!(review["interpretation"], Value::Null);
    assert_eq!(review["staged"], Value::Null);
    // A new file: no existing node written, `revisions` null
    // (docs/features/rev-discipline.md "Review document, stage, queue").
    assert_eq!(review["revisions"], Value::Null);

    let brief = review_brief(
        &pair.env(&pair.main),
        &Globals::default(),
        &ReviewRequest {
            id: id.clone(),
            git: pair.git_env(&pair.main),
        },
    )
    .expect("review --brief");
    let brief_raw = printed(&brief).1;
    assert_eq!(task_common::keys(&brief_raw), set("Proposal"), "--brief");
    let brief = json_of(&brief_raw);
    assert_eq!(brief["kind"], json!(AMENDMENT_KIND));
    assert_eq!(brief["target_ids"], json!([PATH, NEW_ID]));
    assert_eq!(brief["new_text"], Value::Null);
    assert_eq!(brief["diff"], Value::Null);
    assert_eq!(brief["preview"], json!("applies"));
    // The same through the brief propose (MCP's twin).
    let pair_b = Pair::new("ka-r10-brief", "spec-a");
    let outcome = propose_amendment_brief(
        &pair_b.env(&pair_b.linked),
        &Globals::default(),
        &request(&pair_b, &pair_b.linked, PATH, TEXT),
        None,
    )
    .expect("propose --brief");
    let proposed = printed(&outcome).1;
    assert_eq!(task_common::keys(&proposed), set("Proposal"));
    assert_eq!(json_of(&proposed)["new_text"], Value::Null);

    let inbox = pair.inbox(&pair.main, false).expect("inbox");
    let entry = task_common::Ordered::of(&printed_inbox(&inbox).1)
        .get("proposals")
        .at(0)
        .keys();
    assert_eq!(entry, set("InboxEntry"));
}

// ----------------------------------------------------------------- AC-R13

/// AC-R13: an applied and an open amendment export `"queue_schema":7`
/// (format 2, 44 columns a row, `kind` `amendment`); imported into a fresh
/// data directory and exported again, byte for byte; the open one's ID
/// still reserved. M: export skipping the kind.
#[test]
fn ac_r13_a_backup_with_amendments_round_trips_byte_identical() {
    let pair = Pair::new("ka-r13", "spec-a");
    let applied = amended(&pair, &pair.linked, PATH, TEXT);
    pair.approve_ok(&pair.main, &applied);
    let open = amended(
        &pair,
        &pair.linked,
        "docs/features/zero-stop.md",
        &edit(&amending("EDGE-STAM-ZERO"), "{#AC-01}", "{#AC-02}"),
    );
    let dumps = pair.scratch.dir("dumps");
    let env_of = |home: &Path| specengine_cli::Env {
        cwd: pair.main.clone(),
        home: Some(home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let export = |home: &Path, name: &str| -> Vec<u8> {
        let file = dumps.join(name);
        export_state(
            &env_of(home),
            &Globals::default(),
            &ExportStateRequest {
                out: Some(file.clone()),
                now: LATER.to_owned(),
                git: pair.git_env(&pair.main),
            },
        )
        .unwrap_or_else(|error| panic!("export: {error}"));
        std::fs::read(file).expect("the dump")
    };
    let bytes = export(&pair.home, "q.jsonl");
    let text = String::from_utf8(bytes.clone()).expect("UTF-8");
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[0].starts_with(
            "{\"format\":2,\"queue_schema\":7,\"project\":\"lantern-keep\",\"proposals\":2,"
        ),
        "{}",
        lines[0]
    );
    for (line, (id, status)) in lines[1..3]
        .iter()
        .zip([(&applied, "applied"), (&open, "open")])
    {
        let row: Value = serde_json::from_str(line).expect("a row");
        let row = row["proposals"].as_object().expect("a proposals row");
        assert_eq!(row.len(), 44);
        assert_eq!(row["id"], json!(id));
        assert_eq!(row["kind"], json!(AMENDMENT_KIND));
        assert_eq!(row["status"], json!(status));
        assert_eq!(row["base_hash"], Value::Null);
    }
    let fresh = pair.scratch.home("fresh");
    let outcome = import_state(
        &env_of(&fresh),
        &Globals::default(),
        &ImportStateRequest {
            file: dumps.join("q.jsonl"),
        },
        &mut |_: &str| true,
    )
    .expect("import");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(export(&fresh, "again.jsonl"), bytes, "byte-identical");
    let restored = specengine_store::SqliteQueue::open(
        common::data_dir(&fresh).join("lantern-keep.db"),
        "lantern-keep",
    )
    .expect("open");
    let held: Vec<(String, String)> = restored
        .reserved()
        .expect("reserved")
        .into_iter()
        .map(|held| (held.id, held.by))
        .collect();
    assert_eq!(held, [("zero-stop/AC-02".to_owned(), open.clone())]);
    assert_eq!(
        restored.get(&applied).unwrap().unwrap().kind,
        ProposalKind::Amendment
    );
}
