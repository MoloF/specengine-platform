//! `review_proposal`: the owner's consent through an elicitation form, in both
//! protocol eras (07 §1.2 `owner` set; 04 §3;
//! `crates/specengine-mcp/README.md` "Claude Code client").
//!
//! - Legacy session: the form goes out as a server-initiated
//!   `elicitation/create` request and the call waits for the answer.
//! - Stateless 2026-07-28: the first call answers `input_required` with the form
//!   under [`INPUT_KEY`] and an HMAC-sealed `requestState` bound to the proposal
//!   ID; the client repeats the call with `inputResponses` and the state, and the
//!   second call returns the answer.
//!
//! Phase 0 has no proposal queue: any well-formed Latin ID is reviewable, the
//! answer is only returned, and nothing is recorded or written (ADR-0004).
//!
//! The sealed state carries no TTL: with a TTL the round-1 result would depend
//! on the wall clock, and a replay only returns the answer the client supplies
//! again. The key is per process, so a restart invalidates every state.

use rmcp::handler::server::tool::{InputResponses, RequestState};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResponse, CallToolResult, ClientCapabilities, ClientResult, ContentBlock,
    ElicitRequest, ElicitRequestParams, ElicitResult, ElicitationAction, ElicitationSchema,
    EnumSchema, InputRequest, InputRequests, InputRequiredResult, MetaObject, RequestStateCodec,
    SealOptions, ServerRequest,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::server::SpecEngineServer;

/// Tool `_meta` key that makes Claude Code (≥ 2.1.199) prompt the human on every
/// call, even under `bypassPermissions`
/// (`crates/specengine-mcp/README.md` "Claude Code client").
pub(crate) const REQUIRES_USER_INTERACTION: &str = "anthropic/requiresUserInteraction";

/// Key of the form in `inputRequests` / `inputResponses` (stateless era).
pub(crate) const INPUT_KEY: &str = "owner_review";

/// Longest accepted proposal ID, in bytes.
const MAX_ID_LEN: usize = 64;

/// The tool's `_meta`: `{"anthropic/requiresUserInteraction": true}`.
pub(crate) fn requires_user_interaction() -> MetaObject {
    let mut meta = MetaObject::new();
    meta.insert(REQUIRES_USER_INTERACTION.to_owned(), Value::Bool(true));
    meta
}

/// Arguments of `review_proposal`: a flat object (07 §1.1).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviewArgs {
    /// The proposal to review, e.g. `P-0007`. Latin only: ASCII letters, digits,
    /// `-`, `_` and `.`, starting with a letter; at most 64 characters.
    pub proposal_id: String,
}

/// What the owner did with the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FormAction {
    /// The owner submitted the form.
    Accept,
    /// The owner explicitly declined to answer.
    Decline,
    /// The owner dismissed the form without a choice.
    Cancel,
}

/// The owner's decision in a submitted form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// The proposal is approved.
    Approve,
    /// The proposal is rejected.
    Reject,
}

/// The protocol era the answer came through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Era {
    /// `initialize` session; the form went out as `elicitation/create`.
    Legacy,
    /// 2026-07-28; the form went out as `input_required` + `requestState`.
    Stateless,
}

/// `structuredContent` of `review_proposal` (its `outputSchema`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReviewOutcome {
    /// The reviewed proposal.
    pub proposal_id: String,
    /// `accept`, `decline` or `cancel`.
    pub action: FormAction,
    /// `approve` or `reject` when the form was accepted; `null` otherwise.
    pub decision: Option<Decision>,
    /// The owner's comment; `null` when none was given.
    pub comment: Option<String>,
    /// `legacy` or `stateless`.
    pub era: Era,
    /// The protocol version of the call, e.g. `2025-11-25` or `2026-07-28`.
    pub protocol_version: String,
}

