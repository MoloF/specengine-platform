//! Probe tools for the interactive MCP checks of 04 §4 (feature `probes`;
//! re-run on a Claude Code upgrade): the output cap and 2-minute backgrounding.
//! Measured on Claude Code 2.1.283: the output cap counts characters, not
//! tokens — 48 000 pass inline, 104 000 are rejected (48 000 < cap < 104 000)
//! — and `MAX_MCP_OUTPUT_TOKENS` does not raise it; a call longer than 120 s
//! goes to the background.
//! Both are read-only and deterministic; neither is part of the product surface.

use std::time::Duration;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, schemars, tool, tool_router};
use serde::Deserialize;

use crate::server::SpecEngineServer;

/// Bytes per token of the conventional rough estimate; `probe_output` sizes
/// its ASCII filler in these units, so bytes equal characters (the cap itself
/// counts characters: module docs).
const BYTES_PER_TOKEN: usize = 4;

/// The filler unit: one estimated token.
const UNIT: &str = "tok ";

/// Units per line; the last unit of a line ends in `\n` instead of a space.
const UNITS_PER_LINE: usize = 16;

/// Upper bound of `probe_output` (≈ 800 KB of text).
const MAX_TOKENS: u32 = 200_000;

/// Upper bound of `probe_sleep`.
const MAX_SECONDS: u32 = 900;

const _: () = assert!(UNIT.len() == BYTES_PER_TOKEN);

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OutputArgs {
    /// Estimated tokens to return (4 bytes each), at most 200000.
    pub tokens: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SleepArgs {
    /// Seconds to wait before returning, at most 900.
    pub seconds: u32,
}

#[tool_router(router = probe_tools, vis = "pub(crate)")]
impl SpecEngineServer {
    /// Output-limit probe.
    #[tool(
        description = "Measurement probe (owner checklist only): return filler text of exactly \
tokens x 4 bytes (\"tok \" repeated, 16 per line), i.e. about `tokens` tokens by the \
4-bytes-per-token estimate. Read-only. Deterministic; no LLM inside.",
        annotations(
            title = "Probe: large output",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn probe_output(
        &self,
        Parameters(args): Parameters<OutputArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if args.tokens > MAX_TOKENS {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "probe_output: tokens {} exceeds the limit {MAX_TOKENS}.",
                args.tokens
            ))]));
        }
        Ok(CallToolResult::success(vec![ContentBlock::text(filler(
            args.tokens as usize,
        ))]))
    }

    /// Backgrounding probe.
    #[tool(
        description = "Measurement probe (owner checklist only): wait `seconds` seconds, then \
return. Checks whether a long MCP call goes to the background. Read-only. \
Deterministic; no LLM inside.",
        annotations(
            title = "Probe: slow call",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn probe_sleep(
        &self,
        Parameters(args): Parameters<SleepArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if args.seconds > MAX_SECONDS {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "probe_sleep: seconds {} exceeds the limit {MAX_SECONDS}.",
                args.seconds
            ))]));
        }
        tokio::select! {
            () = tokio::time::sleep(Duration::from_secs(u64::from(args.seconds))) => {}
            () = context.ct.cancelled() => {
                return Err(ErrorData::internal_error("probe_sleep: cancelled by the client", None));
            }
        }
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "probe_sleep: slept {} s.",
            args.seconds
        ))]))
    }
}

/// `tokens` units of [`UNIT`], exactly `tokens * BYTES_PER_TOKEN` bytes.
fn filler(tokens: usize) -> String {
    let mut text = String::with_capacity(tokens * BYTES_PER_TOKEN);
    for index in 1..=tokens {
        if index % UNITS_PER_LINE == 0 || index == tokens {
            text.push_str("tok\n");
        } else {
            text.push_str(UNIT);
        }
    }
    text
}
