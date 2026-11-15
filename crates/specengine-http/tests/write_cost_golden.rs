//! docs/features/write-cost.md (RM-0125) WC9, the hub's half: on every
//! fixture corpus, opted in by a committed `[team]` (`ann` its owner), the
//! confirm's GET and POST of an update and of a question answer byte for byte
//! as 0.47.0 did (`fixtures/write-cost-golden/<corpus>.json`, section `http`):
//! each reply's status and body through the real daemon and the test proxy,
//! the stage before it (the library, on an injected clock), and the commit a
//! POST made (`git show` without its dates). The daemon reads the wall clock
//! and commits on it, so beyond `<root>` and `<sha>` the run's timestamps and
//! date are normalized (`<now>`, `<today>`). The shared halves are the CLI's
//! `write_cost_common/mod.rs` and `write_cost_hub/mod.rs`.
//!
//! WC5 later compares a POST after its GET (a D hit) with these answers.
//!
//! M: findings sorted by another key: red.
//!
//! Capture (at `v0.47.0` only): `SPECENGINE_WRITE_COST_CAPTURE=1 cargo
//! nextest run -p specengine-http --test write_cost_golden`.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/write_cost_common/mod.rs"]
mod write_cost_common;

mod write_cost_hub;

use std::collections::BTreeMap;

use serde_json::{Value, json};
use write_cost_common::{CORPORA, Repo, WallClock, normalized, probe, settle};
use write_cost_hub::{
    answered, confirm_get, confirm_post, hub_env, opted_in, propose_update, raise_question, slug,
    stage_approve, start,
};

/// The GET and the POST of `id` (the POST only after a 200), and the commits
/// the POST made, newest first, into `cases` under `kind`.
fn confirmed(
    repo: &Repo,
    server: &common::Server,
    slug: &str,
    kind: &str,
    id: &str,
    cases: &mut BTreeMap<String, Value>,
) {
    let before = repo.head();
    let read = confirm_get(server, slug, id);
    cases.insert(format!("{kind} confirm GET"), answered(&read));
    if read.status != 200 {
        return;
    }
    let written = confirm_post(server, slug, id, &read);
    cases.insert(format!("{kind} confirm POST"), answered(&written));
    let commits: Vec<Value> = repo
        .commits_since(&before)
        .iter()
        .map(|commit| json!(repo.shown(commit)))
        .collect();
    cases.insert(format!("{kind} commits"), json!(commits));
}

/// Every case of one corpus: one hub over a fresh repository.
fn cases(corpus: &str) -> BTreeMap<String, Value> {
    let repo = opted_in("http", corpus);
    let base = repo.head();
    let mut cases = BTreeMap::new();
    let found = match probe(&repo.env(), &repo.git_env()) {
        Ok(Some(found)) => found,
        Ok(None) => {
            cases.insert("probe".to_owned(), Value::Null);
            return cases;
        }
        Err(error) => {
            cases.insert(
                "probe".to_owned(),
                normalized(json!({"error": error}), &repo.root, &[]),
            );
            return cases;
        }
    };
    cases.insert(
        "probe".to_owned(),
        json!({"target": found.target, "path": found.path}),
    );
    let slug = slug(&repo).expect("a slug");
    let mut clock = WallClock::start();
    let server = start(&repo, &hub_env(&repo));

    let (answer, update) = propose_update(&repo, &found, "wc-hub-update");
    cases.insert("update propose".to_owned(), answer);
    if let Some(id) = update {
        cases.insert("update stage".to_owned(), stage_approve(&repo, &id));
        confirmed(&repo, &server, &slug, "update", &id, &mut cases);
    }

    let (answers, question) = raise_question(&repo, &found);
    cases.insert("question raise".to_owned(), json!(answers));
    if let Some(id) = question {
        cases.insert("question stage".to_owned(), stage_approve(&repo, &id));
        confirmed(&repo, &server, &slug, "question", &id, &mut cases);
    }
    drop(server);
    clock.stop();
    let made = repo.commits_since(&base);
    cases
        .into_iter()
        .map(|(case, value)| (case, clock.normalized(normalized(value, &repo.root, &made))))
        .collect()
}

macro_rules! golden {
    ($($name:ident => $corpus:literal,)*) => {
        $(
            #[test]
            fn $name() {
                settle($corpus, "http", cases($corpus));
            }
        )*
        /// Every corpus of [`CORPORA`] has its test above.
        #[test]
        fn every_corpus_has_its_test() {
            let tested: Vec<&str> = vec![$($corpus),*];
            assert_eq!(tested, CORPORA.to_vec());
        }
    };
}

golden! {
    anchor_states => "anchor-states",
    apply_outputs_forge => "apply-outputs/forge",
    apply_outputs_press => "apply-outputs/press",
    auto_mini => "auto-mini",
    card_press => "card-press",
    code_a => "code-a",
    code_bevy => "code-bevy",
    code_index => "code-index",
    corpus_mini => "corpus-mini",
    flow_aside => "flow-aside",
    flow_gaps => "flow-gaps",
    flow_hire => "flow-hire",
    flow_relay => "flow-relay",
    impact => "impact",
    import_layout_four => "import-layout/four",
    import_layout_one => "import-layout/one",
    import_layout_three => "import-layout/three",
    import_layout_two => "import-layout/two",
    parent_ref => "parent-ref",
    pilot_w_one => "pilot-w/one",
    pilot_w_two => "pilot-w/two",
    plant_diary => "plant-diary",
    roadmap_a => "roadmap-a",
    roadmap_b => "roadmap-b",
    spec_a => "spec-a",
    spec_b => "spec-b",
    spec_c => "spec-c",
    studio_lamp => "studio-lamp",
    trace_almanac => "trace-almanac",
    verify_ecs => "verify-ecs",
    verify_tests => "verify-tests",
}
