//! The JSON Schemas of the read tools (task spec `mcp-read`, "Data").
//!
//! Input schemas come from the argument types; output schemas from mirror
//! types of the CLI's `--json` documents (CLI README; the canons
//! `spec-cli-graph.md`, `spec-cli-bundle.md`). The CLI must not depend on
//! rmcp or schemars, so its JSON views cannot derive `JsonSchema`: these
//! types repeat their keys, schema only: nothing is ever built from them,
//! `structuredContent` is the CLI's own document. Every key is `required`
//! (absent = `null`); every subschema is inlined (no `$ref`).

// The mirror types are never built or read: only their schemas are used.
#![allow(dead_code)]

use std::borrow::Cow;
use std::sync::Arc;

use rmcp::model::JsonObject;
use rmcp::schemars::generate::SchemaSettings;
use rmcp::schemars::{self, JsonSchema, Schema, SchemaGenerator};
use serde_json::Value;

/// The input schema of `T`: a flat object, subschemas inlined, without the
/// root `title` and `description` (the type's name and doc).
pub(crate) fn input_schema<T: JsonSchema>() -> Arc<JsonObject> {
    Arc::new(generate::<T>())
}

/// The output schema of the mirror type `T`: as [`input_schema`], and every
/// object's properties all `required`.
pub(crate) fn output_schema<T: JsonSchema>() -> Arc<JsonObject> {
    let mut schema = Value::Object(generate::<T>());
    require_all(&mut schema);
    match schema {
        Value::Object(object) => Arc::new(object),
        _ => Arc::new(JsonObject::new()),
    }
}

/// `T`'s JSON Schema 2020-12 with every subschema inlined.
fn generate<T: JsonSchema>() -> JsonObject {
    let settings = SchemaSettings::draft2020_12().with(|settings| {
        settings.inline_subschemas = true;
    });
    let schema = settings.into_generator().into_root_schema_for::<T>();
    let mut object = match Value::from(schema) {
        Value::Object(object) => object,
        // A boolean schema: never for a struct.
        _ => {
            let mut object = JsonObject::new();
            object.insert("type".to_owned(), Value::String("object".to_owned()));
            object
        }
    };
    object.remove("title");
    object.remove("description");
    object
}

/// Every object schema with `properties` lists them all in `required`.
fn require_all(schema: &mut Value) {
    match schema {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get("properties") {
                let names: Vec<Value> = properties
                    .keys()
                    .map(|name| Value::String(name.clone()))
                    .collect();
                object.insert("required".to_owned(), Value::Array(names));
            }
            for value in object.values_mut() {
                require_all(value);
            }
        }
        Value::Array(items) => {
            for item in items {
                require_all(item);
            }
        }
        _ => {}
    }
}

/// `"type": "null"`: a key the CLI always writes as `null` (`bundle`'s
/// Phase 2 `task`).
pub(crate) struct AlwaysNull;

impl JsonSchema for AlwaysNull {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("AlwaysNull")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        let mut object = JsonObject::new();
        object.insert("type".to_owned(), Value::String("null".to_owned()));
        Schema::from(object)
    }
}

// ------------------------------------------------------------------ shared

/// `left_out`: nodes or links of generated and Tier 3 files left out.
#[derive(JsonSchema)]
pub(crate) struct LeftOut {
    pub generated: usize,
    pub tier3: usize,
}

/// Where a link is written.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum LinkOrigin {
    Frontmatter,
    Inline,
}

/// What a link's target resolved to.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum LinkState {
    Resolved,
    Dangling,
    Skipped,
    Unchecked,
}

/// Which way a link is followed.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum Direction {
    Out,
    In,
}

// -------------------------------------------------------------------- show

/// `spec show --json`.
#[derive(JsonSchema)]
pub(crate) struct ShowDocument {
    /// The REF as given.
    #[schemars(rename = "ref")]
    pub reference: String,
    /// Why nothing was found (the result is then an error); `null` otherwise.
    pub reason: Option<String>,
    pub notes: Vec<String>,
    pub nodes: Vec<ShowNode>,
}

#[derive(JsonSchema)]
pub(crate) struct ShowNode {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub status: Option<String>,
    pub rev: Option<u32>,
    pub tokens_est: u32,
    pub archived: bool,
    pub utf8: bool,
    /// A cut node: only the sections whose heading line its `text` holds.
    pub sections: Vec<String>,
    /// `b3:` and the BLAKE3 of the node's whole span bytes, even when
    /// `text` is cut: the base hash a proposal names.
    pub span_hash: String,
    pub text: String,
    pub truncated: bool,
    pub omitted: Option<ShowOmitted>,
    /// `null` without `with: ["links"]`.
    pub links: Option<ShowLinks>,
}

#[derive(JsonSchema)]
pub(crate) struct ShowOmitted {
    pub lines: [usize; 2],
    /// The hidden sections the tail names, the first in source order.
    pub sections: Vec<String>,
    /// Hidden sections past them.
    pub sections_more: usize,
    /// The holders not shown the tail names, `path:line`, the first in
    /// print order.
    pub holders: Vec<String>,
    /// Holders not shown past them.
    pub holders_more: usize,
}

