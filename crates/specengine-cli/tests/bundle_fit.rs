//! docs/features/spec-cli-bundle.md, the fitting and the hash: AC-06 (the
//! budget sweep and the minimum), AC-07 (a target degrades, never cut),
//! AC-08 (the 40 000-character ceiling and `bundle_hash`), AC-09 (scale:
//! unrelated documents, 1 000 open questions, the capped tail), AC-11
//! (determinism). Scratch copies of `fixtures/spec-a` and `-b`, each run
//! with its own `HOME`; the fixtures are only read. The hash is checked
//! with a BLAKE3 written in `common/bundle.rs` from the specification.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::bundle::{
    LAYERS, blake3_hex, bundle, bundle_json, bundle_json_slow, minimum, names, placed_candidates,
    split_text, tail_names,
};
use common::graph::spec30;
use common::{Scratch, fixture, replace, snapshot, spec, write};
use serde_json::Value;
use specengine_core::tokens_est;

/// The 40 000-character ceiling of every interface (Q7).
const CAP: usize = 40_000;

/// BLAKE3 test vectors of the specification's reference: the empty input
/// and `abc`; the port then agrees with the store's crate on inputs of
/// one block, one chunk and many chunks (input byte `i` = `i % 251`).
#[test]
fn the_test_blake3_matches_the_reference_vectors_and_the_crate() {
    assert_eq!(
        blake3_hex(b""),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
    assert_eq!(
        blake3_hex(b"abc"),
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
    );
    let input: Vec<u8> = (0..102_400usize).map(|i| (i % 251) as u8).collect();
    for len in [
        0, 1, 63, 64, 65, 1023, 1024, 1025, 2048, 2049, 3072, 3073, 4096, 4097, 5120, 8193, 16384,
        31744, 40_000, 102_400,
    ] {
        assert_eq!(
            format!("b3:{}", blake3_hex(&input[..len])),
            specengine_store::b3_hash(&input[..len]),
            "length {len}"
        );
    }
}

/// The body, hash and totals of a JSON bundle agree with themselves, with
/// the text run of the same call, and with BLAKE3 of the body's bytes.
fn agree(home: &Path, root: &Path, args: &[&str], json: &Value) -> String {
    let body = json["body"].as_str().expect("body").to_owned();
    let tokens = json["tokens"].as_u64().unwrap();
    let budget = json["budget"].as_u64().unwrap();
    assert_eq!(tokens, u64::from(tokens_est(&body)), "{args:?}: tokens");
    assert!(tokens <= budget, "{args:?}: {tokens} over {budget}");
    assert_eq!(
        json["chars"].as_u64().unwrap() as usize,
        body.chars().count(),
        "{args:?}"
    );
    assert!(
        body.chars().count() <= CAP,
        "{args:?}: {} chars",
        body.chars().count()
    );
    assert_eq!(
        json["bytes"].as_u64().unwrap() as usize,
        body.len(),
        "{args:?}"
    );
    let hash = format!("b3:{}", blake3_hex(body.as_bytes()));
    assert_eq!(
        json["bundle_hash"],
        hash.as_str(),
        "{args:?}: the hash of the body"
    );
    let run = bundle(home, root, args);
    run.code(0);
    assert!(
        !run.stdout.contains("[truncated"),
        "{args:?}: {}",
        run.show()
    );
    let text = split_text(&run.stdout);
    assert_eq!(text.body, body, "{args:?}: the JSON body is the text body");
    assert_eq!(text.hash, hash, "{args:?}");
    let not_included =
        json["tail"].as_array().unwrap().len() as u64 + json["more"].as_u64().unwrap();
    assert_eq!(
        text.totals,
        format!(
            "tokens {tokens} of {budget}, chars {}, bytes {}, not included {not_included}",
            body.chars().count(),
            body.len()
        ),
        "{args:?}"
    );
    body
}

/// The direct child ID sections of `target` (its file, depth 1 of `spec
/// tree`).
fn child_sections(home: &Path, root: &Path, target: &str) -> BTreeSet<String> {
    let tree = spec30(home, root, &["--json", "tree", target, "--depth", "1"]).json();
    let nodes = tree["nodes"].as_array().expect("nodes");
    let path = nodes[0]["path"].clone();
    nodes
        .iter()
        .filter(|node| node["depth"] == 1 && node["path"] == path)
        .map(|node| node["id"].as_str().expect("an ID section").to_owned())
        .collect()
}

/// AC-06: both fixtures, several targets, each of {the minimum, 100, 300,
/// 1 000, 2 000, 10 000} not below the minimum: `tokens` = `tokens_est` of
/// the printed body ≤ `budget`; `chars` ≤ 40 000; every candidate in
/// exactly one place (an item, a tail line or `more`); the minimum − 1 →
/// exit 2 naming it. M: the budget measured on item estimates alone.
#[test]
fn ac06_every_budget_is_kept_and_every_candidate_placed_once() {
    let scratch = Scratch::new("bundle-ac06");
    let home = scratch.home("h");
    let mut runs = 0;
    let mut tails = 0;
    for (corpus, targets) in [
        (
            "spec-a",
            &[
                "MEC-STAMINA",
                "MEC-SPRINT",
                "R-12",
                "DOM-GAME",
                "DOM-MOVEMENT",
                "DEC-0023",
                "EDGE-SPRINT-EMPTY",
                "RULE-SPRINT-COST",
                "docs/features/stamina-tuning.md",
            ][..],
        ),
        (
            "spec-b",
            &[
                "REQ-001",
                "MOD-CLI",
                "CMD-SYNC",
                "REQ-002",
                "dry-run/CRIT-01",
                "ADR-0001",
            ][..],
        ),
    ] {
        let root = scratch.copy(corpus, corpus);
        for target in targets {
            let all = bundle_json(&home, &root, &[target, "--budget", "4294967295"]);
            let candidates: Vec<String> = placed_candidates(&all);
            let unique: BTreeSet<String> = candidates.iter().cloned().collect();
            assert_eq!(unique.len(), candidates.len(), "{target}: {all}");
            assert_eq!(all["more"], 0);
            let children = child_sections(&home, &root, target);

            let low = minimum(&home, &root, &[target]);
            let below = (low - 1).to_string();
            let run = bundle(&home, &root, &[target, "--budget", &below]);
            run.code(2);
            assert!(run.stdout.is_empty(), "{}", run.show());
            assert!(
                run.stderr.contains(&format!("--budget {below} is below"))
                    && run.stderr.contains(&format!("minimum of {low} tokens")),
                "{}",
                run.show()
            );
            for budget in [low, 100, 300, 1_000, 2_000, 10_000] {
                if budget < low {
                    continue;
                }
                let budget = budget.to_string();
                let args = [*target, "--budget", budget.as_str()];
                let json = bundle_json(&home, &root, &args);
                agree(&home, &root, &args, &json);
                runs += 1;
                let form = json["layers"]["targets"][0]["form"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let mut universe = unique.clone();
                if form != "text" {
                    universe.extend(children.iter().cloned());
                }
                let placed = placed_candidates(&json);
                let tail = tail_names(&json);
                tails += tail.len();
                let mut seen = BTreeSet::new();
                for name in placed.iter().chain(tail.iter()) {
                    assert!(
                        seen.insert(name.clone()),
                        "{target} {budget}: {name} twice: {json}"
                    );
                    assert!(
                        universe.contains(name),
                        "{target} {budget}: {name} not a candidate"
                    );
                }
                assert!(
                    placed.iter().all(|name| unique.contains(name)),
                    "{target} {budget}: {placed:?}"
                );
                assert_eq!(
                    placed.len() + tail.len() + json["more"].as_u64().unwrap() as usize,
                    universe.len(),
                    "{target} at {budget}: placed {placed:?}, tail {tail:?}, more {}",
                    json["more"]
                );
                // The not-included lines as printed, in the tail's order.
                let body = json["body"].as_str().unwrap();
                let listed: Vec<String> = json["tail"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| {
                        format!(
                            "- {} | {} | {} tokens",
                            entry["name"].as_str().unwrap(),
                            entry["title"].as_str().unwrap_or("-"),
                            entry["tokens_est"]
                        )
                    })
                    .collect();
                let more = json["more"].as_u64().unwrap();
                let mut expected = listed.clone();
                if more > 0 {
                    expected.push(format!("- {more} more"));
                }
                match body.rsplit_once("\n## Not included\n") {
                    Some((_, list)) => {
                        assert_eq!(
                            list.lines().collect::<Vec<_>>(),
                            expected,
                            "{target} {budget}"
                        )
                    }
                    None => assert!(expected.is_empty(), "{target} {budget}: {body}"),
                }
                // The outlined target's sections lead the not-included list.
                let layers: Vec<&str> = json["tail"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| entry["layer"].as_str().unwrap())
                    .collect();
                let leading = layers
                    .iter()
                    .take_while(|&&layer| layer == "targets")
                    .count();
                assert!(
                    layers[leading..].iter().all(|&layer| layer != "targets"),
                    "{target} {budget}: {layers:?}"
                );
                // Within a layer by (path, line); ancestors nearest first.
                for layer in LAYERS {
                    if layer == "ancestors" {
                        continue;
                    }
                    let places: Vec<(String, u64)> = json["layers"][layer]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|item| {
                            (
                                item["path"].as_str().unwrap().to_owned(),
                                item["line"].as_u64().unwrap(),
                            )
                        })
                        .collect();
                    let mut sorted = places.clone();
                    sorted.sort();
                    assert_eq!(places, sorted, "{target} {budget} {layer}");
                }
            }
        }
    }
    assert!(runs >= 60, "{runs} runs");
    assert!(tails >= 10, "only {tails} tail lines over the sweep");
}

/// A target of ≥ 3 000 estimated tokens with three child sections, under
/// DOM-MOVEMENT, its padding marked by `zebrafill`.
fn big_target(root: &Path) -> String {
    let pad = |part: &str| -> String {
        (0..140)
            .map(|n| format!("zebrafill {part} line {n} keeps the rule text long enough.\n"))
            .collect()
    };
    let text = format!(
        "---\nid: MEC-BIG\nclass: canon\ntitle: Big\nparent: DOM-MOVEMENT\nstatus: accepted\n\
         owner: owner\nreviewed: 2026-09-20\n---\n\n# Big\n\nA big mechanic in one summary line.\n\n\
         ## First {{#RULE-BIG-ONE}}\n\n{}\n## Second {{#RULE-BIG-TWO}}\n\n{}\n## Third {{#RULE-BIG-THREE}}\n\n{}",
        pad("one"),
        pad("two"),
        pad("three")
    );
    assert!(tokens_est(&text) >= 3_000, "{}", tokens_est(&text));
    write(root, "docs/spec/movement/big.md", &text);
    text
}

/// AC-07: a target ≥ 3 000 tokens, `--budget 2000`: `tokens` ≤ 2 000; the
/// target `form: outline`, its header marked ` | outline`, its summary
/// shown; its three sections lead the tail; none of its padding in the
/// body. At the minimum: the marked header alone (`form: header`). M:
/// layer 1 exempt from the budget; a mid-text cut.
#[test]
fn ac07_a_large_target_degrades_and_is_never_cut() {
    let scratch = Scratch::new("bundle-ac07");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    big_target(&root);
    let show = spec30(&home, &root, &["--json", "show", "MEC-BIG"]).json();
    let own = show["nodes"][0]["tokens_est"].as_u64().unwrap();
    assert!(own >= 3_000, "{own}");

    let args = ["MEC-BIG", "--budget", "2000"];
    let json = bundle_json(&home, &root, &args);
    let body = agree(&home, &root, &args, &json);
    assert!(json["tokens"].as_u64().unwrap() <= 2_000);
    let target = &json["layers"]["targets"][0];
    assert_eq!(target["form"], "outline", "{json}");
    assert_eq!(target["tokens_est"], own);
    assert!(
        body.starts_with(&format!(
            "# Bundle: MEC-BIG\n\n## Targets\nMEC-BIG | mechanic | Big | docs/spec/movement/big.md:1 | {own} tokens | status accepted | outline\nA big mechanic in one summary line.\n"
        )),
        "{body}"
    );
    assert!(!body.contains("zebrafill"), "padding in the body:\n{body}");
    assert!(!body.contains("## First"), "{body}");
    assert_eq!(
        tail_names(&json)[..3],
        ["RULE-BIG-ONE", "RULE-BIG-TWO", "RULE-BIG-THREE"],
        "{json}"
    );
    // The rest of the bundle still fits around the outline.
    assert_eq!(names(&json, "ancestors"), ["DOM-MOVEMENT", "DOM-GAME"]);

    let low = minimum(&home, &root, &["MEC-BIG"]);
    let low_text = low.to_string();
    let args = ["MEC-BIG", "--budget", low_text.as_str()];
    let json = bundle_json(&home, &root, &args);
    let body = agree(&home, &root, &args, &json);
    assert_eq!(json["layers"]["targets"][0]["form"], "header", "{json}");
    assert!(
        body.starts_with(&format!(
            "# Bundle: MEC-BIG\n\n## Targets\nMEC-BIG | mechanic | Big | docs/spec/movement/big.md:1 | {own} tokens | status accepted | outline\n"
        )),
        "{body}"
    );
    assert!(!body.contains("A big mechanic"), "{body}");
    assert!(!body.contains("zebrafill"), "{body}");
    // Full text when it fits.
    let json = bundle_json(&home, &root, &["MEC-BIG", "--budget", "10000"]);
    assert_eq!(json["layers"]["targets"][0]["form"], "text");
    assert_eq!(
        json["body"].as_str().unwrap().matches("zebrafill").count(),
        420
    );
}

/// `count` records `R-<from>`… constraining MEC-STAMINA, each with a
/// summary of `width` characters.
fn constraining_records(root: &Path, from: usize, count: usize, width: usize, word: &str) {
    for n in from..from + count {
        let mut summary = String::new();
        while summary.len() + word.len() + 1 < width {
            summary.push_str(word);
            summary.push(' ');
        }
        write(
            root,
            &format!("docs/records/R/R-{n:02}.md"),
            format!(
                "---\nid: R-{n:02}\nclass: canon\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n\
                 links:\n  constrains: [MEC-STAMINA]\n---\n\n# Constraint {n:02}\n\n{summary}\n"
            ),
        );
    }
}

/// AC-08: thirty neighbours with long summaries at `--budget 100000`:
/// `chars` ≤ 40 000, the stdout over 40 000 characters yet with no
/// `[truncated` line; BLAKE3 of the printed body = `bundle_hash` = the
/// JSON's; the JSON `body` = the text's body. M: the CLI cap cutting the
/// bundle; hashing a structure.
#[test]
fn ac08_the_ceiling_bounds_the_body_and_the_hash_names_it() {
    let scratch = Scratch::new("bundle-ac08");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    constraining_records(&root, 30, 30, 1_500, "longsummary");
    // Small ones after them in path order fill the room the large leave.
    constraining_records(&root, 60, 40, 10, "s");
    let args = ["MEC-STAMINA", "--budget", "100000"];
    let json = bundle_json(&home, &root, &args);
    let body = agree(&home, &root, &args, &json);
    let neighbours = names(&json, "neighbours");
    assert!(neighbours.len() >= 30, "{neighbours:?}");
    let not_included = tail_names(&json).len() + json["more"].as_u64().unwrap() as usize;
    assert!(
        not_included >= 1,
        "the ceiling never bound: {}",
        body.chars().count()
    );
    assert!(body.chars().count() > 39_000, "{}", body.chars().count());
    let run = bundle(&home, &root, &args);
    assert!(
        run.stdout.chars().count() > CAP,
        "the stdout must exceed the ceiling to show it is not cut: {}",
        run.stdout.chars().count()
    );
    assert!(run.stdout.ends_with(&format!(
        "bundle_hash {}\n{}\n",
        json["bundle_hash"].as_str().unwrap(),
        split_text(&run.stdout).totals
    )));
}

/// AC-09 (1): 1 000 unrelated documents leave MEC-STAMINA's bundle and
/// hash as they were.
#[test]
fn ac09_unrelated_documents_change_nothing() {
    let scratch = Scratch::new("bundle-ac09-unrelated");
    let home = scratch.home("h");
    let plain = scratch.copy("spec-a", "plain");
    let root = scratch.copy("spec-a", "copy");
    for n in 0..1_000 {
        write(
            &root,
            &format!("docs/notes/note-{n:04}.md"),
            format!(
                "---\nclass: canon\nowner: owner\nreviewed: 2026-09-20\n---\n\n# Note {n}\n\n\
                 An unrelated note about lantern oil, number {n}.\n"
            ),
        );
    }
    for budget in ["2000", "300", "10000"] {
        let base = bundle(&home, &plain, &["MEC-STAMINA", "--budget", budget]);
        let run = spec(&home, &root, &["bundle", "MEC-STAMINA", "--budget", budget]);
        base.code(0);
        run.code(0);
        assert_eq!(run.stdout, base.stdout, "at {budget}");
    }
}

/// AC-09 (2): 1 000 open questions referring to MEC-STAMINA, each larger
/// than the room left: within the budget, the tail at 20 lines, `more`
/// exact; at the default budget too. M: an uncapped tail.
#[test]
fn ac09_a_thousand_open_questions_keep_the_tail_at_twenty_lines() {
    let scratch = Scratch::new("bundle-ac09-questions");
    let home = scratch.home("h");
    let plain = scratch.copy("spec-a", "plain");
    let base = bundle_json(&home, &plain, &["MEC-STAMINA", "--budget", "10000"]);
    let base_candidates = placed_candidates(&base).len();
    assert_eq!(base_candidates, 7, "{base}");
    let root = scratch.copy("spec-a", "copy");
    replace(
        &root,
        "specengine.toml",
        "TERM = { kind = \"term\",        shape = \"name\" }\n",
        "TERM = { kind = \"term\",        shape = \"name\" }\nOQ   = { kind = \"question\",    width = 4 }\n",
    );
    // About 1 900 estimated tokens each, more than the room the targets
    // leave at either budget: letters of another script weigh one.
    let filler: String = (0..300)
        .map(|_| "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9} ")
        .collect();
    for n in 0..1_000 {
        write(
            &root,
            &format!("docs/records/OQ/OQ-{n:04}.md"),
            format!(
                "---\nid: OQ-{n:04}\nclass: canon\nstatus: open\nworking_answer: A-101\n\
                 refs: [MEC-STAMINA]\nowner: owner\nreviewed: 2026-09-20\n---\n\n# Question {n}\n\n{filler}\n"
            ),
        );
    }
    for (budget, flag) in [(1_500u64, Some("1500")), (2_000, None)] {
        let mut args = vec!["MEC-STAMINA"];
        if let Some(flag) = flag {
            args.extend(["--budget", flag]);
        }
        let json = bundle_json_slow(&home, &root, &args);
        assert_eq!(json["budget"], budget);
        let tokens = json["tokens"].as_u64().unwrap();
        assert!(tokens <= budget, "{tokens} over {budget}");
        let body = json["body"].as_str().unwrap();
        assert_eq!(tokens, u64::from(tokens_est(body)));
        assert_eq!(json["layers"]["targets"][0]["form"], "text");
        let placed = placed_candidates(&json).len();
        let tail = tail_names(&json);
        assert_eq!(tail.len(), 20, "at {budget}: {:?}", json["tail"]);
        let more = json["more"].as_u64().unwrap() as usize;
        assert_eq!(
            placed + tail.len() + more,
            1_000 + base_candidates,
            "at {budget}"
        );
        assert!(
            body.ends_with(&format!("- {more} more\n")),
            "at {budget}: {body}"
        );
        let (_, listed) = body
            .rsplit_once("\n## Not included\n")
            .unwrap_or_else(|| panic!("at {budget}: no not-included list: {body}"));
        let lines: Vec<&str> = listed.lines().collect();
        assert_eq!(
            lines.len(),
            21,
            "at {budget}: 20 tail lines and the more line: {lines:?}"
        );
        assert!(
            lines[..20].iter().all(|line| line.starts_with("- OQ-")),
            "{lines:?}"
        );
    }
}

/// Copies `from` to `to` creating files in reverse path order.
fn copy_reversed(from: &Path, to: &Path) {
    let files: Vec<(String, Vec<u8>)> = snapshot(from)
        .into_iter()
        .filter_map(|(path, bytes)| bytes.map(|bytes| (path, bytes)))
        .collect();
    for (path, bytes) in files.into_iter().rev() {
        write(to, &path, bytes);
    }
}

/// The text and JSON stdout of each call, in order.
fn outputs(home: &Path, root: &Path, calls: &[Vec<String>]) -> Vec<(String, String)> {
    calls
        .iter()
        .map(|args| {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let text = bundle(home, root, &args);
            text.code(0);
            let mut json_args = vec!["--json", "bundle"];
            json_args.extend(&args);
            let json = spec30(home, root, &json_args);
            json.code(0);
            (text.stdout, json.stdout)
        })
        .collect()
}

/// AC-11: copies in opposite file orders, at different roots and `HOME`s
/// give byte-identical text and JSON and the same hash; editing DEC-0023's
/// summary changes MEC-STAMINA's hash, a same-line-count edit inside
/// EDGE-SPRINT-EMPTY does not. M: the absolute root or a `HashMap` order in
/// the body; token counts in item headers.
#[test]
fn ac11_one_corpus_one_bundle_whatever_the_root_order_or_home() {
    let scratch = Scratch::new("bundle-ac11");
    for (corpus, calls) in [
        (
            "spec-a",
            vec![
                vec!["MEC-STAMINA"],
                vec!["MEC-STAMINA", "--budget", "120"],
                vec!["MEC-SPRINT", "R-12", "--budget", "400"],
                vec!["DEC-0023", "--budget", "10000"],
            ],
        ),
        (
            "spec-b",
            vec![
                vec!["MOD-CLI"],
                vec!["MOD-CLI", "--budget", "150"],
                vec!["REQ-001", "REQ-002", "--budget", "10000"],
            ],
        ),
    ] {
        let calls: Vec<Vec<String>> = calls
            .into_iter()
            .map(|call| call.into_iter().map(str::to_owned).collect())
            .collect();
        let first = scratch.copy(corpus, &format!("{corpus}-first"));
        let second: PathBuf = scratch.join(&format!("elsewhere/deeper root/{corpus}-second"));
        copy_reversed(&fixture(corpus), &second);
        let second = fs::canonicalize(&second).unwrap();
        let a = outputs(&scratch.home(&format!("{corpus}-a")), &first, &calls);
        let b = outputs(&scratch.home(&format!("{corpus}-b")), &second, &calls);
        assert_eq!(a, b, "{corpus}");
        for (text, json) in &a {
            let hash = split_text(text).hash;
            assert!(
                json.contains(&format!("\"bundle_hash\":\"{hash}\"")),
                "{corpus}"
            );
            assert!(
                !text.contains(scratch.path().to_str().unwrap()),
                "{corpus}: an absolute path"
            );
            assert!(
                !json.contains(scratch.path().to_str().unwrap()),
                "{corpus}: an absolute path"
            );
        }
    }

    // Edits: DEC-0023's summary moves MEC-STAMINA's hash; a same-line-count
    // edit inside EDGE-SPRINT-EMPTY (MEC-SPRINT's own estimate changes) does
    // not.
    let home = scratch.home("edits");
    let root = scratch.copy("spec-a", "edits");
    let hash = |root: &Path, budget: &str| -> String {
        bundle_json(&home, root, &["MEC-STAMINA", "--budget", budget])["bundle_hash"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let sprint_tokens = |root: &Path| {
        spec30(&home, root, &["--json", "show", "MEC-SPRINT"]).json()["nodes"][0]["tokens_est"]
            .clone()
    };
    let before = (hash(&root, "2000"), hash(&root, "10000"));
    let sprint_before = sprint_tokens(&root);
    replace(
        &root,
        "docs/spec/movement/sprint.md",
        "At zero stamina the sprint ends; see",
        "At zero stamina the sprint ends at once, with a long sigh and a stumble; see",
    );
    assert_ne!(
        sprint_tokens(&root),
        sprint_before,
        "the edit must change MEC-SPRINT's estimate"
    );
    assert_eq!(
        (hash(&root, "2000"), hash(&root, "10000")),
        before,
        "an edit inside EDGE-SPRINT-EMPTY"
    );
    replace(
        &root,
        "docs/records/DEC/DEC-0023.md",
        "Stamina regenerates only at rest (A-101 becomes the rule).",
        "Stamina regenerates only at rest (A-101 is now the rule).",
    );
    let after = (hash(&root, "2000"), hash(&root, "10000"));
    assert_ne!(after.0, before.0, "DEC-0023's summary edit");
    assert_ne!(after.1, before.1, "DEC-0023's summary edit");
}

/// Fitting 1: the minimum is `tokens_est` of the frame — the title, `##
/// Targets`, each target's `spec show` header marked ` | outline` (one
/// empty line between targets) and the reserve `## Not included` + `- <n>
/// more`, n = the candidates and the targets' direct child ID sections;
/// no reserve when n is 0.
#[test]
fn the_minimum_is_the_frame() {
    let scratch = Scratch::new("bundle-frame");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    for (targets, title, n) in [
        (&["MEC-STAMINA"][..], "MEC-STAMINA", Some(7 + 2)),
        (
            &["MEC-SPRINT", "MEC-STAMINA"][..],
            "MEC-SPRINT, MEC-STAMINA",
            None,
        ),
        (&["A-102"][..], "A-102", Some(0)),
    ] {
        let mut frame = format!("# Bundle: {title}\n\n## Targets\n");
        let mut children = 0;
        for (index, target) in targets.iter().enumerate() {
            let show = spec30(&home, &root, &["show", target]);
            let header = show.stdout.lines().next().unwrap();
            if index > 0 {
                frame.push('\n');
            }
            frame.push_str(&format!("{header} | outline\n"));
            children += child_sections(&home, &root, target).len();
        }
        let mut args: Vec<&str> = targets.to_vec();
        args.extend(["--budget", "4294967295"]);
        let all = bundle_json(&home, &root, &args);
        let counted = placed_candidates(&all).len() + children;
        if let Some(n) = n {
            assert_eq!(counted, n, "{targets:?}");
        }
        if counted > 0 {
            frame.push_str(&format!("\n## Not included\n- {counted} more\n"));
        }
        assert_eq!(
            minimum(&home, &root, targets),
            tokens_est(&frame),
            "{targets:?}: {frame}"
        );
        // At the minimum: the marked headers and the list, nothing else.
        let low = tokens_est(&frame).to_string();
        args.truncate(targets.len());
        args.extend(["--budget", low.as_str()]);
        let json = bundle_json(&home, &root, &args);
        assert_eq!(json["body"].as_str().unwrap(), frame, "{targets:?}");
    }
}

/// Fitting 1: a frame over 40 000 characters is exit 2 whatever the
/// budget, naming the minimum and the budget's source; no JSON.
#[test]
fn a_frame_over_the_ceiling_is_refused() {
    let scratch = Scratch::new("bundle-frame-cap");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    replace(
        &root,
        "specengine.toml",
        "TERM = { kind = \"term\",        shape = \"name\" }\n",
        "TERM = { kind = \"term\",        shape = \"name\" }\nNT   = { kind = \"note\",        width = 4 }\n",
    );
    let ids: Vec<String> = (0..700).map(|n| format!("NT-{n:04}")).collect();
    for id in &ids {
        write(
            &root,
            &format!("docs/records/NT/{id}.md"),
            format!(
                "---\nid: {id}\nclass: canon\n---\n\n# A note titled at some length {id}\n\nA note.\n"
            ),
        );
    }
    let mut args: Vec<&str> = vec!["--json", "bundle"];
    args.extend(ids.iter().map(String::as_str));
    args.extend(["--budget", "4294967295"]);
    let run = spec(&home, &root, &args);
    run.code(2);
    assert!(run.stdout.is_empty(), "{}", run.show());
    assert!(
        run.stderr.contains("40000-character ceiling")
            && run.stderr.contains("--budget 4294967295")
            && run.stderr.contains("its minimum is "),
        "{}",
        run.show()
    );
    // Seventy targets: a frame over the default budget names its source.
    let mut args: Vec<&str> = vec!["--json", "bundle"];
    args.extend(ids[..70].iter().map(String::as_str));
    let run = spec(&home, &root, &args);
    run.code(2);
    assert!(run.stdout.is_empty(), "{}", run.show());
    assert!(
        run.stderr
            .contains("the default budget of 2000 is below this bundle's minimum of "),
        "{}",
        run.show()
    );
}
