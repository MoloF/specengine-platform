//! `spec`: parses the arguments, runs one command of the library and prints
//! its answer. stdout carries results only (with `--json`, one document for
//! exit 0 and 1, nothing for exit 2 but `spec check`'s cannot-check
//! report); stderr carries `note:`, `warning:` and error lines. No colour,
//! no timing.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{ColorChoice, CommandFactory as _, Parser, Subcommand};
use specengine_cli::{
    CheckRequest, CheckedTree, CliError, Env, Exit, ExportIndexRequest, Globals, IndexRequest,
    InitRequest, Outcome, SearchRequest, ShowRequest, render_json, render_text,
};
use specengine_store::GitEnv;

#[derive(Parser)]
#[command(
    name = "spec",
    version,
    about = "SpecEngine: find and read a project's spec nodes by ID, over an always-fresh index; check its documents",
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
    /// One JSON document on stdout (exit 0 and 1; `check`: every verdict).
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
    /// Check the documents against the convention; exit 0 clean or observed, 1 blocked, 2 cannot check.
    Check {
        /// Check what `git commit` would record: the git index, config and baseline from it unless --config, --baseline; judged against HEAD.
        #[arg(long)]
        staged: bool,
        /// Check the working tree as a plain check does, config and baseline from disk; judged against HEAD.
        #[arg(long, conflicts_with = "staged")]
        changed: bool,
        /// The debt baseline to use instead of the root's .spec-debt.toml (relative to the current directory).
        #[arg(long, value_name = "F")]
        baseline: Option<PathBuf>,
        /// List every finding, stale and new debt entry, not only the blocking ones.
        #[arg(long)]
        debt: bool,
    },
    /// Export generated output (`index`: the registered index generator's document).
    Export {
        #[command(subcommand)]
        what: Export,
    },
}

#[derive(Subcommand)]
enum Export {
    /// Render the index of the `[[generators]]` entry with `index = true` and write it to `[paths] index`.
    Index {
        /// Print the render instead of writing it.
        #[arg(long, conflicts_with = "json")]
        stdout: bool,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if is_stdout_json_conflict(&error) => {
            return usage_error(&stdout_json_conflict());
        }
        Err(error) => return usage_error(&error),
    };
    // `conflicts_with` sees `--json` only after `index`: clap fills global
    // arguments in after the subcommand's conflict check.
    if cli.json
        && matches!(
            cli.command,
            Command::Export {
                what: Export::Index { stdout: true }
            }
        )
    {
        return usage_error(&stdout_json_conflict());
    }
    let globals = Globals {
        root: cli.root,
        config: cli.config,
    };
    let result = Env::from_process().and_then(|env| run(&env, &globals, cli.command, cli.json));
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

fn run(env: &Env, globals: &Globals, command: Command, json: bool) -> Result<Outcome, CliError> {
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
        Command::Check {
            staged,
            changed,
            baseline,
            debt,
        } => {
            // Git's children see the process's variables; relative `GIT_*`
            // paths resolve against the current directory.
            let git = || GitEnv::new(env.cwd.clone(), std::env::vars_os());
            // clap refuses `--staged` with `--changed`.
            let tree = if staged {
                CheckedTree::Staged(git())
            } else if changed {
                CheckedTree::Changed(git())
            } else {
                CheckedTree::WorkingTree
            };
            Outcome::Check(specengine_cli::check(
                env,
                globals,
                &CheckRequest {
                    tree,
                    baseline,
                    debt,
                    json,
                },
            )?)
        }
        Command::Export {
            what: Export::Index { stdout },
        } => Outcome::Export(specengine_cli::export_index(
            env,
            globals,
            &ExportIndexRequest { stdout },
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

/// `export index --stdout` with `--json`, in any order and whatever else is
/// passed: one usage error, the same text, its usage block `export index`'s.
fn stdout_json_conflict() -> clap::Error {
    const MESSAGE: &str = "the argument '--stdout' cannot be used with '--json'";
    let mut command = Cli::command();
    command.build();
    let index = command
        .find_subcommand_mut("export")
        .and_then(|export| export.find_subcommand_mut("index"));
    match index {
        Some(index) => index.error(ErrorKind::ArgumentConflict, MESSAGE),
        None => Cli::command().error(ErrorKind::ArgumentConflict, MESSAGE),
    }
}

/// clap's own conflict of `--stdout` and `--json` (either named first).
fn is_stdout_json_conflict(error: &clap::Error) -> bool {
    if error.kind() != ErrorKind::ArgumentConflict {
        return false;
    }
    let names = |kind| match error.get(kind) {
        Some(ContextValue::String(name)) => vec![name.as_str()],
        Some(ContextValue::Strings(names)) => names.iter().map(String::as_str).collect(),
        _ => Vec::new(),
    };
    let mut named = names(ContextKind::InvalidArg);
    named.extend(names(ContextKind::PriorArg));
    let has = |flag: &str| {
        named
            .iter()
            .any(|name| name.split_whitespace().next() == Some(flag))
    };
    has("--stdout") && has("--json")
}
