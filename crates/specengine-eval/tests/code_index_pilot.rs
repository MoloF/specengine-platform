//! docs/features/symbol-index.md TE-8: `specengine-eval code-index` over
//! pilots A and B, manual, the owner's run, never CI (`#[ignore]`). Release
//! for AC-10's judgement and dev for the record, in the foreground under a
//! self-terminating timeout, one pilot at a time; a scratch `HOME` the test
//! makes; the child given only `PATH`, that `HOME`, the label's corpus and
//! scheme variables, `SPECENGINE_CODE_ROOTS_<A|B>` and `SPECENGINE_CARGO`
//! as an absolute toolchain path (`rustup which cargo` when unset: rustup's
//! proxy finds no toolchain under a scratch `HOME`, and `roles` would
//! compare layout with layout). Then `spec index` (the CLI library, in
//! process, the same scratch `HOME`) once per code root, for its mentions.
//! The read-only proof (`git --no-optional-locks status` and a (path,
//! kind, size, mtime) listing of the scheme's and the code roots) is equal
//! before and after.
//!
//! Printed, counts only, for the spec's "Implementation": AC-10's figures
//! (`hash_ms` × 300 / `kloc` ≤ 2 000, `one_file_ms` ≤ 200, `noop_ms` ≤ 200,
//! `full_ms` recorded) with the build profile; `twins`; `signatures`;
//! mentions per root; `roles` (Q2's outcome, counted only with
//! `roles.metadata` > 0: else the run fails naming `SPECENGINE_CARGO`).
//! Nothing here names a pilot; its paths come only from the variables:
//!
//! ```text
//! cargo nextest run --release -p specengine-eval --test code_index_pilot --no-run
//! SPECENGINE_PILOT_A=… SPECENGINE_SCHEME_A=… SPECENGINE_CODE_ROOTS_A=dir,dir \
//!   SPECENGINE_CARGO="$(rustup which cargo)" perl -e 'alarm 3600; exec @ARGV' \
//!   cargo nextest run --release -p specengine-eval --test code_index_pilot \
//!   te8_pilot_a --run-ignored only --no-capture --test-threads 1
//! ```
//!
//! (B alike; dev: the same without `--release`.) It fails, never skips,
//! naming an unset variable. The unignored test runs the same helper over
//! a scratch git copy of `fixtures/code-index` with a cargo stand-in.

#![cfg(unix)]

mod pilot;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use specengine_cli::{Env, Globals, IndexRequest, Outcome, index, render_json};

