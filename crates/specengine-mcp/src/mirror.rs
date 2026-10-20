//! The JSON Schemas of the read tools (task spec `mcp-read`, "Data"), of
//! the queue tools (`docs/canon/agent-intake.md` "Review document",
//! "Intake document", "Tools": the review and intake documents; the closed
//! objects of `report_discrepancy`'s input) and of the task tools
//! (`docs/canon/task-package.md` "Package", "MCP": the package, the task
//! commands' document), of `get_roadmap` (`docs/canon/spec-roadmap.md`
//! "Command": `spec roadmap`'s document) and of the code tools
//! (`docs/canon/mcp-code.md` "Mirrors": `spec symbols`, `spec refs`,
//! `spec unmapped`; a symbol target's `roles`, each key a free string or
//! integer, never an enum of the code layer's words; `spec verify --node`,
//! `check_binding`'s: `docs/features/check-binding.md` "Data").
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
/// Phase 2 `task`; `spec verify --node`'s `base`, `task`, `delta.symbols`,
/// `delta.tests_reaching`).
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
    /// `b3:` and the BLAKE3 of the node's meaning (its own text normalised
    /// and, for a document, its declared links; `specengine-norm/v1`);
    /// `null` when it cannot be computed. Never a proposal's base.
    pub norm_hash: Option<String>,
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
    /// Per node whose meaning the change alters (task spec
    /// `rev-discipline`); `null` when the kind writes no existing node or
    /// the preview conflicts or is unavailable.
    pub revisions: Option<Vec<RevisionShape>>,
    /// A drift item's evidence as stored (task spec `drift-queue`); `null`
    /// on any other kind.
    pub drift: Option<DriftShape>,
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
    /// The task it was raised for; `null` unbound.
    pub task_id: Option<String>,
    /// A choice staged for the owner to confirm on a terminal; any local
    /// process can stage one, so it is no answer.
    pub staged: Option<StagedShape>,
    /// When it was staged.
    pub staged_at: Option<String>,
    /// An interpretation's decision, how and why; `null` on any other kind.
    pub interpretation: Option<InterpretationShape>,
    pub notes: Vec<String>,
}

/// An interpretation (task spec `kind-interpretation`, "Data"): the
/// accepted decision it interprets, how it was applied and why
/// interpretation was needed.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
pub(crate) struct InterpretationShape {
    /// The decision's ID.
    pub decision: String,
    pub how: String,
    pub why: String,
}

/// A node whose meaning a proposal changes: its revision read now, what a
/// meaning approve raises it to (`null` when refused), the `spec.lock`
/// entries an editorial one refreshes (`null` when the lock cannot be
/// read) and why a meaning approve refuses the raise (`null` when it
/// raises; the reason names `--editorial`).
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
pub(crate) struct RevisionShape {
    pub id: String,
    pub rev: u32,
    pub to: Option<u32>,
    pub lock_entries: Option<u64>,
    pub refused: Option<String>,
}

/// A drift item's evidence (task spec `drift-queue`, "The item"): its
/// `drift_hash`, its first rows as `spec verify --json` writes them
/// (untyped until Phase 4's drift screen) and how many more.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
pub(crate) struct DriftShape {
    pub hash: String,
    pub rows: Vec<JsonObject>,
    pub more: u64,
}

/// A staged choice (`docs/canon/decision-staging.md` "The stage"): one of
/// two closed objects.
#[derive(JsonSchema)]
#[schemars(untagged)]
pub(crate) enum StagedShape {
    Approve(StagedApprove),
    Reject(StagedReject),
}

/// `spec approve PR` with these flags, staged.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
pub(crate) struct StagedApprove {
    pub decision: ApproveWord,
    /// A discrepancy's option, from 0.
    pub option: Option<u64>,
    /// A question's answer other than its working answer.
    pub answer: Option<String>,
    /// The section its record governs.
    pub canon: Option<String>,
    /// `true`: approved as editorial, revisions kept; else `null`.
    pub editorial: Option<bool>,
    pub note: Option<String>,
    /// The target's span hash when staged (an update's or a section-form
    /// create's).
    pub span_hash: Option<String>,
}

