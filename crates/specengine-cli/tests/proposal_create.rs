//! docs/features/proposal-apply.md, creation and reading: AC-01 (`show`'s
//! `span_hash`, a proposal from the linked worktree, nothing written),
//! AC-02 (a stale base), AC-03 (creation refusals), AC-04 (introduced
//! findings), AC-05 (`inbox` order, `review` keys, determinism), AC-19's
//! ID half (a look-alike proposal ID, `PR` in `[ids]`) and the target forms
//! of "Rules and edge cases", creation 1.
//!
//! Every repository is a scratch copy of `fixtures/spec-a` or
//! `fixtures/spec-b` with a linked worktree on `t1` (`common::proposal`);
//! the queue's commands run through the library with an injected clock.
//! "Refused": exit 1, nothing stored or written, no commit, no ID used.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::path::Path;

use common::bundle::blake3_hex;
use common::proposal::{CASES, NOW, Pair, cannot, edit, json_of, printed, printed_inbox, refused};
use common::{read, read_text, replace, spec, write};
use specengine_cli::{Exit, Globals, Outcome, ProposedText, TEXT_MAX_BYTES, propose_brief};
use specengine_core::ProjectConfig;

fn b3(bytes: &[u8]) -> String {
    format!("b3:{}", blake3_hex(bytes))
}

/// The raw bytes of node `id`'s span in `root/path`, by the parser.
fn span_bytes(root: &Path, path: &str, id: &str) -> Vec<u8> {
    let config = ProjectConfig::from_toml(&read_text(root, "specengine.toml")).expect("config");
    let bytes = read(root, path);
    let parsed = specengine_core::parse(path, &bytes, &config.scheme);
    let node = parsed
        .nodes
        .iter()
        .find(|node| node.id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} in {path}"));
    bytes[node.span.range()].to_vec()
}

/// AC-01 (and AC-20 for both fixtures): `show --json`'s `span_hash` is
/// `b3:` and the BLAKE3 of the span's bytes; `propose` with it from the
/// linked worktree prints `PR-0001`; every file of both worktrees is byte
/// for byte as before and `git status` is empty in both. The stored
/// proposal is bound to the linked worktree, its branch and `HEAD`.
/// M: hash the printed text; validation writes the file.
#[test]
fn ac01_the_show_hash_proposes_pr_0001_from_the_linked_worktree_writing_nothing() {
    for case in &CASES {
        let pair = Pair::of("pa-ac01", case);
        let run = spec(&pair.home, &pair.linked, &["--json", "show", case.target]);
        run.code(0);
        let json = run.json();
        let node = &json["nodes"][0];
        let span = span_bytes(&pair.linked, case.path, case.target);
        let hash = b3(&span);
        assert_eq!(node["span_hash"], hash, "{}: {json}", case.fixture);
        let text = node["text"].as_str().expect("text");
        assert_eq!(text.as_bytes(), span.as_slice(), "{}", case.fixture);
        let new_text = edit(text, case.first.0, case.first.1);

        let before = pair.state();
        let outcome = pair
            .propose(&pair.linked, case.target, &hash, &new_text)
            .unwrap_or_else(|error| panic!("{}: {error}", case.fixture));
        let (stdout, json) = printed(&outcome);
        assert_eq!(stdout, "PR-0001\nintroduced: 0\n", "{}", case.fixture);
        assert_eq!(json_of(&json)["id"], "PR-0001", "{}", case.fixture);
        assert!(
            Outcome::Proposal(Box::new(outcome.clone()))
                .stderr_lines()
                .is_empty(),
            "{}: {outcome:?}",
            case.fixture
        );

        let after = pair.state();
        assert_eq!(after.main_files, before.main_files, "{}", case.fixture);
        assert_eq!(after.linked_files, before.linked_files, "{}", case.fixture);
        assert_eq!(after.refs, before.refs, "{}", case.fixture);
        assert_eq!(after.main_status, "", "{}", case.fixture);
        assert_eq!(after.linked_status, "", "{}", case.fixture);
        assert_eq!(after.main_staged, before.main_staged, "{}", case.fixture);
        assert_eq!(
            after.linked_staged, before.linked_staged,
            "{}",
            case.fixture
        );

        let stored = pair.proposal("PR-0001");
        assert_eq!(stored.status.as_str(), "open");
        assert_eq!(stored.project, case.slug);
        assert_eq!(stored.target_id, case.target);
        assert_eq!(stored.target_path, case.path);
        assert_eq!(stored.place.worktree, pair.linked.to_str().unwrap());
        assert_eq!(stored.place.root_rel, "");
        assert_eq!(stored.place.branch, "t1");
        assert_eq!(stored.place.base_commit, pair.rev(&pair.linked, "HEAD"));
        assert_eq!(
            Path::new(&stored.place.git_common_dir),
            std::fs::canonicalize(pair.main.join(".git")).unwrap()
        );
        assert_eq!(stored.base_hash, hash);
        assert_eq!(stored.base_text.as_bytes(), span.as_slice());
        assert_eq!(stored.new_text, new_text);
        assert_eq!(
            stored.patch_hash,
            b3(format!("{}\n{hash}\n{new_text}", case.target).as_bytes()),
            "07 \u{a7}1.2"
        );
        assert_eq!(stored.created_at, NOW);
        assert_eq!(stored.updated_at, NOW);
        assert!(stored.decided_by.is_none() && stored.applied_commit.is_none());
        assert_eq!(
            pair.events_of("PR-0001"),
            [("proposal.created".to_owned(), None)]
        );
    }
}

