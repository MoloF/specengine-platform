//! MCP server of SpecEngine over stdio (`docs/features/phase-0-spikes.md`, group 4
//! `mcp-stdio`; 07 §1.1–1.2; 04 §3–4).
//!
//! One process serves one client over stdin/stdout in either protocol era, chosen
//! by the client's first message (rmcp 3.5.0 `serve_server`):
//!
//! - **legacy** — `initialize` opens a session; the consent form goes out as a
//!   server-initiated `elicitation/create` request;
//! - **stateless** (2026-07-28) — every request carries `_meta` with the protocol
//!   version and client capabilities; the consent form goes out as a multi
//!   round-trip request (`resultType: "input_required"` + an HMAC-sealed
//!   `requestState`), and the client repeats the call with the answer.
//!
//! Tools: `review_proposal` (the owner's consent tool,
//! `_meta["anthropic/requiresUserInteraction"]: true`); behind feature `probes`,
//! `probe_output` and `probe_sleep` for the owner checklist. No MCP tool ever
//! writes to disk: spec files change only through `apply_proposal` on an owner
//! action (ADR-0004, ADR-0005). Stdout is the protocol channel: nothing else is
//! ever printed there.

#[cfg(feature = "probes")]
mod probes;
mod review;
mod server;

pub use review::{Decision, Era, FormAction, ReviewOutcome};
pub use server::{INSTRUCTIONS, Lifecycle, ServeError, SpecEngineServer, serve_stdio};
