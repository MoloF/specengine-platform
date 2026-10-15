//! docs/features/hash-levels-ron.md (5a-1b) TE-1 over the `.ron` inputs of
//! `fixtures/hash-levels/ron/` and the `ron` section of
//! `fixtures/hash-levels/expected.json`:
//!
//! - the inventory: every `.ron` file, the comments that get a value hash
//!   (in source order, each with the path `comment_anchors` gives it) and
//!   those that get none, or the refusal;
//! - per value, its canonical rendering (the tokens after the recipe's
//!   drops, one space apart) hashes alike: comments, layout and a `,` before
//!   a closer never enter a stream;
//! - per value form (scalar, multi-line string, struct, named struct, tuple,
//!   list, map, nested) an equal pair and a different pair, each naming its
//!   mutation, on LF and on an in-memory CRLF copy;
//! - every value equal on the CRLF copy, keyed by the copy's own ranges;
//! - a file `RonAnalysis` rejects (a lexer error; a structure error) refuses
//!   every value, its detail the first `error_categories` entry.
//!
//! JSON is read here, in the eval crate, because `specengine-code` has no
//! JSON dependency (as 5a-1's `hash_levels_fixture.rs`). The digests' bytes
//! are pinned against the recipe in
//! `crates/specengine-code/tests/hash_levels_ron.rs`.
//!
//! PHASE-B: `ron::value_hashes` and `ron::RonValues` by the spec's names
//! ("Entry point"); `values` is the only function reading that shape.

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::{Path, PathBuf};

use serde_json::Value;
use specengine_code::Digest;
use specengine_code::ron::lexer::LexErrorKind;
use specengine_code::ron::{self, Anchor, RonValues};

// ---------------------------------------------------------------- the fixture

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/hash-levels")
}

/// The `ron` section of `expected.json`.
fn expected() -> Value {
    let path = fixture_root().join("expected.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut all: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    all["ron"].take()
}

fn read(relative: &str) -> String {
    let path = fixture_root().join(relative);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        !text.contains('\r'),
        "{relative}: fixture sources are LF only; the CRLF copy is made in memory"
    );
    text
}

/// Every `.ron` file of the fixture outside `variants/`, relative, sorted.
fn fixture_ron_files() -> Vec<String> {
    let root = fixture_root();
    let mut out = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            let relative = path
                .strip_prefix(&root)
                .expect("below the fixture root")
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                if relative != "variants" {
                    pending.push(path);
                }
            } else if relative.ends_with(".ron") {
                out.push(relative);
            }
        }
    }
    out.sort();
    out
}

fn crlf(source: &str) -> String {
    source.replace('\n', "\r\n")
}

/// `source` with `edit.from` (present exactly once) replaced by `edit.to`.
fn apply(source: &str, edit: &Value, what: &str) -> String {
    let from = edit["from"].as_str().expect("edit.from");
    let to = edit["to"].as_str().expect("edit.to");
    let count = source.matches(from).count();
    assert_eq!(
        count, 1,
        "{what}: edit.from {from:?} must occur exactly once, found {count}"
    );
    source.replacen(from, to, 1)
}

fn text<'v>(value: &'v Value, key: &str, what: &str) -> &'v str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{what}: `{key}` is a string"))
}

fn array<'v>(value: &'v Value, key: &str, what: &str) -> &'v Vec<Value> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("{what}: `{key}` is an array"))
}

// ---------------------------------------------------------------- the values

/// What `ron::value_hashes` gives, copied out of `RonValues`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Values {
    Hashed(Vec<(Range<usize>, Digest)>),
    Refused(&'static str),
}

/// PHASE-B: the one place that reads the shape of `RonValues`.
fn values(source: &str) -> Values {
    match ron::value_hashes(source) {
        RonValues::Hashed(entries) => Values::Hashed(entries),
        RonValues::CannotVerify(detail) => Values::Refused(detail),
    }
}

fn hashed(source: &str, what: &str) -> Vec<(Range<usize>, Digest)> {
    match values(source) {
        Values::Hashed(entries) => entries,
        Values::Refused(detail) => panic!("{what}: refused ({detail}), expected hashed"),
    }
}

