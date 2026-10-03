//! MCP server of SpecEngine over stdio (`crates/specengine-mcp/README.md`;
//! task spec `mcp-read`; 07 §1.1–1.3; 04 §3).
//!
//! One process serves one client over stdin/stdout in either protocol era, chosen
//! by the client's first message (rmcp 3.5.0 `serve_server`):
//!
//! - **legacy** — `initialize` opens a session;
//! - **stateless** (2026-07-28) — every request carries `_meta` with the protocol
//!   version and client capabilities; list and read results carry `ttlMs: 0`.
//!
//! Tools: the four reads `get_tree`, `get_node`, `search`,
//! `get_context_bundle`, each one call into the CLI library (`spec tree`,
//! `show`, `search`, `bundle`), answering what the CLI answers; resources
//! `spec://<slug>/tree` and `spec://<slug>/node/<id>` over the same calls.
//! Behind feature `probes`, the measurement build: the Phase 0 consent demo
//! `review_proposal` (`_meta["anthropic/requiresUserInteraction"]: true`,
//! a form through `elicitation/create` or a multi round-trip request) and
//! `probe_output`, `probe_sleep` for the owner checklist.
//!
//! Reads refresh the project's index in SpecEngine's data directory, as the
//! CLI's reads do; nothing under the project root is ever written: spec
//! files change only through `apply_proposal` on an owner action (ADR-0004,
//! ADR-0005). Stdout is the protocol channel: nothing else is ever printed
//! there.

mod mirror;
#[cfg(feature = "probes")]
mod probes;
mod read;
mod resources;
#[cfg(feature = "probes")]
mod review;
mod server;

pub use read::MAX_RESULT_CHARS;
#[cfg(feature = "probes")]
pub use review::{Decision, Era, FormAction, ReviewOutcome};
pub use server::{INSTRUCTIONS, Lifecycle, ServeError, SpecEngineServer, serve_stdio};
