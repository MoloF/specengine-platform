//! rustfmt as a perturbation, on copies only: the source goes in through stdin
//! and comes back through `--emit stdout`. The corpus is never touched and
//! `mod` children are never followed (rustfmt cannot follow them from stdin).
//!
//! Every call, `--version` included, runs in the directory the cargo calls
//! get (`targets::outside_dir`): the edition is on the command line and the
//! configuration file is an absolute path under `--out`, so the working
//! directory only decides which toolchain the rustup proxy picks, and a
//! `rust-toolchain` file of the corpus or of the harness's own directory
//! never does. A relative `SPECENGINE_RUSTFMT` path is resolved against the
//! harness's own directory first.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Overrides the rustfmt command (e.g. a nightly wrapper); default `rustfmt`.
pub const ENV_RUSTFMT: &str = "SPECENGINE_RUSTFMT";

/// rustfmt defaults; the edition is passed on the command line.
pub const DEFAULT_CONFIG: &str = "";

/// The contrasting configuration: `max_width` and `trailing_comma` flipped.
/// Stable rustfmt rejects `trailing_comma` (unstable) with a warning and
/// formats anyway; the rejection is recorded in the result.
pub const CONTRAST_CONFIG: &str = "max_width = 60\ntrailing_comma = \"Never\"\n";

pub struct Rustfmt {
    command: PathBuf,
    /// The working directory of every call.
    cwd: PathBuf,
    /// Output of `rustfmt --version`, trimmed.
    pub version: String,
}

pub struct Formatted {
    pub text: String,
    /// Options rustfmt refused to apply (from its "can't set" warnings).
    pub rejected_options: Vec<String>,
}

impl Rustfmt {
    /// `None` when no rustfmt answers `--version` run in `cwd`, the
    /// directory every later call runs in too.
    pub fn detect(cwd: &Path) -> Option<Self> {
        let command = std::env::var(ENV_RUSTFMT)
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "rustfmt".to_owned());
        let command = PathBuf::from(command);
        // A bare name is looked up on `PATH`; a path with a directory part
        // names a file from here, not from `cwd`.
        let command = if command.components().count() > 1 {
            std::path::absolute(&command).unwrap_or(command)
        } else {
            command
        };
        let output = Command::new(&command)
            .arg("--version")
            .current_dir(cwd)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        Some(Self {
            command,
            cwd: cwd.to_owned(),
            version,
        })
    }

    /// Formats `source` with the configuration file at `config_path`.
    pub fn format(&self, source: &str, config_path: &Path) -> Result<Formatted, String> {
        let mut child = Command::new(&self.command)
            .args(["--edition", "2024", "--emit", "stdout", "--config-path"])
            .arg(config_path)
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("cannot spawn rustfmt: {error}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            // A broken pipe means rustfmt gave up early; its exit status tells.
            let _ = stdin.write_all(source.as_bytes());
        }
        let output = child
            .wait_with_output()
            .map_err(|error| format!("rustfmt did not finish: {error}"))?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let rejected_options = stderr
            .lines()
            .filter_map(|line| line.split("can't set `").nth(1))
            .filter_map(|rest| rest.split(' ').next())
            .map(str::to_owned)
            .collect();
        if !output.status.success() {
            // Prefer the diagnostic over the "can't set" warnings that precede it.
            let reason = stderr
                .lines()
                .find(|line| line.starts_with("error"))
                .or_else(|| stderr.lines().find(|line| !line.trim().is_empty()))
                .unwrap_or("no diagnostic")
                .to_owned();
            return Err(reason);
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|_| "rustfmt output is not UTF-8".to_owned())?;
        Ok(Formatted {
            text,
            rejected_options,
        })
    }
}