fn keys(entries: &[(Range<usize>, Digest)]) -> Vec<Range<usize>> {
    entries.iter().map(|(range, _)| range.clone()).collect()
}

/// A comment's text: its bytes without the CR before a line comment's end.
fn comment_text(source: &str, range: &Range<usize>) -> String {
    source[range.clone()].trim_end_matches('\r').to_owned()
}

/// The range of the one comment whose whole text is `wanted`.
fn comment(source: &str, wanted: &str, what: &str) -> Range<usize> {
    let hits: Vec<Range<usize>> = ron::analyze(source)
        .comments
        .into_iter()
        .filter(|range| comment_text(source, range) == wanted)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "{what}: exactly one comment {wanted:?}, found {hits:?}"
    );
    hits[0].clone()
}

/// The value hash keyed by the comment `wanted`.
fn digest_of(source: &str, wanted: &str, what: &str) -> Digest {
    let key = comment(source, wanted, what);
    let entries = hashed(source, what);
    let hits: Vec<Digest> = entries
        .iter()
        .filter(|(range, _)| *range == key)
        .map(|(_, digest)| *digest)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "{what}: one entry keyed by {wanted:?} at {key:?}; keys {:?}",
        keys(&entries)
    );
    hits[0]
}

fn anchor_of(source: &str, range: &Range<usize>, what: &str) -> Anchor {
    ron::comment_anchors(source)
        .into_iter()
        .find(|c| c.comment == *range)
        .map(|c| c.anchor)
        .unwrap_or_else(|| panic!("{what}: no anchor for the comment at {range:?}"))
}

fn class(anchor: &Anchor) -> &'static str {
    match anchor {
        Anchor::Path { .. } => "path",
        Anchor::Ambiguous { .. } => "ambiguous",
        Anchor::Unanchored => "unanchored",
        Anchor::CannotVerify => "cannot_verify",
    }
}

/// The texts of the comments `comment_anchors` binds to a path.
fn path_comment_texts(source: &str) -> Vec<String> {
    ron::comment_anchors(source)
        .into_iter()
        .filter(|c| matches!(c.anchor, Anchor::Path { .. }))
        .map(|c| comment_text(source, &c.comment))
        .collect()
}

fn relation(equal: bool) -> &'static str {
    if equal { "equal" } else { "differs" }
}

// ---------------------------------------------------------------- TE-1

#[test]
fn te1_ron_inventory_lists_every_value_of_every_ron_file() {
    let expected = expected();
    let files = expected["files"].as_object().expect("ron.files");
    let mut listed: Vec<String> = files.keys().cloned().collect();
    listed.sort();
    assert_eq!(
        listed,
        fixture_ron_files(),
        "expected.json `ron.files` must list every .ron file outside variants/"
    );
    let mut failures = Vec::new();
    for (file, entry) in files {
        let source = read(file);
        match text(entry, "state", file) {
            "hashed" => {
                let got = hashed(&source, file);
                let wanted = array(entry, "values", file);
                let want_keys: Vec<Range<usize>> = wanted
                    .iter()
                    .map(|value| comment(&source, text(value, "comment", file), file))
                    .collect();
                if keys(&got) != want_keys {
                    failures.push(format!(
                        "{file}: entries keyed {:?}, expected the listed comments at {want_keys:?}",
                        keys(&got)
                    ));
                }
                for (value, key) in wanted.iter().zip(&want_keys) {
                    let path = text(value, "path", file);
                    match anchor_of(&source, key, file) {
                        Anchor::Path { path: bound, .. } if bound == path => {}
                        other => failures.push(format!(
                            "{file}: {:?} anchors {other:?}, expected the path {path}",
                            text(value, "comment", file)
                        )),
                    }
                }
                for none in array(entry, "no_entry", file) {
                    let wanted_text = text(none, "comment", file);
                    let key = comment(&source, wanted_text, file);
                    let got_class = class(&anchor_of(&source, &key, file));
                    if got_class != text(none, "anchor", file) {
                        failures.push(format!(
                            "{file}: {wanted_text:?} anchors {got_class}, expected {}",
                            none["anchor"]
                        ));
                    }
                    if got.iter().any(|(range, _)| *range == key) {
                        failures.push(format!("{file}: {wanted_text:?} has an entry"));
                    }
                }
            }
            "cannot_verify" => {
                if !matches!(values(&source), Values::Refused(_)) {
                    failures.push(format!("{file}: expected cannot_verify, got hashed"));
                }
            }
            other => panic!("{file}: unknown state {other:?}"),
        }
    }
    assert!(failures.is_empty(), "inventory:\n{}", failures.join("\n"));
}

