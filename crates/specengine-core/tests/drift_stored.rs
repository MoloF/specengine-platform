//! docs/features/drift-queue.md "Queue schema 8" (Corrupt: a drift row's
//! JSON off its shape; a row's later keys, 5b's `levels`, 5c's `anchor`,
//! are not) and "Review, inbox, round, stage" (`drift` `{hash, rows,
//! more}` as stored), the core half: `DriftEvidence::parse` and
//! `parse_stored`, and `StoredJson`, the ordered JSON value a review shows
//! a stored row through.
//!
//! - Every key `spec verify --json` writes in a row is required: a `null`
//!   one reads as `None`, a missing one (any of the 15, or a marker's
//!   `line` or `rev`) is off the shape, named by serde.
//! - A row's keys come back in their stored order, a later key kept, byte
//!   for byte; the read view (`DriftRow`) reads past the later key.
//! - `StoredJson` round-trips the compact JSON serde_json writes byte for
//!   byte: string escapes and Unicode, negative, float and 64-bit
//!   integers, nested arrays and objects, empty ones, keys in any order (a
//!   repeated key too); another form of the same JSON comes back in that
//!   compact form, equal as a value.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).
//!
//! M: `present` removed from one field (a missing key read as `null`);
//! `StoredJson` sorting an object's keys; `StoredJson` dropping a key the
//! view does not know.

use serde_json::{Value, json};
use specengine_core::drift::{DriftEvidence, DriftLock, DriftMarker, DriftRow, StoredJson};

/// A 40-character commit of one digit.
const COMMIT: &str = "8888888888888888888888888888888888888888";

/// `CAP`'s `code_ahead` row as the raise writes it: compact, every key, in
/// `spec verify --json`'s order.
fn written_row() -> String {
    format!(
        "{{\"node\":\"LAW-BOUND\",\"relation\":\"configures\",\
         \"file\":\"crates/tally/src/counter.rs\",\"kind\":\"item\",\
         \"symbol\":\"crates/tally::counter::CAP\",\"item_kind\":\"const_item\",\
         \"state\":\"code_ahead\",\"category\":\"code_changed\",\"detail\":null,\
         \"class\":\"check\",\"pre_existing\":true,\"markers\":[{{\"line\":8,\"rev\":2}}],\
         \"node_rev\":2,\"lock\":{{\"rev\":2,\"commit\":\"{COMMIT}\",\
         \"verified_at\":\"2026-10-08\"}},\"compared\":\"ast\"}}"
    )
}

/// The same row as a later build might store it: its keys in another
/// (non-alphabetical) order, 5b's `levels` and 5c's `anchor` among them.
fn later_row() -> String {
    format!(
        "{{\"state\":\"code_ahead\",\"node\":\"LAW-BOUND\",\
         \"levels\":{{\"lock\":[\"sig\",\"body\"],\"markers\":[\"sig\",\"body\"],\
         \"changed\":[\"body\"]}},\
         \"relation\":\"configures\",\"compared\":\"ast\",\
         \"file\":\"crates/tally/src/counter.rs\",\"symbol\":\"crates/tally::counter::CAP\",\
         \"kind\":\"item\",\"item_kind\":\"const_item\",\"detail\":null,\
         \"category\":\"code_changed\",\"pre_existing\":true,\"class\":\"check\",\
         \"markers\":[{{\"rev\":2,\"line\":8}}],\"lock\":{{\"verified_at\":\"2026-10-08\",\
         \"rev\":2,\"commit\":\"{COMMIT}\"}},\"anchor\":{{\"from\":null,\"to\":[1,2]}},\
         \"node_rev\":2}}"
    )
}

/// The view both rows read as.
fn viewed() -> DriftRow {
    DriftRow {
        node: "LAW-BOUND".to_owned(),
        relation: "configures".to_owned(),
        file: "crates/tally/src/counter.rs".to_owned(),
        kind: Some("item".to_owned()),
        symbol: Some("crates/tally::counter::CAP".to_owned()),
        item_kind: Some("const_item".to_owned()),
        state: "code_ahead".to_owned(),
        category: Some("code_changed".to_owned()),
        detail: None,
        class: Some("check".to_owned()),
        pre_existing: true,
        markers: vec![DriftMarker {
            line: 8,
            rev: Some(2),
        }],
        node_rev: Some(2),
        lock: Some(DriftLock {
            rev: 2,
            commit: COMMIT.to_owned(),
            verified_at: "2026-10-08".to_owned(),
        }),
        compared: Some("ast".to_owned()),
    }
}

fn column(rows: &[String], more: u64) -> String {
    format!("{{\"rows\":[{}],\"more\":{more}}}", rows.join(","))
}

/// `value` read as `StoredJson` and written back compact.
fn round_trip(text: &str) -> String {
    let stored: StoredJson = serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("{text} reads as StoredJson: {error}"));
    serde_json::to_string(&stored).expect("StoredJson writes")
}

