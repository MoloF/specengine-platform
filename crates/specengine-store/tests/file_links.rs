//! AC-03 of docs/features/spec-check-links.md: Markdown file links in the
//! store. They are `links` rows with `dst_path` as written (never
//! resolved), `dst_id` NULL, the `mentions` type and the section or
//! document source; `file(path)` gives them back as parsed (also covered
//! for every fixture file by `projection.rs`); an incremental update after
//! link edits equals a fresh index. No schema change: the rows are the
//! existing `links` columns.

#![cfg(unix)]

mod common;

use common::{Corpus, Scratch, assert_equals_fresh};
use specengine_model::{LinkOrigin, LinkTarget};
use specengine_store::SpecIndex;

const README: &str = "docs/spec/movement/README.md";

/// The `links` rows of the dump for one file, as JSON arrays:
/// `[project, root, path, ord, src, type, dst_id, dst_path, link]`.
fn link_rows(dump: &str, path: &str) -> Vec<serde_json::Value> {
    dump.lines()
        .filter_map(|line| line.strip_prefix("links\t"))
        .map(|row| serde_json::from_str::<serde_json::Value>(row).expect("a JSON row"))
        .filter(|row| row[2] == path)
        .collect()
}

#[test]
fn spec_a_dump_has_the_two_file_links_as_dst_path_rows() {
    let scratch = Scratch::new("file-links-dump");
    let corpus = Corpus::copy_of("spec-a", &scratch, "spec-a");
    let dump = corpus.fresh_dump(&scratch);
    let rows = link_rows(&dump, README);
    let mut view: Vec<(i64, String, String, serde_json::Value, serde_json::Value)> = rows
        .iter()
        .map(|row| {
            (
                row[3].as_i64().expect("ord"),
                row[4].as_str().unwrap_or("-").to_owned(),
                row[5].as_str().expect("type").to_owned(),
                row[6].clone(),
                row[7].clone(),
            )
        })
        .collect();
    view.sort_by_key(|row| row.0);
    assert_eq!(
        view,
        [
            (
                0,
                "DOM-MOVEMENT".to_owned(),
                "mentions".to_owned(),
                serde_json::Value::Null,
                serde_json::json!("stamina.md")
            ),
            (
                1,
                "DOM-MOVEMENT".to_owned(),
                "mentions".to_owned(),
                serde_json::Value::Null,
                serde_json::json!("sprint.md")
            ),
            (
                2,
                "RULE-MOVE-SPEEDS".to_owned(),
                "mentions".to_owned(),
                serde_json::json!("MEC-STAMINA"),
                serde_json::Value::Null
            ),
        ],
        "{rows:#?}"
    );
    // The `link` column is the link as parsed (the spec's Data JSON).
    let ord0 = rows.iter().find(|row| row[3] == 0).expect("ord 0").clone();
    let link: serde_json::Value = match &ord0[8] {
        serde_json::Value::String(text) => serde_json::from_str(text).expect("link JSON"),
        other => other.clone(),
    };
    assert_eq!(
        link,
        serde_json::json!({
            "src": "DOM-MOVEMENT",
            "type": "mentions",
            "origin": "inline",
            "dst": {"path": "stamina.md", "span": [180, 190]}
        })
    );
}

#[test]
fn spec_b_dump_keeps_the_link_base_link_as_written() {
    let scratch = Scratch::new("file-links-spec-b");
    let corpus = Corpus::copy_of("spec-b", &scratch, "spec-b");
    let dump = corpus.fresh_dump(&scratch);
    let rows = link_rows(&dump, "docs/records/REQ/REQ-001.md");
    assert_eq!(rows.len(), 1, "{rows:#?}");
    assert_eq!(rows[0][4], "REQ-001");
    assert_eq!(rows[0][6], serde_json::Value::Null, "no dst_id");
    assert_eq!(
        rows[0][7], "spec/cli.md",
        "dst_path as written, never resolved through `link_base`"
    );
}

#[test]
fn file_reads_back_file_links_as_parsed() {
    let scratch = Scratch::new("file-links-read");
    let corpus = Corpus::copy_of("spec-a", &scratch, "spec-a");
    let index = corpus.fresh(&scratch);
    let stored = index
        .file(README)
        .expect("file")
        .and_then(|file| file.parsed)
        .expect("parsed");
    let parsed = specengine_core::parse(README, &corpus.bytes(README), &corpus.scheme);
    assert_eq!(stored, parsed);
    let paths: Vec<&str> = stored
        .links
        .iter()
        .filter(|link| link.origin == LinkOrigin::Inline)
        .filter_map(|link| match &link.dst {
            LinkTarget::Path(target) => Some(target.path.as_str()),
            LinkTarget::Reference(_) => None,
        })
        .collect();
    assert_eq!(paths, ["stamina.md", "sprint.md"]);
}

#[test]
fn incremental_equals_fresh_after_link_edits() {
    let scratch = Scratch::new("file-links-incremental");
    let corpus = Corpus::copy_of("spec-a", &scratch, "spec-a");
    let mut index = corpus.fresh(&scratch);
    assert_equals_fresh(&index, &corpus, &scratch, "the start");

    corpus.replace(README, "(stamina.md)", "(stamina.md#regeneration)");
    corpus.update(&mut index);
    assert_equals_fresh(&index, &corpus, &scratch, "an anchor added");

    corpus.replace(
        README,
        "[sprint](sprint.md).",
        "[sprint][s].\n\n[s]: ../gone.md",
    );
    corpus.update(&mut index);
    assert_equals_fresh(&index, &corpus, &scratch, "a reference definition");

    corpus.replace(README, "[stamina](stamina.md#regeneration)", "stamina");
    corpus.update(&mut index);
    assert_equals_fresh(&index, &corpus, &scratch, "a link removed");

    let stored = index
        .file(README)
        .expect("file")
        .and_then(|file| file.parsed)
        .expect("parsed");
    let paths: Vec<&str> = stored
        .links
        .iter()
        .filter_map(|link| match &link.dst {
            LinkTarget::Path(target) => Some(target.path.as_str()),
            LinkTarget::Reference(_) => None,
        })
        .collect();
    assert_eq!(paths, ["../gone.md"], "only the definition is left");
}