/// The value of each listed comment hashes as its canonical rendering does:
/// `/* v */ <canonical>`, a one-value file, on LF and on the CRLF copy.
#[test]
fn te1_ron_values_hash_their_canonical_tokens() {
    let expected = expected();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (file, entry) in expected["files"].as_object().expect("ron.files") {
        if text(entry, "state", file) != "hashed" {
            continue;
        }
        let source = read(file);
        for value in array(entry, "values", file) {
            let wanted = text(value, "comment", file);
            let canonical = text(value, "canonical", file);
            let rendering = format!("/* v */ {canonical}\n");
            let rendered = hashed(&rendering, &format!("{file}: rendering of {wanted:?}"));
            assert!(
                rendered.len() == 1 && rendered[0].0 == (0..7),
                "{file}: premise: the rendering of {wanted:?} is a one-value file: {:?}",
                keys(&rendered)
            );
            let want = rendered[0].1;
            for (mode, copy) in [("LF", source.clone()), ("CRLF", crlf(&source))] {
                let got = digest_of(&copy, wanted, file);
                if got != want {
                    failures.push(format!(
                        "{file} ({mode}): {wanted:?} hashes otherwise than its tokens {canonical:?}"
                    ));
                }
            }
            checked += 1;
        }
    }
    assert!(checked >= 25, "every listed value checked: {checked}");
    assert!(failures.is_empty(), "canonical:\n{}", failures.join("\n"));
}

/// A rejected file refuses every value, the comments with a path anchor
/// included, on LF and on the CRLF copy (Q7).
#[test]
fn te1_ron_rejected_files_refuse_every_value() {
    let expected = expected();
    let mut refused = 0;
    let mut lexer_only = false;
    let mut structure_only = false;
    for (file, entry) in expected["files"].as_object().expect("ron.files") {
        if text(entry, "state", file) != "cannot_verify" {
            continue;
        }
        let source = read(file);
        let detail = text(entry, "detail", file);
        let categories: Vec<&str> = array(entry, "categories", file)
            .iter()
            .map(|c| c.as_str().expect("a category"))
            .collect();
        assert_eq!(
            categories.first(),
            Some(&detail),
            "{file}: the detail is the first category"
        );
        let path_comments: Vec<&str> = array(entry, "path_comments", file)
            .iter()
            .map(|c| c.as_str().expect("a comment text"))
            .collect();
        assert!(!path_comments.is_empty(), "{file}: a refusal covers values");
        for (mode, copy) in [("LF", source.clone()), ("CRLF", crlf(&source))] {
            assert_eq!(
                ron::analyze(&copy).error_categories,
                categories,
                "{file} ({mode}): premise: RonAnalysis's categories by hand"
            );
            assert_eq!(
                path_comment_texts(&copy),
                path_comments,
                "{file} ({mode}): premise: the comments with a path anchor"
            );
            match values(&copy) {
                Values::Refused(got) => {
                    assert_eq!(
                        got,
                        ron::analyze(&copy).error_categories[0],
                        "{file} ({mode}): the detail is the first category"
                    );
                    assert_eq!(got, detail, "{file} ({mode})");
                }
                hashed => panic!("{file} ({mode}): every value refused, got {hashed:?}"),
            }
        }
        let lexical = [
            LexErrorKind::UnterminatedString,
            LexErrorKind::UnterminatedBlockComment,
            LexErrorKind::UnterminatedChar,
            LexErrorKind::UnterminatedAttribute,
            LexErrorKind::StrayToken,
        ]
        .map(LexErrorKind::as_str);
        let is_lexical = |c: &&str| lexical.iter().any(|l| l == c);
        lexer_only |= categories.iter().all(is_lexical);
        structure_only |= !categories.iter().any(is_lexical);
        refused += 1;
    }
    assert!(refused >= 3, "the fixture holds its rejected files");
    assert!(lexer_only, "a file with a lexer error alone");
    assert!(structure_only, "a file with a structure error alone");
}

