//! docs/features/hash-levels.md (5a-1) RD-1, "The body field" pinned to
//! the grammar: `specengine-code`'s `BODY_FIELDS` against tree-sitter-rust's
//! `node-types.json` (`grammar::NODE_TYPES`). Read here because
//! `specengine-code` reads no JSON. Re-pinned with every grammar bump.

use serde_json::Value;
use specengine_code::grammar::NODE_TYPES;
use specengine_code::items::{ITEM_KINDS, MEMBER_KINDS};
use specengine_code::{BODY_FIELDS, BodySlot};

/// "The body field" as the spec's table names it: the kind and where its
/// body sits.
const SPEC_TABLE: [(&str, &str); 12] = [
    ("function_item", "body"),
    ("function_signature_item", "-"),
    ("struct_item", "body"),
    ("enum_item", "body"),
    ("union_item", "body"),
    ("trait_item", "body"),
    ("impl_item", "body"),
    ("mod_item", "body"),
    ("const_item", "value"),
    ("static_item", "value"),
    ("type_item", "type"),
    ("macro_definition", "macro_rule children"),
];

/// The sorted type names of a `node-types.json` field or `children` entry.
fn type_names(entry: &Value) -> Vec<String> {
    let mut names: Vec<String> = entry["types"]
        .as_array()
        .unwrap_or_else(|| panic!("no types in {entry}"))
        .iter()
        .map(|t| t["type"].as_str().expect("a type name").to_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn body_field_table_matches_the_grammar() {
    let types: Vec<Value> = serde_json::from_str(NODE_TYPES).expect("node-types.json");
    let node = |kind: &str| -> &Value {
        types
            .iter()
            .find(|entry| entry["type"] == kind && entry["named"] == true)
            .unwrap_or_else(|| panic!("{kind} is no node type of the grammar"))
    };
    // One row per collected kind, and nothing else.
    let mut kinds: Vec<&str> = ITEM_KINDS.iter().chain(MEMBER_KINDS).copied().collect();
    kinds.sort_unstable();
    kinds.dedup();
    let mut rows: Vec<&str> = BODY_FIELDS.iter().map(|row| row.kind).collect();
    rows.sort_unstable();
    assert_eq!(
        rows, kinds,
        "BODY_FIELDS holds one row per ITEM_KINDS and MEMBER_KINDS kind"
    );

    let mut failures = Vec::new();
    for row in BODY_FIELDS {
        let entry = node(row.kind);
        let mut want: Vec<String> = row.types.iter().map(|t| (*t).to_owned()).collect();
        want.sort();
        let spec = SPEC_TABLE
            .iter()
            .find(|(kind, _)| *kind == row.kind)
            .map(|(_, slot)| *slot)
            .expect("the spec's table names every kind");
        match row.field {
            BodySlot::Field(name) => {
                if spec != name {
                    failures.push(format!(
                        "{}: field {name}, the spec's table {spec}",
                        row.kind
                    ));
                }
                let field = &entry["fields"][name];
                if !field.is_object() {
                    failures.push(format!("{}: the grammar has no field {name}", row.kind));
                    continue;
                }
                if type_names(field) != want {
                    failures.push(format!(
                        "{}.{name}: types {want:?}, the grammar {:?}",
                        row.kind,
                        type_names(field)
                    ));
                }
                if field["required"].as_bool() != Some(row.required) {
                    failures.push(format!(
                        "{}.{name}: required {}, the grammar {}",
                        row.kind, row.required, field["required"]
                    ));
                }
                if field["multiple"].as_bool() != Some(false) {
                    failures.push(format!("{}.{name}: a field of several nodes", row.kind));
                }
            }
            BodySlot::Children => {
                if spec != "macro_rule children" {
                    failures.push(format!("{}: children, the spec's table {spec}", row.kind));
                }
                let children = &entry["children"];
                if type_names(children) != want {
                    failures.push(format!(
                        "{}: children {want:?}, the grammar {:?}",
                        row.kind,
                        type_names(children)
                    ));
                }
                if children["required"].as_bool() != Some(row.required) {
                    failures.push(format!(
                        "{}: children required {}, the grammar {}",
                        row.kind, row.required, children["required"]
                    ));
                }
            }
            BodySlot::Absent => {
                if spec != "-" {
                    failures.push(format!("{}: no body, the spec's table {spec}", row.kind));
                }
                if !row.types.is_empty() || row.required {
                    failures.push(format!("{}: a bodiless row with types", row.kind));
                }
                for field in ["body", "value"] {
                    if entry["fields"].get(field).is_some() {
                        failures.push(format!("{}: the grammar has a {field} field", row.kind));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "BODY_FIELDS off the grammar:\n{}",
        failures.join("\n")
    );
}
