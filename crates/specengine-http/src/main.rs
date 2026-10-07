//! `specengine-http --root DIR… [--port N]`: the daemon's read surface
//! (task specs `daemon-read`, `ui-live`: `graph` and the plain `check`), a
//! read-only HTTP adapter over the CLI library, in the foreground, on
//! `127.0.0.1` only.
//!
//! Start (docs/features/daemon-read.md "Description and interactions"):
//! each `--root` canonicalised and its own `specengine.toml` read, a slug
//! required, two roots of one slug refused; nothing is opened in the data
//! directory; then the bind. Stdout: `serving <slug> <root>` per root, in
//! `--root` order, then `listening http://127.0.0.1:<port>`. A start that
//! fails exits 2 before listening with one `specengine-http: <reason>` on
//! stderr. SIGINT and SIGTERM end the process (every write a read makes is
//! one SQLite transaction).
//!
//! - [`start`]: the arguments' projects;
//! - [`app`]: the router, the fence, the read endpoints, the refused
//!   decision;
//! - [`answer`]: one CLI library call per request on the blocking pool,
//!   serialized per project, and its response (a check's report a 200
//!   whatever its verdict);
//! - [`args`]: the query string and the path's REF;
//! - [`tail`]: the live tail of the queue's `events` (SSE).

mod answer;
mod app;
mod args;
mod start;
mod tail;

use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use specengine_cli::{Env, Exit};

#[derive(Parser)]
#[command(
    name = "specengine-http",
    version,
    about = "SpecEngine's read surface over HTTP on 127.0.0.1: the CLI's documents and the queue's live events"
)]
struct Cli {
    /// A project root to serve: its own specengine.toml, a [project] slug
    /// required. Repeat it to serve more; listed in this order.
    #[arg(long, value_name = "DIR", required = true)]
    root: Vec<PathBuf>,
    /// The port on 127.0.0.1; 0 takes a free one.
    #[arg(long, value_name = "N", default_value_t = 7777)]
    port: u16,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return usage_error(&error),
    };
    let env = match Env::from_process() {
        Ok(env) => env,
        Err(error) => return cannot_start(&error.message),
    };
    let projects = match start::projects(&env, &cli.root) {
        Ok(projects) => projects,
        Err(reason) => return cannot_start(&reason),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return cannot_start(&format!("cannot start the async runtime: {error}")),
    };
    runtime.block_on(serve(projects, cli.port))
}

/// Binds `127.0.0.1:<port>`, prints the start lines and serves until the
/// process is ended.
async fn serve(projects: Vec<start::Project>, port: u16) -> ExitCode {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => return cannot_start(&format!("cannot listen on {address}: {error}")),
    };
    let port = match listener.local_addr() {
        Ok(local) => local.port(),
        Err(error) => return cannot_start(&format!("cannot read the bound address: {error}")),
    };
    let mut lines = String::new();
    for project in &projects {
        lines.push_str(&format!(
            "serving {} {}\n",
            project.slug,
            project.root.display()
        ));
    }
    lines.push_str(&format!("listening http://127.0.0.1:{port}\n"));
    {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(lines.as_bytes());
        let _ = stdout.flush();
    }
    let state = Arc::new(app::App::new(projects, port));
    match axum::serve(listener, app::router(state)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "specengine-http: the server stopped: {error}"
            );
            ExitCode::FAILURE
        }
    }
}

/// Exit 2 before listening: `specengine-http: <reason>` on stderr.
fn cannot_start(reason: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr().lock(), "specengine-http: {reason}");
    ExitCode::from(Exit::CannotRun.code())
}

/// Help and version go to stdout, exit 0; any other parse error (no
/// `--root`, an unknown flag such as `--host`) exits 2 on stderr.
fn usage_error(error: &clap::Error) -> ExitCode {
    if !error.use_stderr() {
        let _ = error.print();
        return ExitCode::SUCCESS;
    }
    let rendered = error.render().to_string();
    let message = rendered.strip_prefix("error: ").unwrap_or(&rendered);
    cannot_start(message.trim_end())
}