#[test]
fn te1_ron_pairs_hold_on_lf_and_on_crlf() {
    let expected = expected();
    let mut failures = Vec::new();
    for pair in array(&expected, "pairs", "ron") {
        let id = text(pair, "id", "a pair");
        let file = text(pair, "file", id);
        let a = text(pair, "a", id);
        let b = pair.get("b").map_or(a, |b| b.as_str().expect("pair.b"));
        let source_a = read(file);
        let source_b = match pair.get("edit") {
            Some(edit) => apply(&source_a, edit, id),
            None => {
                assert_ne!(a, b, "{id}: a pair without an edit names two comments");
                source_a.clone()
            }
        };
        let want = text(pair, "expect", id);
        assert!(matches!(want, "equal" | "differs"), "{id}: {want:?}");
        for (mode, a_text, b_text) in [
            ("LF", source_a.clone(), source_b.clone()),
            ("CRLF", crlf(&source_a), crlf(&source_b)),
        ] {
            let got = relation(digest_of(&a_text, a, id) == digest_of(&b_text, b, id));
            if got != want {
                failures.push(format!(
                    "{id} ({mode}): {got}, expected {want} -- mutation: {}",
                    text(pair, "mutation", id)
                ));
            }
        }
    }
    assert!(failures.is_empty(), "pairs:\n{}", failures.join("\n"));
}

/// Every form has an equal pair and a different pair; every pair names its
/// mutation; ids are unique.
#[test]
fn te1_ron_pairs_cover_every_value_form() {
    let expected = expected();
    let forms: Vec<&str> = array(&expected, "forms", "ron")
        .iter()
        .map(|f| f.as_str().expect("a form"))
        .collect();
    for form in [
        "scalar",
        "multiline-string",
        "struct",
        "named-struct",
        "tuple",
        "list",
        "map",
        "nested",
    ] {
        assert!(forms.contains(&form), "TE-1 names the form {form}");
    }
    let mut ids = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for pair in array(&expected, "pairs", "ron") {
        let id = text(pair, "id", "a pair");
        assert!(ids.insert(id.to_owned()), "pair id {id} is not unique");
        assert!(
            !text(pair, "mutation", id).trim().is_empty(),
            "pair {id} names no mutation"
        );
        let form = text(pair, "form", id);
        assert!(
            form == "root" || forms.contains(&form),
            "{id}: unknown form {form}"
        );
        seen.insert((form.to_owned(), text(pair, "expect", id).to_owned()));
    }
    let mut missing = Vec::new();
    for form in &forms {
        for want in ["equal", "differs"] {
            if !seen.contains(&((*form).to_owned(), want.to_owned())) {
                missing.push(format!("{form}: no {want} pair"));
            }
        }
    }
    assert!(missing.is_empty(), "coverage:\n{}", missing.join("\n"));
}

/// The CRLF copy of a hashed file: the same digests in the same order, each
/// keyed by the copy's own range of the same comment.
#[test]
fn te1_ron_every_value_is_equal_on_the_crlf_copy() {
    let expected = expected();
    let mut checked = 0;
    for (file, entry) in expected["files"].as_object().expect("ron.files") {
        if text(entry, "state", file) != "hashed" {
            continue;
        }
        let lf = read(file);
        let copy = crlf(&lf);
        let lf_entries = hashed(&lf, file);
        let copy_entries = hashed(&copy, file);
        assert_eq!(lf_entries.len(), copy_entries.len(), "{file}");
        for ((lf_key, lf_digest), (copy_key, copy_digest)) in lf_entries.iter().zip(&copy_entries) {
            let shift = lf[..lf_key.start].matches('\n').count();
            assert_eq!(copy_key.start, lf_key.start + shift, "{file}: {lf_key:?}");
            assert_eq!(
                comment_text(&copy, copy_key).replace("\r\n", "\n"),
                comment_text(&lf, lf_key),
                "{file}: the same comment"
            );
            assert_eq!(
                lf_digest,
                copy_digest,
                "{file}: {:?} under CRLF",
                comment_text(&lf, lf_key)
            );
            checked += 1;
        }
    }
    assert!(checked >= 25, "every value checked: {checked}");
}
