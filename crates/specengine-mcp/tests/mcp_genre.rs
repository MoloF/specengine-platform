//! AC-12 of docs/features/mcp-read.md (ADR-0008, 07 §1.2 P2-3): a scan of
//! `crates/specengine-mcp/src` finds no string literal equal to a kind of
//! `fixtures/spec-a` or `fixtures/spec-b` and no P2-3 word in a text; a
//! synthetic non-Rust corpus (garden planning) yields no P2-3 word in any
//! tool or resource answer, nor in the wire texts (descriptions,
//! `instructions`).
//!
//! Scope of the scan (deviation 11): string literals are the naive pairs of
//! `"` on lines outside `//` comments (as core `check_genre.rs`), so a
//! quoted example (`[\"mechanic\"]`) is a literal of its own. The kind scan
//! covers the modules of the default build: those `lib.rs` declares under
//! `#[cfg(feature = "probes")]` (the Phase 0 consent demo `review.rs`, whose
//! form field is named `decision`, and `probes.rs`) are left out and named
//! by the test. The P2-3 scan covers every source file; a word is whole
//! (ASCII letters, digits, `_`, `-` are word characters), so
//! `CARGO_PKG_NAME` is not the word `cargo`. The game-only kinds
//! `mechanic`, `edge-case` and `domain` must not appear even as words in a
//! literal.
//!
//! M: a `kinds` enum; "mechanic" in an example.
//!
//! AC-11 of docs/features/agent-intake.md: the garden corpus, committed in
//! a scratch git repository, through the queue tools — a question hit by
//! an accepted ruling (`CHOICE-001`, the decision class of the garden's own
//! prefixes), a question stored, asked again (a queue hit), `get_proposal`
//! of it, a discrepancy and a change stored, their reviews — and every
//! tool's text: no P2-3 word. The kind scan above covers the queue tools'
//! sources (spec-a's kinds hold `question` and `decision`). M: a
//! `"question"` literal in `specengine-mcp/src`.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::Path;

use common::read::{ERAS, STACK_WORDS, Session, TOOLS, has_word};
use common::*;
use serde_json::json;

/// Every `kind = "…"` of the two fixtures' configs.
fn fixture_kinds() -> Vec<String> {
    let mut kinds = Vec::new();
    for name in ["spec-a", "spec-b"] {
        let text = read_text(&fixture(name), "specengine.toml");
        for part in text.split("kind").skip(1) {
            let Some(rest) = part.trim_start().strip_prefix('=') else {
                continue;
            };
            let Some(rest) = rest.trim_start().strip_prefix('"') else {
                continue;
            };
            if let Some((kind, _)) = rest.split_once('"') {
                kinds.push(kind.to_owned());
            }
        }
    }
    kinds.sort();
    kinds.dedup();
    kinds
}

/// Every string literal of a Rust source's lines outside `//` comments.
fn literals(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (number, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for literal in line.split('"').skip(1).step_by(2) {
            found.push((number + 1, literal.to_owned()));
        }
    }
    found
}

/// The modules `lib.rs` compiles only with feature `probes`.
fn probes_only_modules(src: &Path) -> Vec<String> {
    let lib = fs::read_to_string(src.join("lib.rs")).expect("lib.rs");
    let lines: Vec<&str> = lib.lines().map(str::trim).collect();
    let mut modules = Vec::new();
    for pair in lines.windows(2) {
        if pair[0] == "#[cfg(feature = \"probes\")]"
            && let Some(name) = pair[1]
                .strip_prefix("mod ")
                .and_then(|rest| rest.strip_suffix(';'))
        {
            modules.push(format!("{name}.rs"));
        }
    }
    modules.sort();
    modules
}

