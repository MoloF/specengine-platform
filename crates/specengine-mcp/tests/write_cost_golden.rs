//! docs/features/write-cost.md (RM-0125) WC9, the MCP server's half: on
//! every fixture corpus, `propose_change` of each kind (update and create
//! with and without `dry_run`, an amendment, an interpretation) answers
//! byte for byte as 0.47.0 did (`fixtures/write-cost-golden/<corpus>.json`,
//! section `mcp`): the whole JSON-RPC reply, over the real binary on stdio,
//! a cleared environment and a scratch `HOME`. The server reads the wall
//! clock, so the run's timestamps and date are normalized (`<now>`,
//! `<today>`), as the root (`<root>`). The shared half, the corpora, the
//! probe node and the texts, is the CLI's `write_cost_common/mod.rs`.
//!
//! M: findings sorted by another key: red.
//!
//! Capture (at `v0.47.0` only): `SPECENGINE_WRITE_COST_CAPTURE=1 cargo
//! nextest run -p specengine-mcp --test write_cost_golden`.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/write_cost_common/mod.rs"]
mod write_cost_common;

use std::collections::BTreeMap;

use common::Home;
use common::read::{Era, Session};
use serde_json::{Value, json};
use write_cost_common::{
    CORPORA, Probe, Repo, WallClock, appended, first_markdown, front_matter, new_file, normalized,
    probe, settle, sibling, without_keys,
};

const ROLE: &str = "spec-writer";
const MODEL: &str = "claude-opus-5-5";

/// A create's gating similar entries, by name, when it stored nothing.
fn gating(reply: &Value) -> Vec<String> {
    let document = &reply["result"]["structuredContent"];
    if !document["id"].is_null() {
        return Vec::new();
    }
    document["similar"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["gates"] == json!(true))
        .filter_map(|entry| {
            entry["id"]
                .as_str()
                .or_else(|| entry["path"].as_str())
                .map(str::to_owned)
        })
        .collect()
}

/// The creates of a new file beside `path`, with and without `dry_run`: a
/// first ask a similar entry gates is asked again naming each.
fn creates(session: &mut Session, path: &str, front: &str, out: &mut BTreeMap<String, Value>) {
    let front = without_keys(front, &["id", "aliases"]);
    for (case, name, tag, dry) in [
        (
            "propose_change create dry_run",
            "zz-wc-mcp-dry.md",
            "wc-mcp-dry",
            true,
        ),
        ("propose_change create", "zz-wc-mcp.md", "wc-mcp", false),
    ] {
        let mut args = json!({
            "kind": "create",
            "target": sibling(path, name),
            "text": new_file(&front, "Write cost probe", tag),
            "rationale": "A new probe file.",
            "author_role": ROLE,
            "author_model": MODEL,
            "dry_run": dry,
        });
        let reply = session.call("propose_change", args.clone());
        let names = gating(&reply);
        out.insert(case.to_owned(), reply);
        if !names.is_empty() && !dry {
            args["distinct_from"] = json!(names);
            out.insert(
                format!("{case}, distinct"),
                session.call("propose_change", args),
            );
        }
    }
}

/// Every case of one corpus: one server session over a fresh repository.
fn cases(corpus: &str) -> BTreeMap<String, Value> {
    let repo = Repo::new("mcp", corpus);
    let mut cases = BTreeMap::new();
    let found = probe(&repo.env(), &repo.git_env());
    let mut clock = WallClock::start();
    let mut session = Session::open(Era::Stateless, &[], Some(&repo.root), Home::At(&repo.home));
    match found {
        Err(error) => {
            cases.insert("probe".to_owned(), json!({"error": error}));
        }
        Ok(None) => {
            cases.insert("probe".to_owned(), Value::Null);
            if let Some(path) = first_markdown(&repo.root) {
                let file = std::fs::read_to_string(repo.root.join(&path)).unwrap_or_default();
                creates(&mut session, &path, &front_matter(&file), &mut cases);
            }
        }
        Ok(Some(probe)) => {
            let Probe {
                target,
                path,
                base,
                text,
                decision,
                front,
            } = probe;
            cases.insert("probe".to_owned(), json!({"target": target, "path": path}));
            let update = |tag: &str, dry: bool| {
                json!({
                    "kind": "update",
                    "target": target,
                    "base": base,
                    "text": appended(&text, tag),
                    "rationale": format!("Probe {target}."),
                    "author_role": ROLE,
                    "author_model": MODEL,
                    "dry_run": dry,
                })
            };
            for (case, args) in [
                (
                    "propose_change update dry_run",
                    update("wc-mcp-update-dry", true),
                ),
                ("propose_change update", update("wc-mcp-update", false)),
            ] {
                cases.insert(case.to_owned(), session.call("propose_change", args));
            }
            creates(&mut session, &path, &front, &mut cases);
            let amending = format!(
                "{}class: spec\nstatus: draft\nlinks:\n  amends: [{target}]\n",
                without_keys(
                    &front,
                    &["id", "aliases", "links", "class", "status", "tier"]
                )
            );
            let amendment = json!({
                "kind": "amendment",
                "target": sibling(&path, "zz-wc-mcp-amends.md"),
                "text": new_file(&amending, "Write cost amendment", "wc-mcp-amends"),
                "rationale": "Amend the probe.",
                "author_role": ROLE,
                "author_model": MODEL,
            });
            cases.insert(
                "propose_change amendment".to_owned(),
                session.call("propose_change", amendment),
            );
            let interpretation = json!({
                "kind": "interpretation",
                "target": target,
                "base": base,
                "decision": decision,
                "how": "Applied as written.",
                "why": "The decision left the wording open.",
                "text": appended(&text, "wc-mcp-interpret"),
                "rationale": "Interpret it.",
                "author_role": ROLE,
                "author_model": MODEL,
            });
            cases.insert(
                "propose_change interpretation".to_owned(),
                session.call("propose_change", interpretation),
            );
        }
    }
    drop(session.finish());
    clock.stop();
    cases
        .into_iter()
        .map(|(case, value)| (case, clock.normalized(normalized(value, &repo.root, &[]))))
        .collect()
}

macro_rules! golden {
    ($($name:ident => $corpus:literal,)*) => {
        $(
            #[test]
            fn $name() {
                settle($corpus, "mcp", cases($corpus));
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
