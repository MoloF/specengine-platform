//! docs/features/verify-tests.md (8a-1) TE-8 (P12, S11) and RD-2's door
//! half: source scans, no build. 8a's files (the core, store and CLI
//! sources that name the `verify.tests` event, the probe or the process
//! group calls, and core `verify.rs`) name no runner and no runner flag
//! (`nextest`, `test(=`, `--exact`, `cargo test`: ADR-0008, ADR-0027);
//! and no door but the owner's terminal reaches the run (ADR-0044): no MCP
//! tool, HTTP route, UI call, hook, CI step or plugin text names the
//! library's entry or `verify --tests`. Each test names the mutation that
//! turns it red (`M:`).

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::repository_root;

/// PHASE-B: the library's entry of `spec verify --tests` (the spec names
/// none; chosen with `verify_tests_common`).
const ENTRY: &str = "verify_tests";

/// The probe's test name ("Order" step 8): a file holding it is 8a's.
const PROBE: &str = "specengine_verify_probe_no_such_test";

/// Every regular file under `dir` (none when it is absent), sorted, with
/// `keep` true.
fn files(dir: &Path, keep: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if path.file_name().is_some_and(|name| name == "node_modules") {
                continue;
            }
            out.extend(files(&path, keep));
        } else if kind.is_file() && keep(&path) {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn is_rust(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "rs")
}

/// A source without its own `#[cfg(test)] mod` (over fixtures, by design).
fn scanned(path: &Path) -> String {
    let whole = fs::read_to_string(path).unwrap_or_default();
    whole
        .find("\n#[cfg(test)]\nmod ")
        .map_or(whole.clone(), |at| whole[..at].to_owned())
}

/// `text` holds `word` as a whole identifier.
fn has_identifier(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(ident)
            && !text[at + word.len()..].chars().next().is_some_and(ident)
    })
}

/// TE-8 (P12, S11): core `verify.rs` and every core, store and CLI source
/// naming the `verify.tests` event, the probe, `killpg` or `waitid` (8a's
/// files, the runner module among them) hold none of `nextest`, `test(=`,
/// `--exact`, `cargo test`. M: a runner word (`cargo test`) in a template
/// default or a hint.
#[test]
fn verify_tests_sources_name_no_runner() {
    let root = repository_root();
    let mut scope = vec![root.join("crates/specengine-core/src/verify.rs")];
    for crate_dir in [
        "crates/specengine-core/src",
        "crates/specengine-store/src",
        "crates/specengine-cli/src",
    ] {
        for path in files(&root.join(crate_dir), &is_rust) {
            let text = scanned(&path);
            if (text.contains("verify.tests")
                || text.contains(PROBE)
                || text.contains("killpg")
                || text.contains("waitid"))
                && !scope.contains(&path)
            {
                scope.push(path);
            }
        }
    }
    let named = |needle: &str| scope.iter().any(|path| scanned(path).contains(needle));
    assert!(named("killpg"), "the runner module is scanned: {scope:#?}");
    assert!(named(PROBE), "the probe's module is scanned: {scope:#?}");
    assert!(
        named("verify.tests"),
        "the event's modules are scanned: {scope:#?}"
    );
    let mut offences = Vec::new();
    for path in &scope {
        let text = scanned(path);
        for (n, line) in text.lines().enumerate() {
            for word in ["nextest", "test(=", "--exact", "cargo test"] {
                if line.contains(word) {
                    offences.push(format!(
                        "{}:{}: {word}",
                        path.strip_prefix(&root).unwrap_or(path).display(),
                        n + 1
                    ));
                }
            }
        }
    }
    assert!(offences.is_empty(), "{}", offences.join("\n"));
}

/// RD-2 (ADR-0044, P12's scan for the runner's entry): no MCP or HTTP
/// source names the library's entry (`verify_tests`, its request type) or
/// a `--tests` argument; no UI source, hook, CI workflow or plugin file
/// names `verify --tests` or the entry. The entry exists in the CLI
/// library, so the scan is not vacuous. M: an MCP or HTTP path to the
/// runner.
#[test]
fn mcp_and_http_never_run_tests() {
    let root = repository_root();
    let lib = fs::read_to_string(root.join("crates/specengine-cli/src/lib.rs")).expect("lib.rs");
    assert!(
        has_identifier(&lib, ENTRY),
        "the CLI library exports `{ENTRY}` (PHASE-B name)"
    );
    let mut offences = Vec::new();
    for dir in ["crates/specengine-mcp/src", "crates/specengine-http/src"] {
        for path in files(&root.join(dir), &is_rust) {
            let text = scanned(&path);
            if has_identifier(&text, ENTRY)
                || text.contains("VerifyTestsRequest")
                || text.contains("\"--tests\"")
                || text.contains("verify --tests")
            {
                offences.push(path.display().to_string());
            }
        }
    }
    for dir in ["ui/src", ".githooks", ".github/workflows", "plugin"] {
        for path in files(&root.join(dir), &|_: &Path| true) {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if text.contains("verify --tests") || has_identifier(&text, ENTRY) {
                offences.push(path.display().to_string());
            }
        }
    }
    assert!(
        offences.is_empty(),
        "a door to the runner:\n{}",
        offences.join("\n")
    );
}
