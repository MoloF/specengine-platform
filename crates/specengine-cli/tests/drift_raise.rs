//! docs/features/drift-queue.md, the raise (`spec propose drift`): TE-1,
//! TE-3's raise half, TE-9, TE-10 and the named RD-2, RD-3, RD-14 tests,
//! over scratch repositories of `fixtures/code-a` with the golden lock
//! `fixtures/spec-lock/code-a.lock` committed (and of
//! `fixtures/verify-ecs`, an ECS-style game crate, for the genre).
//!
//! Each scenario edits the tree and commits the edit: the raise judges
//! the commit `HEAD` names, never the disk. Every stored item is checked
//! against `spec verify --json`'s rows of its node at that commit (its
//! non-`ok` rows, `pre_existing` `true`, in verify's key order), its
//! summary the rows' distinct `state[ category]`, its severity `high` iff
//! a row is class `fix`, its text line `created PR | node | severity |
//! summary` ("Data": "The item", "Triggers", "Output"). The expected texts
//! of notes and refusals are the spec's.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod verify_common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use drift_common::{
    BOUNDS, COUNTER, COUNTER_MD, LIMITS, RAISED, Raised, STAMINA, cap_changed, drifted, dump,
    ecs_drifted, proposal, proposals, raise, raise_at, raise_with_task, raised, sql,
};
use lock_common::LockRepo;
use serde_json::{Value, json};
use specengine_core::drift::{HashedEntry, HashedRow, drift_hash_input};
use specengine_core::intake::IntakeSeverity;
use specengine_core::verify::{MarkerRev, State};
use specengine_store::{Proposal, ProposalKind, ProposalStatus};
use verify_common::{clone_dir, golden};

const SLUG: &str = "code-a";

/// `spec verify --json`'s non-`ok` rows of `node` at the worktree (the
/// commit, every edit committed), `pre_existing` set as an item stores it.
fn verify_rows(repo: &LockRepo, node: &str) -> Vec<Value> {
    let run = repo.spec_piped(&repo.root, &["verify", "--json"]);
    let document = run.json();
    document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["node"] == node && row["state"] != "ok")
        .cloned()
        .map(|mut row| {
            row["pre_existing"] = json!(true);
            row
        })
        .collect()
}

/// The label a row gives an item's summary: `state[ category]`.
fn label(row: &Value) -> String {
    match row["category"].as_str() {
        Some(category) => format!("{} {category}", row["state"].as_str().expect("state")),
        None => row["state"].as_str().expect("state").to_owned(),
    }
}

/// The item `raised` created for `node`, checked against "The item" and
/// verify's rows; its summary holds every label of `labels`.
fn assert_item(
    repo: &LockRepo,
    raised: &Raised,
    slug: &str,
    node: &str,
    labels: &[&str],
) -> Proposal {
    let (id, _) = raised
        .created()
        .into_iter()
        .find(|(_, created)| created == node)
        .unwrap_or_else(|| panic!("no item for {node}: {}", raised.json_text()));
    let stored = proposal(repo, slug, &id);
    assert_eq!(stored.kind, ProposalKind::Drift, "{id}");
    assert_eq!(stored.status, ProposalStatus::Open, "{id}");
    assert_eq!(stored.target_id, node, "{id}");
    assert_eq!(stored.target_path, "spec.lock", "{id}");
    assert_eq!(
        stored.place.base_commit,
        repo.head(),
        "{id}: the commit judged"
    );
    assert_eq!(stored.place.branch, "main", "{id}");
    assert!(stored.intake.is_none() && stored.record.is_none(), "{id}");
    assert_eq!(stored.diagnostics, [], "{id}");
    let item = stored.drift.as_ref().expect("a drift item's evidence");
    let rows: Vec<Value> = item
        .evidence
        .rows
        .iter()
        .map(|row| serde_json::to_value(row).expect("a row"))
        .collect();
    assert_eq!(
        rows,
        verify_rows(repo, node),
        "{id}: verify's rows of {node}"
    );
    assert_eq!(item.evidence.more, 0, "{id}");
    let mut seen: Vec<String> = Vec::new();
    for row in &rows {
        let label = label(row);
        if !seen.contains(&label) {
            seen.push(label);
        }
    }
    assert_eq!(item.summary, seen.join(", "), "{id}: the summary");
    for want in labels {
        assert!(
            seen.iter().any(|label| label == want),
            "{id}: no `{want}` in {seen:?}"
        );
    }
    let fix = rows.iter().any(|row| row["class"] == "fix");
    let severity = if fix {
        IntakeSeverity::High
    } else {
        IntakeSeverity::Normal
    };
    assert_eq!(item.severity, severity, "{id}");
    let hash = item.hash.strip_prefix("b3:").expect("b3:");
    assert!(
        hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "{id}: {}",
        item.hash
    );
    let line = format!(
        "created {id} | {node} | {} | {}",
        severity.as_str(),
        item.summary
    );
    assert!(
        raised.text().lines().any(|printed| printed == line),
        "no line {line:?} in:\n{}",
        raised.text()
    );
    stored
}

/// The summary line "Output" gives a raise.
fn summary_line(raised: &Raised, repo: &LockRepo) -> String {
    let document = raised.json();
    format!(
        "propose drift: {} created, {} hit(s); {} rows of {} nodes judged at {} on main",
        document["created"].as_array().expect("created").len(),
        document["hits"].as_array().expect("hits").len(),
        document["judged"]["rows"],
        document["judged"]["nodes"],
        repo.head()
    )
}

// ------------------------------------------------------------------ TE-1

