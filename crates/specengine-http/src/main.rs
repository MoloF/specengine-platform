//! `specengine-http --root DIR… [--port N]`: the daemon's read surface
//! (task specs `daemon-read`, `ui-live`: `graph` and the plain `check`), an
//! HTTP adapter over the CLI library that reads, and writes only a
//! proposal's staged choice (task spec `decision-staging`), in the
//! foreground, on `127.0.0.1` only.
//!
//! Start (docs/features/daemon-read.md "Description and interactions";
//! docs/features/spec-serve.md for the data directory): each `--root`
//! canonicalised and its own `specengine.toml` read, a slug required, two
//! roots of one slug refused; the data directory refused inside any root
//! (nothing created), else created; its `daemon.lock` locked exclusively
//! (one daemon per data directory; held by another past 500 ms: exit 2
//! naming it); a hub's `proposal-floor` stored once under it (task spec
//! `apply-outputs`: created exclusively, never rewritten; an invalid one
//! exits 2); then the bind, and `daemon.json` written with the bound
//! port. Stdout: `serving <slug> <root>` per root, in `--root` order, a
//! hub's `public <origin>`, then `listening http://127.0.0.1:<port>`. A
//! `--public-origin` is judged before all of it, so a refusal creates
//! nothing. A start that fails exits 2 before listening with one
//! `specengine-http: <reason>` on stderr. SIGINT and
//! SIGTERM end the process (every write a read makes is one SQLite
//! transaction); any end frees the lock, the discovery file stays.
//!
//! - [`start`]: the arguments' projects and their data directory;
//! - [`daemon`]: the lock and the discovery file;
//! - [`app`]: the router, the fence, the read endpoints, the stage route
//!   (POST stages a choice, DELETE clears it; confirmed only on a
//!   terminal);
//! - [`answer`]: one CLI library call per request on the blocking pool,
//!   serialized per project, and its response (a check's report a 200
//!   whatever its verdict; a stage's status by its cause);
//! - [`args`]: the query string and the path's REF;
//! - [`tail`]: the live tail of the queue's `events` (SSE);
//! - [`ui`]: the web UI compiled in, `/` and `/assets/<file>` (task spec
//!   `ui-embed`);
//! - [`hub`]: `--public-origin`, a hub's fence and its one trusted login
//!   (task spec `hub-identity`); absent, a local daemon, nothing of it runs.

mod answer;
mod app;
mod args;
mod confirm;
mod daemon;
mod hub;
mod start;
mod tail;
mod ui;

use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use specengine_cli::{Env, Exit};

use crate::hub::Hub;

#[derive(Parser)]
#[command(
    name = "specengine-http",
    version,
    about = "SpecEngine over HTTP on 127.0.0.1: the CLI's documents and the queue's live events; it reads, and stages a choice confirmed on a terminal"
)]
struct Cli {
    /// A project root to serve: its own specengine.toml, a [project] slug
    /// required. Repeat it to serve more; listed in this order.
    #[arg(long, value_name = "DIR", required = true)]
    root: Vec<PathBuf>,
    /// The port on 127.0.0.1; 0 takes a free one.
    #[arg(long, value_name = "N", default_value_t = 7777)]
    port: u16,
    /// A hub's public origin, https://host[:port], served to named people
    /// through a proxy on this host. Absent: a local daemon.
    #[arg(long, value_name = "URL")]
    public_origin: Option<String>,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return usage_error(&error),
    };
    // Before anything is created: a refused origin leaves no data directory.
    let hub = match cli.public_origin.as_deref().map(Hub::parse).transpose() {
        Ok(hub) => hub,
        Err(reason) => return cannot_start(&reason),
    };
    let env = match Env::from_process() {
        Ok(env) => env,
        Err(error) => return cannot_start(&error.message),
    };
    let served = match start::served(&env, &cli.root) {
        Ok(served) => served,
        Err(reason) => return cannot_start(&reason),
    };
    let lock = match daemon::lock(&served.data_dir) {
        Ok(lock) => lock,
        Err(reason) => return cannot_start(&reason),
    };
    // A hub's own `PR-` range (task spec `apply-outputs`): its floor stored
    // once, under the lock and before listening; a local start never
    // writes or reads it.
    if hub.is_some()
        && let Err(reason) = specengine_cli::store_hub_floor(&served.data_dir)
    {
        return cannot_start(&reason);
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return cannot_start(&format!("cannot start the async runtime: {error}")),
    };
    runtime.block_on(serve(served.projects, cli.port, lock, hub))
}

/// Binds `127.0.0.1:<port>`, writes `daemon.json` beside the held `lock`,
/// prints the start lines and serves until the process is ended (the lock
/// held throughout). A hub binds the same address and keeps the same
/// `daemon.json`.
async fn serve(
    projects: Vec<start::Project>,
    port: u16,
    lock: daemon::Lock,
    hub: Option<Hub>,
) -> ExitCode {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => return cannot_start(&format!("cannot listen on {address}: {error}")),
    };
    let port = match listener.local_addr() {
        Ok(local) => local.port(),
        Err(error) => return cannot_start(&format!("cannot read the bound address: {error}")),
    };
    let url = format!("http://127.0.0.1:{port}");
    if let Err(reason) = daemon::publish(&lock, &projects, port, &url) {
        return cannot_start(&reason);
    }
    let mut lines = String::new();
    for project in &projects {
        lines.push_str(&format!(
            "serving {} {}\n",
            project.slug,
            project.root.display()
        ));
    }
    if let Some(hub) = &hub {
        lines.push_str(&format!("public {}\n", hub.origin()));
    }
    lines.push_str(&format!("listening {url}\n"));
    {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(lines.as_bytes());
        let _ = stdout.flush();
    }
    let state = Arc::new(app::App::new(projects, port, hub));
    let served = axum::serve(listener, app::router(state)).await;
    drop(lock);
    match served {
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
