//! The pure half of the agent intake (`docs/canon/agent-intake.md` "Tools",
//! "Stored", "Dedup", "Rules" 2): the three queue kinds that never apply
//! (a `decision`: task spec `kind-decision`), their closed enums, the caps
//! every caller checks (UTF-8 bytes), the field checks in input order, and
//! the normalised text the queue's dedup compares; beside them the names of
//! the three kinds that apply, and an interpretation's fields
//! ([`Interpretation`]). Nothing is
//! read, written or resolved here, and no project's word appears
//! (ADR-0008): the kinds and enums are the engine's.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::proposal::{TEXT_MAX_BYTES, author_field_problem};

/// The kind of a proposal replacing one node's span.
pub const UPDATE_KIND: &str = "update";

/// The kind of a proposal adding a node: a new spec file, or new `{#ID}`
/// sections inside one node's span (task spec `proposal-kinds`).
pub const CREATE_KIND: &str = "create";

/// The kind of an `update` carrying an accepted decision into a rule: the
/// decision it interprets, how it was applied and why interpretation was
/// needed (task spec `kind-interpretation`); applied as an update.
pub const INTERPRETATION_KIND: &str = "interpretation";

/// The kind of a question raised to the owner: a queue record only, never
/// applied (owner's Q2).
pub const QUESTION_KIND: &str = "question";

/// The kind of a reported discrepancy between the spec and what was
/// observed: a queue record only, never applied.
pub const DISCREPANCY_KIND: &str = "discrepancy";

/// The kind of a decision taken during work, by an agent or the owner: a
/// question's fields, its decision the working answer, recorded as a
/// question's when the owner confirms it; never applied (task spec
/// `kind-decision`).
pub const DECISION_KIND: &str = "decision";

/// The most `node_ids` of one item.
pub const NODE_IDS_MAX: usize = 16;
/// The most bytes of a question's or a decision's `text` and a
/// discrepancy's `summary`.
pub const SUMMARY_MAX: usize = 1024;
/// The most bytes of `working_answer` and `price_of_other`.
pub const ANSWER_MAX: usize = 2048;
/// The most `evidence` items.
pub const EVIDENCE_MAX: usize = 8;
/// The most bytes of an evidence item's `file` and `qpath`.
pub const LOCATION_MAX: usize = 512;
/// The most bytes of an evidence item's `observed` and `documented`.
pub const EVIDENCE_TEXT_MAX: usize = 1024;
/// The fewest `options` of a discrepancy.
pub const OPTIONS_MIN: usize = 2;
/// The most `options` of a discrepancy.
pub const OPTIONS_MAX: usize = 6;
/// The most bytes of an option's `label`.
pub const LABEL_MAX: usize = 128;
/// The most bytes of an option's `effect` and `price`.
pub const OPTION_TEXT_MAX: usize = 512;
/// The most `distinct_from` entries: an item with more hits than this can
/// never be stored (a known limit).
pub const DISTINCT_MAX: usize = 64;
/// The most bytes of one `distinct_from` entry.
pub const DISTINCT_ITEM_MAX: usize = 256;
/// The most bytes of a rationale (`propose update` and a proposed patch).
pub const RATIONALE_MAX: usize = 4096;
/// The most bytes of an interpretation's `how` and `why`.
pub const INTERPRETATION_TEXT_MAX: usize = 2048;
/// An evidence item's `lines` are below this line number.
pub const LINE_LIMIT: u64 = 1_000_000_000;

/// How much an item matters to the owner; it only orders the queue and
/// never blocks anything (ADR-0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntakeSeverity {
    High,
    Normal,
    Low,
}

impl IntakeSeverity {
    pub const ALL: [Self; 3] = [Self::High, Self::Normal, Self::Low];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Normal => "normal",
            Self::Low => "low",
        }
    }

    /// The severity named exactly `text`.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.as_str() == text)
    }
}

/// How the observed state departs from the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GapType {
    Missing,
    Partial,
    Contradicts,
    Unrequested,
}

impl GapType {
    pub const ALL: [Self; 4] = [
        Self::Missing,
        Self::Partial,
        Self::Contradicts,
        Self::Unrequested,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Partial => "partial",
            Self::Contradicts => "contradicts",
            Self::Unrequested => "unrequested",
        }
    }

    /// The gap type named exactly `text`.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.as_str() == text)
    }
}