/// A scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-code-index-pilot-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch");
        Self(fs::canonicalize(&path).expect("canonical scratch"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The toolchain's `tool` by absolute path: `variable` when set, else
/// `rustup which tool` (as `hash_levels_eval.rs`).
fn toolchain_binary(variable: &str, tool: &str) -> PathBuf {
    if let Some(value) = std::env::var_os(variable).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        assert!(
            path.is_absolute() && path.is_file(),
            "{variable} must name the toolchain's {tool} by absolute path, got {}",
            path.display()
        );
        return path;
    }
    let output = Command::new("rustup")
        .current_dir("/")
        .args(["which", tool])
        .output()
        .unwrap_or_else(|error| {
            panic!("set {variable} to the toolchain's {tool} (absolute path), or put rustup on PATH: {error}")
        });
    assert!(
        output.status.success(),
        "rustup which {tool}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    assert!(
        path.is_absolute() && path.is_file(),
        "rustup which {tool} gave {}",
        path.display()
    );
    path
}

/// One TE-8 run's inputs.
struct Inputs {
    corpus: PathBuf,
    scheme: PathBuf,
    roots: Vec<String>,
    cargo: PathBuf,
}

/// The run: the harness read-only, then one `spec index` per code root;
/// the figures printed; returns the envelope's result and the mentions per
/// root.
fn run(label: &str, suffix: &str, inputs: &Inputs) -> (Value, Vec<(String, u64)>) {
    let variables = pilot::variables(label);
    let scratch = Scratch::new(label);
    assert!(
        !scratch.0.starts_with(&inputs.corpus) && !inputs.corpus.starts_with(&scratch.0),
        "{label}: the scratch directory and the corpus must not nest"
    );
    let mut proof_roots = pilot::scheme_roots(&inputs.scheme, variables.scheme);
    proof_roots.extend(inputs.roots.iter().cloned());
    let roots_value = PathBuf::from(inputs.roots.join(","));
    let roots_variable = format!("SPECENGINE_CODE_ROOTS_{suffix}");
    let output = pilot::run_read_only_over(
        "code-index",
        label,
        &scratch.0,
        &inputs.corpus,
        &proof_roots,
        &[
            (variables.corpus, inputs.corpus.as_path()),
            (variables.scheme, inputs.scheme.as_path()),
            (roots_variable.as_str(), roots_value.as_path()),
            ("SPECENGINE_CARGO", inputs.cargo.as_path()),
        ],
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    let envelope: Value = serde_json::from_str(stdout.trim()).expect("one JSON envelope");
    let result = envelope["result"].clone();

    // Mentions per root: the CLI library, a config copy per root outside
    // the corpus, the index in a scratch HOME.
    let home = scratch.0.join("index-home");
    fs::create_dir_all(&home).expect("home");
    let scheme_text = fs::read_to_string(&inputs.scheme).expect("the scheme");
    let before = pilot::proof(&inputs.corpus, &proof_roots);
    let mut mentions = Vec::new();
    for (at, root) in inputs.roots.iter().enumerate() {
        let config = scratch.0.join(format!("config-{at}.toml"));
        fs::write(
            &config,
            format!("{scheme_text}\n[code]\nroots = [{root:?}]\n"),
        )
        .expect("a config copy");
        let env = Env {
            cwd: inputs.corpus.clone(),
            home: Some(home.as_os_str().to_owned()),
            xdg_data_home: None,
        };
        let globals = Globals {
            root: Some(inputs.corpus.clone()),
            config: Some(config),
        };
        let outcome = index(&env, &globals, &IndexRequest { full: false })
            .unwrap_or_else(|error| panic!("{label}: spec index over a root: {error:?}"));
        let json: Value =
            serde_json::from_str(&render_json(&Outcome::Index(outcome))).expect("index JSON");
        mentions.push((
            format!("root {}", at + 1),
            json["code"]["mentions"].as_u64().expect("code.mentions"),
        ));
    }
    let after = pilot::proof(&inputs.corpus, &proof_roots);
    assert!(
        before == after,
        "{label}: the per-root index changed the corpus"
    );

    print(label, &result, &mentions);
    let metadata = result["roles"]["metadata"]
        .as_u64()
        .expect("roles.metadata");
    assert!(
        metadata > 0,
        "{label}: no package answered `cargo metadata`: `roles` would compare layout with \
         layout (Q25); set SPECENGINE_CARGO to the toolchain's cargo by absolute path"
    );
    (result, mentions)
}

/// The figures, counts only.
fn print(label: &str, result: &Value, mentions: &[(String, u64)]) {
    let profile = if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    };
    let number = |key: &str| result[key].as_f64().unwrap_or(f64::NAN);
    let kloc = number("kloc");
    let rate = number("hash_ms") * 300.0 / kloc;
    println!("[{label}] profile {profile}");
    println!(
        "[{label}] kloc {kloc:.3}, rs_files {}, ron_files {}, bytes {}, items {}",
        result["rs_files"], result["ron_files"], result["bytes"], result["items"]
    );
    println!(
        "[{label}] AC-10: hash_ms {} (x 300 / kloc = {rate:.0} <= 2000: {}), one_file_ms {} (<= 200: {}), \
         noop_ms {} (<= 200: {}), full_ms {} (recorded), one_file_parsed {}, noop_parsed {}, \
         db_bytes {}, refs_ms {}{}",
        result["hash_ms"],
        rate <= 2000.0,
        result["one_file_ms"],
        number("one_file_ms") <= 200.0,
        result["noop_ms"],
        number("noop_ms") <= 200.0,
        result["full_ms"],
        result["one_file_parsed"],
        result["noop_parsed"],
        result["db_bytes"],
        result["refs_ms"],
        if profile == "release" {
            ""
        } else {
            " (dev: recorded, not judged)"
        }
    );
    println!("[{label}] twins {}", result["twins"]);
    println!("[{label}] signatures {}", result["signatures"]);
    println!("[{label}] roles {}", result["roles"]);
    for (root, count) in mentions {
        println!("[{label}] mentions, {root}: {count}");
    }
}

fn pilot_inputs(label: &str, suffix: &str) -> Inputs {
    let variables = pilot::variables(label);
    let corpus = pilot::required(variables.corpus, "the pilot's worktree");
    let corpus =
        fs::canonicalize(&corpus).unwrap_or_else(|error| panic!("{}: {error}", variables.corpus));
    let scheme = pilot::required(variables.scheme, "the pilot's scheme with [paths]");
    let roots_variable = format!("SPECENGINE_CODE_ROOTS_{suffix}");
    let roots: Vec<String> = std::env::var(&roots_variable)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("set {roots_variable} to the code roots, comma-separated"))
        .split(',')
        .map(|root| root.trim().to_owned())
        .collect();
    Inputs {
        corpus,
        scheme,
        roots,
        cargo: toolchain_binary("SPECENGINE_CARGO", "cargo"),
    }
}

