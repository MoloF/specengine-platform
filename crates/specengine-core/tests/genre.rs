//! AC-14 of docs/features/spec-parser.md: genre independence (ADR-0008).
//!
//! `fixtures/spec-a` (a game, English prose) and `fixtures/spec-b` (a
//! command-line tool, Russian prose, legacy Cyrillic prefixes as
//! `aliases_from`) have disjoint `[ids]` prefixes, kinds and layouts; each is
//! read through its own `specengine.toml` alone and passes the *same* test
//! functions against its own `expected.json`. No prefix of either scheme is
//! a string literal in the model or core sources.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;
use specengine_model::{AnchorOrigin, IdScheme, ParsedFile};

use common::{
    corpus_scheme, fixture, render_diagnostics, render_link, repository_root, text_of,
    walked_md_files,
};

const CORPORA: [&str; 2] = ["spec-a", "spec-b"];

fn expected(corpus: &Path) -> Value {
    let path = corpus.join("expected.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("expected.json"))
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The corpus's walked documents, parsed (a record template outside the
/// walk is no document of it).
fn parse_corpus(corpus: &Path, scheme: &IdScheme) -> Vec<(String, Vec<u8>, ParsedFile)> {
    walked_md_files(corpus)
        .into_iter()
        .map(|(path, bytes)| {
            let parsed = specengine_core::parse(&path, &bytes, scheme);
            (path, bytes, parsed)
        })
        .collect()
}

fn opt_str(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn str_list(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("expected.json: `{key}` is a list"))
        .iter()
        .map(|v| v.as_str().expect("string").to_owned())
        .collect()
}

// ---------------------------------------------------------------- shared checks

/// The scheme is the corpus's own: exactly the prefixes `expected.json` lists.
fn scheme_is_read_from_the_corpus(name: &str) {
    let corpus = fixture(name);
    let scheme = corpus_scheme(&corpus);
    let prefixes: Vec<&str> = scheme
        .prefixes()
        .iter()
        .map(|p| p.prefix.as_str())
        .collect();
    let listed = str_list(&expected(&corpus), "prefixes");
    assert_eq!(prefixes, listed, "{name}: prefixes of specengine.toml");
}

/// Every file of the corpus matches `expected.json`: document ID, kind,
/// title, summary, parent, sections, anchors (`origin:name`), links,
/// diagnostics.
fn corpus_matches_expected(name: &str) {
    let corpus = fixture(name);
    let scheme = corpus_scheme(&corpus);
    let expected = expected(&corpus);
    let files = expected["files"].as_object().expect("files");
    let parsed = parse_corpus(&corpus, &scheme);

    let on_disk: BTreeSet<&str> = parsed.iter().map(|(p, _, _)| p.as_str()).collect();
    let listed: BTreeSet<&str> = files.keys().map(String::as_str).collect();
    assert_eq!(
        on_disk, listed,
        "{name}: files of the corpus vs expected.json"
    );

    let mut failures = Vec::new();
    for (path, bytes, parsed) in &parsed {
        let want = &files[path.as_str()];
        let document = parsed.document().expect("a UTF-8 file has a document node");
        let mut check = |what: &str, got: String, want: String| {
            if got != want {
                failures.push(format!(
                    "{name}/{path}: {what}\n   got: {got}\n  want: {want}"
                ));
            }
        };
        check(
            "id",
            format!("{:?}", document.id),
            format!("{:?}", opt_str(want, "id")),
        );
        check(
            "kind",
            format!("{:?}", document.kind),
            format!("{:?}", opt_str(want, "kind")),
        );
        check(
            "title",
            format!("{:?}", document.title),
            format!("{:?}", opt_str(want, "title")),
        );
        check(
            "summary",
            format!(
                "{:?}",
                document.summary.map(|s| text_of(bytes, s).to_owned())
            ),
            format!("{:?}", opt_str(want, "summary")),
        );
        check(
            "parent",
            format!("{:?}", document.parent.as_ref().map(|p| p.id.clone())),
            format!("{:?}", opt_str(want, "parent")),
        );
        let sections: Vec<Value> = parsed
            .sections()
            .iter()
            .map(|s| {
                let mut object = serde_json::Map::new();
                object.insert("id".into(), s.id.clone().into());
                object.insert("kind".into(), s.kind.clone().into());
                object.insert("level".into(), s.level.into());
                if let Some(parent) = &s.parent {
                    object.insert("parent".into(), parent.id.clone().into());
                }
                object.insert("title".into(), s.title.clone().into());
                if let Some(rev) = s.rev {
                    object.insert("rev".into(), rev.into());
                }
                if !s.classes.is_empty() {
                    object.insert("classes".into(), s.classes.clone().into());
                }
                Value::Object(object)
            })
            .collect();
        check(
            "sections",
            serde_json::to_string(&sections).unwrap(),
            serde_json::to_string(&want["sections"]).unwrap(),
        );
        // `origin:name`, in source order (a heading's slug before its attribute).
        let anchors: Vec<String> = parsed
            .anchors
            .iter()
            .map(|a| {
                let origin = match a.origin {
                    AnchorOrigin::Slug => "slug",
                    AnchorOrigin::Attr => "attr",
                    AnchorOrigin::Html => "html",
                };
                format!("{origin}:{}", a.name)
            })
            .collect();
        check(
            "anchors",
            format!("{anchors:?}"),
            format!("{:?}", str_list(want, "anchors")),
        );
        let links: Vec<String> = parsed.links.iter().map(render_link).collect();
        check(
            "links",
            format!("{links:#?}"),
            format!("{:#?}", str_list(want, "links")),
        );
        check(
            "diagnostics",
            format!("{:?}", render_diagnostics(parsed)),
            format!("{:?}", str_list(want, "diagnostics")),
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A node without a declared `kind` takes its prefix's kind; every prefix of
/// the scheme is exercised by at least one such node (document or section).
fn kinds_come_from_the_corpus_scheme(name: &str) {
    let corpus = fixture(name);
    let scheme = corpus_scheme(&corpus);
    let expected = expected(&corpus);
    let mut covered = BTreeSet::new();
    for (path, _, parsed) in parse_corpus(&corpus, &scheme) {
        let declared = expected["files"][path.as_str()]["declared_kind"]
            .as_bool()
            .unwrap_or(false);
        for (index, node) in parsed.nodes.iter().enumerate() {
            let Some(id) = &node.id else { continue };
            if index == 0 && declared {
                continue;
            }
            let prefix_kind = scheme.kind_of_id(id).expect("a node ID is of the scheme");
            assert_eq!(
                node.kind.as_deref(),
                Some(prefix_kind),
                "{name}/{path}: node {id} takes its kind from [ids]"
            );
            covered.insert(id.split('-').next().unwrap().to_owned());
        }
    }
    let all: BTreeSet<String> = scheme.prefixes().iter().map(|p| p.prefix.clone()).collect();
    assert_eq!(
        covered, all,
        "{name}: every prefix has an undeclared-kind node"
    );
}

// ------------------------------------------------------------------- per corpus

#[test]
fn spec_a_scheme_is_read_from_the_corpus() {
    scheme_is_read_from_the_corpus("spec-a");
}

#[test]
fn spec_b_scheme_is_read_from_the_corpus() {
    scheme_is_read_from_the_corpus("spec-b");
}

#[test]
fn spec_a_matches_expected() {
    corpus_matches_expected("spec-a");
}

#[test]
fn spec_b_matches_expected() {
    corpus_matches_expected("spec-b");
}

#[test]
fn spec_a_kinds_come_from_its_scheme() {
    kinds_come_from_the_corpus_scheme("spec-a");
}

#[test]
fn spec_b_kinds_come_from_its_scheme() {
    kinds_come_from_the_corpus_scheme("spec-b");
}

// ------------------------------------------------------------ across the corpora

#[test]
fn the_two_corpora_differ_in_prefixes_kinds_and_language() {
    let a = corpus_scheme(&fixture("spec-a"));
    let b = corpus_scheme(&fixture("spec-b"));
    let names = |s: &IdScheme| -> BTreeSet<String> {
        s.prefixes()
            .iter()
            .flat_map(|p| std::iter::once(p.prefix.clone()).chain(p.aliases_from.iter().cloned()))
            .collect()
    };
    let shared: Vec<String> = names(&a).intersection(&names(&b)).cloned().collect();
    assert!(shared.is_empty(), "prefixes or aliases shared: {shared:?}");
    let kinds = |s: &IdScheme| -> BTreeSet<String> {
        s.prefixes().iter().map(|p| p.kind.clone()).collect()
    };
    assert_ne!(
        kinds(&a),
        kinds(&b),
        "the two corpora must not share one kind set"
    );
    // spec-b carries a Cyrillic alias and Russian prose; spec-a is ASCII prose.
    assert!(
        b.prefixes()
            .iter()
            .flat_map(|p| p.aliases_from.iter())
            .any(|alias| alias
                .chars()
                .any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))),
        "spec-b has a Cyrillic aliases_from"
    );
    let cyrillic_files = walked_md_files(&fixture("spec-b"))
        .iter()
        .filter(|(_, bytes)| {
            String::from_utf8_lossy(bytes)
                .chars()
                .filter(|c| ('\u{0400}'..='\u{04FF}').contains(c))
                .count()
                > 20
        })
        .count();
    assert!(
        cyrillic_files >= 5,
        "spec-b prose is Russian ({cyrillic_files} files)"
    );
}

/// No prefix or alias of either `[ids]` is a string literal in the model or
/// core sources: the core knows no subject domain. The scan reads core's
/// `record.rs` too (docs/features/decision-apply.md AC-03: a record's
/// prefix comes only from the project's table).
#[test]
fn no_prefix_is_a_string_literal_in_model_or_core_sources() {
    let mut names = BTreeSet::new();
    for corpus in CORPORA {
        for spec in corpus_scheme(&fixture(corpus)).prefixes() {
            names.insert(spec.prefix.clone());
            names.extend(spec.aliases_from.iter().cloned());
        }
    }
    let mut offenders = Vec::new();
    let mut scanned = BTreeSet::new();
    for krate in ["specengine-model", "specengine-core"] {
        let src = repository_root().join("crates").join(krate).join("src");
        let mut stack = vec![src];
        let mut files = 0;
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).expect("src readable") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                files += 1;
                if let Ok(relative) = path.strip_prefix(repository_root()) {
                    scanned.insert(relative.to_string_lossy().into_owned());
                }
                let text = fs::read_to_string(&path).expect("UTF-8 source");
                for (number, line) in text.lines().enumerate() {
                    for name in &names {
                        // `"NAME"`, `"NAME-…"` or `"…-NAME…"` as a literal.
                        let quoted = format!("\"{name}\"");
                        let as_id = format!("\"{name}-");
                        if line.contains(&quoted) || line.contains(&as_id) {
                            offenders.push(format!(
                                "{}:{}: {}",
                                path.display(),
                                number + 1,
                                line.trim()
                            ));
                        }
                    }
                }
            }
        }
        assert!(files > 0, "{krate}: no source files");
    }
    assert!(
        scanned.contains("crates/specengine-core/src/record.rs"),
        "the record source is scanned: {scanned:?}"
    );
    assert!(
        offenders.is_empty(),
        "fixture prefixes hard-coded in model/core:\n{}",
        offenders.join("\n")
    );
}
