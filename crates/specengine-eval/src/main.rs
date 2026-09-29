//! `specengine-eval`: the permanent measurement harness (contract:
//! `crates/specengine-eval/README.md`).
//!
//! One subcommand per measurement over a corpus whose path enters at run time.
//! The corpus is read-only; stdout carries exactly one JSON envelope of
//! aggregates (never a file name, path or ID); stderr carries a human summary.
//! Exit 0 = measured (a timed-out measurement is the string `"timeout"` in
//! `result`), 2 = refused before anything was written, 1 = internal failure.

mod ast_hash;
mod bevy;
mod census;
mod harness;
mod parse;
#[cfg(all(feature = "ra", unix))]
mod ra;
mod ron;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;

use crate::harness::{Corpus, HarnessError, Outcome};

#[derive(Parser)]
#[command(
    name = "specengine-eval",
    version,
    about = "Measurement harness over pilot corpora: read-only, JSON aggregates on stdout"
)]
struct Cli {
    #[command(subcommand)]
    measurement: Measurement,
}

#[derive(Subcommand)]
enum Measurement {
    /// Stability of the normalized AST hash under rustfmt and comment stripping (05 §5.2).
    AstHash(CommonArgs),
    /// `.ron` marker extraction by the own lexer (05 §5.3; verdict `lexer`, 05 §9).
    Ron(CommonArgs),
    /// Dry-run census of a spec corpus by `specengine-import`; the convention
    /// comes from `--config` (default: `census.toml` at the corpus root) (08 §4.3).
    Census(CommonArgs),
    /// The spec parser of `specengine-core` over the census's documents:
    /// front-matter, `{#ID}` sections, references, token estimates. Files
    /// from `--config` as `census`; IDs from `--scheme`.
    Parse(ParseArgs),
    /// Syntactic Bevy registration detector; with `--dump`, compared against a
    /// `bevy_dev_tools::schedule_data` dump (`app_data.ron`) (05 §5.1).
    #[command(alias = "bevy")]
    BevyDetector(BevyArgs),
    /// rust-analyzer as a library (`ra_ap_*` 0.0.352), each load with and
    /// without the proc-macro server in a worker process of its own: cold
    /// load, peak RSS, warm re-analysis, share of items with a moniker
    /// (layer C, 05 §5.1). `--timeout` is the budget of each load.
    #[cfg(all(feature = "ra", unix))]
    Ra(RaArgs),
    /// One load of `ra`, started by `ra` itself.
    #[cfg(all(feature = "ra", unix))]
    #[command(hide = true)]
    RaWorker(ra::WorkerArgs),
}

/// `ra`: the shared arguments plus where cargo may write and which loads to run.
#[cfg(all(feature = "ra", unix))]
#[derive(Args, Clone)]
pub struct RaArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    /// Target directory of every cargo invocation of the loads (build
    /// scripts, proc macros, `.rustc_info.json`); default
    /// `<out>/ra/<label>/target`. Refused (exit 2) under the corpus.
    #[arg(long, value_name = "DIR")]
    pub cargo_target_dir: Option<PathBuf>,
    /// Loads to measure: without the proc-macro server (no build scripts),
    /// with it (build scripts into the target directory), or both.
    #[arg(long, value_enum, default_value_t = ra::Modes::Both)]
    pub proc_macros: ra::Modes,
}

/// `parse`: the shared arguments plus the ID scheme.
#[derive(Args, Clone)]
pub struct ParseArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    /// `specengine.toml` whose `[ids]` table is the ID scheme; read-only.
    /// Default: `SPECENGINE_SCHEME_A` / `_B` for `--label pilot-a` /
    /// `pilot-b` when set, else `specengine.toml` at the corpus root.
    /// Missing or invalid: refused (exit 2) with `file:line: message`.
    #[arg(long, value_name = "TOML")]
    pub scheme: Option<PathBuf>,
}

/// `bevy-detector`: the shared arguments plus the optional schedule dump.
#[derive(Args, Clone)]
pub struct BevyArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    /// `app_data.ron` written by `SerializeSchedulesPlugin` of an instrumented
    /// build; read-only. Unreadable or not a Bevy 0.19 dump: refused (exit 2).
    #[arg(long, value_name = "RON")]
    pub dump: Option<PathBuf>,
}