/// `spec reject PR --reason T`, staged.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
pub(crate) struct StagedReject {
    pub decision: RejectWord,
    pub reason: String,
}

/// An approve's word.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum ApproveWord {
    Approve,
}

/// A reject's word.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum RejectWord {
    Reject,
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

/// The drift document: `spec propose drift --json` (task spec
/// `drift-queue`, "Output").
#[derive(JsonSchema)]
pub(crate) struct DriftDocument {
    /// The commit judged; `null` when refused before the place was read.
    pub commit: Option<String>,
    pub branch: Option<String>,
    /// `null` when refused before judging.
    pub judged: Option<DriftJudged>,
    pub created: Vec<DriftCreated>,
    pub hits: Vec<DriftHit>,
    pub notes: Vec<String>,
}

/// The rows (of the nodes judged, `ok` ones too) and the nodes judged.
#[derive(JsonSchema)]
pub(crate) struct DriftJudged {
    pub rows: u64,
    pub nodes: u64,
}

/// An item stored.
#[derive(JsonSchema)]
pub(crate) struct DriftCreated {
    pub id: String,
    pub node: String,
    pub severity: IntakeSeverity,
    /// Its rows' distinct states and categories.
    pub summary: String,
}

/// An item found instead of a new one: queued (`open`, `approved`) or
/// settled (`rejected`, the same evidence).
#[derive(JsonSchema)]
pub(crate) struct DriftHit {
    pub id: String,
    pub node: String,
    pub status: ProposalState,
    /// Its evidence equals the evidence judged now.
    pub same: bool,
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

// ------------------------------------------------------------------- tasks

/// The task package: `spec task show --json`
/// (`docs/canon/task-package.md` "Package"); the model's `TaskPackage`,
/// key for key.
#[derive(JsonSchema)]
pub(crate) struct TaskPackage {
    /// Raised when a key is removed, renamed or re-meant.
    pub schema_version: u32,
    pub id: String,
    pub project: String,
    pub status: TaskState,
    pub title: Option<String>,
    pub goal: Option<String>,
    /// `[project] profile`, verbatim.
    pub profile: Option<String>,
    /// A snapshot node changed or gone in the compared place; `null` when
    /// it cannot be told (a note says why).
    pub stale: Option<bool>,
    pub targets: Vec<PackageTarget>,
    pub criteria: Vec<TaskCriterion>,
    pub affected_nodes: Vec<String>,
    pub plan: Option<String>,
    pub assumptions: Vec<PackageAssumption>,
    pub open_proposals: Vec<PackageProposal>,
    pub owner_notes: Vec<OwnerNote>,
    /// `[]` until code bindings exist.
    pub bindings: Vec<Binding>,
    pub spec_snapshot: Option<SpecSnapshot>,
    /// `[]` when not stale, `null` when staleness cannot be told.
    pub snapshot_diff: Option<Vec<SnapshotDiff>>,
    pub claim: Option<TaskClaim>,
    pub runs: Vec<TaskRun>,
    pub bundle: PackageBundle,
    /// The task's place on the plan; `null` without `[roadmap]`, with one
    /// that cannot be read (a note says why), or when the task executes no
    /// open item.
    pub roadmap: Option<TaskRoadmap>,
    pub author: ProposalAuthor,
    pub created_at: String,
    pub updated_at: String,
    pub notes: Vec<String>,
}

/// The open item a task executes, as get_roadmap lists the task on it:
/// read at the reading root, per read.
#[derive(JsonSchema)]
pub(crate) struct TaskRoadmap {
    /// The item's ID.
    pub item: String,
    pub group: RoadmapGroupKind,
    /// The item's `release` as written; `null` when absent or no string.
    pub release: Option<String>,
    /// 1-based within its release; `null` outside one.
    pub position: Option<u64>,
    /// The open items of its group.
    pub of: u64,
    /// The item's `priority` as written.
    pub priority: Option<String>,
    /// The open items it depends on, in written order.
    pub waits_on: Vec<String>,
}

/// A task's state.
#[derive(JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub(crate) enum TaskState {
    Draft,
    Analysis,
    Review,
    ChangesRequested,
    Ready,
    InProgress,
    InReview,
    Done,
    Accepted,
    Cancelled,
}