/// One piece of a discrepancy's evidence, stored verbatim (unverified):
/// every key written, an absent one `null`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// Where it was observed, as the agent names it.
    pub file: String,
    /// The symbol's qualified path, when there is one.
    #[serde(default)]
    pub qpath: Option<String>,
    /// `N` or `N-M`.
    #[serde(default)]
    pub lines: Option<String>,
    pub observed: String,
    pub documented: String,
}

/// One priced way to settle a discrepancy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeOption {
    pub label: String,
    pub effect: String,
    pub price: String,
}

/// A discrepancy's proposed text: stored as a linked `update`, decided on
/// its own.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedPatch {
    /// An ID among the item's `node_ids`.
    pub target: String,
    /// The span hash the text was written against.
    pub base: String,
    pub text: String,
    pub rationale: String,
}

/// The fields of an interpretation, in stored order: its decision, how it
/// was applied, why interpretation was needed (`propose_change` takes them
/// by these names).
pub const INTERPRETATION_FIELDS: [&str; 3] = ["decision", "how", "why"];

/// What an interpretation carries beside an update's fields: the accepted
/// decision it interprets (its document's `id:`), how the decision was
/// applied and why interpretation was needed, verbatim. Stored as the
/// compact JSON `{"decision":…,"how":…,"why":…}`, keys in this order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interpretation {
    pub decision: String,
    pub how: String,
    pub why: String,
}

impl Interpretation {
    /// Why it cannot be stored or read as one: a blank value, a `how` or
    /// `why` over [`INTERPRETATION_TEXT_MAX`] bytes; `None`: it can.
    pub fn problem(&self) -> Option<FieldProblem> {
        if self.decision.trim().is_empty() {
            return Some(FieldProblem::new("decision", blank()));
        }
        interpretation_text_problem("how", &self.how)
            .or_else(|| interpretation_text_problem("why", &self.why))
    }
}

/// A kind's name with its indefinite article: `an update`, `a create`.
pub fn with_article(kind: &str) -> String {
    let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    format!("{article} {kind}")
}

/// An interpretation's `how` or `why` (`field`, as the caller names it):
/// not blank, at most [`INTERPRETATION_TEXT_MAX`] bytes.
pub fn interpretation_text_problem(field: &str, value: &str) -> Option<FieldProblem> {
    required(field, value, INTERPRETATION_TEXT_MAX)
}

/// A reported discrepancy as given (the MCP arguments but the author's,
/// the CLI's `--input` document).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscrepancyInput {
    pub node_ids: Vec<String>,
    pub summary: String,
    pub gap_type: GapType,
    pub severity: IntakeSeverity,
    pub evidence: Vec<Evidence>,
    pub options: Vec<IntakeOption>,
    /// An index into `options`.
    pub recommendation: u64,
    #[serde(default)]
    pub working_answer: Option<String>,
    #[serde(default)]
    pub proposed_patch: Option<ProposedPatch>,
    #[serde(default)]
    pub distinct_from: Option<Vec<String>>,
}

/// A decision taken during work as given (the MCP arguments but the
/// author's, the CLI's `--input` document): what had to be decided
/// (`text`), the decision (`working_answer`), what another answer would
/// cost, and the rule's new text as an optional linked update. No field is
/// named `decision`: that name keeps its other meanings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionInput {
    pub node_ids: Vec<String>,
    /// What had to be decided.
    pub text: String,
    /// The decision taken, worked on until the owner confirms it.
    pub working_answer: String,
    pub price_of_other: String,
    /// Absent or `null`: `normal`.
    #[serde(default)]
    pub severity: Option<IntakeSeverity>,
    #[serde(default)]
    pub proposed_patch: Option<ProposedPatch>,
    #[serde(default)]
    pub distinct_from: Option<Vec<String>>,
}

/// A question as given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionInput<'a> {
    pub node_ids: &'a [String],
    pub text: &'a str,
    pub working_answer: &'a str,
    pub price_of_other: &'a str,
    pub distinct_from: &'a [String],
}

/// The author's self-declared fields as given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorInput<'a> {
    pub role: Option<&'a str>,
    pub model: Option<&'a str>,
    pub run: Option<&'a str>,
}

/// Why one field of an item is refused: `<field>: <problem>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldProblem {
    /// As the input names it: `evidence[2].observed`.
    pub field: String,
    pub problem: String,
}

impl FieldProblem {
    fn new(field: impl Into<String>, problem: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            problem: problem.into(),
        }
    }
}

impl fmt::Display for FieldProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.problem)
    }
}

