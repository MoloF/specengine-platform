//! `spec`: parses the arguments, runs one command of the library and prints
//! its answer. stdout carries results only (with `--json`, one document for
//! exit 0 and 1, nothing for exit 2); stderr carries `note:`, `warning:`
//! and error lines. No colour, no timing.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ColorChoice, Parser, Subcommand};
use specengine_cli::{
    CliError, Env, Exit, Globals, IndexRequest, InitRequest, Outcome, SearchRequest, ShowRequest,
    render_json, render_text,
};

#[derive(Parser)]
#[command(
    name = "spec",
    version,
    about = "SpecEngine: find and read a project's spec nodes by ID, over an always-fresh index",
    color = ColorChoice::Never
)]
struct Cli {
    /// The project root (no walk up from the current directory).
    #[arg(long, global = true, value_name = "DIR")]
    root: Option<PathBuf>,
    /// The config to read instead of `<root>/specengine.toml`; without
    /// --root the root is the current directory.
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,
    /// One JSON document on stdout (exit 0 and 1).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create specengine.toml with a `[project] slug` at the project root.
    Init {
        /// The slug to write; default: derived from the directory name.
        #[arg(long, value_name = "S")]
        slug: Option<String>,
    },
    /// Bring the project's index up to date with its files.
    Index {
        /// Re-parse every file.
        #[arg(long)]
        full: bool,
    },
    /// Full-text search over the spec nodes (terms of 3 or more characters).
    Search {
        #[arg(required = true, num_args = 1.., value_name = "QUERY")]
        query: Vec<String>,
        /// Keep only nodes of this kind (repeatable).
        #[arg(long = "kind", value_name = "K")]
        kinds: Vec<String>,
        /// Hits at most, 1 to 200 (default 20).
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        limit: Option<i64>,
        /// Include Tier 3 (archived) documents.
        #[arg(long)]
        archive: bool,
    },
    /// Print a node by ID, alias, `slug/ID`, `ID#SECTION` or a root-relative `.md` path.
    Show {
        #[arg(value_name = "REF")]
        reference: String,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return usage_error(&error),
    };
    let globals = Globals {
        root: cli.root,
        config: cli.config,
    };
    let result = Env::from_process().and_then(|env| run(&env, &globals, cli.command));
    match result {
        Ok(outcome) => {
            let mut stderr = std::io::stderr().lock();
            for line in outcome.stderr_lines() {
                let _ = writeln!(stderr, "{line}");
            }
            let stdout = if cli.json {
                render_json(&outcome)
            } else {
                render_text(&outcome)
            };
            // A closed stdout (`| head`) is no failure of the command.
            let _ = std::io::stdout().lock().write_all(stdout.as_bytes());
            ExitCode::from(outcome.exit().code())
        }
        Err(error) => {
            let _ = writeln!(std::io::stderr().lock(), "{error}");
            ExitCode::from(error.exit.code())
        }
    }
}

fn run(env: &Env, globals: &Globals, command: Command) -> Result<Outcome, CliError> {
    Ok(match command {
        Command::Init { slug } => {
            Outcome::Init(specengine_cli::init(env, globals, &InitRequest { slug })?)
        }
        Command::Index { full } => {
            Outcome::Index(specengine_cli::index(env, globals, &IndexRequest { full })?)
        }
        Command::Search {
            query,
            kinds,
            limit,
            archive,
        } => Outcome::Search(specengine_cli::search(
            env,
            globals,
            &SearchRequest {
                terms: query,
                kinds,
                limit,
                archive,
            },
        )?),
        Command::Show { reference } => Outcome::Show(specengine_cli::show(
            env,
            globals,
            &ShowRequest { reference },
        )?),
    })
}

/// Help and version go to stdout, exit 0; any other parse error is a usage
/// error on stderr, `spec: …`, exit 2.
fn usage_error(error: &clap::Error) -> ExitCode {
    if !error.use_stderr() {
        let _ = error.print();
        return ExitCode::SUCCESS;
    }
    let rendered = error.render().to_string();
    let message = rendered.strip_prefix("error: ").unwrap_or(&rendered);
    let _ = writeln!(std::io::stderr().lock(), "spec: {}", message.trim_end());
    ExitCode::from(Exit::CannotRun.code())
}