/// How a run ended, as reported.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum RunOutcome {
    Completed,
    Partial,
    Failed,
    Abandoned,
}

#[derive(JsonSchema)]
pub(crate) struct PackageTarget {
    pub id: Option<String>,
    pub path: Option<String>,
    /// Free: the project's kinds.
    pub kind: Option<String>,
    pub title: Option<String>,
}

#[derive(JsonSchema)]
pub(crate) struct TaskCriterion {
    #[schemars(rename = "ref")]
    pub reference: Option<String>,
    pub text: Option<String>,
}

#[derive(JsonSchema)]
pub(crate) struct PackageAssumption {
    pub proposal: String,
    pub text: String,
}

#[derive(JsonSchema)]
pub(crate) struct PackageProposal {
    pub id: String,
    /// Free: the queue's kinds.
    pub kind: String,
    pub status: ProposalState,
    pub target_ids: Vec<String>,
    pub task_id: Option<String>,
    pub summary: Option<String>,
}

#[derive(JsonSchema)]
pub(crate) struct OwnerNote {
    pub at: String,
    pub note: String,
}

/// A code binding of a node: none until they exist.
pub(crate) struct Binding;

impl JsonSchema for Binding {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Binding")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        let mut object = JsonObject::new();
        object.insert("type".to_owned(), Value::String("object".to_owned()));
        Schema::from(object)
    }
}

#[derive(JsonSchema)]
pub(crate) struct SpecSnapshot {
    pub at: String,
    pub place: SnapshotPlace,
    pub nodes: Vec<SnapshotNode>,
}

#[derive(JsonSchema)]
pub(crate) struct SnapshotPlace {
    pub worktree: String,
    pub root_rel: String,
    pub branch: String,
    pub commit: String,
}

#[derive(JsonSchema)]
pub(crate) struct SnapshotNode {
    pub id: String,
    pub path: String,
    pub span_hash: String,
}

#[derive(JsonSchema)]
pub(crate) struct SnapshotDiff {
    pub id: String,
    pub path: String,
    pub span_hash: String,
    /// `null` past the package's total.
    pub diff: Option<String>,
    pub cut: bool,
}

#[derive(JsonSchema)]
pub(crate) struct TaskClaim {
    pub at: String,
    pub role: String,
    pub worktree: String,
    pub branch: String,
}

#[derive(JsonSchema)]
pub(crate) struct TaskRun {
    pub run: u64,
    pub role: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub outcome: Option<RunOutcome>,
    pub summary: Option<String>,
    pub changed_files: Vec<String>,
}

#[derive(JsonSchema)]
pub(crate) struct PackageBundle {
    pub node_ids: Vec<String>,
    pub budget: u32,
    pub bundle_hash: Option<String>,
}

/// `spec task claim|plan|report|complete --json`.
#[derive(JsonSchema)]
pub(crate) struct TaskDocument {
    pub id: Option<String>,
    pub status: Option<TaskState>,
    /// The run the command opened or closed.
    pub run: Option<u64>,
    /// A refusal's reason last.
    pub notes: Vec<String>,
}

// ----------------------------------------------------------------- roadmap
//
// `spec roadmap --json` (`docs/canon/spec-roadmap.md` "Command"): the
// project's releases, priorities, units and statuses are free strings, as
// its `[roadmap]` and its items write them.

