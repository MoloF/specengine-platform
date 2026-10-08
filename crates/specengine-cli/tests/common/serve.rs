//! A daemon for the CLI tests that must run beside one
//! (docs/features/spec-serve.md AC-D9): `spec serve ROOT... --port 0`, the
//! build directory's `specengine-http` beside `spec` exec'd in its place.

use std::ffi::OsString;
use std::io::{BufRead as _, BufReader, Read as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::SPEC;

/// A listening daemon; stopped by its own PID (`SIGKILL`, reaped) on drop,
/// never by name, and under a 120 s alarm in any case (`perl` execs
/// `spec`, `spec` execs the daemon: one PID throughout).
pub struct Daemon {
    child: Child,
    /// Its `listening` URL.
    pub url: String,
}

impl Daemon {
    /// `spec serve <roots> --port 0` in `cwd` with exactly `vars`, until
    /// its `listening` line (60 s at most, else killed and the test
    /// fails).
    pub fn start(vars: &[(OsString, OsString)], cwd: &Path, roots: &[&Path]) -> Self {
        let beside: PathBuf = Path::new(SPEC).with_file_name("specengine-http");
        assert!(
            beside.is_file(),
            "{}: build it first (`cargo build -p specengine-http`; `cargo nextest run \
             --workspace` builds it)",
            beside.display()
        );
        let mut child = Command::new("/usr/bin/perl")
            .args(["-e", "alarm 120; exec @ARGV", SPEC, "serve"])
            .args(roots)
            .args(["--port", "0"])
            .env_clear()
            .envs(vars.iter().map(|(name, value)| (name, value)))
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spec serve starts");
        let stdout = child.stdout.take().expect("stdout");
        let mut stderr = child.stderr.take().expect("stderr");
        let (lines, received) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if lines.send(line).is_err() {
                    break;
                }
            }
        });
        let errors = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            text
        });
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            match received.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(line) => {
                    if let Some(url) = line.strip_prefix("listening ") {
                        return Self {
                            child,
                            url: url.to_owned(),
                        };
                    }
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "spec serve did not listen: {}",
                        errors.join().unwrap_or_default()
                    );
                }
            }
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