#[tool_router(router = review_tools, vis = "pub(crate)")]
impl SpecEngineServer {
    /// The owner's consent tool (07 §1.2 `owner` set); the Phase 0 demo,
    /// built under feature `probes` only.
    #[tool(
        description = "Ask the human owner to approve or reject a SpecEngine proposal and \
return the owner's answer. Shows the owner a form (decision: approve or reject; \
optional comment) and returns the form action (accept, decline, cancel), the \
decision and the comment. Every call prompts the human. Phase 0: no proposal \
queue yet; any Latin ID works, nothing is recorded, no file is written. \
Deterministic; no LLM inside.",
        annotations(
            title = "Review a proposal (owner)",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        ),
        meta = requires_user_interaction(),
        output_schema = rmcp::handler::server::tool::schema_for_output::<ReviewOutcome>()
    )]
    async fn review_proposal(
        &self,
        Parameters(args): Parameters<ReviewArgs>,
        RequestState(request_state): RequestState,
        InputResponses(input_responses): InputResponses,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.reviewer()
            .review(args, request_state, input_responses, &context)
            .await
    }
}

/// The submitted form: `{decision: approve|reject, comment?: string}`.
#[derive(Deserialize)]
struct FormContent {
    decision: Decision,
    #[serde(default)]
    comment: Option<String>,
}

/// Payload sealed into the stateless `requestState`.
#[derive(Serialize, Deserialize)]
struct SealedReview {
    proposal_id: String,
}

/// Runs reviews; holds the per-process `requestState` key.
pub(crate) struct Reviewer {
    /// `Err` holds why no key could be drawn; stateless reviews then fail softly.
    codec: Result<RequestStateCodec, String>,
}

impl Reviewer {
    pub(crate) fn new() -> Self {
        let mut key = [0u8; RequestStateCodec::MIN_KEY_LENGTH];
        let codec = getrandom::fill(&mut key)
            .map_err(|error| format!("no OS randomness for the requestState key: {error}"))
            .and_then(|()| {
                RequestStateCodec::try_new(key.to_vec()).map_err(|error| error.to_string())
            });
        Self { codec }
    }

    /// One `review_proposal` call in either era.
    pub(crate) async fn review(
        &self,
        args: ReviewArgs,
        request_state: Option<String>,
        input_responses: Option<rmcp::model::InputResponses>,
        context: &RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let proposal_id = args.proposal_id;
        if let Err(problem) = check_proposal_id(&proposal_id) {
            return Ok(tool_error(problem));
        }
        let version = context.protocol_version();
        let version_label = version
            .as_ref()
            .map_or_else(|| "unknown".to_owned(), |v| v.as_str().to_owned());
        // The same gate rmcp applies to `InputRequiredResult`: 2026-07-28 or newer.
        let stateless = version.as_ref().is_some_and(|v| !v.has_initialize());

        if stateless && let Some(sealed) = request_state {
            return self.resume(proposal_id, &sealed, input_responses, version_label);
        }
        if !context
            .client_capabilities()
            .as_ref()
            .is_some_and(supports_form_elicitation)
        {
            return Ok(tool_error(format!(
                "review_proposal {proposal_id}: the client did not declare form elicitation, \
                 so the owner cannot be asked from this session. Nothing was recorded. Ask the \
                 owner directly."
            )));
        }
        let form = form_params(&proposal_id)?;
        if stateless {
            self.ask_stateless(&proposal_id, form)
        } else {
            Ok(ask_legacy(proposal_id, form, version_label, context).await)
        }
    }

    /// Stateless round 1: `input_required` with the form and a sealed state.
    fn ask_stateless(
        &self,
        proposal_id: &str,
        form: ElicitRequestParams,
    ) -> Result<CallToolResponse, ErrorData> {
        let codec = match &self.codec {
            Ok(codec) => codec,
            Err(problem) => {
                return Ok(tool_error(format!(
                    "review_proposal {proposal_id}: the server cannot seal the form state \
                     ({problem}). Nothing was recorded."
                )));
            }
        };
        let context = associated_data(proposal_id);
        let options = SealOptions::new().associated_data(&context);
        let sealed = codec
            .seal_json_with(
                &SealedReview {
                    proposal_id: proposal_id.to_owned(),
                },
                &options,
            )
            .map_err(|error| {
                ErrorData::internal_error(format!("cannot seal requestState: {error}"), None)
            })?;
        let mut requests = InputRequests::new();
        requests.insert(
            INPUT_KEY.to_owned(),
            InputRequest::Elicitation(ElicitRequest::new(form)),
        );
        Ok(InputRequiredResult::new(Some(requests), Some(sealed)).into())
    }

