//! The server handler: server info and `instructions`, the protocol versions
//! served, the tool routers, the resources, and the stdio loop.

use std::borrow::Cow;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{
    CacheScope, Implementation, ListResourceTemplatesResult, ListResourcesResult,
    PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams, ReadResourceResponse,
    ReadResourceResult, ServerCapabilities, ServerConfig,
};
use rmcp::service::{QuitReason, RequestContext, ServerInitializeError};
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, tool_handler};
use specengine_cli::{Env, Globals, OUTPUT_CAP_CHARS};

use crate::read::holds_number;
use crate::resources;

/// Server `instructions` (07 §1.1): Claude Code truncates them at 2 048
/// characters and defers tool definitions behind tool search, so this is the
/// server's most important text. ASCII only, so bytes equal characters.
pub const INSTRUCTIONS: &str = "\
SpecEngine keeps a project's specification (requirements, assumptions, questions, \
decisions, acceptance criteria) as Markdown files in git next to the code. These \
tools read it as the `spec` command line does: content is what `spec <command> 2>&1` \
prints, structuredContent its --json document.

Tools:
- get_tree(root?, depth?, kinds?, archive?) = spec tree: the containment tree, no \
bodies.
- get_node(id, with?, archive?) = spec show: a node's current text; with [\"links\"] \
adds its links both ways.
- search(query, kinds?, limit?, archive?) = spec search: full-text search.
- get_context_bundle(node_ids, budget?) = spec bundle: the context of nodes within a \
token budget, named by its bundle_hash.

A node is named by any REF: an ID, an alias, slug/ID, ID#SECTION or a root-relative \
.md path. Hints in answers use the command's flags: --kind is kinds, ROOT is root, \
--links is with [\"links\"]; --depth, --limit, --budget and --archive keep their \
names. Answers are cut at 40000 characters, with a tail saying how to narrow them. A \
REF naming nothing, or a call that cannot run (no specengine.toml: run `spec init`), \
is an error result with the reason. Resources for @-mentions: spec://<slug>/tree and \
spec://<slug>/node/<REF, percent-encoded>.

Rules:
- Deterministic: one state, one result; no LLM inside.
- Each call reads the files as they are now. Reads refresh SpecEngine's index in its \
data directory; nothing under the project root is written. Spec files change only \
through apply_proposal on an owner action.
- Nothing is blocked by a discrepancy.
- IDs are Latin only; a look-alike ID is refused with its Latin fix.";

/// Appended to [`INSTRUCTIONS`] when the measurement tools are built in.
#[cfg(feature = "probes")]
const PROBE_INSTRUCTIONS: &str = "

Measurement build (feature probes), for the owner's MCP checklist only: \
review_proposal(proposal_id), the Phase 0 consent demo, asks the human owner through \
a form to approve or reject and returns the answer; every call prompts the human, \
nothing is recorded. probe_output(tokens) returns filler of about that many tokens; \
probe_sleep(seconds) waits, then returns.";

/// Claude Code truncates `instructions` and tool descriptions at this length (04 §4).
const TEXT_LIMIT: usize = 2048;

const _: () = assert!(INSTRUCTIONS.is_ascii());
const _: () = assert!(INSTRUCTIONS.len() <= TEXT_LIMIT);
// The cap the instructions state is the CLI's.
const _: () = assert!(holds_number(
    INSTRUCTIONS,
    "cut at ",
    OUTPUT_CAP_CHARS,
    " characters"
));
#[cfg(feature = "probes")]
const _: () = assert!(PROBE_INSTRUCTIONS.is_ascii());
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
///
/// Holds no project state: each read finds the project from the process's
/// current directory and `globals` when it runs, and keeps nothing after it.
pub struct SpecEngineServer {
    lifecycle: Lifecycle,
    /// `--root`, `--config`: the CLI globals of every read.
    globals: Globals,
    #[cfg(feature = "probes")]
    reviewer: crate::review::Reviewer,
    tool_router: ToolRouter<Self>,
}

impl SpecEngineServer {
    /// A server accepting the eras of `lifecycle`, reading with `globals`.
    /// Under feature `probes`, draws the per-process `requestState` key of the
    /// consent demo from the OS; without one, stateless reviews answer with a
    /// tool error and everything else keeps working.
    pub fn new(lifecycle: Lifecycle, globals: Globals) -> Self {
        let tool_router = Self::read_tools();
        #[cfg(feature = "probes")]
        let tool_router = tool_router + Self::review_tools() + Self::probe_tools();
        Self {
            lifecycle,
            globals,
            #[cfg(feature = "probes")]
            reviewer: crate::review::Reviewer::new(),
            tool_router,
        }
    }

    /// The CLI globals of every read.
    pub(crate) fn globals(&self) -> &Globals {
        &self.globals
    }

    /// The consent demo's reviewer.
    #[cfg(feature = "probes")]
    pub(crate) fn reviewer(&self) -> &crate::review::Reviewer {
        &self.reviewer
    }
}

/// Whether the request's protocol carries the cache hints `ttlMs` and
/// `cacheScope` (2026-07-28 and later; SEP-2549).
fn cache_hints(context: &RequestContext<RoleServer>) -> bool {
    context
        .protocol_version()
        .is_some_and(|version| version >= ProtocolVersion::V_2026_07_28)
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SpecEngineServer {
    fn get_info(&self) -> ServerConfig {
        #[cfg(not(feature = "probes"))]
        let instructions = INSTRUCTIONS.to_owned();
        #[cfg(feature = "probes")]
        let instructions = format!("{INSTRUCTIONS}{PROBE_INSTRUCTIONS}");
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(instructions)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(self.lifecycle.protocol_versions())
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let cursor = request.and_then(|request| request.cursor);
        let globals = self.globals.clone();
        // No usable current directory: no project is found.
        let page = tokio::task::spawn_blocking(move || match Env::from_process() {
            Ok(env) => resources::list(&env, &globals, cursor.as_deref()),
            Err(_) => Ok(resources::Page::default()),
        })
        .await
        .map_err(|_| ErrorData::internal_error("internal error: resources/list failed", None))??;
        let mut result = ListResourcesResult::with_all_items(page.resources);
        result.next_cursor = page.next_cursor;
        if cache_hints(&context) {
            result = result.with_ttl_ms(0).with_cache_scope(CacheScope::Public);
        }
        Ok(result)
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        let mut result = ListResourceTemplatesResult::with_all_items(resources::templates());
        if cache_hints(&context) {
            result = result.with_ttl_ms(0).with_cache_scope(CacheScope::Public);
        }
        Ok(result)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let uri = request.uri;
        let parsed = resources::parse(&uri)?;
        let globals = self.globals.clone();
        let contents = tokio::task::spawn_blocking(move || {
            let env = Env::from_process()
                .map_err(|error| ErrorData::internal_error(error.message, None))?;
            resources::read(&env, &globals, &uri, &parsed)
        })
        .await
        .map_err(|_| ErrorData::internal_error("internal error: resources/read failed", None))??;
        let mut result = ReadResourceResult::new(vec![contents]);
        if cache_hints(&context) {
            result = result.with_ttl_ms(0).with_cache_scope(CacheScope::Public);
        }
        Ok(result.into())
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

/// Serves one MCP client over stdin/stdout until the client closes stdin,
/// reading with `globals` (`--root`, `--config`). A client that closes stdin
/// before its first message ends a clean session.
pub async fn serve_stdio(lifecycle: Lifecycle, globals: Globals) -> Result<(), ServeError> {
    let running = match SpecEngineServer::new(lifecycle, globals)
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