/// AC-02: `--base` h1 after a committed change of the span (h2): exit 1
/// naming h2, refused, and the next proposal is still `PR-0001`.
/// M: base comparison dropped.
#[test]
fn ac02_a_stale_base_is_refused_naming_the_current_hash() {
    let pair = Pair::new("pa-ac02", "spec-a");
    let target = "EDGE-SPRINT-EMPTY";
    let (h1, text1) = pair.span(&pair.linked, target);
    replace(
        &pair.linked,
        "docs/spec/movement/sprint.md",
        "the sprint ends;",
        "the sprint ends now;",
    );
    pair.commit_all(&pair.linked, "a span change");
    let (h2, text2) = pair.span(&pair.linked, target);
    assert_ne!(h1, h2);

    let before = pair.state();
    let outcome = pair.propose(
        &pair.linked,
        target,
        &h1,
        &edit(&text1, "is not a reference", "is never a reference"),
    );
    let reason = refused(&outcome, "a stale base");
    assert!(reason.contains(&h2), "names the current hash: {reason}");
    let lines = Outcome::Proposal(Box::new(outcome.unwrap())).stderr_lines();
    assert_eq!(lines.last().unwrap(), &format!("spec: {reason}"));
    assert_eq!(pair.state(), before, "refused: nothing changed");
    assert!(pair.proposals().is_empty(), "nothing stored");
    assert!(pair.events().is_empty(), "no event");

    let outcome = pair
        .propose(
            &pair.linked,
            target,
            &h2,
            &edit(&text2, "is not a reference", "is never a reference"),
        )
        .unwrap();
    assert_eq!(
        outcome.document.id.as_deref(),
        Some("PR-0001"),
        "no ID used"
    );
}

/// Asserts `outcome` refused and nothing changed since `before`.
fn assert_creation_refused(
    pair: &Pair,
    before: &common::proposal::State,
    outcome: &Result<specengine_cli::ProposalOutcome, specengine_cli::CliError>,
    context: &str,
) -> String {
    let reason = refused(outcome, context);
    assert_eq!(&pair.state(), before, "{context}: nothing changed");
    assert!(pair.proposals().is_empty(), "{context}: nothing stored");
    reason
}