    /// Stateless round 2: verify the echoed state, read the answer.
    fn resume(
        &self,
        proposal_id: String,
        sealed: &str,
        input_responses: Option<rmcp::model::InputResponses>,
        version_label: String,
    ) -> Result<CallToolResponse, ErrorData> {
        let codec = self.codec.as_ref().map_err(|problem| {
            ErrorData::invalid_params(
                format!("requestState cannot be verified by this server: {problem}"),
                None,
            )
        })?;
        let opened: SealedReview = codec
            .open_json_with(sealed, &associated_data(&proposal_id))
            .map_err(|error| {
                ErrorData::invalid_params(
                    format!(
                        "requestState rejected for proposal {proposal_id}: {error}; call \
                         review_proposal again without requestState"
                    ),
                    None,
                )
            })?;
        if opened.proposal_id != proposal_id {
            return Err(ErrorData::invalid_params(
                format!("requestState belongs to another proposal than {proposal_id}"),
                None,
            ));
        }
        let Some(answer) = input_responses.and_then(|mut map| map.remove(INPUT_KEY)) else {
            return Err(ErrorData::invalid_params(
                format!("inputResponses.{INPUT_KEY} is missing from the retried call"),
                None,
            ));
        };
        let answer: ElicitResult = serde_json::from_value(answer).map_err(|error| {
            ErrorData::invalid_params(
                format!("inputResponses.{INPUT_KEY} is not an elicitation result: {error}"),
                None,
            )
        })?;
        Ok(finish(proposal_id, answer, Era::Stateless, version_label))
    }
}

/// Legacy: send `elicitation/create` and wait for the owner, or for the
/// client's cancellation of the tool call.
async fn ask_legacy(
    proposal_id: String,
    form: ElicitRequestParams,
    version_label: String,
    context: &RequestContext<RoleServer>,
) -> CallToolResponse {
    let request = ServerRequest::ElicitRequest(ElicitRequest::new(form));
    let reply = tokio::select! {
        reply = context.peer.send_request(request) => reply,
        () = context.ct.cancelled() => {
            return tool_error(format!(
                "review_proposal {proposal_id}: the call was cancelled before the owner answered. \
                 Nothing was recorded."
            ));
        }
    };
    match reply {
        Ok(ClientResult::ElicitResult(answer)) => {
            finish(proposal_id, answer, Era::Legacy, version_label)
        }
        Ok(_) => tool_error(format!(
            "review_proposal {proposal_id}: the client answered the form request with \
             something other than an elicitation result. Nothing was recorded."
        )),
        Err(error) => tool_error(format!(
            "review_proposal {proposal_id}: the form could not be shown ({error}). Nothing was \
             recorded."
        )),
    }
}

/// Turns the owner's answer into the tool result.
fn finish(
    proposal_id: String,
    answer: ElicitResult,
    era: Era,
    version: String,
) -> CallToolResponse {
    let (action, decision, comment) = match answer.action {
        ElicitationAction::Accept => {
            let content = answer.content.unwrap_or(Value::Null);
            match serde_json::from_value::<FormContent>(content) {
                Ok(form) => (
                    FormAction::Accept,
                    Some(form.decision),
                    form.comment
                        .map(|comment| comment.trim().to_owned())
                        .filter(|comment| !comment.is_empty()),
                ),
                Err(error) => {
                    return tool_error(format!(
                        "review_proposal {proposal_id}: the submitted form is not \
                         {{decision: approve|reject, comment?: string}} ({error}). Nothing was \
                         recorded."
                    ));
                }
            }
        }
        ElicitationAction::Decline => (FormAction::Decline, None, None),
        _ => (FormAction::Cancel, None, None),
    };
    let outcome = ReviewOutcome {
        proposal_id,
        action,
        decision,
        comment,
        era,
        protocol_version: version,
    };
    let structured = match serde_json::to_value(&outcome) {
        Ok(value) => value,
        Err(error) => return tool_error(format!("cannot encode the review outcome: {error}")),
    };
    let mut result = CallToolResult::structured(structured);
    result.content = vec![ContentBlock::text(prose(&outcome))];
    result.into()
}