/// `spec roadmap --json`: the whole plan's keys, then what is shown.
#[derive(JsonSchema)]
pub(crate) struct RoadmapDocument {
    /// `release` as given; `null` without one.
    pub release: Option<String>,
    /// `pool: true`: the pool and the unplaced items only.
    pub pool: bool,
    /// Why `release` lists nothing (the result is then an error); `null`
    /// otherwise.
    pub reason: Option<String>,
    /// `[roadmap] prefix`.
    pub prefix: String,
    /// `[roadmap] dir`.
    pub dir: String,
    /// `[roadmap] unit`, verbatim.
    pub unit: Option<String>,
    /// The first release holding an open item.
    pub current: Option<String>,
    pub counts: RoadmapCounts,
    /// The releases from the first through `[roadmap] mvp_cut`; `null`
    /// without it.
    pub cut: Option<RoadmapCut>,
    /// Every release.
    pub total: RoadmapSum,
    /// The groups whose header is shown, in order.
    pub groups: Vec<RoadmapGroup>,
    /// This repository's open tasks executing no open item, as shown.
    pub pool_tasks: Vec<RoadmapPoolTask>,
    /// The plan's notes.
    pub notes: Vec<String>,
    pub truncated: bool,
    /// The listed items and pool tasks not shown.
    pub omitted: usize,
}

#[derive(JsonSchema)]
pub(crate) struct RoadmapCounts {
    pub open: usize,
    pub done: usize,
}

#[derive(JsonSchema)]
pub(crate) struct RoadmapCut {
    pub release: String,
    pub min: u64,
    pub max: u64,
    pub unestimated: u64,
}

/// The estimated bounds added; `unestimated` the open items without one.
#[derive(JsonSchema)]
pub(crate) struct RoadmapSum {
    pub min: u64,
    pub max: u64,
    pub unestimated: u64,
}

#[derive(JsonSchema)]
pub(crate) struct RoadmapGroup {
    pub group: RoadmapGroupKind,
    /// A release's name; `null` for the pool and the unplaced items.
    pub name: Option<String>,
    pub current: bool,
    pub mvp_cut: bool,
    pub open: usize,
    pub done: usize,
    /// Over its open items.
    pub sum: RoadmapSum,
    /// Its open items shown, in order.
    pub items: Vec<RoadmapItem>,
}

/// A group of the plan.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum RoadmapGroupKind {
    Release,
    Pool,
    Unplaced,
}

#[derive(JsonSchema)]
pub(crate) struct RoadmapItem {
    /// 1-based within its release; `null` outside one.
    pub position: Option<usize>,
    pub id: String,
    pub title: Option<String>,
    pub path: String,
    /// `status:` as written.
    pub status: Option<String>,
    /// `release:` as written; `null` when absent or no string.
    pub release: Option<String>,
    /// `priority:` as written.
    pub priority: Option<String>,
    /// `null`: unestimated.
    pub estimate: Option<RoadmapEstimate>,
    /// The open items it depends on, in written order.
    pub waits_on: Vec<String>,
    /// A member of a `depends_on` cycle of its release.
    pub cycle: bool,
    /// This repository's open tasks executing it, by number.
    pub tasks: Vec<RoadmapItemTask>,
}

#[derive(JsonSchema)]
pub(crate) struct RoadmapEstimate {
    pub min: u64,
    pub max: u64,
}

#[derive(JsonSchema)]
pub(crate) struct RoadmapItemTask {
    pub id: String,
    pub status: TaskState,
}

#[derive(JsonSchema)]
pub(crate) struct RoadmapPoolTask {
    pub id: String,
    pub status: TaskState,
    pub title: Option<String>,
    /// As stored: IDs, or an id-less document's path.
    pub targets: Vec<String>,
}

// -------------------------------------------------------------------- code
//
// The code reads' documents (`docs/canon/spec-cli-refs.md` "JSON",
// `docs/canon/symbol-reads.md` "spec symbols", "spec unmapped"): the CLI's
// closed words are enums; what the code or the project names (an item's
// kind, a layer, a category, a level, a node's kind) is a free string.