/// The first problem of a question's fields, in input order: `node_ids`,
/// `text`, `working_answer`, `price_of_other`, `distinct_from`, then the
/// author's.
pub fn question_problem(
    question: &QuestionInput<'_>,
    author: AuthorInput<'_>,
) -> Option<FieldProblem> {
    node_ids_problem(question.node_ids)
        .or_else(|| required("text", question.text, SUMMARY_MAX))
        .or_else(|| required("working_answer", question.working_answer, ANSWER_MAX))
        .or_else(|| required("price_of_other", question.price_of_other, ANSWER_MAX))
        .or_else(|| distinct_problem(question.distinct_from))
        .or_else(|| author_problem(author))
}

/// The first problem of a discrepancy's fields, in input order: `node_ids`,
/// `summary`, `evidence`, `options`, `recommendation`, `working_answer`,
/// `proposed_patch`, `distinct_from`, then the author's.
pub fn discrepancy_problem(
    input: &DiscrepancyInput,
    author: AuthorInput<'_>,
) -> Option<FieldProblem> {
    node_ids_problem(&input.node_ids)
        .or_else(|| required("summary", &input.summary, SUMMARY_MAX))
        .or_else(|| evidence_problem(&input.evidence))
        .or_else(|| options_problem(&input.options))
        .or_else(|| recommendation_problem(input.recommendation, input.options.len()))
        .or_else(|| {
            optional(
                "working_answer",
                input.working_answer.as_deref(),
                ANSWER_MAX,
            )
        })
        .or_else(|| input.proposed_patch.as_ref().and_then(patch_problem))
        .or_else(|| distinct_problem(input.distinct_from.as_deref().unwrap_or_default()))
        .or_else(|| author_problem(author))
}

/// The first problem of a decision's fields, in input order: `node_ids`,
/// `text`, `working_answer`, `price_of_other`, `proposed_patch`,
/// `distinct_from`, then the author's.
pub fn decision_problem(input: &DecisionInput, author: AuthorInput<'_>) -> Option<FieldProblem> {
    node_ids_problem(&input.node_ids)
        .or_else(|| required("text", &input.text, SUMMARY_MAX))
        .or_else(|| required("working_answer", &input.working_answer, ANSWER_MAX))
        .or_else(|| required("price_of_other", &input.price_of_other, ANSWER_MAX))
        .or_else(|| input.proposed_patch.as_ref().and_then(patch_problem))
        .or_else(|| distinct_problem(input.distinct_from.as_deref().unwrap_or_default()))
        .or_else(|| author_problem(author))
}

/// A rationale over [`RATIONALE_MAX`] (`propose update`'s `--rationale`).
pub fn rationale_problem(field: &str, rationale: &str) -> Option<FieldProblem> {
    over(field, rationale, RATIONALE_MAX)
}

/// The text the queue's dedup compares: every run of Unicode whitespace one
/// space, trimmed, lower-cased.
pub fn normalized_summary(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// `lines` reads `N` or `N-M`, 1 ≤ N ≤ M < [`LINE_LIMIT`].
pub fn lines_are_valid(lines: &str) -> bool {
    let number = |text: &str| -> Option<u64> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        text.parse::<u64>()
            .ok()
            .filter(|&number| (1..LINE_LIMIT).contains(&number))
    };
    match lines.split_once('-') {
        None => number(lines).is_some(),
        Some((first, last)) => match (number(first), number(last)) {
            (Some(first), Some(last)) => first <= last,
            _ => false,
        },
    }
}

fn node_ids_problem(node_ids: &[String]) -> Option<FieldProblem> {
    if node_ids.is_empty() {
        return Some(FieldProblem::new(
            "node_ids",
            format!("none: name 1 to {NODE_IDS_MAX} nodes by ID"),
        ));
    }
    if node_ids.len() > NODE_IDS_MAX {
        return Some(FieldProblem::new(
            "node_ids",
            format!("{} IDs; at most {NODE_IDS_MAX}", node_ids.len()),
        ));
    }
    node_ids
        .iter()
        .enumerate()
        .find(|(_, id)| id.trim().is_empty())
        .map(|(index, _)| FieldProblem::new(format!("node_ids[{index}]"), blank()))
}