#[test]
#[ignore = "manual pilot run: SPECENGINE_PILOT_A, SPECENGINE_SCHEME_A, SPECENGINE_CODE_ROOTS_A, SPECENGINE_CARGO"]
fn te8_pilot_a_code_index() {
    run("pilot-a", "A", &pilot_inputs("pilot-a", "A"));
}

#[test]
#[ignore = "manual pilot run: SPECENGINE_PILOT_B, SPECENGINE_SCHEME_B, SPECENGINE_CODE_ROOTS_B, SPECENGINE_CARGO"]
fn te8_pilot_b_code_index() {
    run("pilot-b", "B", &pilot_inputs("pilot-b", "B"));
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    for entry in fs::read_dir(from).expect("fixture").flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy(&entry.path(), &target);
        } else if entry.file_name() != "expected.json" {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
        }
    }
}

/// The helper itself over a scratch git copy of `fixtures/code-index`
/// (its scheme copied outside it, `[paths]` written, `[code]` left out:
/// the roots come from the variable), with a cargo stand-in answering the
/// layout's own table: the run is read-only, `roles` counts metadata
/// answers, the mentions per root add up to the fixture's.
#[test]
fn the_pilot_run_works_on_a_fixture_copy() {
    // The corpus lives in a scratch of its own: `run`'s must not nest it.
    let scratch = Scratch::new("fixture-corpus");
    let corpus = scratch.0.join("corpus");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-index");
    copy(&fixture, &corpus);
    pilot::git_init(&corpus);
    let config = fs::read_to_string(corpus.join("specengine.toml")).expect("config");
    let at = config.find("\n[code]\n").expect("a [code] table");
    let scheme = scratch.0.join("scheme.toml");
    fs::write(
        &scheme,
        format!("{}\n[paths]\nroots = [\"docs\"]\n", &config[..=at]),
    )
    .expect("the scheme copy");
    let cargo = scratch.0.join("cargo.sh");
    fs::write(
        &cargo,
        "#!/bin/sh\nfor last in \"$@\"; do :; done\ndir=$(dirname \"$last\")\n\
         printf '{\"workspace_root\":\"%s\",\"packages\":[{\"manifest_path\":\"%s\",\"targets\":[\
         {\"kind\":[\"lib\"],\"name\":\"inkwell_core\",\"src_path\":\"%s/src/lib.rs\"},\
         {\"kind\":[\"bin\"],\"name\":\"inkwell-core\",\"src_path\":\"%s/src/main.rs\"},\
         {\"kind\":[\"bin\"],\"name\":\"stamp\",\"src_path\":\"%s/src/bin/stamp.rs\"},\
         {\"kind\":[\"test\"],\"name\":\"flow\",\"src_path\":\"%s/tests/flow.rs\"}]}]}\\n' \
         \"$dir\" \"$last\" \"$dir\" \"$dir\" \"$dir\" \"$dir\"\n",
    )
    .expect("the stand-in");
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).expect("executable");
    let inputs = Inputs {
        corpus: fs::canonicalize(&corpus).unwrap(),
        scheme,
        roots: vec!["crates".to_owned(), "data".to_owned()],
        cargo,
    };
    let (result, mentions) = run("pilot-a", "A", &inputs);
    assert_eq!(result["items"], 31, "{result}");
    assert_eq!(result["roles"]["differ"], 0, "{result}");
    let total: u64 = mentions.iter().map(|(_, count)| count).sum();
    assert_eq!(
        total, 11,
        "the fixture's mentions, split by root: {mentions:?}"
    );
}