/// The code files a read covered.
#[derive(JsonSchema)]
pub(crate) struct CodeCount {
    /// Code files read and analysed.
    pub files: usize,
    pub rs: usize,
    pub ron: usize,
    /// Files and names skipped for not being UTF-8.
    pub skipped: usize,
}

/// How a marker relates its code to its node.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum MarkerRelation {
    Implements,
    Verifies,
    Configures,
    Assumes,
}

/// How a written ID fared.
#[derive(JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub(crate) enum IdState {
    Resolved,
    Alias,
    Ambiguous,
    Homoglyph,
    OtherProject,
    Queue,
    Dangling,
}

/// A marker's written revision against its node's.
#[derive(JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub(crate) enum RevState {
    Unversioned,
    BadRev,
    RevUnknown,
    Current,
    SpecAhead,
    Predated,
}

/// A marker's level list: the default, declared, or why it is invalid.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum LevelsState {
    Default,
    Declared,
    Unclosed,
    Unknown,
    Duplicate,
    Empty,
}

/// Why a name is shown, not resolved.
#[derive(JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub(crate) enum CodeAmbiguity {
    PathAttribute,
    Unrooted,
    Duplicate,
}

/// What a marker binds, or where a mention belongs.
#[derive(JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub(crate) enum BindingKind {
    Item,
    Module,
    Ron,
    Unanchored,
    CannotVerify,
    File,
}

/// A marker's or a mention's binding, every key present.
#[derive(JsonSchema)]
pub(crate) struct CodeBinding {
    pub kind: BindingKind,
    /// An item's or a module's qpath; a data anchor as `<path>#<field path>`.
    pub qpath: Option<String>,
    /// The item's kind as the code layer names it.
    pub item_kind: Option<String>,
    /// An item's first line.
    pub line: Option<usize>,
    pub ambiguity: Option<CodeAmbiguity>,
    /// Why nothing is bound, or why it cannot be verified.
    pub category: Option<String>,
}

/// `spec refs --json`.
#[derive(JsonSchema)]
pub(crate) struct RefsDocument {
    /// The REF as given.
    #[schemars(rename = "ref")]
    pub reference: String,
    /// Why the REF names nothing (the result is then an error); `null`
    /// otherwise.
    pub reason: Option<String>,
    pub notes: Vec<String>,
    pub nodes: Vec<RefsNode>,
    /// `null` when the REF names nothing.
    pub code: Option<CodeCount>,
    /// `null` when the REF names nothing.
    pub omitted: Option<RefsOmitted>,
}

#[derive(JsonSchema)]
pub(crate) struct RefsNode {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    /// Written, else 1; `null` when written but unreadable.
    pub rev: Option<u32>,
    pub rev_written: bool,
    pub markers: Vec<RefsMarker>,
    pub mentions: Vec<RefsMention>,
}

#[derive(JsonSchema)]
pub(crate) struct RefsMarker {
    pub relation: MarkerRelation,
    pub path: String,
    pub line: usize,
    pub column: usize,
    /// The ID and the revision as written.
    pub token: String,
    pub id: String,
    pub rev: Option<u32>,
    pub id_state: IdState,
    /// The Latin form of a look-alike ID.
    pub fix: Option<String>,
    pub rev_state: RevState,
    /// `null` when the list is invalid.
    pub levels: Option<Vec<String>>,
    pub levels_state: LevelsState,
    pub note: Option<String>,
    pub binding: CodeBinding,
}

#[derive(JsonSchema)]
pub(crate) struct RefsMention {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub written: String,
    pub id_state: IdState,
    pub fix: Option<String>,
    pub binding: CodeBinding,
}

/// The rows the cap left out.
#[derive(JsonSchema)]
pub(crate) struct RefsOmitted {
    pub markers: usize,
    pub mentions: usize,
}

/// `spec symbols`' one form.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum CodeForm {
    Query,
    Node,
    File,
    At,
}

/// What a target is.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum CodeTarget {
    Item,
    Module,
    Ron,
}

