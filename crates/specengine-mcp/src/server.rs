//! The server handler: server info and `instructions`, the protocol versions
//! served, the tool routers, and the stdio loop.

use std::borrow::Cow;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::{InputResponses, RequestState};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResponse, Implementation, ProtocolVersion, ServerCapabilities, ServerConfig,
};
use rmcp::service::{QuitReason, RequestContext, ServerInitializeError};
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, tool, tool_handler, tool_router};

use crate::review::{ReviewArgs, Reviewer};

/// Server `instructions` (07 §1.1): Claude Code truncates them at 2 048
/// characters and defers tool definitions behind tool search, so this is the
/// server's most important text. ASCII only, so bytes equal characters.
pub const INSTRUCTIONS: &str = "\
SpecEngine keeps a project's specification (requirements, assumptions, questions, \
decisions, acceptance criteria) as files in git next to the code and binds its \
records to code symbols. This is the Phase 0 build of its MCP server: one owner \
tool, no proposal queue behind it yet.

review_proposal(proposal_id) asks the human owner, through a form, to approve or \
reject a proposal, and returns the owner's answer: the form action (accept, \
decline or cancel), the decision (approve or reject) and an optional comment. \
Every call prompts the human; an agent cannot answer it for them. Call it only \
when the owner should decide now, once per proposal; after a decline or cancel, \
do not call it again unless the owner asks. In Phase 0 any Latin proposal ID \
works and nothing is recorded.

Rules:
- No tool writes files. Spec files change only through apply_proposal on an \
owner action.
- Tools are deterministic: one state, one result. There is no LLM inside.
- Nothing is blocked by a discrepancy: keep working on the working answer while \
a proposal waits for the owner.
- IDs are Latin only: ASCII letters, digits, '-', '_' and '.'.";

/// Appended to [`INSTRUCTIONS`] when the probe tools are built in.
#[cfg(feature = "probes")]
const PROBE_INSTRUCTIONS: &str = "

Measurement build (feature probes): probe_output(tokens) returns filler text of \
about that many tokens; probe_sleep(seconds) waits and returns. Use them only \
when the owner runs the interactive MCP checklist.";

/// Claude Code truncates `instructions` and tool descriptions at this length (04 §4).
const TEXT_LIMIT: usize = 2048;

const _: () = assert!(INSTRUCTIONS.len() <= TEXT_LIMIT);
#[cfg(feature = "probes")]
const _: () = assert!(INSTRUCTIONS.len() + PROBE_INSTRUCTIONS.len() <= TEXT_LIMIT);

/// Which protocol eras the server accepts.
///
/// rmcp 3.5.0 has no server-side lifecycle mode: `serve_server` picks the era
/// from the client's first message (`initialize` → legacy session; any other
/// request carrying 2026-07-28 `_meta` → stateless). The only server-side knob
/// is the set of protocol versions served, which is what this selects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lifecycle {
    /// Both eras: legacy `initialize` sessions up to 2025-11-25 and stateless
    /// 2026-07-28 requests (the `ClientLifecycleMode::Auto` counterpart).
    #[default]
    Auto,
    /// Legacy only: versions up to 2025-11-25. A stateless 2026-07-28 request is
    /// refused with `UNSUPPORTED_PROTOCOL_VERSION`; exists so the scripted
    /// stateless-era check (`tests/mcp_stdio.rs`) can show it discriminates.
    Legacy,
}

impl Lifecycle {
    /// The protocol versions served, oldest first.
    fn protocol_versions(self) -> &'static [ProtocolVersion] {
        let newest = match self {
            Self::Auto => &ProtocolVersion::V_2026_07_28,
            Self::Legacy => &ProtocolVersion::LATEST_WITH_INITIALIZE,
        };
        ProtocolVersion::known_up_to(newest)
    }
}

/// The SpecEngine MCP server: one instance per stdio connection.
pub struct SpecEngineServer {
    lifecycle: Lifecycle,
    reviewer: Reviewer,
    tool_router: ToolRouter<Self>,
}

impl SpecEngineServer {
    /// A server accepting the eras of `lifecycle`. Draws the per-process
    /// `requestState` key from the OS; without one, stateless reviews answer
    /// with a tool error and everything else keeps working.
    pub fn new(lifecycle: Lifecycle) -> Self {
        let tool_router = Self::core_tools();
        #[cfg(feature = "probes")]
        let tool_router = tool_router + Self::probe_tools();
        Self {
            lifecycle,
            reviewer: Reviewer::new(),
            tool_router,
        }
    }
}

#[tool_router(router = core_tools)]
impl SpecEngineServer {
    /// The owner's consent tool (07 §1.2 `owner` set).
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
        meta = crate::review::requires_user_interaction(),
        output_schema = rmcp::handler::server::tool::schema_for_output::<crate::review::ReviewOutcome>()
    )]
    async fn review_proposal(
        &self,
        Parameters(args): Parameters<ReviewArgs>,
        RequestState(request_state): RequestState,
        InputResponses(input_responses): InputResponses,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.reviewer
            .review(args, request_state, input_responses, &context)
            .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SpecEngineServer {
    fn get_info(&self) -> ServerConfig {
        #[cfg(not(feature = "probes"))]
        let instructions = INSTRUCTIONS.to_owned();
        #[cfg(feature = "probes")]
        let instructions = format!("{INSTRUCTIONS}{PROBE_INSTRUCTIONS}");
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(instructions)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(self.lifecycle.protocol_versions())
    }
}

/// Why the stdio server stopped abnormally.
#[derive(Debug)]
pub enum ServeError {
    /// The first message was neither `initialize`, `ping` nor a request with
    /// complete 2026-07-28 `_meta`; rmcp answered it with an error and ended.
    Start(Box<ServerInitializeError>),
    /// The service task failed.
    Join(String),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(error) => write!(f, "MCP session did not start: {error}"),
            Self::Join(error) => write!(f, "MCP service task failed: {error}"),
        }
    }
}

impl std::error::Error for ServeError {}

/// Serves one MCP client over stdin/stdout until the client closes stdin.
/// A client that closes stdin before its first message ends a clean session.
pub async fn serve_stdio(lifecycle: Lifecycle) -> Result<(), ServeError> {
    let running = match SpecEngineServer::new(lifecycle)
        .serve(rmcp::transport::stdio())
        .await
    {
        Ok(running) => running,
        Err(ServerInitializeError::ConnectionClosed(_)) => return Ok(()),
        Err(error) => return Err(ServeError::Start(Box::new(error))),
    };
    match running.waiting().await {
        Ok(QuitReason::JoinError(error)) => Err(ServeError::Join(error.to_string())),
        Ok(_) => Ok(()),
        Err(error) => Err(ServeError::Join(error.to_string())),
    }
}