/// Arguments shared by every measurement: the CLI contract of the crate README.
#[derive(Args, Clone)]
pub struct CommonArgs {
    /// Corpus root, read-only. Without it: `SPECENGINE_PILOT_A` / `_B` when
    /// `--label` is `pilot-a` / `pilot-b`, otherwise the measurement's fixture.
    #[arg(long, value_name = "DIR")]
    pub pilot: Option<PathBuf>,
    /// Scratch directory for per-file detail; refused (exit 2) when it lies under the corpus.
    #[arg(long, value_name = "DIR")]
    pub out: PathBuf,
    /// Corpus label in the JSON envelope: `pilot-a`, `pilot-b` or `fixtures`.
    /// It names the detail directory under `--out`, so anything but one plain
    /// path component (empty, `.`, `..`, a separator, absolute) is refused (exit 2).
    #[arg(long, value_name = "LABEL")]
    pub label: Option<String>,
    /// Measurement-specific configuration (TOML): the corpus convention of
    /// `census` and `parse`; `ast-hash`, `ron`, `bevy-detector` and `ra` take none.
    #[arg(long, value_name = "TOML")]
    pub config: Option<PathBuf>,
    /// Wall-clock budget in seconds; on overrun `result` is the string "timeout" (exit 0).
    /// `ra`: the budget of each load; an overrun makes that load's unreported fields "timeout".
    #[arg(long, value_name = "SECONDS", default_value_t = 600)]
    pub timeout: u64,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.measurement {
        Measurement::AstHash(args) => measure("ast-hash", &args, ast_hash::FIXTURE, ast_hash::run),
        Measurement::Ron(args) => measure("ron", &args, ron::FIXTURE, ron::run),
        Measurement::Census(args) => {
            let config = args.config.clone();
            measure_with(
                "census",
                &args,
                census::FIXTURE,
                move |root: &Path| census::load_config(config.as_deref(), root),
                census::run,
            )
        }
        Measurement::Parse(args) => {
            let config = args.common.config.clone();
            let scheme = args.scheme.clone();
            let label = args.common.label.clone();
            measure_with(
                "parse",
                &args.common,
                parse::FIXTURE,
                move |root: &Path| {
                    parse::prepare(root, config.as_deref(), scheme.as_deref(), label.as_deref())
                },
                parse::run,
            )
        }
        Measurement::BevyDetector(args) => {
            let dump = args.dump.clone();
            measure_with(
                "bevy-detector",
                &args.common,
                bevy::FIXTURE,
                move |_: &Path| bevy::load_dump(dump.as_deref()),
                bevy::run,
            )
        }
        #[cfg(all(feature = "ra", unix))]
        Measurement::Ra(args) => {
            let target_dir = args.cargo_target_dir.clone();
            let modes = args.proc_macros;
            let per_load = args.common.timeout;
            // The harness budget covers every load; each load is bounded by
            // `--timeout` inside `ra::run`, which kills its processes.
            let mut outer = args.common.clone();
            outer.timeout = ra::outer_budget(per_load, modes);
            measure_with(
                "ra",
                &outer,
                ra::FIXTURE,
                move |root: &Path| ra::prepare(root, target_dir.as_deref(), per_load, modes),
                move |corpus, setup| ra::run(corpus, setup, modes, per_load),
            )
        }
        #[cfg(all(feature = "ra", unix))]
        Measurement::RaWorker(args) => ra::worker_main(args),
    }
}

/// The one JSON object on stdout.
#[derive(Serialize)]
struct Envelope {
    measurement: &'static str,
    label: String,
    versions: Versions,
    wall_ms: u128,
    result: Value,
}

#[derive(Serialize)]
struct Versions {
    #[serde(rename = "tree-sitter")]
    tree_sitter: &'static str,
    #[serde(rename = "tree-sitter-rust")]
    tree_sitter_rust: &'static str,
    abi: usize,
}

/// Resolves the corpus, runs `run` under the budget and prints the envelope.
fn measure<R>(
    name: &'static str,
    args: &CommonArgs,
    fixture: &str,
    run: fn(&Corpus) -> Result<R, String>,
) -> ExitCode
where
    R: Serialize + Send + 'static,
{
    measure_with(
        name,
        args,
        fixture,
        |_| Ok(()),
        move |corpus, ()| run(corpus),
    )
}

/// [`measure`] with a `setup` over the corpus root that runs before anything
/// is written (a refusal on error, exit 2) and hands its value to `run`.
fn measure_with<C, R>(
    name: &'static str,
    args: &CommonArgs,
    fixture: &str,
    setup: impl FnOnce(&Path) -> Result<C, String>,
    run: impl FnOnce(&Corpus, C) -> Result<R, String> + Send + 'static,
) -> ExitCode
where
    C: Send + 'static,
    R: Serialize + Send + 'static,
{
    let (corpus, prepared) = match harness::prepare_with(args, fixture, setup) {
        Ok(prepared) => prepared,
        Err(HarnessError::Refused(message)) => {
            eprintln!("{name}: refused: {message}");
            return ExitCode::from(2);
        }
        Err(HarnessError::Internal(message)) => {
            eprintln!("{name}: internal failure: {message}");
            return ExitCode::from(1);
        }
    };
    let label = corpus.label.clone();
    let started = Instant::now();
    let outcome = harness::run_with_timeout(Duration::from_secs(args.timeout), move || {
        run(&corpus, prepared)
    });
    let result = match outcome {
        Outcome::Finished(Ok(result)) => match serde_json::to_value(result) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("{name}: internal failure: {error}");
                return ExitCode::from(1);
            }
        },
        Outcome::Finished(Err(message)) => {
            eprintln!("{name}: internal failure: {message}");
            return ExitCode::from(1);
        }
        Outcome::TimedOut => {
            eprintln!("{name}: timed out after {} s", args.timeout);
            Value::String("timeout".to_owned())
        }
        Outcome::Panicked => {
            eprintln!("{name}: internal failure: the measurement panicked");
            return ExitCode::from(1);
        }
    };
    let grammar = specengine_code::grammar_info();
    let envelope = Envelope {
        measurement: name,
        label,
        versions: Versions {
            tree_sitter: grammar.tree_sitter,
            tree_sitter_rust: grammar.tree_sitter_rust,
            abi: grammar.abi,
        },
        wall_ms: started.elapsed().as_millis(),
        result,
    };
    match serde_json::to_string(&envelope) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{name}: internal failure: {error}");
            ExitCode::from(1)
        }
    }
}