// ------------------------------------------------------------ rows as stored

/// The rows as stored: a later build's row (keys out of alphabetical and
/// out of verify's order, `levels` and `anchor` added) and the raise's own
/// row each come back byte for byte, in their order; the view reads both
/// as the same row and keeps `more`. M: `StoredJson` sorting an object's
/// keys; `StoredJson` dropping a key the view does not know.
#[test]
fn stored_rows_keep_their_key_order_and_later_keys() {
    let rows = [later_row(), written_row()];
    let text = column(&rows, 3);
    let (evidence, stored) = DriftEvidence::parse_stored(&text).expect("a column of its shape");
    assert_eq!(evidence.rows, [viewed(), viewed()]);
    assert_eq!(evidence.more, 3);
    assert_eq!(evidence, DriftEvidence::parse(&text).expect("parse"));
    assert_eq!(stored.len(), 2);
    for (row, written) in stored.iter().zip(&rows) {
        assert_eq!(&serde_json::to_string(row).unwrap(), written, "as stored");
    }
    assert_eq!(
        serde_json::to_string(&stored).unwrap(),
        format!("[{}]", rows.join(",")),
        "the rows' array as stored"
    );
    // Each stored row reads as the JSON value of its text.
    for (row, written) in stored.iter().zip(&rows) {
        assert_eq!(
            serde_json::to_value(row).unwrap(),
            serde_json::from_str::<Value>(written).unwrap()
        );
    }
    // The later keys' values are kept whole.
    let later = serde_json::to_value(&stored[0]).unwrap();
    assert_eq!(
        later["levels"],
        json!({"lock": ["sig", "body"], "markers": ["sig", "body"], "changed": ["body"]})
    );
    assert_eq!(later["anchor"], json!({"from": null, "to": [1, 2]}));
}

/// A known key holding `null` is valid: every nullable key of a row and a
/// marker's `rev` `null` read as `None`, and the row comes back as stored.
/// M: a nullable key made non-nullable.
#[test]
fn known_keys_may_be_null() {
    let row = "{\"node\":\"GOAL-01\",\"relation\":\"implements\",\"file\":\"src/a.rs\",\
               \"kind\":null,\"symbol\":null,\"item_kind\":null,\"state\":\"broken_binding\",\
               \"category\":null,\"detail\":null,\"class\":null,\"pre_existing\":false,\
               \"markers\":[{\"line\":1,\"rev\":null}],\"node_rev\":null,\"lock\":null,\
               \"compared\":null}"
        .to_owned();
    let text = column(std::slice::from_ref(&row), 0);
    let (evidence, stored) = DriftEvidence::parse_stored(&text).expect("nulls are valid");
    let read = &evidence.rows[0];
    assert_eq!(
        (
            &read.kind,
            &read.symbol,
            &read.item_kind,
            &read.category,
            &read.detail,
            &read.class,
            read.node_rev,
            &read.lock,
            &read.compared,
            read.markers[0].rev,
        ),
        (
            &None, &None, &None, &None, &None, &None, None, &None, &None, None
        )
    );
    assert_eq!(serde_json::to_string(&stored[0]).unwrap(), row);
}

/// A row missing any key `spec verify --json` writes is off the shape:
/// each of the 15 keys of a row and each of a marker's two, removed in
/// turn, makes `parse` and `parse_stored` fail, serde naming the key; the
/// same key `null` (where nullable) is valid. M: `present` removed from
/// one field.
#[test]
fn a_row_missing_any_key_is_off_its_shape() {
    let full: Value = serde_json::from_str(&written_row()).unwrap();
    let keys: Vec<String> = full.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys.len(), 15, "{keys:?}");
    let mut read_back = Vec::new();
    for key in &keys {
        let mut row = full.clone();
        row.as_object_mut().unwrap().remove(key);
        let text = column(&[row.to_string()], 0);
        match DriftEvidence::parse(&text) {
            Ok(_) => read_back.push(key.clone()),
            Err(why) => assert!(
                why.contains("missing field") && why.contains(&format!("`{key}`")),
                "{key}: {why}"
            ),
        }
        assert_eq!(
            DriftEvidence::parse_stored(&text).is_err(),
            DriftEvidence::parse(&text).is_err(),
            "{key}: parse_stored agrees with parse"
        );
    }
    for key in ["line", "rev"] {
        let mut row = full.clone();
        row["markers"][0].as_object_mut().unwrap().remove(key);
        let text = column(&[row.to_string()], 0);
        match DriftEvidence::parse_stored(&text) {
            Ok(_) => read_back.push(format!("markers[].{key}")),
            Err(why) => assert!(why.contains(&format!("`{key}`")), "{key}: {why}"),
        }
    }
    assert!(
        read_back.is_empty(),
        "rows missing a key read as a drift row: {read_back:?}"
    );
}

// ---------------------------------------------------------------- StoredJson

