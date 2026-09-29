//! AC-17 of docs/features/spec-parser.md: output depends only on (path,
//! bytes, scheme). The fixture corpora parsed twice, and once in reversed
//! file order, serialise to byte-identical JSON; maps serialise in source
//! order.

mod common;

use specengine_model::IdScheme;

use common::{corpus_scheme, fixture, json, md_files};

const CORPORA: [&str; 3] = ["spec-a", "spec-b", "corpus-mini"];

/// The corpus as one JSON document, files in path order, parsed in `order`.
fn corpus_json(files: &[(String, Vec<u8>)], scheme: &IdScheme, reversed: bool) -> String {
    let mut indices: Vec<usize> = (0..files.len()).collect();
    if reversed {
        indices.reverse();
    }
    let mut parsed: Vec<(usize, String)> = indices
        .into_iter()
        .map(|i| {
            let (path, bytes) = &files[i];
            (i, json(&specengine_core::parse(path, bytes, scheme)))
        })
        .collect();
    parsed.sort_by_key(|(i, _)| *i);
    let mut out = String::from("[");
    for (n, (_, file)) in parsed.iter().enumerate() {
        if n > 0 {
            out.push(',');
        }
        out.push_str(file);
    }
    out.push(']');
    out
}

#[test]
fn parsing_twice_and_in_reverse_order_gives_identical_json() {
    for corpus in CORPORA {
        let dir = fixture(corpus);
        let files = md_files(&dir);
        let first = corpus_json(&files, &corpus_scheme(&dir), false);
        let second = corpus_json(&files, &corpus_scheme(&dir), false);
        let reversed = corpus_json(&files, &corpus_scheme(&dir), true);
        assert_eq!(first, second, "{corpus}: second run differs");
        assert_eq!(first, reversed, "{corpus}: reversed order differs");
        assert!(first.len() > 1_000, "{corpus}: non-trivial output");
    }
}

#[test]
fn repeated_parses_of_map_heavy_front_matter_are_identical() {
    // Maps with many keys: any hash-ordered map in the output would show
    // within a few repetitions.
    let mut yaml = String::from("---\nid: R-12\nraised_by:\n");
    for key in ["k", "b", "x", "a", "m", "z", "c", "q", "e", "t", "d", "s"] {
        yaml.push_str(&format!("  {key}: {key}{key}\n"));
    }
    yaml.push_str("links:\n");
    for link_type in [
        "verifies",
        "answers",
        "depends_on",
        "amends",
        "constrains",
        "revises",
    ] {
        yaml.push_str(&format!("  {link_type}: [A-101]\n"));
    }
    for key in ["x_zeta", "x_alpha", "x_mid", "x_beta", "x_omega", "x_gamma"] {
        yaml.push_str(&format!("{key}: {{b: 1, a: 2, c: 3, e: 4, d: 5}}\n"));
    }
    yaml.push_str("---\n\n# T\n");
    let scheme = corpus_scheme(&fixture("spec-a"));
    let first = json(&specengine_core::parse("m.md", yaml.as_bytes(), &scheme));
    for round in 0..20 {
        let again = json(&specengine_core::parse("m.md", yaml.as_bytes(), &scheme));
        assert_eq!(again, first, "round {round}");
    }
    // Source order, not sorted order: `k` comes first as written.
    let k = first.find("\"k\":\"kk\"").expect("raised_by.k");
    let a = first.find("\"a\":\"aa\"").expect("raised_by.a");
    assert!(k < a, "raised_by keeps source order");
    let zeta = first.find("x_zeta").unwrap();
    let alpha = first.find("x_alpha").unwrap();
    assert!(zeta < alpha, "extra keeps source order");
}

#[test]
fn the_path_only_names_the_file() {
    let dir = fixture("spec-a");
    let scheme = corpus_scheme(&dir);
    for (path, bytes) in md_files(&dir) {
        let a = specengine_core::parse(&path, &bytes, &scheme);
        let mut b = specengine_core::parse("elsewhere/other.md", &bytes, &scheme);
        assert_eq!(b.path, "elsewhere/other.md");
        b.path = a.path.clone();
        assert_eq!(json(&a), json(&b), "{path}");
    }
}