#[test]
fn ac12_the_sources_hold_no_fixture_kind_and_no_stack_word() {
    let src = repository_root().join("crates/specengine-mcp/src");
    let kinds = fixture_kinds();
    assert!(
        kinds.contains(&"mechanic".to_owned()) && kinds.contains(&"flag".to_owned()),
        "{kinds:?}"
    );
    let skipped = probes_only_modules(&src);
    assert_eq!(
        skipped,
        ["probes.rs", "review.rs"],
        "the probes-only modules left out of the kind scan"
    );
    let mut files: Vec<_> = fs::read_dir(&src)
        .expect("src")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    assert!(files.len() >= 8, "{files:?}");
    let mut offenders = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let source = fs::read_to_string(path).expect("source");
        for (line, literal) in literals(&source) {
            if !skipped.contains(&name) {
                if kinds.contains(&literal) {
                    offenders.push(format!("{name}:{line}: the kind literal {literal:?}"));
                }
                for word in ["mechanic", "edge-case", "domain"] {
                    if has_word(&literal, word) {
                        offenders.push(format!("{name}:{line}: {word:?} in {literal:?}"));
                    }
                }
            }
            for word in STACK_WORDS {
                if has_word(&literal, word) {
                    offenders.push(format!(
                        "{name}:{line}: the P2-3 word {word:?} in {literal:?}"
                    ));
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
}

/// A garden-planning corpus: no programming stack, its own prefixes and
/// kinds (as the CLI's bundle tests).
fn garden(root: &Path) {
    write(
        root,
        "specengine.toml",
        "[project]\nslug = \"garden-plan\"\n\n[ids]\n\
         PLOT   = { kind = \"area\",     shape = \"name\" }\n\
         CROP   = { kind = \"plant\",    shape = \"name\" }\n\
         CARE   = { kind = \"routine\",  shape = \"name\" }\n\
         ASK    = { kind = \"doubt\",    width = 2 }\n\
         GUESS  = { kind = \"hunch\",    width = 2 }\n\
         CHOICE = { kind = \"ruling\",   width = 3 }\n\
         CHK    = { kind = \"proof\",    width = 2, scope = \"feature\" }\n\
         WORD   = { kind = \"glossary\", shape = \"name\" }\n",
    );
    let canon = |id: &str, extra: &str, title: &str, body: &str| {
        format!(
            "---\nid: {id}\nclass: canon\n{extra}owner: gardener\nreviewed: 2026-09-20\n---\n\n# {title}\n\n{body}"
        )
    };
    write(
        root,
        "docs/spec/garden.md",
        canon(
            "PLOT-GARDEN",
            "status: accepted\n",
            "The garden",
            "Raised beds behind the house.\n",
        ),
    );
    write(
        root,
        "docs/spec/tomato.md",
        canon(
            "CROP-TOMATO",
            "parent: PLOT-GARDEN\nstatus: accepted\nlinks:\n  depends_on: [CROP-BASIL]\n  \
             derived_from: [GUESS-01]\n  uses_term: [WORD-mulch]\n",
            "Tomatoes",
            "Tomatoes grow in the sunny bed.\n\n## Watering {#CARE-WATER}\n\nWater at dawn.\n",
        ),
    );
    write(
        root,
        "docs/spec/basil.md",
        canon(
            "CROP-BASIL",
            "parent: PLOT-GARDEN\nlinks:\n  constrains: [CROP-TOMATO]\n",
            "Basil",
            "Basil shades the tomato roots.\n",
        ),
    );
    write(
        root,
        "docs/records/ASK-01.md",
        "---\nid: ASK-01\nclass: canon\nstatus: open\nworking_answer: GUESS-01\nrefs: [CROP-TOMATO]\n\
         owner: gardener\nreviewed: 2026-09-20\n---\n\n# Water twice in a heat wave?\n\nOnly if the soil is dry.\n",
    );
    write(
        root,
        "docs/records/GUESS-01.md",
        canon(
            "GUESS-01",
            "status: open\n",
            "Dawn watering is enough",
            "Assumed for a mild summer.\n",
        ),
    );
    write(
        root,
        "docs/records/CHOICE-001.md",
        "---\nid: CHOICE-001\nclass: decision\nstatus: accepted\ndate: 2026-09-01\n\
         canon: docs/spec/tomato.md#watering\nscope: [watering]\n---\n\n# Drip lines for the beds\n\n\
         The beds get drip lines.\n",
    );
    write(
        root,
        "docs/records/WORD-mulch.md",
        canon(
            "WORD-mulch",
            "",
            "Mulch",
            "A layer of straw over the soil.\n",
        ),
    );
    write(
        root,
        "docs/features/watering.md",
        "---\nclass: spec\nstatus: draft\nscope: [watering]\n---\n\n# Watering schedule\n\n\
         Sets the schedule.\n\n## Criteria\n\n### Soil stays moist {#CHK-01}\n\n\
         Checks CROP-TOMATO after a dry week.\n",
    );
}

#[test]
fn ac12_a_non_rust_corpus_gets_no_stack_word() {
    let scratch = Scratch::new("genre");
    let home = scratch.home("h");
    let root = scratch.dir("garden");
    garden(&root);
    let calls = [
        ("get_tree", json!({})),
        (
            "get_tree",
            json!({"root": "PLOT-GARDEN", "depth": 1, "kinds": ["plant"]}),
        ),
        ("get_node", json!({"id": "CROP-TOMATO", "with": ["links"]})),
        ("get_node", json!({"id": "CROP-TOMATO#CARE-WATER"})),
        ("get_node", json!({"id": "watering/CHK-01"})),
        ("get_node", json!({"id": "CROP-NOPE"})),
        ("get_node", json!({"id": "CROP-TOMATO", "archive": true})),
        ("search", json!({"query": "tomato"})),
        ("search", json!({"query": "no"})),
        (
            "get_context_bundle",
            json!({"node_ids": ["CROP-TOMATO"], "budget": 10000}),
        ),
        (
            "get_context_bundle",
            json!({"node_ids": ["CROP-TOMATO"], "budget": 150}),
        ),
        (
            "get_context_bundle",
            json!({"node_ids": ["CROP-TOMATO"], "budget": 1}),
        ),
        ("get_tree", json!({"depth": -1})),
    ];
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
        let mut texts = Vec::new();
        let list = session.tools();
        for name in tool_names(&list) {
            texts.push(tool(&list, &name).to_string());
        }
        assert!(tool_names(&list).len() >= TOOLS.len(), "{era:?}");
        if era == common::read::Era::Stateless {
            texts.push(result(&session.request("server/discover", json!({}))).to_string());
        }
        for (name, args) in &calls {
            texts.push(session.call(name, args.clone()).to_string());
        }
        let mut uris = vec!["spec://garden-plan/node/CROP-BASIL".to_owned()];
        for page in session.all_resources() {
            texts.push(page.to_string());
            for resource in page["resources"].as_array().unwrap() {
                uris.push(resource["uri"].as_str().unwrap().to_owned());
            }
        }
        for uri in &uris {
            texts.push(session.read(uri).to_string());
        }
        texts.push(
            session
                .read("spec://garden-plan/node/CROP-NOPE")
                .to_string(),
        );
        // Unescape the JSON once so `\n` does not glue words together.
        for text in &texts {
            let plain = text.replace("\\n", " ").replace("\\\"", " ");
            for word in STACK_WORDS {
                assert!(
                    !has_word(&plain, word),
                    "{era:?}: the P2-3 word {word:?} in {}",
                    clip(text)
                );
            }
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
    // The legacy `initialize` instructions too.
    let (_server, init) = Server::legacy(&[], json!({}));
    let instructions = init["instructions"].as_str().expect("instructions");
    for word in STACK_WORDS {
        assert!(
            !has_word(instructions, word),
            "{word:?} in the instructions"
        );
    }
}

/// The words of P2-3 in `text` (JSON unescaped once), as the read test
/// checks them.
fn stack_words(text: &str) -> Vec<&'static str> {
    let plain = text.replace("\\n", " ").replace("\\\"", " ");
    STACK_WORDS
        .into_iter()
        .filter(|word| has_word(&plain, word))
        .collect()
}

/// AC-11 of docs/features/agent-intake.md: the garden corpus in git, the
/// queue tools in both eras: `CARE-WATER` asked → the accepted ruling
/// `CHOICE-001` (its title the answer), nothing stored; `CROP-BASIL` asked
/// → stored; asked again → its hit; `get_proposal` of it; a discrepancy
/// and a change stored and reviewed. No P2-3 word in any answer or tool
/// text. M: a `"question"` literal in `specengine-mcp/src` (the kind scan).
#[test]
fn ac11_the_garden_through_the_queue_tools_gets_no_stack_word() {
    let scratch = Scratch::new("genre-intake");
    let home = scratch.home("h");
    let root = scratch.dir("garden");
    garden(&root);
    let sandbox = git::Sandbox::new(scratch.path());
    sandbox.init(&root);
    sandbox.add_all(&root);
    sandbox.commit(&root, "the garden");
    let ask = |node: &str, text: &str| {
        json!({"node_ids": [node], "text": text, "working_answer": "only when dry",
            "price_of_other": "a second hose", "author_role": "gardener"})
    };
    let mut texts = Vec::new();
    for (round, era) in ERAS.into_iter().enumerate() {
        let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
        let list = session.tools();
        for name in TOOLS {
            texts.push(tool(&list, name).to_string());
        }
        let reply = session.call("ask_question", ask("CARE-WATER", "Water twice a day?"));
        let document = result(&reply)["structuredContent"].clone();
        assert_eq!(
            document["hits"],
            json!([{"id": "CHOICE-001", "source": "corpus", "status": "accepted",
                "path": "docs/records/CHOICE-001.md", "answer": "Drip lines for the beds",
                "record": "CHOICE-001"}]),
            "{era:?}: {document}"
        );
        assert_eq!(document["created"], json!(false));
        texts.push(reply.to_string());
        let reply = session.call(
            "ask_question",
            ask("CROP-BASIL", "Basil beside the tomato?"),
        );
        let document = result(&reply)["structuredContent"].clone();
        if round == 0 {
            assert_eq!(document["id"], json!("PR-0001"), "{document}");
        } else {
            assert_eq!(document["hits"][0]["id"], json!("PR-0001"), "{document}");
        }
        texts.push(reply.to_string());
        let reply = session.call(
            "ask_question",
            ask("CROP-BASIL", "basil  BESIDE the tomato?"),
        );
        assert_eq!(
            result(&reply)["structuredContent"]["hits"][0]["id"],
            json!("PR-0001"),
            "{era:?}"
        );
        texts.push(reply.to_string());
        let base = result(&session.call("get_node", json!({"id": "CROP-BASIL"})))
            ["structuredContent"]["nodes"][0]["span_hash"]
            .clone();
        let report = json!({
            "node_ids": ["CROP-BASIL", "CROP-TOMATO"],
            "summary": format!("Basil grows in the shade ({era:?})."),
            "gap_type": "partial", "severity": "normal",
            "evidence": [{"file": "notes/bed.txt", "lines": "4", "observed": "shade",
                "documented": "sun"}],
            "options": [{"label": "move", "effect": "move the basil", "price": "an hour"},
                {"label": "keep", "effect": "note the shade", "price": "none"}],
            "recommendation": 1, "distinct_from": ["CHOICE-001"],
            "author_role": "gardener"
        });
        let reply = session.call("report_discrepancy", report);
        assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
        texts.push(reply.to_string());
        let reply = session.call(
            "propose_change",
            json!({"kind": "update", "target": "CROP-BASIL", "base": base,
                "text": "---\nid: CROP-BASIL\nclass: canon\nparent: PLOT-GARDEN\nlinks:\n  constrains: [CROP-TOMATO]\nowner: gardener\nreviewed: 2026-09-20\n---\n\n# Basil\n\nBasil shades the tomato roots in July.\n",
                "rationale": "When.", "author_role": "gardener"}),
        );
        assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
        texts.push(reply.to_string());
        for number in 1..=4 {
            let reply = session.call(
                "get_proposal",
                json!({"proposal_id": format!("PR-{number:04}")}),
            );
            texts.push(reply.to_string());
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
    let kinds: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("\"kind\":\"discrepancy\""))
        .cloned()
        .collect();
    assert!(!kinds.is_empty(), "a discrepancy was reviewed");
    for text in &texts {
        assert!(
            stack_words(text).is_empty(),
            "{:?} in {}",
            stack_words(text),
            clip(text)
        );
    }
}