/// How a target meets the query, the best first.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum CodeTier {
    Exact,
    Suffix,
    Name,
}

/// `spec symbols --json`.
#[derive(JsonSchema)]
pub(crate) struct SymbolsDocument {
    pub form: CodeForm,
    /// The query as given; `null` in the other forms.
    pub query: Option<String>,
    /// The node's REF as given; `null` in the other forms.
    pub node: Option<String>,
    /// The file's path (file, at); `null` in the other forms.
    pub path: Option<String>,
    /// The line (at); `null` in the other forms.
    pub line: Option<usize>,
    /// Why the node's REF names nothing (the result is then an error);
    /// `null` otherwise.
    pub reason: Option<String>,
    pub notes: Vec<String>,
    pub symbols: Vec<SymbolsTarget>,
    /// Every target the form names, the limit's cut included.
    pub total: usize,
    /// `null` when the REF names nothing.
    pub omitted: Option<SymbolsOmitted>,
    /// `null` when the REF names nothing.
    pub code: Option<CodeCount>,
}

#[derive(JsonSchema)]
pub(crate) struct SymbolsTarget {
    pub target: CodeTarget,
    /// An item's or a module's qpath; a data path as `<path>#<field path>`.
    pub qpath: String,
    pub name: Option<String>,
    pub discriminator: Option<String>,
    /// The item's kind as the code layer names it.
    pub item_kind: Option<String>,
    pub path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub signature: Option<String>,
    pub ambiguity: Option<CodeAmbiguity>,
    /// The layer whose rules made the row.
    pub layer: Option<String>,
    /// The query form only.
    pub tier: Option<CodeTier>,
    /// The mentions bound to the target itself.
    pub mentions: Option<usize>,
    pub bindings: Vec<SymbolsBinding>,
    /// The role rows shown on an item, by layer, word, path, line; an
    /// empty list on a module or a data path; `null` when the project's
    /// code table lists no frameworks.
    pub roles: Option<Vec<SymbolsRole>>,
}

/// One role row of a target, every key present; each value a free string
/// or integer, as the code layer gives it.
#[derive(JsonSchema)]
pub(crate) struct SymbolsRole {
    /// The row's word.
    pub role: String,
    /// The layer whose rules made the row.
    pub layer: String,
    /// The row's own file.
    pub path: String,
    /// The row's own line.
    pub line: usize,
    /// Where the row was read.
    pub source: String,
    /// How its subject is written; `null` when the row has none.
    pub form: Option<String>,
    /// What text the row was read from.
    pub origin: String,
    /// The group stored with the row; `null` when it has none.
    pub group: Option<String>,
    /// Linked, ambiguous, or the row's certainty.
    pub state: String,
    /// Why the row is uncertain; `null` otherwise.
    pub detail: Option<String>,
    /// How many targets an ambiguous row names; `null` otherwise.
    pub candidates: Option<usize>,
}

#[derive(JsonSchema)]
pub(crate) struct SymbolsBinding {
    pub relation: MarkerRelation,
    pub line: usize,
    pub column: usize,
    pub token: String,
    pub id: String,
    pub rev: Option<u32>,
    pub id_state: IdState,
    pub fix: Option<String>,
    pub rev_state: RevState,
    pub levels: Option<Vec<String>>,
    pub levels_state: LevelsState,
    pub note: Option<String>,
    /// The IDs of the nodes the marker is listed under, `, `-joined.
    pub node: Option<String>,
}

/// The targets the cap left out.
#[derive(JsonSchema)]
pub(crate) struct SymbolsOmitted {
    pub symbols: usize,
}

/// `spec unmapped --json`.
#[derive(JsonSchema)]
pub(crate) struct UnmappedDocument {
    /// The path as given, a trailing `/` dropped; `null` for every root.
    pub path: Option<String>,
    pub rows: Vec<UnmappedRow>,
    /// The sums over the path (every root without one); a file: its row.
    pub total: UnmappedRow,
    /// A file's unmapped items, in source order.
    pub unmapped: Vec<UnmappedItem>,
    pub notes: Vec<String>,
    pub omitted: UnmappedOmitted,
    pub code: CodeCount,
}