/// AC-03: refused (exit 1): an unknown ID; an ID declared in two files (the
/// same span bytes in both, so only the holder rule refuses); spec-b's
/// `GLS-task-branch` (`class: generated`); spec-a's `R-12` (`immutable_text`);
/// a text dropping `{#ID}`, adding an ID section, changing the level,
/// adding a same-level heading. Every case but the unknown ID carries the
/// current base. M: drop any one.
#[test]
fn ac03_creation_refusals() {
    let pair = Pair::new("pa-ac03", "spec-a");
    let target = "EDGE-SPRINT-EMPTY";
    let before = pair.state();

    let outcome = pair.propose(&pair.main, "RULE-NOPE", "b3:00", "## Nope {#RULE-NOPE}\n");
    let reason = assert_creation_refused(&pair, &before, &outcome, "an unknown ID");
    assert!(reason.contains("RULE-NOPE"), "{reason}");

    // The same section, byte for byte, in a second file.
    let span = span_bytes(
        &pair.main,
        "docs/spec/movement/stamina.md",
        "EDGE-STAM-ZERO",
    );
    let mut dup = b"---\nclass: canon\n---\n\n# Again\n\n".to_vec();
    dup.extend_from_slice(&span);
    dup.push(b'\n');
    write(&pair.main, "docs/spec/dup.md", &dup);
    pair.commit_all(&pair.main, "a second holder");
    let base = b3(&span);
    let text = edit(
        std::str::from_utf8(&span).unwrap(),
        "immediately",
        "at once",
    );
    let before = pair.state();
    let outcome = pair.propose(&pair.main, "EDGE-STAM-ZERO", &base, &text);
    let reason = assert_creation_refused(&pair, &before, &outcome, "two holders");
    assert!(
        reason.contains("docs/spec/dup.md") && reason.contains("docs/spec/movement/stamina.md"),
        "names both files: {reason}"
    );

    // R-12: immutable text.
    let (hash, text) = pair.span(&pair.main, "R-12");
    let outcome = pair.propose(
        &pair.main,
        "R-12",
        &hash,
        &edit(&text, "short delay", "brief delay"),
    );
    let reason = assert_creation_refused(&pair, &before, &outcome, "R-12 immutable");
    assert!(reason.contains("immutable"), "{reason}");

    // Structure: each with the current base.
    let (hash, text) = pair.span(&pair.main, target);
    let heading = "### Empty tank {#EDGE-SPRINT-EMPTY .playtest}";
    for (label, new_text) in [
        ("{#ID} dropped", edit(&text, heading, "### Empty tank")),
        (
            "an ID section added",
            format!("{text}\n\n#### Extra {{#EDGE-SPRINT-EXTRA}}\n\nMore words."),
        ),
        (
            "the level changed",
            edit(&text, "### Empty tank", "#### Empty tank"),
        ),
        (
            "a same-level heading added",
            format!("{text}\n\n### Another tank\n\nMore words."),
        ),
        (
            "a higher-level heading added",
            format!("{text}\n\n## Another cost\n\nMore words."),
        ),
    ] {
        let outcome = pair.propose(&pair.main, target, &hash, &new_text);
        let reason = assert_creation_refused(&pair, &before, &outcome, label);
        assert!(reason.contains(target), "{label}: {reason}");
    }
    // The control: the same base with a clean text is stored as PR-0001.
    let outcome = pair
        .propose(
            &pair.main,
            target,
            &hash,
            &edit(&text, "the sprint ends;", "the sprint ends at once;"),
        )
        .unwrap();
    assert_eq!(outcome.document.id.as_deref(), Some("PR-0001"));

    // spec-b: a generated document.
    let pair = Pair::new("pa-ac03-b", "spec-b");
    let before = pair.state();
    let (hash, text) = pair.span(&pair.main, "GLS-task-branch");
    let outcome = pair.propose(
        &pair.main,
        "GLS-task-branch",
        &hash,
        &edit(&text, "GLS-worktree", "GLS-worktree (see)"),
    );
    let reason = assert_creation_refused(&pair, &before, &outcome, "generated");
    assert!(reason.contains("generated"), "{reason}");
}