/// TE-1, the golden base: of code-a's nodes only `PIECE-COUNTER` drifts
/// (`spec_ahead marker_behind`); `GOAL-01` (`cannot_verify`,
/// `bad_marker`), `LAW-STEP` and `PREM-001` (`unlocked`, never locked,
/// `predated`), `LAW-ODD` (`cannot_verify rev_unknown`) and
/// `cap-check/PASS-01` (`bad_marker bad_levels`) raise nothing. The text
/// is the item line and the summary line; the JSON every key. M: a state
/// that never raises taken as a trigger.
#[test]
fn golden_sweep_raises_only_the_drifting_node() {
    let repo = golden("dr-golden");
    let raised = raised(&repo, &[]);
    assert_eq!(
        raised.created_nodes(),
        ["PIECE-COUNTER"],
        "{}",
        raised.text()
    );
    let item = assert_item(
        &repo,
        &raised,
        SLUG,
        "PIECE-COUNTER",
        &["spec_ahead marker_behind"],
    );
    assert_eq!(item.id, "PR-0001");
    let lines: Vec<String> = raised.text().lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[1], summary_line(&raised, &repo));
    let document = raised.json();
    let keys: Vec<&str> = document
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        ["branch", "commit", "created", "hits", "judged", "notes"],
        "{document}"
    );
    let printed = raised.json_text();
    let at = |key: &str| printed.find(&format!("\"{key}\":")).expect(key);
    assert!(
        at("commit") < at("branch")
            && at("branch") < at("judged")
            && at("judged") < at("created")
            && at("created") < at("hits")
            && at("hits") < at("notes"),
        "the document's order: {printed}"
    );
    assert_eq!(document["commit"], json!(repo.head()));
    assert_eq!(document["branch"], json!("main"));
    assert_eq!(document["hits"], json!([]));
    assert_eq!(document["notes"], json!([]));
    assert_eq!(
        document["created"][0],
        json!({"id": "PR-0001", "node": "PIECE-COUNTER",
            "severity": item.drift.as_ref().unwrap().severity.as_str(),
            "summary": item.drift.as_ref().unwrap().summary})
    );
    for quiet in [
        "GOAL-01",
        "LAW-STEP",
        "PREM-001",
        "LAW-ODD",
        "cap-check/PASS-01",
    ] {
        assert!(
            !verify_rows(&repo, quiet).is_empty(),
            "{quiet} has findings"
        );
    }
}

/// TE-1 `code_ahead code_changed`: the locked `CAP` changed by its hash
/// and committed: `LAW-BOUND` raised. M: a changed item read `ok`.
#[test]
fn code_changed_raises_its_node() {
    let repo = drifted("dr-code-changed", cap_changed);
    let raised = raised(&repo, &[]);
    assert_eq!(raised.created_nodes(), ["LAW-BOUND", "PIECE-COUNTER"]);
    assert_item(
        &repo,
        &raised,
        SLUG,
        "LAW-BOUND",
        &["code_ahead code_changed"],
    );
}

/// TE-1 `code_ahead code_changed` of a RON value (by its blob) and its
/// file's root entry: `LAW-BOUND` and `PIECE-COUNTER`.
#[test]
fn ron_value_changed_raises_its_nodes() {
    let repo = drifted("dr-ron-changed", |repo| {
        repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    });
    let raised = raised(&repo, &[]);
    assert_eq!(raised.created_nodes(), ["LAW-BOUND", "PIECE-COUNTER"]);
    assert_item(
        &repo,
        &raised,
        SLUG,
        "LAW-BOUND",
        &["code_ahead code_changed"],
    );
    assert_item(
        &repo,
        &raised,
        SLUG,
        "PIECE-COUNTER",
        &["code_ahead code_changed"],
    );
}

/// TE-1 `code_ahead raised`: `LAW-BOUND`'s text and `rev` raised, `CAP`'s
/// marker raised to it and `CAP` changed.
#[test]
fn raised_marker_with_changed_code_raises_its_node() {
    let repo = drifted("dr-raised", |repo| {
        repo.replace(COUNTER_MD, "#LAW-BOUND rev=2", "#LAW-BOUND rev=3");
        repo.replace(
            COUNTER_MD,
            "The count never exceeds the cap.",
            "The count never exceeds the cap, ever.",
        );
        repo.replace(
            COUNTER,
            "@configures LAW-BOUND@2",
            "@configures LAW-BOUND@3",
        );
        cap_changed(repo);
    });
    let raised = raised(&repo, &[]);
    assert!(raised.created_nodes().contains(&"LAW-BOUND".to_owned()));
    assert_item(&repo, &raised, SLUG, "LAW-BOUND", &["code_ahead raised"]);
}

/// TE-1 `spec_ahead marker_behind`: `CAP`'s marker lowered below its
/// node's revision.
#[test]
fn lagging_marker_raises_its_node() {
    let repo = drifted("dr-lagging", |repo| {
        repo.replace(
            COUNTER,
            "@configures LAW-BOUND@2",
            "@configures LAW-BOUND@1",
        );
    });
    let raised = raised(&repo, &[]);
    assert_eq!(raised.created_nodes(), ["LAW-BOUND", "PIECE-COUNTER"]);
    assert_item(
        &repo,
        &raised,
        SLUG,
        "LAW-BOUND",
        &["spec_ahead marker_behind"],
    );
}

/// TE-1 `spec_ahead spec_changed`: `PIECE-COUNTER`'s `rev` raised with its
/// text, code unchanged.
#[test]
fn raised_revision_raises_spec_changed() {
    let repo = drifted("dr-spec-changed", |repo| {
        repo.replace(COUNTER_MD, "rev: 3\n", "rev: 4\n");
        repo.replace(
            COUNTER_MD,
            "A count that grows one step",
            "A count that grows by one step",
        );
    });
    let raised = raised(&repo, &[]);
    assert_eq!(raised.created_nodes(), ["PIECE-COUNTER"]);
    assert_item(
        &repo,
        &raised,
        SLUG,
        "PIECE-COUNTER",
        &["spec_ahead spec_changed", "spec_ahead marker_behind"],
    );
}

/// TE-1 `spec_ahead same_revision`: `LAW-BOUND`'s text edited at its
/// locked revision.
#[test]
fn text_changed_at_its_revision_raises_same_revision() {
    let repo = drifted("dr-same-revision", |repo| {
        repo.replace(
            COUNTER_MD,
            "The count never exceeds the cap.",
            "The count never exceeds the cap, ever.",
        );
    });
    let raised = raised(&repo, &[]);
    assert_eq!(raised.created_nodes(), ["LAW-BOUND", "PIECE-COUNTER"]);
    assert_item(
        &repo,
        &raised,
        SLUG,
        "LAW-BOUND",
        &["spec_ahead same_revision"],
    );
}

/// TE-1 `conflict`: `LAW-BOUND`'s text at its revision and `CAP` both
/// changed.
#[test]
fn spec_and_code_changed_raises_conflict() {
    let repo = drifted("dr-conflict", |repo| {
        repo.replace(
            COUNTER_MD,
            "The count never exceeds the cap.",
            "The count never exceeds the cap, ever.",
        );
        cap_changed(repo);
    });
    let raised = raised(&repo, &[]);
    assert_eq!(raised.created_nodes(), ["LAW-BOUND", "PIECE-COUNTER"]);
    let item = assert_item(&repo, &raised, SLUG, "LAW-BOUND", &["conflict"]);
    assert_eq!(item.drift.unwrap().severity, IntakeSeverity::High);
}