#[derive(JsonSchema)]
pub(crate) struct UnmappedRow {
    /// No trailing `/`; `null` for the total of every root.
    pub path: Option<String>,
    pub dir: bool,
    pub units: usize,
    pub mapped: usize,
    /// The mapped units in percent, one decimal, half up; `null` without a
    /// unit.
    pub percent: Option<f64>,
    pub mentioned: usize,
}

#[derive(JsonSchema)]
pub(crate) struct UnmappedItem {
    pub qpath: String,
    pub item_kind: String,
    pub path: String,
    /// Its first line.
    pub line: usize,
}

/// The rows and items the cap left out.
#[derive(JsonSchema)]
pub(crate) struct UnmappedOmitted {
    pub rows: usize,
    pub unmapped: usize,
}

// ------------------------------------------------------------------ verify
//
// `spec verify --node REF [--symbol QPATH] --json`, `check_binding`'s
// document (`docs/canon/spec-verify-output.md`, `docs/canon/spec-verify.md`
// "States", `docs/canon/spec-verify-anchors.md`; the CLI's `VerifyJson`,
// key for key): the CLI's closed words are enums; an item's kind, a
// category, a detail and a level are free strings. Under `--node` the
// base, the task, `delta.symbols` and `delta.tests_reaching` are always
// `null`.

/// `spec verify --node --json`.
#[derive(JsonSchema)]
pub(crate) struct VerifyDocument {
    /// The mode the verdict used; `null` when the config's mode cannot be
    /// read.
    pub mode: Option<VerifyMode>,
    /// No base: the working tree alone.
    pub base: AlwaysNull,
    /// No task.
    pub task: AlwaysNull,
    /// The IDs (a holder without one: its citation) of the nodes the REF names, in holder order.
    pub node: Vec<String>,
    /// The symbol the rows are kept to; `null` without one.
    pub symbol: Option<String>,
    pub verdict: VerifyVerdict,
    pub counts: VerifyCounts,
    pub cascade: VerifyCascade,
    /// The rows printed, `ok` ones too, in key order.
    pub rows: Vec<VerifyRowShape>,
    pub delta: VerifyDelta,
    /// The run causes, in order.
    pub cannot: Vec<String>,
    pub notes: Vec<String>,
    pub omitted: VerifyOmitted,
}

/// A verify mode.
#[derive(JsonSchema)]
#[schemars(rename_all = "kebab-case")]
pub(crate) enum VerifyMode {
    Observe,
    EnforceIntroduced,
    Enforce,
}

/// What a run says.
#[derive(JsonSchema)]
#[schemars(rename_all = "kebab-case")]
pub(crate) enum VerifyVerdict {
    Clean,
    Observed,
    Blocked,
    CannotVerify,
}

/// Every count over the rows kept, the printed and the cut ones.
#[derive(JsonSchema)]
pub(crate) struct VerifyCounts {
    pub rows: usize,
    pub ok: usize,
    pub fix: usize,
    pub check: usize,
    pub cannot: usize,
    pub pre_existing: usize,
    pub skipped: usize,
}

/// How the rows with a lock entry were compared.
#[derive(JsonSchema)]
pub(crate) struct VerifyCascade {
    pub by_blob: usize,
    pub by_ast: usize,
}

/// One row: a lock key (a keyless one: `kind`, `symbol`, `item_kind`
/// `null`), its state, class and facts.
#[derive(JsonSchema)]
pub(crate) struct VerifyRowShape {
    pub node: String,
    pub relation: MarkerRelation,
    pub file: String,
    pub kind: Option<VerifyKind>,
    pub symbol: Option<String>,
    /// The item's kind as the code layer names it.
    pub item_kind: Option<String>,
    pub state: VerifyState,
    pub category: Option<String>,
    pub detail: Option<String>,
    /// `null` for a row that holds.
    pub class: Option<VerifyClass>,
    pub pre_existing: bool,
    pub markers: Vec<VerifyMarker>,
    /// The node's revision; `null` unknown or no node.
    pub node_rev: Option<u32>,
    pub lock: Option<VerifyLock>,
    /// `null` without a lock entry, decided earlier, or nothing compared.
    pub compared: Option<VerifyCompared>,
    pub levels: Option<VerifyLevels>,
    /// `null` without a lock entry, or when the row cannot be verified.
    pub anchor: Option<VerifyAnchor>,
}

