//! Repository chores for SpecEngine.
//!
//! For now this is only the enforcement of the documentation convention
//! (`docs/canon/documentation-system.md` §11). Once SpecEngine has its own
//! `spec check`, these checks move there (ADR-0013, ADR-0022).

mod docs;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: cargo xtask docs <command> [--root DIR]

  docs check            all §11 checks: budgets, front-matter, canon:, superseded-by,
                        index and generated documents have not drifted from their source
  docs index [--write]  build docs/index.md from front-matter (without --write: to stdout)
  docs budget           document sizes against caps and the working set W (§3–§4)
  --root DIR            run over DIR instead of this repository (the parity test of
                        `spec check`, docs/features/spec-check.md AC-20)";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut root = repo_root();
    if let Some(at) = args.iter().position(|arg| arg == "--root") {
        match args.get(at + 1) {
            Some(dir) => {
                root = PathBuf::from(dir);
                args.drain(at..=at + 1);
            }
            None => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let result = match args.as_slice() {
        ["docs", "check"] => docs::check::run(&root),
        ["docs", "index"] => docs::index::run(&root, false),
        ["docs", "index", "--write"] => docs::index::run(&root, true),
        ["docs", "budget"] => docs::budget::run(&root),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(err) => {
            eprintln!("xtask: {err}");
            ExitCode::from(2)
        }
    }
}

/// Repository root: the parent of the `xtask` crate directory.
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(PathBuf::from).unwrap_or(manifest)
}