/// TE-1 `broken_binding file_gone`: the RON file deleted.
#[test]
fn deleted_file_raises_file_gone() {
    let repo = drifted("dr-file-gone", |repo| {
        fs::remove_file(repo.root.join(LIMITS)).expect("remove");
    });
    let raised = raised(&repo, &[]);
    assert_eq!(raised.created_nodes(), ["LAW-BOUND", "PIECE-COUNTER"]);
    assert_item(
        &repo,
        &raised,
        SLUG,
        "LAW-BOUND",
        &["broken_binding file_gone"],
    );
    assert_item(
        &repo,
        &raised,
        SLUG,
        "PIECE-COUNTER",
        &["broken_binding file_gone"],
    );
}

/// TE-1 `broken_binding symbol_gone`: the bound test function renamed.
#[test]
fn renamed_symbol_raises_symbol_gone() {
    let repo = drifted("dr-symbol-gone", |repo| {
        repo.replace(BOUNDS, "fn stops_at_cap", "fn stops_at_the_cap");
    });
    let raised = raised(&repo, &[]);
    assert!(raised.created_nodes().contains(&"LAW-BOUND".to_owned()));
    assert_item(
        &repo,
        &raised,
        SLUG,
        "LAW-BOUND",
        &["broken_binding symbol_gone"],
    );
}

/// TE-1 `broken_binding marker_gone`: a locked item's and a locked RON
/// value's markers removed.
#[test]
fn removed_marker_raises_marker_gone() {
    let repo = drifted("dr-marker-gone", |repo| {
        repo.replace(COUNTER, " // @configures LAW-BOUND@2", "");
        repo.replace(LIMITS, " // @configures LAW-BOUND@2", "");
    });
    let raised = raised(&repo, &[]);
    assert_eq!(raised.created_nodes(), ["LAW-BOUND", "PIECE-COUNTER"]);
    assert_item(
        &repo,
        &raised,
        SLUG,
        "LAW-BOUND",
        &["broken_binding marker_gone"],
    );
}

