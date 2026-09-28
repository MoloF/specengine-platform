//! `specengine-mcp`: the SpecEngine MCP server over stdio.
//!
//! Stdin/stdout carry JSON-RPC and nothing else; the only human text is one
//! line on stderr when the session fails. Exit 0 = the client closed the
//! session; 1 = the session did not start or the service failed.

use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use specengine_mcp::{Lifecycle, serve_stdio};

#[derive(Parser)]
#[command(
    name = "specengine-mcp",
    version,
    about = "SpecEngine MCP server over stdio (legacy `initialize` and stateless 2026-07-28)"
)]
struct Cli {
    /// Protocol eras served: `auto` = both (default); `legacy` = `initialize`
    /// sessions only (versions up to 2025-11-25), refusing 2026-07-28 requests.
    #[arg(long, value_enum, default_value_t = LifecycleArg::Auto)]
    lifecycle: LifecycleArg,
}

#[derive(Clone, Copy, ValueEnum)]
enum LifecycleArg {
    Auto,
    Legacy,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let lifecycle = match cli.lifecycle {
        LifecycleArg::Auto => Lifecycle::Auto,
        LifecycleArg::Legacy => Lifecycle::Legacy,
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("specengine-mcp: cannot start the async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(serve_stdio(lifecycle)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("specengine-mcp: {error}");
            ExitCode::FAILURE
        }
    }
}
