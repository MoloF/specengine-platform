//! The JSON Schemas of the read tools (task spec `mcp-read`, "Data") and of
//! the queue tools (`docs/canon/agent-intake.md` "Review document",
//! "Intake document", "Tools": the review and intake documents; the closed
//! objects of `report_discrepancy`'s input).
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

// ------------------------------------------------------------------- queue

/// The review document: `spec review --brief --json`, `spec propose update
/// --brief --json` (`docs/canon/agent-intake.md` "Review document").
#[derive(JsonSchema)]
pub(crate) struct ReviewDocument {
    pub id: Option<String>,
    pub project: Option<String>,
    /// Free: the queue's kinds.
    pub kind: Option<String>,
    pub status: Option<ProposalState>,
    pub target_id: Option<String>,
    pub target_path: Option<String>,
    pub worktree: Option<String>,
    pub branch: Option<String>,
    pub base_commit: Option<String>,
    pub base_hash: Option<String>,
    /// `null` in a brief answer.
    pub base_text: Option<String>,
    /// `null` in a brief answer.
    pub new_text: Option<String>,
    pub patch_hash: Option<String>,
    pub rationale: Option<String>,
    pub author: Option<ProposalAuthor>,
    pub diagnostics: Option<Vec<ProposalFinding>>,
    /// `null` in a brief answer.
    pub diff: Option<String>,
    pub preview: Option<Preview>,
    /// `null` in a brief answer.
    pub conflict: Option<String>,
    pub decided_by: Option<String>,
    pub decided_at: Option<String>,
    pub decision_note: Option<String>,
    pub applied_commit: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub target_ids: Vec<String>,
    pub severity: Option<IntakeSeverity>,
    pub gap_type: Option<GapType>,
    pub summary: Option<String>,
    pub working_answer: Option<String>,
    pub price_of_other: Option<String>,
    pub evidence: Vec<Evidence>,
    pub options: Vec<IntakeOption>,
    pub recommendation: Option<u64>,
    pub distinct_from: Vec<String>,
    pub linked: Option<String>,
    /// A question's or discrepancy's decision record, once approved.
    pub record_id: Option<String>,
    pub record_path: Option<String>,
    pub record_title: Option<String>,
    /// `null` in a brief answer.
    pub record_text: Option<String>,
    /// The owner's choice.
    pub choice: Option<Choice>,
    pub notes: Vec<String>,
}

/// The owner's choice of a decided question or discrepancy: one key.
#[derive(JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub(crate) enum Choice {
    /// A discrepancy's option, by index from 0.
    Option(u64),
    /// A question's working answer (`true`).
    WorkingAnswer(bool),
    /// Another answer to a question.
    Answer(String),
}

/// A proposal's state.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum ProposalState {
    Open,
    Approved,
    Applied,
    Rejected,
}

/// Who raised it.
#[derive(JsonSchema)]
pub(crate) struct ProposalAuthor {
    #[schemars(rename = "type")]
    pub author_type: AuthorType,
    pub role: Option<String>,
    pub model: Option<String>,
    pub run: Option<String>,
}

#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum AuthorType {
    Human,
    Agent,
}

/// One finding an update introduces.
#[derive(JsonSchema)]
pub(crate) struct ProposalFinding {
    pub code: String,
    pub severity: FindingSeverity,
    pub path: String,
    pub line: usize,
    pub subject: String,
    pub message: String,
}

#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum FindingSeverity {
    Error,
    Warning,
}

/// What approving an open or approved update now would do.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum Preview {
    Applies,
    Rebases,
    Conflicts,
    Unavailable,
}

/// How much an item matters; it only orders the queue.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum IntakeSeverity {
    High,
    Normal,
    Low,
}

/// How the observed state departs from the spec.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum GapType {
    Missing,
    Partial,
    Contradicts,
    Unrequested,
}

/// One piece of evidence, as the agent gave it (agent-written data).
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
pub(crate) struct Evidence {
    /// Where it was observed.
    pub file: String,
    /// The symbol's qualified path.
    pub qpath: Option<String>,
    /// `N` or `N-M`.
    pub lines: Option<String>,
    /// What is there.
    pub observed: String,
    /// What the spec says.
    pub documented: String,
}

/// One priced way to settle a discrepancy (agent-written data).
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
pub(crate) struct IntakeOption {
    pub label: String,
    /// What choosing it does.
    pub effect: String,
    /// What it costs.
    pub price: String,
}

/// A proposed patch, as `propose_change` takes it (input only).
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
pub(crate) struct ProposedPatch {
    /// One of `node_ids`.
    pub target: String,
    /// get_node's `span_hash` of the target.
    pub base: String,
    /// The new text, inline.
    pub text: String,
    /// Why.
    pub rationale: String,
}

/// The intake document: `spec propose question --json`, `spec propose
/// discrepancy --json`.
#[derive(JsonSchema)]
pub(crate) struct IntakeDocument {
    /// The stored item's proposal ID; `null` when nothing was stored.
    pub id: Option<String>,
    pub created: bool,
    pub hits: Vec<IntakeMatch>,
    pub related: Vec<IntakeMatch>,
    /// The linked update of a proposed patch.
    pub linked: Option<String>,
    pub diagnostics: Vec<ProposalFinding>,
    pub notes: Vec<String>,
}

/// A hit or related item.
#[derive(JsonSchema)]
pub(crate) struct IntakeMatch {
    /// A decision's `id:` or a proposal's ID.
    pub id: Option<String>,
    pub source: MatchSource,
    /// `accepted`, or the proposal's state.
    pub status: String,
    /// A decision's path, an applied proposal's record path.
    pub path: Option<String>,
    /// A hit's answer: the decision's title, a rejected proposal's reason,
    /// an applied one's record title.
    pub answer: Option<String>,
    /// A hit's decision record: a decision's id, an applied proposal's
    /// record_id.
    pub record: Option<String>,
}

#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum MatchSource {
    Corpus,
    Queue,
}