/// TE-1 `broken_binding node_gone`: `LAW-BOUND` renamed, its leftover
/// markers removed: the gone ID is the item's node (its entries in the
/// lock, no node of it at the commit), raised by a sweep and named; the
/// renamed node (never locked) is not.
#[test]
fn gone_node_raises_node_gone_under_its_id() {
    let repo = drifted("dr-node-gone", |repo| {
        repo.replace(COUNTER_MD, "#LAW-BOUND rev=2", "#LAW-LIMIT rev=2");
        let counter = repo.read(COUNTER);
        let without: String = counter
            .lines()
            .map(|line| match line.find("// @") {
                Some(at) if line[at..].contains("LAW-BOUND") => line[..at].trim_end().to_owned(),
                _ => line.to_owned(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        repo.write(COUNTER, format!("{without}\n"));
        for file in [BOUNDS, LIMITS] {
            let text = repo.read(file);
            let without: String = text
                .lines()
                .map(|line| match line.find("// @") {
                    Some(at) if line[at..].contains("LAW-BOUND") => {
                        line[..at].trim_end().to_owned()
                    }
                    _ => line.to_owned(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            repo.write(file, format!("{without}\n"));
        }
    });
    let raised = raised(&repo, &[]);
    assert!(
        raised.created_nodes().contains(&"LAW-BOUND".to_owned()),
        "{}",
        raised.text()
    );
    assert!(!raised.created_nodes().contains(&"LAW-LIMIT".to_owned()));
    assert_item(
        &repo,
        &raised,
        SLUG,
        "LAW-BOUND",
        &["broken_binding node_gone"],
    );
    // Named by its gone ID: the same item, a hit.
    let named = raise(&repo, &["LAW-BOUND"]);
    named.ok("named gone ID");
    let hits = named.hits();
    assert_eq!(hits.len(), 1, "{}", named.json_text());
    assert_eq!(hits[0].1, "LAW-BOUND");
}

/// TE-1 on the second fixture (an ECS-style crate, every binding sound):
/// a system's body changed and committed raises its perk alone, every row
/// class `check`: severity `normal`. M: `high` whatever the class.
#[test]
fn ecs_system_changed_raises_a_normal_item() {
    let repo = ecs_drifted("dr-ecs", |repo| {
        repo.replace(
            STAMINA,
            "(stamina.current + rate * dt).min(stamina.max)",
            "(stamina.current + rate * dt * 2.0).min(stamina.max)",
        );
    });
    let raised = raised(&repo, &[]);
    assert_eq!(raised.created_nodes(), ["PERK-REGEN"], "{}", raised.text());
    let item = assert_item(
        &repo,
        &raised,
        "verify-ecs",
        "PERK-REGEN",
        &["code_ahead code_changed"],
    );
    let drift = item.drift.expect("drift");
    assert_eq!(drift.severity, IntakeSeverity::Normal);
    assert_eq!(drift.summary, "code_ahead code_changed");
}

// --------------------------------------------- lock-levels.md TE-4

/// `regen_system`'s symbol in verify-ecs.
const REGEN_SYMBOL: &str = "crates/game::stamina::regen_system";

/// `regen_system`'s first line in `source`.
fn regen_line(source: &[u8]) -> usize {
    String::from_utf8_lossy(source)
        .lines()
        .position(|line| line.starts_with("pub fn regen_system("))
        .expect("regen_system")
        + 1
}

/// The `drift_hash` docs/features/lock-levels.md "Drift evidence" gives
/// `PERK-REGEN`'s item when its one non-`ok` row is `regen_system`'s
/// `code_ahead code_changed` at `HEAD` (its `@1` marker), built here from
/// the lock at `HEAD` (`lock_code`: the entry's `levels`, `sig_hash`,
/// `body_hash`, `deps_hash` lines as written, LF-joined) and `code`, the
/// row's fact.
fn regen_drift_hash(repo: &LockRepo, code: &str) -> String {
    let lock = repo.lock_text();
    let block = lock_common::node_blocks(&lock, "PERK-REGEN")
        .into_iter()
        .find(|block| block.contains(&format!("\nsymbol = \"{REGEN_SYMBOL}\"\n")))
        .expect("regen_system's entry");
    let field = |key: &str| {
        block
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key} = ")))
            .unwrap_or_else(|| panic!("no {key}"))
            .to_owned()
    };
    let lock_code: Vec<&str> = block
        .lines()
        .filter(|line| {
            ["levels = ", "sig_hash = ", "body_hash = ", "deps_hash = "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .collect();
    assert_eq!(lock_code.len(), 4, "{block}");
    let lock_code = lock_code.join("\n");
    let rev: u32 = field("rev").parse().expect("a revision");
    let spec_hash = field("spec_hash").trim_matches('"').to_owned();
    let file_blob = field("file_blob").trim_matches('"').to_owned();
    let norm = repo.norm_hash("PERK-REGEN");
    let row = HashedRow {
        node: "PERK-REGEN",
        relation: "implements",
        file: STAMINA,
        kind: Some("item"),
        symbol: Some(REGEN_SYMBOL),
        item_kind: Some("function_item"),
        state: State::CodeAhead,
        category: Some("code_changed"),
        detail: None,
        marker_revs: vec![MarkerRev::Written(1)],
        entry: Some(HashedEntry {
            rev,
            spec_hash: &spec_hash,
            code: Some(&lock_code),
            file_blob: &file_blob,
        }),
        rev: Some(1),
        norm_hash: Some(&norm),
        code: Some(code),
    };
    specengine_store::b3_hash(drift_hash_input(&[row]).as_bytes())
}

/// lock-levels.md TE-4 (`item_code_names_its_changed_levels`; Q13): an
/// item compared changed stores, in its row's `code`, `<level>:<digest>`
/// per changed level of `sig` and `body`, this run's digest, `,`-joined
/// in canonical order: a body edit `body:<digest>`, a signature edit
/// `sig:<digest>`, an edit of both `sig:<digest>,body:<digest>`; the
/// entry's evidence (`lock_code`) its `levels` and three digest lines,
/// never a fingerprint. The item's `drift_hash` is checked against the
/// hash built from those facts by hand; never the item's whole digest.
#[test]
fn item_code_names_its_changed_levels() {
    let cases: [(&str, &str, &str, &[&str]); 3] = [
        (
            "body",
            "(stamina.current + rate * dt).min(stamina.max)",
            "(stamina.current + rate * dt * 2.0).min(stamina.max)",
            &["body"],
        ),
        (
            "sig",
            "pub fn regen_system(stamina: &mut Stamina, rate: f32, dt: f32)",
            "pub fn regen_system(stamina: &mut Stamina, rate: f32, dt: f64)",
            &["sig"],
        ),
        (
            "both",
            "pub fn regen_system(stamina: &mut Stamina, rate: f32, dt: f32) {\n    // Rest restores a fixed amount per second.\n    stamina.current = (stamina.current + rate * dt)",
            "pub fn regen_system(stamina: &mut Stamina, rate: f32, step: f32) {\n    // Rest restores a fixed amount per second.\n    stamina.current = (stamina.current + rate * step)",
            &["sig", "body"],
        ),
    ];
    let mut failures = Vec::new();
    for (label, from, to, changed) in cases {
        let repo = ecs_drifted(&format!("dr-levels-{label}"), |repo| {
            repo.replace(STAMINA, from, to);
        });
        let head = repo.head();
        let before = repo.show_blob(&format!("{head}~1"), STAMINA);
        let after = repo.show_blob(&head, STAMINA);
        let old = lock_common::item_code(
            &before,
            regen_line(&before),
            "function_item",
            &["sig", "body"],
        );
        let new = lock_common::item_code(
            &after,
            regen_line(&after),
            "function_item",
            &["sig", "body"],
        );
        // The premise: exactly the named levels changed.
        let differs = [("sig", old.sig != new.sig), ("body", old.body != new.body)];
        let named: Vec<&str> = differs
            .iter()
            .filter(|(_, differs)| *differs)
            .map(|(level, _)| *level)
            .collect();
        assert_eq!(named, changed, "{label}: the premise");
        let code: Vec<String> = changed
            .iter()
            .map(|level| match *level {
                "sig" => format!("sig:{}", new.sig),
                _ => format!("body:{}", new.body),
            })
            .collect();
        let code = code.join(",");

        let raised = raised(&repo, &[]);
        assert_eq!(
            raised.created_nodes(),
            ["PERK-REGEN"],
            "{label}: {}",
            raised.text()
        );
        let item = assert_item(
            &repo,
            &raised,
            "verify-ecs",
            "PERK-REGEN",
            &["code_ahead code_changed"],
        );
        let stored = item.drift.expect("drift").hash;
        let want = regen_drift_hash(&repo, &code);
        if stored != want {
            failures.push(format!(
                "{label}: code {code}: stored {stored}, built {want}"
            ));
        }
        // Never the whole digest, nor the blob.
        let whole = regen_drift_hash(&repo, &new.body);
        assert_ne!(stored, whole, "{label}: a digest without its level");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ------------------------------------------------------------------ RD-2

/// RD-2: a sweep never queues a never-locked node (`LAW-STEP`'s and
/// `PREM-001`'s `unlocked` rows); named, `LAW-STEP` is queued, its
/// `unlocked` rows in the item. M: `unlocked` raising in a sweep.
#[test]
fn sweep_never_queues_a_never_locked_node() {
    let repo = golden("dr-unlocked");
    let sweep = raised(&repo, &[]);
    for never in ["LAW-STEP", "PREM-001"] {
        assert!(
            verify_rows(&repo, never)
                .iter()
                .any(|row| row["state"] == "unlocked" && row["lock"].is_null()),
            "{never} has unlocked rows"
        );
        assert!(
            !sweep.created_nodes().contains(&never.to_owned()),
            "{never} queued by a sweep: {}",
            sweep.text()
        );
    }
    let named = raised(&repo, &["LAW-STEP"]);
    assert_eq!(named.created_nodes(), ["LAW-STEP"], "{}", named.text());
    assert_item(&repo, &named, SLUG, "LAW-STEP", &["unlocked"]);
    assert_eq!(named.json()["judged"]["nodes"], json!(1));
}

/// RD-2: `cannot_verify` alone raises nothing: `LAW-ODD` (only
/// `cannot_verify rev_unknown`), named, is noted `no drift at HEAD` and
/// stores nothing. M: `cannot_verify` raising.
#[test]
fn cannot_verify_alone_raises_nothing() {
    let repo = golden("dr-cannot");
    let rows = verify_rows(&repo, "LAW-ODD");
    assert!(
        !rows.is_empty() && rows.iter().all(|row| row["state"] == "cannot_verify"),
        "{rows:?}"
    );
    let named = raised(&repo, &["LAW-ODD"]);
    assert_eq!(named.created(), []);
    assert_eq!(named.hits(), []);
    assert_eq!(named.notes(), ["LAW-ODD: no drift at HEAD"]);
    assert!(
        named
            .stderr()
            .contains(&"note: LAW-ODD: no drift at HEAD".to_owned()),
        "{:?}",
        named.stderr()
    );
    assert!(proposals(&repo, SLUG).is_empty(), "nothing stored");
}

/// RD-2: a named node whose `unlocked` rows are all `key_problem` (a file
/// path the lock cannot hold) is a note, never an item:
/// `note: GOAL-01: 1 binding(s) cannot be locked (key_problem); spec verify
/// names each`, then `no drift at HEAD`. M: a `key_problem` row raising.
#[test]
fn named_key_problem_is_a_note() {
    let repo = drifted("dr-key-problem", |repo| {
        repo.write(
            "crates/tally/src/we\\ird.rs",
            "// @implements GOAL-01\npub fn odd() {}\n",
        );
    });
    let rows = verify_rows(&repo, "GOAL-01");
    assert!(
        rows.iter()
            .any(|row| row["state"] == "unlocked" && row["detail"] == "key_problem"),
        "{rows:?}"
    );
    let named = raised(&repo, &["GOAL-01"]);
    assert_eq!(named.created(), [], "{}", named.json_text());
    assert_eq!(
        named.notes(),
        [
            "GOAL-01: 1 binding(s) cannot be locked (key_problem); spec verify names each",
            "GOAL-01: no drift at HEAD"
        ]
    );
    assert!(proposals(&repo, SLUG).is_empty());
}

/// "Triggers", "The item": a named node with more than 64 findings stores
/// the first 64 by key and counts the rest in `more`; its hash covers
/// every row. M: every row stored; `more` dropped.
#[test]
fn the_first_64_rows_are_stored_the_rest_counted() {
    let repo = drifted("dr-many", |repo| {
        let mut text = String::new();
        for n in 0..70 {
            text.push_str(&format!(
                "// @implements LAW-STEP\npub fn step_{n:02}() {{}}\n\n"
            ));
        }
        repo.write("crates/tally/src/many.rs", text);
        repo.append("crates/tally/src/lib.rs", "pub mod many;\n");
    });
    let all = verify_rows(&repo, "LAW-STEP");
    assert!(all.len() > 64, "{} rows", all.len());
    let named = raised(&repo, &["LAW-STEP"]);
    let (id, _) = named.created().remove(0);
    let item = proposal(&repo, SLUG, &id).drift.expect("drift");
    let stored: Vec<Value> = item
        .evidence
        .rows
        .iter()
        .map(|row| serde_json::to_value(row).unwrap())
        .collect();
    assert_eq!(stored, all[..64], "the first 64 by key");
    assert_eq!(item.evidence.more, (all.len() - 64) as u64);
}

// ------------------------------------------------------------------ RD-3

/// RD-3: an edit not committed is never judged: `CAP` changed on disk
/// raises nothing for `LAW-BOUND` until committed. M: the disk judged.
#[test]
fn uncommitted_edit_is_not_queued() {
    let repo = golden("dr-uncommitted");
    cap_changed(&repo);
    let before = raised(&repo, &[]);
    assert_eq!(
        before.created_nodes(),
        ["PIECE-COUNTER"],
        "{}",
        before.text()
    );
    repo.commit_all("the cap");
    let after = raised(&repo, &[]);
    assert_eq!(after.created_nodes(), ["LAW-BOUND"], "{}", after.text());
    assert_item(
        &repo,
        &after,
        SLUG,
        "LAW-BOUND",
        &["code_ahead code_changed"],
    );
}

/// RD-3: the commit's side alone: with the committed `counter.rs`
/// unreadable on disk (mode 000) the raise stores what a twin repository
/// stores (the same drift column and hash), no `unreadable` row. M: a
/// working-tree file read.
#[test]
fn raise_ignores_an_unreadable_disk_file() {
    let repo = drifted("dr-unreadable", cap_changed);
    let twin = drifted("dr-unreadable-twin", cap_changed);
    assert_eq!(repo.head(), twin.head(), "the same commits");
    let path = repo.root.join(COUNTER);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let raised = raise(&repo, &[]);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod back");
    raised.ok("an unreadable disk file");
    let twin_raised = raised_twin(&twin);
    assert_eq!(raised.text(), twin_raised.text());
    assert_eq!(raised.json_text(), twin_raised.json_text());
    for (mine, theirs) in proposals(&repo, SLUG).iter().zip(proposals(&twin, SLUG)) {
        assert_eq!(mine.drift, theirs.drift, "{}", mine.id);
        let rows = &mine.drift.as_ref().unwrap().evidence.rows;
        assert!(
            rows.iter()
                .all(|row| row.category.as_deref() != Some("unreadable")),
            "{rows:?}"
        );
    }
}

fn raised_twin(repo: &LockRepo) -> Raised {
    raised(repo, &[])
}

/// RD-3: the raise judges the commit its place bound, never `HEAD` read
/// again: a clean filter on `specengine.toml` moves `main` while the
/// config's state is read (after the place); the item is of the bound
/// commit (`CAP` changed: `LAW-BOUND`), `base_commit` it, though `main`
/// names the earlier commit after the raise. M: `HEAD` re-read.
#[test]
fn raise_judges_the_bound_base_commit() {
    let repo = golden("dr-bound");
    let earlier = repo.head();
    cap_changed(&repo);
    repo.commit_all("the cap");
    let bound = repo.head();
    let flag = repo.scratch.join("move-head");
    let git = repo
        .git
        .git_program()
        .to_str()
        .expect("a UTF-8 git")
        .to_owned();
    let script = repo.scratch.join("filter.sh");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nif [ -f '{flag}' ]; then rm -f '{flag}'; '{git}' update-ref refs/heads/main {earlier}; fi\ncat\n",
            flag = flag.display()
        ),
    )
    .expect("the filter");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable");
    repo.git(&["config", "filter.move.clean", script.to_str().unwrap()]);
    fs::create_dir_all(repo.root.join(".git/info")).expect("info");
    fs::write(
        repo.root.join(".git/info/attributes"),
        "specengine.toml filter=move\n",
    )
    .expect("attributes");
    // The config's stat changed, its bytes not: git status hashes it again
    // through the filter.
    let config = repo.root.join("specengine.toml");
    let file = fs::File::options().write(true).open(&config).expect("open");
    file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(60))
        .expect("mtime");
    drop(file);
    fs::write(&flag, "").expect("arm the filter");
    let raised = raise(&repo, &[]);
    raised.ok("raise while HEAD moves");
    assert!(!flag.exists(), "the filter ran during the raise");
    assert_eq!(
        repo.git_text(&["rev-parse", "refs/heads/main"]),
        earlier,
        "main moved under the raise"
    );
    let document = raised.json();
    assert_eq!(document["commit"], json!(bound), "the bound commit");
    assert!(
        raised.created_nodes().contains(&"LAW-BOUND".to_owned()),
        "the bound commit judged: {}",
        raised.text()
    );
    let law = proposals(&repo, SLUG)
        .into_iter()
        .find(|stored| stored.target_id == "LAW-BOUND")
        .expect("LAW-BOUND's item");
    assert_eq!(law.place.base_commit, bound);
}

/// RD-3: a REF only the working tree has is refused, exit 1: `` `LAW-FRESH`
/// names no node at HEAD: commit it first ``; a REF naming nothing is
/// refused too, not with that clause; nothing stored. M: the REF resolved
/// over the working tree.
#[test]
fn uncommitted_ref_is_refused() {
    let repo = golden("dr-uncommitted-ref");
    repo.append(
        "docs/spec/limits.md",
        "\n## Fresh {#LAW-FRESH}\n\nA law not yet committed.\n",
    );
    let refused = raise(&repo, &["LAW-FRESH"]);
    assert_eq!(
        refused.refused("a working-tree node"),
        "`LAW-FRESH` names no node at HEAD: commit it first"
    );
    assert_eq!(refused.text(), "", "nothing on stdout");
    let nothing = raise(&repo, &["LAW-NOPE"]);
    let reason = nothing.refused("a REF naming nothing");
    assert!(reason.contains("LAW-NOPE"), "{reason}");
    assert!(!reason.contains("commit it first"), "{reason}");
    assert!(proposals(&repo, SLUG).is_empty());
}

// ------------------------------------------------------------ RD-14, TE-9

/// RD-14 / TE-9: the same commit raised from another root (a copy), from
/// a subdirectory, in a repository built in reverse creation order and on
/// a CRLF checkout (`core.autocrlf`): stdout, JSON and the stored items'
/// drift, hash, summary and severity byte-identical. M: walk order or the
/// disk leaking into the rows.
#[test]
fn drift_raise_is_byte_identical_across_roots_and_order() {
    let repo = drifted("dr-same", cap_changed);
    let first = raised(&repo, &[]);
    let reference = (first.text(), first.json_text(), items(&repo));

    // Another root: a byte copy of the repository.
    let copy = clone_dir(&repo, "copy");
    let other = LockRepo {
        scratch: common::Scratch::new("dr-same-other"),
        git: common::git::Sandbox::new(repo.scratch.path()),
        root: copy,
        home: repo.scratch.home("copy-home"),
    };
    let from_copy = raised(&other, &[]);
    assert_eq!(
        (from_copy.text(), from_copy.json_text(), items(&other)),
        reference
    );

    // Another cwd: a subdirectory of the root.
    let sub = drifted("dr-same-sub", cap_changed);
    let from_sub = raise_at(&sub, &sub.root.join("crates/tally"), &[], RAISED);
    from_sub.ok("from a subdirectory");
    assert_eq!(
        (from_sub.text(), from_sub.json_text(), items(&sub)),
        reference
    );

    // Reverse creation order: the fixture's files written last-first.
    let reversed = LockRepo::uncommitted("dr-same-reversed");
    let mut files: Vec<(String, Option<Vec<u8>>)> =
        common::snapshot(&reversed.root).into_iter().collect();
    for (path, _) in &files {
        let full = reversed.root.join(path);
        if full.is_file() {
            fs::remove_file(&full).expect("remove");
        }
    }
    files.reverse();
    for (path, bytes) in files {
        if let Some(bytes) = bytes {
            reversed.write(&path, bytes);
        }
    }
    reversed.git.init(&reversed.root);
    reversed.git.add_all(&reversed.root);
    reversed.git.commit(&reversed.root, "the fixture");
    reversed.write(
        "spec.lock",
        fs::read(verify_common::golden_file()).expect("the golden lock"),
    );
    reversed.commit_all("the lock");
    cap_changed(&reversed);
    reversed.commit_all("the edit");
    assert_eq!(reversed.head(), repo.head(), "the same commits");
    let from_reversed = raised(&reversed, &[]);
    assert_eq!(
        (
            from_reversed.text(),
            from_reversed.json_text(),
            items(&reversed)
        ),
        reference
    );

    // A CRLF checkout of the same commit.
    let crlf = drifted("dr-same-crlf", cap_changed);
    crlf.git(&["config", "core.autocrlf", "true"]);
    for (path, bytes) in common::snapshot(&crlf.root) {
        if bytes.is_some() && !path.starts_with(".git/") && path != ".git" {
            fs::remove_file(crlf.root.join(&path)).expect("remove");
        }
    }
    crlf.git(&["checkout", "--", "."]);
    assert!(crlf.read(COUNTER).contains("\r\n"), "a CRLF checkout");
    let from_crlf = raised(&crlf, &[]);
    assert_eq!(
        (from_crlf.text(), from_crlf.json_text(), items(&crlf)),
        reference
    );
}

/// The stored items as the raise leaves them, place aside: `(target,
/// severity, summary, drift column, hash)`.
fn items(repo: &LockRepo) -> Vec<(String, String, String, String, String)> {
    proposals(repo, SLUG)
        .into_iter()
        .map(|stored| {
            let item = stored.drift.expect("drift");
            (
                stored.target_id,
                item.severity.as_str().to_owned(),
                item.summary,
                serde_json::to_string(&item.evidence).unwrap(),
                item.hash,
            )
        })
        .collect()
}

// ------------------------------------------------------------ TE-3, TE-10

/// What a refused or failed raise must leave as it was: the repository
/// (`HEAD`, refs, `git status` with untracked files, the index, every
/// file, `spec.lock`) and the queue (`dump()`: every row and event).
fn assert_unchanged(repo: &LockRepo, before: &(lock_common::RepoState, String), context: &str) {
    let after = (repo.state(), dump(repo, SLUG));
    assert_eq!(after.0, before.0, "{context}: the repository");
    assert_eq!(after.1, before.1, "{context}: the queue");
}

/// TE-3, the raise's exits 1: a REF naming nothing, a REF naming several
/// nodes, a working-tree-only node, a refused `--task`, a `specengine.toml`
/// modified, staged, untracked: each refused with its reason (the dirty
/// config's in the spec's words), nothing on stdout, the repository and
/// the queue unchanged.
#[test]
fn raise_exits_1_change_nothing() {
    let repo = drifted("dr-exit-1", |repo| {
        // A second feature defining `PASS-01`: the bare ID names two nodes.
        repo.write(
            "docs/features/step-check.md",
            "---\nclass: spec\nstatus: draft\nscope: [crates/tally]\n---\n\n# Step check\n\n\
             ## Acceptance criteria\n\n### One step adds one {#PASS-01}\n\nStep once from zero: \
             it reads 1.\n",
        );
    });
    raised(&repo, &[]);
    let before = (repo.state(), dump(&repo, SLUG));
    for (label, references, task) in [
        ("a REF naming nothing", vec!["LAW-NOPE"], None),
        ("a REF naming several nodes", vec!["PASS-01"], None),
        ("an unknown task", vec![], Some("T-0099")),
    ] {
        let refused = raise_with_task(&repo, &repo.root, &references, RAISED, task);
        let reason = refused.refused(label);
        assert!(!reason.is_empty(), "{label}");
        assert_eq!(refused.text(), "", "{label}: nothing on stdout");
        assert_eq!(refused.json()["created"], json!([]), "{label}");
        assert_unchanged(&repo, &before, label);
    }
    // The config dirty: modified, then staged.
    let config = repo.read("specengine.toml");
    repo.write("specengine.toml", format!("{config}\n# edited\n"));
    let before_dirty = (repo.state(), dump(&repo, SLUG));
    let refused = raise(&repo, &[]);
    assert_eq!(
        refused.refused("modified config"),
        "`specengine.toml` is modified: drift is judged under the config committed at HEAD; \
         commit it first; nothing queued"
    );
    assert_unchanged(&repo, &before_dirty, "modified config");
    repo.git(&["add", "specengine.toml"]);
    let before_staged = (repo.state(), dump(&repo, SLUG));
    let refused = raise(&repo, &[]);
    let reason = refused.refused("staged config");
    assert!(
        reason.starts_with("`specengine.toml` is ")
            && reason.ends_with("commit it first; nothing queued"),
        "{reason}"
    );
    assert_unchanged(&repo, &before_staged, "staged config");
}

/// TE-3, the raise's exits 2: 17 REFs, a malformed author, a look-alike
/// REF, a detached `HEAD`, an unborn `HEAD`, a root outside git, a queue
/// of a newer schema: exit 2 naming its cause; the repository and the
/// queue unchanged. M: a check after the insert.
#[test]
fn raise_exits_2_change_nothing() {
    let repo = golden("dr-exit-2");
    raised(&repo, &[]);
    let before = (repo.state(), dump(&repo, SLUG));
    let many: Vec<String> = (1..=17).map(|n| format!("LAW-{n}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    let message = raise(&repo, &many).cannot("17 REFs");
    assert!(
        message.contains("17 REFs: name at most 16 nodes, or none for every drifting node"),
        "{message}"
    );
    assert_unchanged(&repo, &before, "17 REFs");
    let message = raise(&repo, &["LAW-B\u{041E}UND"]).cannot("a look-alike");
    assert!(!message.is_empty());
    assert_unchanged(&repo, &before, "a look-alike");
    let bad_author = specengine_cli::propose_drift(
        &repo.env(&repo.root),
        &specengine_cli::Globals::default(),
        &specengine_cli::DriftRequest {
            references: Vec::new(),
            author_role: Some("not a role!".to_owned()),
            author_model: None,
            run: None,
            now: RAISED.to_owned(),
            git: repo.git_env(&repo.root),
        },
        None,
    );
    assert_eq!(
        bad_author.expect_err("a malformed author").exit,
        specengine_cli::Exit::CannotRun
    );
    assert_unchanged(&repo, &before, "a malformed author");

    // Detached HEAD.
    let head = repo.head();
    repo.git(&["checkout", "-q", "--detach", &head]);
    let before_detached = (repo.state(), dump(&repo, SLUG));
    let message = raise(&repo, &[]).cannot("a detached HEAD");
    assert!(message.to_lowercase().contains("detached"), "{message}");
    assert_unchanged(&repo, &before_detached, "a detached HEAD");
    repo.git(&["checkout", "-q", "main"]);

    // A queue of a newer schema.
    sql(&repo, SLUG, "PRAGMA user_version = 9;");
    let message = raise(&repo, &[]).cannot("a newer queue");
    assert!(message.contains('9') && message.contains('8'), "{message}");
    assert_eq!(sql(&repo, SLUG, "PRAGMA user_version;"), "9", "left alone");
    sql(&repo, SLUG, "PRAGMA user_version = 8;");
    assert_unchanged(&repo, &before, "a newer queue");

    // An unborn HEAD; a root outside git.
    let unborn = LockRepo::uncommitted("dr-exit-2-unborn");
    unborn.git.init(&unborn.root);
    let message = raise(&unborn, &[]).cannot("an unborn HEAD");
    assert!(!message.is_empty());
    assert!(proposals(&unborn, SLUG).is_empty());
    let loose = LockRepo::uncommitted("dr-exit-2-loose");
    let message = raise(&loose, &[]).cannot("no worktree");
    assert!(!message.is_empty());
    assert!(proposals(&loose, SLUG).is_empty());
}

/// TE-10: `fixtures/spec-c` (no `[code]`) committed: exit 2 naming
/// `[code]`, nothing stored. M: a corpus without code judged.
#[test]
fn a_project_without_code_exits_2_naming_it() {
    let repo = LockRepo::of("dr-spec-c", "spec-c", "repo", &[]);
    let slug = "dock-hire";
    let before = repo.state();
    let message = raise(&repo, &[]).cannot("spec-c");
    assert!(message.contains("[code]"), "{message}");
    assert_eq!(repo.state(), before);
    assert!(proposals(&repo, slug).is_empty());
}

/// TE-3 through the binary (stdin null): a refusal prints nothing on
/// stdout and its reason on stderr, exit 1; exit 2 likewise; a raise that
/// answers exits 0 with the text. The process clock never reaches the
/// rows.
#[test]
fn the_binary_exits_as_the_library() {
    let repo = drifted("dr-binary", cap_changed);
    let run = repo.spec_piped(&repo.root, &["propose", "drift", "LAW-NOPE"]);
    run.code(1);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.contains("LAW-NOPE"), "{}", run.show());
    let run = repo.spec_piped(
        &repo.root,
        &["propose", "drift", "--author-role", "not a role!"],
    );
    run.code(2);
    assert_eq!(run.stdout, "");
    let run = repo.spec_piped(&repo.root, &["propose", "drift", "--json"]);
    run.code(0);
    let document = run.json();
    assert_eq!(document["created"].as_array().map(Vec::len), Some(2));
    let law = proposals(&repo, SLUG)
        .into_iter()
        .find(|stored| stored.target_id == "LAW-BOUND")
        .expect("LAW-BOUND's item");
    assert_eq!(law.author.role, None, "no agent flags: the owner's");
    let twin = drifted("dr-binary-twin", cap_changed);
    raised(&twin, &[]);
    let theirs = proposals(&twin, SLUG)
        .into_iter()
        .find(|stored| stored.target_id == "LAW-BOUND")
        .unwrap();
    assert_eq!(law.drift, theirs.drift, "the clock never in the rows");
}

// ----------------------------------------------------------- notes parity

/// `docs/features/drift-queue.md` "Output" (iteration 2, notes parity): at
/// a REF refusal the earlier REF's ignored-`@rev` note (`` `@2` is
/// ignored: `spec propose drift` judges the node at HEAD ``) is a `note:`
/// line on stderr and a JSON note alike, the JSON notes being every
/// stderr note in order, then the reason. M: the refusal's document notes
/// not built from the messages (`notes_of` unused).
#[test]
fn ref_refusal_notes_on_both_streams() {
    let repo = drifted("dr-notes-ref", cap_changed);
    let refused = raise(&repo, &["LAW-BOUND@2", "LAW-NOPE"]);
    let reason = refused.refused("a REF naming nothing after an `@rev`");
    let stderr: Vec<String> = refused
        .stderr()
        .into_iter()
        .filter_map(|line| line.strip_prefix("note: ").map(str::to_owned))
        .collect();
    assert_eq!(
        stderr,
        ["`@2` is ignored: `spec propose drift` judges the node at HEAD"],
        "{:?}",
        refused.stderr()
    );
    let mut want = stderr.clone();
    want.push(reason.clone());
    assert_eq!(refused.notes(), want, "{}", refused.json_text());
    assert!(proposals(&repo, SLUG).is_empty(), "nothing stored");
}

/// A `git` first on a scratch `PATH`: the real one, except that once the
/// file `armed` exists, the first `ls-tree` (the raise's listing of the
/// commit, after its task check's inputs are read) moves it to `next`, and
/// the first `cat-file --batch` session ending after that (the commit's
/// read, finished before the insert) cancels `T-0001` in `db` through the
/// system's `sqlite3`: the task changes between the raise's own check and
/// the insert's. Every git argument is logged to `log`. The `PATH` value.
fn cancelling_git(
    repo: &LockRepo,
    db: &std::path::Path,
) -> (std::ffi::OsString, std::path::PathBuf, std::path::PathBuf) {
    let dir = repo.scratch.dir("cancelling-git");
    let real = repo
        .git
        .git_program()
        .to_str()
        .expect("a UTF-8 git")
        .to_owned();
    let armed = dir.join("armed");
    let next = dir.join("next");
    let log = dir.join("log");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{log}'\n\
         case \" $* \" in *' ls-tree '*) [ -f '{armed}' ] && mv '{armed}' '{next}' ;; esac\n\
         '{real}' \"$@\"\ncode=$?\n\
         case \" $* \" in *' cat-file --batch '*)\n\
         if [ -f '{next}' ]; then rm -f '{next}'; echo '-- cancelled' >> '{log}'; \
         /usr/bin/sqlite3 -cmd '.timeout 5000' '{db}' \
         \"UPDATE tasks SET status = 'cancelled' WHERE id = 'T-0001';\" >/dev/null || exit 70; fi ;;\n\
         esac\nexit $code\n",
        log = log.display(),
        armed = armed.display(),
        next = next.display(),
        db = db.display(),
    );
    let shim = dir.join("git");
    fs::write(&shim, script).expect("the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("executable");
    (repo.git.path_with(&dir), armed, log)
}

/// `docs/features/drift-queue.md` "Output" (iteration 2, notes parity):
/// a `--task` refused at the insert (the task cancelled after the raise's
/// own check, by a `PATH` shim) keeps the notes judged before it: the
/// named `GOAL-01`'s `key_problem` note and its `no drift at HEAD` are
/// `note:` lines on stderr and JSON notes alike, in order, then the
/// reason ``--task: `T-0001` is cancelled: …``; nothing stored. M: the
/// notes not pushed into the messages before the insert's refusal.
#[test]
fn insert_task_refusal_notes_on_both_streams() {
    let repo = drifted("dr-notes-task", |repo| {
        cap_changed(repo);
        repo.write(
            "crates/tally/src/we\\ird.rs",
            "// @implements GOAL-01\npub fn odd() {}\n",
        );
    });
    // The task, of this repository, bound-to before the shim cancels it.
    raised(&repo, &["LAW-STEP"]);
    let mut queue = drift_common::queue_of(&repo, SLUG).expect("a queue");
    let common = fs::canonicalize(repo.root.join(".git")).expect("the common dir");
    let task = queue
        .create_task(
            &specengine_store::NewTask {
                git_common_dir: common.to_str().expect("UTF-8").to_owned(),
                title: Some("Bound".to_owned()),
                goal: None,
                targets: vec!["LAW-BOUND".to_owned()],
                author: specengine_core::proposal::Author::human(),
            },
            RAISED,
        )
        .expect("a task");
    assert_eq!(task.id, "T-0001");
    drop(queue);
    let before = dump(&repo, SLUG);
    let (path, armed, log) = cancelling_git(&repo, &drift_common::db(&repo, SLUG));
    fs::write(&armed, "").expect("arm the shim");
    let mut vars = repo.git.vars();
    vars.retain(|(name, _)| name != "PATH");
    vars.push(("PATH".into(), path));
    let outcome = specengine_cli::propose_drift(
        &repo.env(&repo.root),
        &specengine_cli::Globals::default(),
        &specengine_cli::DriftRequest {
            references: vec!["GOAL-01".to_owned(), "LAW-BOUND".to_owned()],
            author_role: Some(drift_common::ROLE.to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: RAISED.to_owned(),
            git: specengine_cli::GitEnv::new(&repo.root, vars),
        },
        Some("T-0001"),
    );
    let refused = Raised { outcome };
    let shown_log = fs::read_to_string(&log).unwrap_or_default();
    let reason = refused.refused(&format!(
        "the task cancelled mid-raise; git calls:\n{shown_log}"
    ));
    assert!(
        shown_log.contains("-- cancelled"),
        "the shim cancelled the task:\n{shown_log}"
    );
    assert!(
        reason.starts_with("--task: `T-0001` is cancelled: "),
        "{reason}\n{shown_log}"
    );
    let stderr: Vec<String> = refused
        .stderr()
        .into_iter()
        .filter_map(|line| line.strip_prefix("note: ").map(str::to_owned))
        .collect();
    assert_eq!(
        stderr,
        [
            "GOAL-01: 1 binding(s) cannot be locked (key_problem); spec verify names each",
            "GOAL-01: no drift at HEAD",
        ],
        "{:?}",
        refused.stderr()
    );
    let mut want = stderr.clone();
    want.push(reason.clone());
    assert_eq!(refused.notes(), want, "{}", refused.json_text());
    // Nothing stored but the task's cancellation.
    let after = dump(&repo, SLUG);
    assert_eq!(
        after
            .lines()
            .filter(|line| line.starts_with("proposals\t"))
            .count(),
        before
            .lines()
            .filter(|line| line.starts_with("proposals\t"))
            .count(),
        "no item stored"
    );
}