/// "Rules and edge cases", creation 1 (owner's answers 2, 3): a legacy
/// alias, a document path, `ID#SECTION`, `ID@rev` and `[[ID]]` are refused
/// (exit 1) naming the canonical ID; `project:ID`, a bad author field, an
/// unreadable text file exit 2; a text over 1 MiB or not UTF-8 is refused.
/// A bare feature-scoped ID is stored as `slug/ID`.
#[test]
fn non_canonical_targets_and_bad_inputs() {
    let pair = Pair::new("pa-forms", "spec-a");
    let before = pair.state();
    let (hash, text) = pair.span(&pair.main, "EDGE-SPRINT-EMPTY");
    let new_text = edit(&text, "the sprint ends;", "the sprint ends at once;");
    for (written, canonical) in [
        ("QST-031", "Q-031"),
        ("docs/spec/movement/sprint.md", "MEC-SPRINT"),
        ("MEC-SPRINT#EDGE-SPRINT-EMPTY", "EDGE-SPRINT-EMPTY"),
        ("EDGE-SPRINT-EMPTY@2", "EDGE-SPRINT-EMPTY"),
        ("[[EDGE-SPRINT-EMPTY]]", "EDGE-SPRINT-EMPTY"),
    ] {
        let outcome = pair.propose(&pair.main, written, &hash, &new_text);
        let reason = assert_creation_refused(&pair, &before, &outcome, written);
        assert!(
            reason.contains(&format!("`{canonical}`")),
            "{written}: names `{canonical}`: {reason}"
        );
    }
    let outcome = pair.propose(&pair.main, "shared:EDGE-SPRINT-EMPTY", &hash, &new_text);
    cannot(&outcome, "project:ID");

    let too_long = "x".repeat(129);
    for bad in ["has space", "", "\u{e9}t\u{e9}", too_long.as_str()] {
        let mut request = pair.request(&pair.main, "EDGE-SPRINT-EMPTY", &hash, &new_text);
        request.author_role = Some(bad.to_owned());
        cannot(
            &pair.propose_with(&pair.main, request),
            "a bad author field",
        );
    }
    let mut request = pair.request(&pair.main, "EDGE-SPRINT-EMPTY", &hash, &new_text);
    request.text = ProposedText::File(pair.scratch.join("missing.md"));
    cannot(
        &pair.propose_with(&pair.main, request),
        "an unreadable text file",
    );

    let mut request = pair.request(&pair.main, "EDGE-SPRINT-EMPTY", &hash, &new_text);
    request.text = ProposedText::Given(vec![b'x'; TEXT_MAX_BYTES + 1]);
    let reason = assert_creation_refused(
        &pair,
        &before,
        &pair.propose_with(&pair.main, request),
        "over 1 MiB",
    );
    assert!(reason.contains("1 MiB"), "{reason}");
    let mut request = pair.request(&pair.main, "EDGE-SPRINT-EMPTY", &hash, &new_text);
    request.text = ProposedText::Given(b"### Empty tank \xff\n".to_vec());
    assert_creation_refused(
        &pair,
        &before,
        &pair.propose_with(&pair.main, request),
        "not UTF-8",
    );
    assert!(pair.events().is_empty());

    // The text from a file, relative to the current directory; a human
    // author (no role, model or run).
    write(&pair.scratch.join("texts"), "new.md", &new_text);
    let mut request = pair.request(&pair.main, "EDGE-SPRINT-EMPTY", &hash, "");
    request.text = ProposedText::File("../texts/new.md".into());
    request.author_role = None;
    request.author_model = None;
    let outcome = pair.propose_with(&pair.main, request).unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let stored = pair.proposal("PR-0001");
    assert_eq!(stored.new_text, new_text);
    assert_eq!(
        stored.author.provenance(),
        "human role=unknown model=unknown run=unknown"
    );
    // Exactly 1 MiB is not refused for its size.
    let mut exact = format!("{new_text}\n\n").into_bytes();
    exact.resize(TEXT_MAX_BYTES, b'x');
    let mut request = pair.request(&pair.main, "EDGE-SPRINT-EMPTY", &hash, "");
    request.text = ProposedText::Given(exact);
    let outcome = pair.propose_with(&pair.main, request).unwrap();
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "exactly 1 MiB: {:?}",
        outcome.refusal
    );
    assert_eq!(pair.proposal("PR-0002").new_text.len(), TEXT_MAX_BYTES);

    // spec-b: the bare feature-scoped `CRIT-01` is stored as its slug's.
    let pair = Pair::new("pa-forms-b", "spec-b");
    let (hash, text) = pair.span(&pair.main, "dry-run/CRIT-01");
    let outcome = pair
        .propose(
            &pair.main,
            "CRIT-01",
            &hash,
            &edit(&text, "sync --dry-run", "sync --dry-run --all"),
        )
        .unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(pair.proposal("PR-0001").target_id, "dry-run/CRIT-01");
    assert_eq!(
        pair.proposal("PR-0001").target_path,
        "docs/features/dry-run.md"
    );
}