#[derive(JsonSchema)]
pub(crate) struct ShowLinks {
    pub outgoing: Vec<ShowLink>,
    pub incoming: Vec<ShowLink>,
    pub left_out: LeftOut,
    pub omitted: usize,
}

#[derive(JsonSchema)]
pub(crate) struct ShowLink {
    #[schemars(rename = "type")]
    pub link_type: String,
    pub origin: LinkOrigin,
    pub at: Option<String>,
    pub name: Option<String>,
    pub written: String,
    pub path: String,
    pub line: usize,
    pub state: LinkState,
    pub reason: Option<String>,
}

// ------------------------------------------------------------------ search

/// `spec search --json`.
#[derive(JsonSchema)]
pub(crate) struct SearchDocument {
    pub query: String,
    pub kinds: Vec<String>,
    pub limit: usize,
    pub archive: bool,
    pub hits: Vec<SearchHit>,
    pub truncated: bool,
    pub tier3_left_out: u32,
    pub notes: Vec<String>,
}

#[derive(JsonSchema)]
pub(crate) struct SearchHit {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    pub ord: usize,
    pub archived: bool,
    pub snippet: String,
}

// -------------------------------------------------------------------- tree

/// `spec tree --json`.
#[derive(JsonSchema)]
pub(crate) struct TreeDocument {
    /// The ROOT as given; `null` without one.
    #[schemars(rename = "ref")]
    pub root: Option<String>,
    /// Why ROOT names nothing (the result is then an error); `null` otherwise.
    pub reason: Option<String>,
    pub notes: Vec<String>,
    pub depth: Option<usize>,
    pub kinds: Vec<String>,
    pub archive: bool,
    pub left_out: LeftOut,
    pub truncated: bool,
    pub nodes: Vec<TreeLine>,
}

#[derive(JsonSchema)]
pub(crate) struct TreeLine {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    pub depth: usize,
    pub parent: Option<String>,
    pub mark: Option<TreeMark>,
    pub status: Option<String>,
    pub rev: Option<u32>,
    pub tokens_est: u32,
    pub archived: bool,
}

/// Why a line is a root although it declares a parent.
#[derive(JsonSchema)]
#[schemars(rename_all = "kebab-case")]
pub(crate) enum TreeMark {
    DanglingParent,
    ParentCycle,
}

// ------------------------------------------------------------------ bundle

/// `spec bundle --json`.
#[derive(JsonSchema)]
pub(crate) struct BundleDocument {
    pub refs: Vec<String>,
    /// Why a REF names nothing (the result is then an error); `null` otherwise.
    pub reason: Option<String>,
    pub notes: Vec<String>,
    /// Phase 2: always `null`.
    pub task: AlwaysNull,
    pub budget: Option<u32>,
    pub tokens: Option<u32>,
    pub chars: Option<usize>,
    pub bytes: Option<usize>,
    pub bundle_hash: Option<String>,
    pub body: Option<String>,
    pub layers: Option<BundleLayers>,
    pub tail: Option<Vec<BundleTail>>,
    pub more: Option<usize>,
}

/// Every layer, in print order, its items (`[]` when empty).
#[derive(JsonSchema)]
pub(crate) struct BundleLayers {
    pub targets: Vec<BundleItem>,
    pub open_questions: Vec<BundleItem>,
    pub ancestors: Vec<BundleItem>,
    pub criteria: Vec<BundleItem>,
    pub bindings: Vec<BundleItem>,
    pub decisions: Vec<BundleItem>,
    pub neighbours: Vec<BundleItem>,
    pub terms: Vec<BundleItem>,
    pub tests: Vec<BundleItem>,
}

#[derive(JsonSchema)]
pub(crate) struct BundleItem {
    pub name: String,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    pub form: ItemForm,
    pub status: Option<String>,
    pub via: Option<Vec<FollowedType>>,
    pub working_answer: Option<WorkingAnswer>,
    pub tokens_est: u32,
    pub archived: bool,
}

/// How an item is shown.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum ItemForm {
    Text,
    Outline,
    Header,
    Summary,
}

#[derive(JsonSchema)]
pub(crate) struct FollowedType {
    #[schemars(rename = "type")]
    pub link_type: String,
    pub direction: Direction,
}

#[derive(JsonSchema)]
pub(crate) struct WorkingAnswer {
    pub name: Option<String>,
    pub written: String,
    pub path: String,
    pub line: usize,
    pub state: LinkState,
}

#[derive(JsonSchema)]
pub(crate) struct BundleTail {
    pub name: String,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    pub tokens_est: u32,
    pub layer: LayerKey,
}

/// A layer's key.
#[derive(JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub(crate) enum LayerKey {
    Targets,
    OpenQuestions,
    Ancestors,
    Criteria,
    Bindings,
    Decisions,
    Neighbours,
    Terms,
    Tests,
}