/// `StoredJson` round-trips the compact JSON serde_json writes byte for
/// byte, and reads as the same JSON value: strings with every escape
/// serde_json writes (quote, backslash, `\n`, `\r`, `\t`, `\b`, `\f`, the
/// other control characters as `\u00XX`) and raw Unicode (two-, three-
/// and four-byte UTF-8, U+2028, DEL), negative, float and the extreme
/// 64-bit integers, nested arrays and objects, empty ones, keys escaped,
/// out of order and repeated. M: `StoredJson` sorting an object's keys;
/// dropping a key; a number or a string written in another form.
#[test]
fn stored_json_round_trips_compact_json_byte_for_byte() {
    let strings = [
        "\"\"".to_owned(),
        "\"plain\"".to_owned(),
        "\"q\\\" b\\\\ s/ n\\n r\\r t\\t b\\b f\\f\"".to_owned(),
        "\"nul\\u0000 soh\\u0001 us\\u001f esc\\u001b\"".to_owned(),
        "\"del\u{7f} e\u{e9} cjk\u{6f22}\u{5b57} ls\u{2028} emoji\u{1f600}\"".to_owned(),
    ];
    let numbers = [
        "0",
        "7",
        "-1",
        "64",
        "9223372036854775807",
        "-9223372036854775808",
        "18446744073709551615",
        "1.5",
        "-0.25",
        "0.1",
        "1.0",
        "3.141592653589793",
        "1e-7",
        "1e+300",
        "-2.5e-300",
        "1.7976931348623157e+308",
    ];
    for number in numbers {
        // Each is the form serde_json writes for its value.
        let value: Value = serde_json::from_str(number).unwrap();
        assert_eq!(value.to_string(), number, "serde_json's own form");
    }
    let structures = [
        "null",
        "true",
        "false",
        "[]",
        "{}",
        "[[],{},[{}],{\"\":\"\"}]",
        "{\"a\":{},\"b\":[]}",
        "{\"z\":0,\"y\":{\"x\":[{\"w\":null,\"v\":false,\"u\":[[[]]]}]},\"a\":-1}",
        "{\"a\":[1,[2,[3,{\"b\":{\"c\":[]}}]]],\"d\":{}}",
        "{\"k\\\"ey\":1,\"\u{e9}\":2,\"\\n\":3,\"\\u0001\":4,\"\":5}",
        "[-9223372036854775808,18446744073709551615,-0.5,1e-7,\"\\t\",null,{}]",
    ];
    let repeated = "{\"b\":1,\"a\":2,\"b\":3}";
    let mut cases: Vec<String> = Vec::new();
    cases.extend(strings);
    cases.extend(numbers.iter().map(|number| (*number).to_owned()));
    cases.extend(structures.iter().map(|text| (*text).to_owned()));
    cases.push(repeated.to_owned());
    let mut changed = Vec::new();
    for text in &cases {
        let back = round_trip(text);
        if &back != text {
            changed.push(format!("{text} -> {back}"));
            continue;
        }
        // Deterministic: a second pass writes the same bytes.
        assert_eq!(round_trip(&back), back, "{text}");
        if text != repeated {
            assert_eq!(
                serde_json::from_str::<Value>(&back).unwrap(),
                serde_json::from_str::<Value>(text).unwrap(),
                "{text}: the same JSON value"
            );
        }
    }
    assert!(
        changed.is_empty(),
        "not byte for byte:\n{}",
        changed.join("\n")
    );
    // A whole column of such values, as rows: kept whole.
    let row = format!("{{\"extra\":{},{}", structures[7], &written_row()[1..]);
    let (_, stored) = DriftEvidence::parse_stored(&column(std::slice::from_ref(&row), 0))
        .expect("a later key of any shape");
    assert_eq!(serde_json::to_string(&stored[0]).unwrap(), row);
}

/// Another form of the same JSON (blanks, `\u` escapes serde_json writes
/// raw, `\/`, an exponent it writes otherwise) comes back in the compact
/// form serde_json writes, equal as a value: the column is compact JSON as
/// the raise wrote it, so "as stored" is that form.
#[test]
fn stored_json_writes_other_forms_compact() {
    for (text, compact) in [
        (
            " { \"b\" : 1 ,\n \"a\" : [ 1 , 2 ] } ",
            "{\"b\":1,\"a\":[1,2]}",
        ),
        ("\"\\u00e9\\u6f22\"", "\"\u{e9}\u{6f22}\""),
        ("\"\\ud83d\\ude00\"", "\"\u{1f600}\""),
        ("\"a\\/b\"", "\"a/b\""),
        ("\"\\u007f\"", "\"\u{7f}\""),
        ("1E2", "100.0"),
        ("1.50", "1.5"),
    ] {
        assert_eq!(round_trip(text), compact, "{text}");
        assert_eq!(
            serde_json::from_str::<Value>(compact).unwrap(),
            serde_json::from_str::<Value>(text).unwrap(),
            "{text}: the same value"
        );
    }
}