/// Creation 1 with agent-intake (iteration 2): an author field outside its
/// grammar exits 2 naming it as the intake commands and the MCP tools name
/// it — `author_role`, `author_model`, `run`, the first in that order —
/// never the flag; the library, `--brief` and the binary alike (stderr
/// `spec: <field>: <problem>`), nothing stored, nothing changed. M:
/// `Author::new`'s flag naming restored.
#[test]
fn a_bad_author_field_exits_2_naming_its_field() {
    let pair = Pair::new("pa-author", "spec-a");
    let before = pair.state();
    let (hash, text) = pair.span(&pair.main, "EDGE-SPRINT-EMPTY");
    let new_text = edit(&text, "the sprint ends;", "the sprint ends at once;");
    let long = "m".repeat(129);
    let ascii = "printable ASCII without spaces only";
    let cases = [
        (Some("a b"), None, None, format!("author_role: {ascii}")),
        (None, Some("m m"), None, format!("author_model: {ascii}")),
        (
            None,
            Some(long.as_str()),
            None,
            "author_model: 129 bytes; at most 128".to_owned(),
        ),
        (
            None,
            None,
            Some(""),
            "run: empty; give a value or leave the option out".to_owned(),
        ),
        (
            Some("\u{e9}t\u{e9}"),
            None,
            Some(""),
            format!("author_role: {ascii}"),
        ),
        (
            None,
            Some("m m"),
            Some(""),
            format!("author_model: {ascii}"),
        ),
    ];
    for (role, model, run, problem) in cases {
        let want = format!("spec: {problem}");
        let mut request = pair.request(&pair.linked, "EDGE-SPRINT-EMPTY", &hash, &new_text);
        request.author_role = role.map(str::to_owned).or(request.author_role);
        request.author_model = model.map(str::to_owned).or(request.author_model);
        request.run = run.map(str::to_owned);
        let message = cannot(&pair.propose_with(&pair.linked, request.clone()), &want);
        assert_eq!(message, want);
        let message = cannot(
            &propose_brief(&pair.env(&pair.linked), &Globals::default(), &request),
            &want,
        );
        assert_eq!(message, want, "--brief");

        let mut args = vec![
            "propose",
            "update",
            "EDGE-SPRINT-EMPTY",
            "--base",
            &hash,
            "--text-file",
            "-",
            "--rationale",
            "Bad author.",
        ];
        for (flag, value) in [
            ("--author-role", role),
            ("--author-model", model),
            ("--run", run),
        ] {
            if let Some(value) = value {
                args.extend([flag, value]);
            }
        }
        for brief in [false, true] {
            if brief {
                args.push("--brief");
            }
            let run = pair.spec_piped(&pair.linked, &args, new_text.as_bytes());
            run.code(2);
            assert_eq!(run.stderr, format!("{want}\n"), "{}", run.show());
            assert_eq!(run.stdout, "", "{}", run.show());
        }
        assert_eq!(pair.state(), before, "{want}: nothing changed");
    }
    assert!(!pair.db().exists() || pair.proposals().is_empty());
}

