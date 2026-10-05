//! `spec`: parses the arguments, runs one command of the library and prints
//! its answer. stdout carries results only (with `--json`, one document for
//! exit 0 and 1, nothing for exit 2 but `spec check`'s cannot-check
//! report); stderr carries `note:`, `warning:` and error lines. No colour,
//! no timing.
//!
//! The one check that stays here: `spec approve`, `spec reject` and `spec
//! import-state` run only when stdin is a terminal (else exit 2, nothing
//! read or logged, `import-state`'s file not opened); the
//! library asks its question through a callback, which prints it on stderr
//! as `... [y/N] ` and reads one line: only `y` or `yes` consents. The clock
//! (`now`, UTC) and the process's variables for git are passed in as well.

use std::io::{BufRead as _, IsTerminal as _, Read as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{ColorChoice, CommandFactory as _, Parser, Subcommand};
use specengine_cli::{
    ApproveRequest, BundleRequest, CheckRequest, CheckedTree, CliError, Env, Exit,
    ExportIndexRequest, ExportStateRequest, Globals, GraphRequest, ImportStateRequest,
    InboxRequest, IndexRequest, InitRequest, Outcome, ProposeRequest, ProposedText, RejectRequest,
    ReviewRequest, SearchRequest, ShowRequest, TEXT_MAX_BYTES, TreeRequest, render_json,
    render_text,
};
use specengine_core::proposal::utc_timestamp;
use specengine_store::GitEnv;

#[derive(Parser)]
#[command(
    name = "spec",
    version,
    about = "SpecEngine: find and read a project's spec nodes by ID, over an always-fresh index; check its documents; propose changes and apply them as commits",
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
        /// Also list each node's links, outgoing and incoming, before its text.
        #[arg(long)]
        links: bool,
        /// With --links: links written in Tier 3 (archived) documents too.
        #[arg(long)]
        archive: bool,
    },
    /// Print the containment tree (`parent:` and section nesting) from ROOT, or from the roots under `[paths] spec`.
    Tree {
        // Not named `root`: that argument id is the global --root.
        /// Any form `show` takes.
        #[arg(value_name = "ROOT")]
        start: Option<String>,
        /// Levels below the roots at most (0: the roots only).
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        depth: Option<i64>,
        /// Keep only the lines of this kind (repeatable).
        #[arg(long = "kind", value_name = "K")]
        kinds: Vec<String>,
        /// Include Tier 3 (archived) documents.
        #[arg(long)]
        archive: bool,
    },
    /// Walk the typed links from REF: what it depends on, or with --impact what an edit of it reaches.
    Graph {
        /// Any form `show` takes.
        #[arg(value_name = "REF")]
        reference: String,
        /// Follow the impact table: depends_on, derived_from, verifies, uses_term back, constrains forward.
        #[arg(long)]
        impact: bool,
        /// Follow only this link type (repeatable): outgoing; with --impact in its table direction, else incoming.
        #[arg(long = "type", value_name = "T")]
        types: Vec<String>,
        /// Distance from REF at most; default unbounded.
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        depth: Option<i64>,
        /// Include links written in Tier 3 (archived) documents.
        #[arg(long)]
        archive: bool,
    },
    /// Assemble the context of REF within a budget of estimated tokens: the targets, open questions, ancestors, criteria, decisions, neighbours, terms; the rest named by ID.
    Bundle {
        /// Any form `show` takes; several are bundled together.
        #[arg(required = true, num_args = 1.., value_name = "REF")]
        references: Vec<String>,
        /// Estimated tokens at most (default: `[budgets] bundle_node`, else 2000).
        #[arg(long, value_name = "N", allow_negative_numbers = true)]
        budget: Option<i64>,
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
    /// Export generated output (`index`: the registered index generator's document; `state`: a backup of the proposal queue).
    Export {
        #[command(subcommand)]
        what: Export,
    },
    /// Propose a change to one node; no file is touched until the owner approves it.
    Propose {
        #[command(subcommand)]
        kind: Propose,
    },
    /// List the current repository's open and approved proposals, by ID.
    Inbox {
        /// Every state, applied and rejected included.
        #[arg(long)]
        all: bool,
    },
    /// Print one proposal: its target, place, author, rationale, diff, and what approving it now would do (apply it, or complete it by its own commit already on its branch).
    Review {
        #[arg(value_name = "PR")]
        id: String,
    },
    /// Apply an open proposal in its worktree as one commit, or complete an open or approved one whose own commit is already on its branch, with no new commit; asks for consent on the terminal (completing an approved one does not ask).
    Approve {
        #[arg(value_name = "PR")]
        id: String,
        /// A note kept with the decision.
        #[arg(long, value_name = "T")]
        note: Option<String>,
    },
    /// Reject an open or approved proposal, never one whose `Proposal:` commit is on its branch; asks for consent on the terminal.
    Reject {
        #[arg(value_name = "PR")]
        id: String,
        /// Why the proposal is rejected (non-empty).
        #[arg(long, value_name = "T")]
        reason: String,
    },
    /// Restore a dump written by `export state` into the project's empty proposal queue, rows as stored; asks for consent on the terminal.
    ImportState {
        /// The dump, relative to the current directory.
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

#[derive(Subcommand)]
enum Propose {
    /// Replace the text of node ID (a section's heading to its end, or a document's whole file).
    Update {
        /// An ID or `slug/ID`.
        #[arg(value_name = "ID")]
        target: String,
        /// The span hash the text was written against (`spec show`: `span b3:…`, JSON `span_hash`).
        #[arg(long, value_name = "HASH")]
        base: String,
        /// The new text: a file, or `-` for stdin (UTF-8, at most 1 MiB).
        #[arg(long = "text-file", value_name = "F")]
        text_file: PathBuf,
        /// Why; the commit's body when applied.
        #[arg(long, value_name = "T")]
        rationale: String,
        /// The proposing agent's role (any of the three: an agent's proposal).
        #[arg(long = "author-role", value_name = "R")]
        author_role: Option<String>,
        /// The proposing agent's model.
        #[arg(long = "author-model", value_name = "M")]
        author_model: Option<String>,
        /// The proposing agent's run.
        #[arg(long, value_name = "ID")]
        run: Option<String>,
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
    /// Write the project's whole proposal queue (every row of `proposals` and `events`) as a JSONL dump outside the repository, to restore with `import-state`.
    State {
        /// The new file to write, relative to the current directory (default: `<data dir>/backups/<slug>-<UTC time>.jsonl`).
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
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
        Command::Show {
            reference,
            links,
            archive,
        } => Outcome::Show(specengine_cli::show(
            env,
            globals,
            &ShowRequest {
                reference,
                links,
                archive,
            },
        )?),
        Command::Tree {
            start,
            depth,
            kinds,
            archive,
        } => Outcome::Tree(specengine_cli::tree(
            env,
            globals,
            &TreeRequest {
                root: start,
                depth,
                kinds,
                archive,
            },
        )?),
        Command::Graph {
            reference,
            impact,
            types,
            depth,
            archive,
        } => Outcome::Graph(specengine_cli::graph(
            env,
            globals,
            &GraphRequest {
                reference,
                impact,
                types,
                depth,
                archive,
            },
        )?),
        Command::Bundle { references, budget } => Outcome::Bundle(specengine_cli::bundle(
            env,
            globals,
            &BundleRequest { references, budget },
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
        Command::Export {
            what: Export::State { out },
        } => Outcome::StateExport(specengine_cli::export_state(
            env,
            globals,
            &ExportStateRequest {
                out,
                now: now(),
                git: process_git(env),
            },
        )?),
        Command::Propose {
            kind:
                Propose::Update {
                    target,
                    base,
                    text_file,
                    rationale,
                    author_role,
                    author_model,
                    run,
                },
        } => {
            let text = if text_file.as_os_str() == "-" {
                ProposedText::Given(read_stdin_text()?)
            } else {
                ProposedText::File(text_file)
            };
            Outcome::Proposal(Box::new(specengine_cli::propose(
                env,
                globals,
                &ProposeRequest {
                    target,
                    base,
                    text,
                    rationale,
                    author_role,
                    author_model,
                    run,
                    now: now(),
                    git: process_git(env),
                },
            )?))
        }
        Command::Inbox { all } => Outcome::Inbox(specengine_cli::inbox(
            env,
            globals,
            &InboxRequest {
                all,
                git: process_git(env),
            },
        )?),
        Command::Review { id } => Outcome::Proposal(Box::new(specengine_cli::review(
            env,
            globals,
            &ReviewRequest {
                id,
                git: process_git(env),
            },
        )?)),
        Command::Approve { id, note } => {
            require_terminal("approve")?;
            let mut consent = ask;
            Outcome::Proposal(Box::new(specengine_cli::approve(
                env,
                globals,
                &ApproveRequest {
                    id,
                    note,
                    now: now(),
                    git: process_git(env),
                },
                &mut consent,
            )?))
        }
        Command::Reject { id, reason } => {
            require_terminal("reject")?;
            let mut consent = ask;
            Outcome::Proposal(Box::new(specengine_cli::reject(
                env,
                globals,
                &RejectRequest {
                    id,
                    reason,
                    now: now(),
                    git: process_git(env),
                },
                &mut consent,
            )?))
        }
        Command::ImportState { file } => {
            require_terminal("import-state")?;
            let mut consent = ask;
            Outcome::StateImport(specengine_cli::import_state(
                env,
                globals,
                &ImportStateRequest { file },
                &mut consent,
            )?)
        }
    })
}

/// The process's directory and variables, for git (the library drops the
/// local `GIT_*` ones before running it in a proposal's worktree).
fn process_git(env: &Env) -> GitEnv {
    GitEnv::new(env.cwd.clone(), std::env::vars_os())
}

/// The clock: now, UTC, `YYYY-MM-DDTHH:MM:SSZ`.
fn now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        });
    utc_timestamp(seconds)
}

/// `--text-file -`: stdin, at most one byte more than the library takes.
fn read_stdin_text() -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take(TEXT_MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            CliError::cannot(format!("spec: cannot read the text from stdin: {error}"))
        })?;
    Ok(bytes)
}

/// `spec approve`, `spec reject` and `spec import-state` decide on a
/// terminal only: a stdin that is no terminal (a pipe, an agent's shell)
/// exits 2 before anything is read or logged.
fn require_terminal(command: &str) -> Result<(), CliError> {
    if std::io::stdin().is_terminal() {
        return Ok(());
    }
    Err(CliError::cannot(format!(
        "spec: `spec {command}` asks the owner for consent on a terminal, and stdin is not one \
         (a pipe, a script or an agent's shell): run it in a terminal; nothing changed"
    )))
}

/// The consent prompt: `question` on stderr, one answer line from the
/// terminal; only `y` or `yes` consents.
fn ask(question: &str) -> bool {
    let mut stderr = std::io::stderr().lock();
    let _ = write!(stderr, "{question} ");
    let _ = stderr.flush();
    drop(stderr);
    let mut answer = String::new();
    match std::io::stdin().lock().read_line(&mut answer) {
        Ok(_) => matches!(answer.trim(), "y" | "yes"),
        Err(_) => false,
    }
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