/// The model-facing text: a status line, the comment, the no-write note, a hint.
fn prose(outcome: &ReviewOutcome) -> String {
    let id = &outcome.proposal_id;
    let status = match (outcome.action, outcome.decision) {
        (FormAction::Accept, Some(Decision::Approve)) => "approved by the owner",
        (FormAction::Accept, Some(Decision::Reject)) => "rejected by the owner",
        (FormAction::Decline, _) => "the owner declined to answer",
        _ => "the owner dismissed the form",
    };
    let mut text = format!("review_proposal {id}: {status}.\n");
    if let Some(comment) = &outcome.comment {
        text.push_str(&format!("Owner comment: {comment}\n"));
    }
    text.push_str(
        "Nothing was recorded or written: Phase 0 has no proposal queue, and spec files \
         change only through apply_proposal on an owner action.\n",
    );
    let hint = match outcome.action {
        FormAction::Accept => "Hint: continue according to the owner's decision.",
        _ => {
            "Hint: do not call review_proposal again for this proposal unless the owner asks; \
             keep working on the working answer."
        }
    };
    text.push_str(hint);
    text
}

/// The form: `decision` (required enum approve/reject) and `comment` (optional string).
fn form_params(proposal_id: &str) -> Result<ElicitRequestParams, ErrorData> {
    let decision = EnumSchema::builder(vec!["approve".to_owned(), "reject".to_owned()])
        .title("Decision")
        .description("Approve or reject the proposal.")
        .build();
    let requested_schema = ElicitationSchema::builder()
        .required_enum_schema("decision", decision)
        .optional_string_with("comment", |schema| {
            schema
                .title("Comment")
                .description("Optional note returned to the agent.")
        })
        .build()
        .map_err(|error| {
            ErrorData::internal_error(format!("invalid review form schema: {error}"), None)
        })?;
    Ok(ElicitRequestParams::FormElicitationParams {
        meta: None,
        message: format!(
            "SpecEngine asks you to review proposal {proposal_id}. Phase 0 demo: there is no \
             proposal queue yet, so your answer only goes back to the agent; nothing is \
             recorded and no file is written."
        ),
        requested_schema,
    })
}

/// Form elicitation is supported when `elicitation` is declared with `form`,
/// or with neither mode (the pre-2025-11-25 shape, which means form).
fn supports_form_elicitation(capabilities: &ClientCapabilities) -> bool {
    capabilities
        .elicitation
        .as_ref()
        .is_some_and(|elicitation| elicitation.form.is_some() || elicitation.url.is_none())
}

/// Binds a sealed state to this tool and this proposal.
fn associated_data(proposal_id: &str) -> Vec<u8> {
    format!("specengine-mcp/review_proposal\0{proposal_id}").into_bytes()
}

/// ADR-0009: IDs are Latin only. Returns the problem for the model to read.
fn check_proposal_id(id: &str) -> Result<(), String> {
    let Some(first) = id.chars().next() else {
        return Err("review_proposal: proposal_id is empty.".to_owned());
    };
    if let Some(bad) = id
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
    {
        let kind = if bad.is_alphabetic() {
            "a non-Latin letter"
        } else {
            "a character outside ASCII letters, digits, '-', '_' and '.'"
        };
        return Err(format!(
            "review_proposal: proposal_id {id:?} contains {kind} {bad:?} (U+{:04X}); IDs are \
             Latin only (ADR-0009).",
            u32::from(bad)
        ));
    }
    if !first.is_ascii_alphabetic() {
        return Err(format!(
            "review_proposal: proposal_id {id:?} must start with an ASCII letter."
        ));
    }
    if id.len() > MAX_ID_LEN {
        return Err(format!(
            "review_proposal: proposal_id is {} characters long; at most {MAX_ID_LEN}.",
            id.len()
        ));
    }
    Ok(())
}

/// A tool-level error (`isError: true`) the model can read and act on.
fn tool_error(message: String) -> CallToolResponse {
    CallToolResult::error(vec![ContentBlock::text(message)]).into()
}