/// AC-04: a text citing an undeclared ID: `review` lists that finding as
/// introduced (and `propose` prints it); a clean text: none; a finding the
/// tree already had, in the edited file and even inside the edited span,
/// never. M: validate the unpatched tree; attribute all.
#[test]
fn ac04_only_findings_the_edit_introduces_are_stored() {
    let pair = Pair::new("pa-ac04", "spec-a");
    let target = "EDGE-SPRINT-EMPTY";
    let (hash, text) = pair.span(&pair.linked, target);
    let citing = edit(
        &text,
        "the sprint ends;",
        "the sprint ends (see RULE-UNDECLARED);",
    );
    let outcome = pair.propose(&pair.linked, target, &hash, &citing).unwrap();
    let (stdout, _) = printed(&outcome);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[..2], ["PR-0001", "introduced: 1"], "{stdout}");
    assert_eq!(lines.len(), 3, "{stdout}");
    assert!(
        lines[2].contains("docs/spec/movement/sprint.md:24: ")
            && lines[2].contains("RULE-UNDECLARED"),
        "{stdout}"
    );
    let review = pair.review_ok(&pair.main, "PR-0001");
    let found = review.document.diagnostics.clone().unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].subject, "RULE-UNDECLARED");
    assert_eq!(found[0].path, "docs/spec/movement/sprint.md");
    assert_eq!(found[0].line, 24);
    let (text_out, json) = printed(&review);
    assert!(text_out.contains("\ndiagnostics: 1\n  "), "{text_out}");
    assert_eq!(
        json_of(&json)["diagnostics"][0]["subject"],
        "RULE-UNDECLARED"
    );

    // A clean text: none, though the file has a pre-existing finding (the
    // `depends-cycle` at line 9).
    let clean = edit(&text, "the sprint ends;", "the sprint ends at once;");
    let outcome = pair.propose(&pair.linked, target, &hash, &clean).unwrap();
    let (stdout, _) = printed(&outcome);
    assert_eq!(stdout, "PR-0002\nintroduced: 0\n");
    assert_eq!(pair.proposal("PR-0002").diagnostics, []);

    // spec-b: CMD-SYNC's span holds `docs/spec/cli.md:25`'s homoglyph and
    // mixed-script mention; a clean edit that also moves them a line down
    // introduces nothing.
    let pair = Pair::new("pa-ac04-b", "spec-b");
    let (hash, text) = pair.span(&pair.linked, "CMD-SYNC");
    assert!(
        text.contains("R\u{0415}Q-003"),
        "the pre-existing finding is in the span"
    );
    let moved = edit(&text, "`sync`", "`sync`\n(an added line)");
    let outcome = pair
        .propose(&pair.linked, "CMD-SYNC", &hash, &moved)
        .unwrap();
    assert_eq!(printed(&outcome).0, "PR-0001\nintroduced: 0\n");
    assert_eq!(pair.proposal("PR-0001").diagnostics, []);
}

/// The Data keys of `review` in order.
const REVIEW_KEYS: [&str; 37] = [
    "id",
    "project",
    "kind",
    "status",
    "target_id",
    "target_path",
    "worktree",
    "branch",
    "base_commit",
    "base_hash",
    "base_text",
    "new_text",
    "patch_hash",
    "rationale",
    "author",
    "diagnostics",
    "diff",
    "preview",
    "conflict",
    "decided_by",
    "decided_at",
    "decision_note",
    "applied_commit",
    "created_at",
    "updated_at",
    "target_ids",
    "severity",
    "gap_type",
    "summary",
    "working_answer",
    "price_of_other",
    "evidence",
    "options",
    "recommendation",
    "distinct_from",
    "linked",
    "notes",
];

/// The keys of `review`'s text: every line not indented and not empty.
fn text_keys(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with("  "))
        .map(|line| {
            line.split_once(':')
                .unwrap_or_else(|| panic!("no key in {line:?}"))
                .0
                .to_owned()
        })
        .collect()
}