fn evidence_problem(evidence: &[Evidence]) -> Option<FieldProblem> {
    if evidence.is_empty() {
        return Some(FieldProblem::new(
            "evidence",
            format!("none: give 1 to {EVIDENCE_MAX} items"),
        ));
    }
    if evidence.len() > EVIDENCE_MAX {
        return Some(FieldProblem::new(
            "evidence",
            format!("{} items; at most {EVIDENCE_MAX}", evidence.len()),
        ));
    }
    evidence.iter().enumerate().find_map(|(index, item)| {
        let field = |name: &str| format!("evidence[{index}].{name}");
        required(&field("file"), &item.file, LOCATION_MAX)
            .or_else(|| optional(&field("qpath"), item.qpath.as_deref(), LOCATION_MAX))
            .or_else(|| {
                item.lines
                    .as_deref()
                    .filter(|lines| !lines_are_valid(lines))
                    .map(|lines| {
                        FieldProblem::new(
                            field("lines"),
                            format!(
                                "{lines:?} is not `N` or `N-M` with 1 <= N <= M < {LINE_LIMIT}"
                            ),
                        )
                    })
            })
            .or_else(|| required(&field("observed"), &item.observed, EVIDENCE_TEXT_MAX))
            .or_else(|| required(&field("documented"), &item.documented, EVIDENCE_TEXT_MAX))
    })
}

fn options_problem(options: &[IntakeOption]) -> Option<FieldProblem> {
    if !(OPTIONS_MIN..=OPTIONS_MAX).contains(&options.len()) {
        return Some(FieldProblem::new(
            "options",
            format!(
                "{} option(s); give {OPTIONS_MIN} to {OPTIONS_MAX}, each priced",
                options.len()
            ),
        ));
    }
    options.iter().enumerate().find_map(|(index, option)| {
        let field = |name: &str| format!("options[{index}].{name}");
        required(&field("label"), &option.label, LABEL_MAX)
            .or_else(|| required(&field("effect"), &option.effect, OPTION_TEXT_MAX))
            .or_else(|| required(&field("price"), &option.price, OPTION_TEXT_MAX))
    })
}

fn recommendation_problem(recommendation: u64, options: usize) -> Option<FieldProblem> {
    let in_range = usize::try_from(recommendation).is_ok_and(|index| index < options);
    (!in_range).then(|| {
        FieldProblem::new(
            "recommendation",
            format!(
                "{recommendation} is no index into the {options} options (0 to {})",
                options.saturating_sub(1)
            ),
        )
    })
}

fn patch_problem(patch: &ProposedPatch) -> Option<FieldProblem> {
    required("proposed_patch.target", &patch.target, usize::MAX)
        .or_else(|| required("proposed_patch.base", &patch.base, usize::MAX))
        .or_else(|| over("proposed_patch.text", &patch.text, TEXT_MAX_BYTES))
        .or_else(|| required("proposed_patch.rationale", &patch.rationale, RATIONALE_MAX))
}

fn distinct_problem(distinct_from: &[String]) -> Option<FieldProblem> {
    if distinct_from.len() > DISTINCT_MAX {
        return Some(FieldProblem::new(
            "distinct_from",
            format!("{} entries; at most {DISTINCT_MAX}", distinct_from.len()),
        ));
    }
    distinct_from.iter().enumerate().find_map(|(index, entry)| {
        let field = format!("distinct_from[{index}]");
        if entry.is_empty() {
            return Some(FieldProblem::new(field, "empty: name a hit's ID or path"));
        }
        if entry.chars().any(char::is_control) {
            return Some(FieldProblem::new(field, "holds a control character"));
        }
        over(&field, entry, DISTINCT_ITEM_MAX)
    })
}

/// The first author field outside its grammar ([`author_field_problem`]),
/// named as the tools name it: `author_role`, `author_model`, `run`.
pub fn author_problem(author: AuthorInput<'_>) -> Option<FieldProblem> {
    [
        ("author_role", author.role),
        ("author_model", author.model),
        ("run", author.run),
    ]
    .into_iter()
    .find_map(|(field, value)| {
        value
            .and_then(author_field_problem)
            .map(|problem| FieldProblem::new(field, problem))
    })
}

/// A required string: not blank, at most `max` bytes.
fn required(field: &str, value: &str, max: usize) -> Option<FieldProblem> {
    if value.trim().is_empty() {
        return Some(FieldProblem::new(field, blank()));
    }
    over(field, value, max)
}

/// An optional string: when given, as a required one.
fn optional(field: &str, value: Option<&str>, max: usize) -> Option<FieldProblem> {
    value.and_then(|value| required(field, value, max))
}

fn over(field: &str, value: &str, max: usize) -> Option<FieldProblem> {
    (value.len() > max)
        .then(|| FieldProblem::new(field, format!("{} bytes; at most {max}", value.len())))
}

fn blank() -> &'static str {
    "blank: give a value"
}