/// What a lock entry binds.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum VerifyKind {
    Item,
    Module,
    Ron,
}

/// A row's state.
#[derive(JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub(crate) enum VerifyState {
    CannotVerify,
    Predated,
    BadMarker,
    BrokenBinding,
    Unlocked,
    SpecAhead,
    CodeAhead,
    Conflict,
    Ok,
}

/// A finding's class.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum VerifyClass {
    Fix,
    Check,
    Cannot,
}

/// How a row's code was compared with its lock entry.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum VerifyCompared {
    Blob,
    Ast,
}

/// One marker of a row: its line and written revision (`null`:
/// unversioned or malformed).
#[derive(JsonSchema)]
pub(crate) struct VerifyMarker {
    pub line: usize,
    pub rev: Option<u32>,
}

/// A row's lock entry.
#[derive(JsonSchema)]
pub(crate) struct VerifyLock {
    pub rev: u32,
    pub commit: String,
    pub verified_at: String,
}

/// A row's levels: the entry's, its markers' united, and those whose
/// digest changed (`null` when none was compared).
#[derive(JsonSchema)]
pub(crate) struct VerifyLevels {
    pub lock: Vec<String>,
    pub markers: Vec<String>,
    pub changed: Option<Vec<String>>,
}

/// Where a row's binding is: a moved row's old place, an ambiguous row's
/// candidates.
#[derive(JsonSchema)]
pub(crate) struct VerifyAnchor {
    pub state: VerifyAnchorState,
    pub from: Option<VerifyPlace>,
    pub candidates: Option<Vec<VerifyPlace>>,
}

/// How a row's binding was found.
#[derive(JsonSchema)]
#[schemars(rename_all = "lowercase")]
pub(crate) enum VerifyAnchorState {
    Exact,
    Moved,
    Ambiguous,
    Lost,
}

/// A binding's place: its key without the node and relation.
#[derive(JsonSchema)]
pub(crate) struct VerifyPlace {
    pub file: String,
    pub kind: VerifyKind,
    pub symbol: String,
    /// The item's kind as the code layer names it.
    pub item_kind: Option<String>,
}

/// The bindings that moved, are ambiguous, lost or stale, and the nodes
/// with a finding, over the rows not pre-existing.
#[derive(JsonSchema)]
pub(crate) struct VerifyDelta {
    pub symbols: AlwaysNull,
    pub bindings: VerifyBindings,
    pub affected_nodes: Vec<String>,
    pub tests_reaching: AlwaysNull,
}

#[derive(JsonSchema)]
pub(crate) struct VerifyBindings {
    pub moved: Vec<VerifyMoved>,
    pub ambiguous: Vec<VerifyKey>,
    pub lost: Vec<VerifyKey>,
    pub stale: Vec<VerifyKey>,
}

/// A moved binding: its lock entry's key and its own.
#[derive(JsonSchema)]
pub(crate) struct VerifyMoved {
    pub from: VerifyKey,
    pub to: VerifyKey,
}

/// A row's key.
#[derive(JsonSchema)]
pub(crate) struct VerifyKey {
    pub node: String,
    pub relation: MarkerRelation,
    pub file: String,
    pub kind: Option<VerifyKind>,
    pub symbol: Option<String>,
    pub item_kind: Option<String>,
}

/// The rows the cut left out.
#[derive(JsonSchema)]
pub(crate) struct VerifyOmitted {
    pub rows: usize,
}