/// AC-05: `inbox` lists by ID; `review` has every key of Data, in order,
/// absent = `null`, none starting `block`; two runs 1 s apart print
/// byte-identical text and JSON (`review` and `inbox`). M: relative age.
#[test]
fn ac05_inbox_order_review_keys_and_byte_identical_reruns() {
    let pair = Pair::new("pa-ac05", "spec-a");
    let long = format!("{}\nsecond line", "word ".repeat(30));
    for (target, from, to, rationale) in [
        (
            "EDGE-SPRINT-EMPTY",
            "the sprint ends;",
            "the sprint ends at once;",
            "First.",
        ),
        (
            "RULE-STAM-REGEN",
            "rate \u{d7} 0.5",
            "rate \u{d7} 0.4",
            long.as_str(),
        ),
        ("EDGE-STAM-ZERO", "immediately", "at once", "Third."),
    ] {
        let (hash, text) = pair.span(&pair.linked, target);
        let mut request = pair.request(&pair.linked, target, &hash, &edit(&text, from, to));
        request.rationale = rationale.to_owned();
        let outcome = pair.propose_with(&pair.linked, request).unwrap();
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }
    let inbox = pair.inbox(&pair.main, false).unwrap();
    let (text, json) = printed_inbox(&inbox);
    let ids: Vec<&str> = text
        .lines()
        .map(|line| line.split(" | ").next().unwrap())
        .collect();
    assert_eq!(ids, ["PR-0001", "PR-0002", "PR-0003"], "{text}");
    let second = text.lines().nth(1).unwrap();
    let rationale = second.rsplit(" | ").next().unwrap();
    assert_eq!(rationale.chars().count(), 80, "{second}");
    assert!(rationale.ends_with('\u{2026}'), "{second}");
    assert_eq!(
        text.lines().next().unwrap(),
        format!("PR-0001 | update | open | EDGE-SPRINT-EMPTY | t1 | {NOW} | First.")
    );
    let json = json_of(&json);
    assert_eq!(
        json.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["notes", "proposals"]
    );
    assert_eq!(json["proposals"].as_array().unwrap().len(), 3);
    for entry in json["proposals"].as_array().unwrap() {
        let mut keys: Vec<&str> = entry
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "branch",
                "created_at",
                "id",
                "kind",
                "rationale",
                "severity",
                "status",
                "summary",
                "target_id"
            ]
        );
    }

    let review = pair.review_ok(&pair.main, "PR-0001");
    let (text, json) = printed(&review);
    assert_eq!(text_keys(&text), REVIEW_KEYS, "{text}");
    let value = json_of(&json);
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let mut want = REVIEW_KEYS.to_vec();
    want.sort_unstable();
    assert_eq!(keys, want, "{json}");
    // JSON in Data's order: each key's first position rises.
    let positions: Vec<usize> = REVIEW_KEYS
        .iter()
        .map(|key| json.find(&format!("\"{key}\":")).unwrap())
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{json}");
    assert!(
        keys.iter().all(|key| !key.starts_with("block")),
        "no key starts `block`"
    );
    assert!(text_keys(&text).iter().all(|key| !key.starts_with("block")));
    for absent in [
        "conflict",
        "decided_by",
        "decided_at",
        "decision_note",
        "applied_commit",
    ] {
        assert!(value[absent].is_null(), "{absent}: {json}");
        assert!(
            text.contains(&format!("\n{absent}: -\n")),
            "{absent}: {text}"
        );
    }
    assert_eq!(value["preview"], "applies");
    assert_eq!(value["author"]["type"], "agent");
    assert_eq!(value["author"]["role"], "spec-writer");
    assert!(value["author"]["run"].is_null());
    assert!(
        value["diff"].as_str().unwrap().starts_with(
            "--- base docs/spec/movement/sprint.md\n+++ proposed docs/spec/movement/sprint.md\n@@ "
        ),
        "{json}"
    );
    assert!(text.contains(&format!("\ncreated_at: {NOW}\n")), "{text}");

    let inbox_first = printed_inbox(&pair.inbox(&pair.main, false).unwrap());
    let review_first = printed(&pair.review_ok(&pair.main, "PR-0002"));
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert_eq!(
        printed_inbox(&pair.inbox(&pair.main, false).unwrap()),
        inbox_first
    );
    assert_eq!(
        printed(&pair.review_ok(&pair.main, "PR-0002")),
        review_first
    );
    assert_eq!(
        printed(&pair.review_ok(&pair.main, "PR-0001")),
        (text, json)
    );
}

/// AC-19, the ID half: `PR-0001` written with U+0420 (Cyrillic Er) for `P`
/// exits 2 naming `PR-0001` in `review`, `approve` and `reject`; a
/// non-canonical Latin ID (`PR-1`) names no proposal (exit 1). `PR` in
/// `[ids]` — a prefix, or an `aliases_from` entry reading `PR` — makes
/// every queue command exit 2. M: accept the look-alike.
#[test]
fn ac19_a_look_alike_proposal_id_and_pr_in_ids_exit_2() {
    let pair = Pair::new("pa-ac19-ids", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends now;",
    );
    assert_eq!(id, "PR-0001");
    let look_alike = "\u{0420}R-0001".to_owned();
    assert_ne!(look_alike, "PR-0001");
    let before = pair.state();
    let events = pair.events().len();
    let message = cannot(&pair.review(&pair.main, &look_alike), "review");
    assert!(message.contains("`PR-0001`"), "{message}");
    let (outcome, questions) =
        pair.approve_answer(&pair.main, &look_alike, true, pair.git_env(&pair.main));
    let message = cannot(&outcome, "approve");
    assert!(message.contains("`PR-0001`"), "{message}");
    assert!(questions.is_empty());
    let (outcome, questions) = pair.reject_answer(&pair.main, &look_alike, "x", true);
    let message = cannot(&outcome, "reject");
    assert!(message.contains("`PR-0001`"), "{message}");
    assert!(questions.is_empty());
    assert_eq!(pair.state(), before);
    assert_eq!(pair.events().len(), events, "no event");
    for written in ["PR-1", "PR-00001", "pr-0001", "PR-0002"] {
        refused(&pair.review(&pair.main, written), written);
        let (outcome, questions) =
            pair.approve_answer(&pair.main, written, true, pair.git_env(&pair.main));
        refused(&outcome, written);
        assert!(questions.is_empty(), "{written}");
    }
    assert_eq!(pair.state(), before);
    assert_eq!(pair.events().len(), events, "no event");

    // `PR` in `[ids]`: as a prefix, as a Latin alias, as a look-alike alias.
    let config = read_text(&pair.main, "specengine.toml");
    let (hash, text) = pair.span(&pair.main, "EDGE-SPRINT-EMPTY");
    let new_text = edit(&text, "the sprint ends;", "the sprint ends now;");
    for (label, line) in [
        (
            "a prefix",
            "PR   = { kind = \"pull\", width = 4 }\n".to_owned(),
        ),
        (
            "an alias",
            "PULL = { kind = \"pull\", width = 4, aliases_from = [\"PR\"] }\n".to_owned(),
        ),
        (
            "a look-alike alias",
            "PULL = { kind = \"pull\", width = 4, aliases_from = [\"\u{0420}R\"] }\n".to_owned(),
        ),
    ] {
        write(&pair.main, "specengine.toml", format!("{config}{line}"));
        let message = cannot(
            &pair.propose(&pair.main, "EDGE-SPRINT-EMPTY", &hash, &new_text),
            label,
        );
        assert!(message.contains("PR"), "{label}: {message}");
        cannot(&pair.inbox(&pair.main, true), label);
        cannot(&pair.review(&pair.main, "PR-0001"), label);
        let (outcome, questions) =
            pair.approve_answer(&pair.main, "PR-0001", true, pair.git_env(&pair.main));
        cannot(&outcome, label);
        assert!(questions.is_empty(), "{label}");
        let (outcome, _) = pair.reject_answer(&pair.main, "PR-0001", "x", true);
        cannot(&outcome, label);
        assert_eq!(pair.proposals().len(), 1, "{label}");
        assert_eq!(pair.events().len(), events, "{label}: no event");
    }
    write(&pair.main, "specengine.toml", config);
    assert_eq!(pair.proposal("PR-0001").status.as_str(), "open");
}
