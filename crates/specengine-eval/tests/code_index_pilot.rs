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
//!
//! docs/features/bevy-roles.md TE-7 (E1, S20, R9): `te7_pilot_<a|b>_
//! bevy_roles` run the same measurement twice, `SPECENGINE_CODE_FRAMEWORKS_
//! <A|B>` unset then set (it must be set: the run fails naming it), and
//! print `framework_roles` (per word, subjects by state, uncertain),
//! `roles_ms`, and `full_ms`, `one_file_ms`, `noop_ms` without and with the
//! opt-in against AC-10 (`one_file_ms` > 200: slice 6c); then a warm `spec
//! index` with the opt-in (the CLI library, a config copy outside the
//! corpus, the median of five after one warming run) against
//! `docs/canon/mcp-read.md` "Latency"'s 1 s. Counts and times only:
//!
//! ```text
//! SPECENGINE_PILOT_A=… SPECENGINE_SCHEME_A=… SPECENGINE_CODE_ROOTS_A=dir,dir \
//!   SPECENGINE_CODE_FRAMEWORKS_A=bevy SPECENGINE_CARGO="$(rustup which cargo)" \
//!   perl -e 'alarm 3600; exec @ARGV' cargo nextest run --release -p specengine-eval \
//!   --test code_index_pilot te7_pilot_a --run-ignored only --no-capture --test-threads 1
//! ```
//!
//! The unignored `the_roles_pilot_run_works_on_the_code_bevy_copy` runs it
//! over a scratch git copy of `fixtures/code-bevy` with a cargo stand-in.

#![cfg(unix)]

mod pilot;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use serde_json::Value;
use specengine_cli::{Env, Globals, IndexRequest, Outcome, index, render_json};

/// A scratch directory, removed on drop; unique per process and call (two
/// unignored runs share a label).
struct Scratch(PathBuf);

static SCRATCHES: AtomicUsize = AtomicUsize::new(0);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-code-index-pilot-{name}-{}-{}",
            std::process::id(),
            SCRATCHES.fetch_add(1, Ordering::SeqCst)
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
    /// bevy-roles E1: `SPECENGINE_CODE_FRAMEWORKS_<A|B>` for the child,
    /// comma-separated; `None`: unset (the opt-in off).
    frameworks: Option<String>,
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
    let frameworks_variable = format!("SPECENGINE_CODE_FRAMEWORKS_{suffix}");
    let frameworks_value = inputs.frameworks.as_deref().map(PathBuf::from);
    let mut set: Vec<(&str, &Path)> = vec![
        (variables.corpus, inputs.corpus.as_path()),
        (variables.scheme, inputs.scheme.as_path()),
        (roots_variable.as_str(), roots_value.as_path()),
        ("SPECENGINE_CARGO", inputs.cargo.as_path()),
    ];
    if let Some(value) = &frameworks_value {
        set.push((frameworks_variable.as_str(), value.as_path()));
    }
    let output = pilot::run_read_only_over(
        "code-index",
        label,
        &scratch.0,
        &inputs.corpus,
        &proof_roots,
        &set,
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
    println!(
        "[{label}] framework_roles {}, roles_ms {}",
        result["framework_roles"], result["roles_ms"]
    );
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
        frameworks: None,
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
        frameworks: None,
    };
    let (result, mentions) = run("pilot-a", "A", &inputs);
    assert_eq!(result["items"], 31, "{result}");
    assert_eq!(result["roles"]["differ"], 0, "{result}");
    assert!(result["framework_roles"].is_null(), "{result}");
    let total: u64 = mentions.iter().map(|(_, count)| count).sum();
    assert_eq!(
        total, 11,
        "the fixture's mentions, split by root: {mentions:?}"
    );
}

// --------------------------------------------------- bevy-roles TE-7

/// A warm `spec index` with the opt-in (bevy-roles TE-7, S20): the CLI
/// library in process, a config copy outside the corpus (the scheme,
/// `[code]` with every root and `frameworks`), a `HOME` under `scratch`;
/// one warming run (its JSON `code.roles` an object), then five timed;
/// their median in milliseconds. The corpus is only read.
fn warm_index_ms(label: &str, scratch: &Path, inputs: &Inputs, frameworks: &str) -> u128 {
    let scheme_text = fs::read_to_string(&inputs.scheme).expect("the scheme");
    let roots: Vec<String> = inputs
        .roots
        .iter()
        .map(|root| format!("{root:?}"))
        .collect();
    let names: Vec<String> = frameworks
        .split(',')
        .map(|name| format!("{:?}", name.trim()))
        .collect();
    let config = scratch.join("config-warm.toml");
    fs::write(
        &config,
        format!(
            "{scheme_text}\n[code]\nroots = [{}]\nframeworks = [{}]\n",
            roots.join(", "),
            names.join(", ")
        ),
    )
    .expect("the warm config");
    let home = scratch.join("warm-home");
    fs::create_dir_all(&home).expect("home");
    let env = Env {
        cwd: inputs.corpus.clone(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(inputs.corpus.clone()),
        config: Some(config),
    };
    let mut times = Vec::new();
    for at in 0..6 {
        let started = Instant::now();
        let outcome = index(&env, &globals, &IndexRequest { full: false })
            .unwrap_or_else(|error| panic!("{label}: warm spec index: {error:?}"));
        let ms = started.elapsed().as_millis();
        if at == 0 {
            let json: Value =
                serde_json::from_str(&render_json(&Outcome::Index(outcome))).expect("index JSON");
            assert!(
                json["code"]["roles"].is_object(),
                "{label}: the opt-in is on: {}",
                json["code"]
            );
        } else {
            times.push(ms);
        }
    }
    times.sort_unstable();
    times[times.len() / 2]
}

/// TE-7 over `inputs`: the measurement off, then on (`frameworks`), the
/// warm `spec index` on; the figures printed; returns (off, on, warm ms).
fn te7(label: &str, suffix: &str, inputs: &mut Inputs, frameworks: &str) -> (Value, Value, u128) {
    inputs.frameworks = None;
    println!("[{label}] TE-7: the opt-in off");
    let (off, _) = run(label, suffix, inputs);
    assert!(off["framework_roles"].is_null(), "{label}: {off}");
    inputs.frameworks = Some(frameworks.to_owned());
    println!("[{label}] TE-7: the opt-in on");
    let (on, _) = run(label, suffix, inputs);
    assert!(on["framework_roles"].is_object(), "{label}: {on}");
    assert!(on["roles_ms"].is_u64(), "{label}: {on}");
    for key in ["full_ms", "one_file_ms", "noop_ms"] {
        println!("[{label}] TE-7 {key}: off {}, on {}", off[key], on[key]);
    }
    let one_file = on["one_file_ms"].as_f64().unwrap_or(f64::NAN);
    println!(
        "[{label}] TE-7 one_file_ms with the opt-in <= 200: {} (else slice 6c)",
        one_file <= 200.0
    );

    let variables = pilot::variables(label);
    let mut proof_roots = pilot::scheme_roots(&inputs.scheme, variables.scheme);
    proof_roots.extend(inputs.roots.iter().cloned());
    let scratch = Scratch::new(&format!("{label}-warm"));
    let before = pilot::proof(&inputs.corpus, &proof_roots);
    let warm = warm_index_ms(label, &scratch.0, inputs, frameworks);
    let after = pilot::proof(&inputs.corpus, &proof_roots);
    assert!(
        before == after,
        "{label}: the warm index changed the corpus"
    );
    println!(
        "[{label}] TE-7 warm spec index with the opt-in: median {warm} ms of 5 (<= 1000: {})",
        warm <= 1000
    );
    (off, on, warm)
}

/// `SPECENGINE_CODE_FRAMEWORKS_<suffix>`, which TE-7 needs.
fn frameworks_of(suffix: &str) -> String {
    let variable = format!("SPECENGINE_CODE_FRAMEWORKS_{suffix}");
    std::env::var(&variable)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("set {variable} to the frameworks (comma-separated) for TE-7"))
}

#[test]
#[ignore = "manual pilot run: SPECENGINE_PILOT_A, SPECENGINE_SCHEME_A, SPECENGINE_CODE_ROOTS_A, SPECENGINE_CODE_FRAMEWORKS_A, SPECENGINE_CARGO"]
fn te7_pilot_a_bevy_roles() {
    let frameworks = frameworks_of("A");
    let mut inputs = pilot_inputs("pilot-a", "A");
    te7("pilot-a", "A", &mut inputs, &frameworks);
}

#[test]
#[ignore = "manual pilot run: SPECENGINE_PILOT_B, SPECENGINE_SCHEME_B, SPECENGINE_CODE_ROOTS_B, SPECENGINE_CODE_FRAMEWORKS_B, SPECENGINE_CARGO"]
fn te7_pilot_b_bevy_roles() {
    let frameworks = frameworks_of("B");
    let mut inputs = pilot_inputs("pilot-b", "B");
    te7("pilot-b", "B", &mut inputs, &frameworks);
}

/// The cargo stand-in of code-bevy: each manifest's package with the
/// layout's own target (`crates/farm`: the lib; `crates/farm-app`: the bin).
const BEVY_CARGO: &str = r#"#!/bin/sh
for last in "$@"; do :; done
dir=$(dirname "$last")
case "$dir" in
  */farm-app) printf '{"workspace_root":"%s","packages":[{"manifest_path":"%s","targets":[{"kind":["bin"],"name":"farm-app","src_path":"%s/src/main.rs"}]}]}\n' "$dir" "$last" "$dir" ;;
  *) printf '{"workspace_root":"%s","packages":[{"manifest_path":"%s","targets":[{"kind":["lib"],"name":"farm","src_path":"%s/src/lib.rs"}]}]}\n' "$dir" "$last" "$dir" ;;
esac
"#;

/// TE-7's helper over a scratch git copy of `fixtures/code-bevy` (its
/// scheme copied outside it, `[paths]` written, `[code]` left out: roots
/// and frameworks come from the variables), a cargo stand-in answering
/// the layout's tables: off, `framework_roles` `null`; on (`bevy`),
/// `expected.json`'s `code_roles` and a `roles_ms` count; the warm index
/// a count; the corpus only read.
#[test]
fn the_roles_pilot_run_works_on_the_code_bevy_copy() {
    let scratch = Scratch::new("bevy-corpus");
    let corpus = scratch.0.join("corpus");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-bevy");
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
    fs::write(&cargo, BEVY_CARGO).expect("the stand-in");
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).expect("executable");
    let mut inputs = Inputs {
        corpus: fs::canonicalize(&corpus).unwrap(),
        scheme,
        roots: vec!["crates".to_owned(), "assets".to_owned()],
        cargo,
        frameworks: None,
    };
    let (off, on, _warm) = te7("pilot-a", "A", &mut inputs, "bevy");
    assert_eq!(off["items"], 41, "{off}");
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(fixture.join("expected.json")).expect("expected.json"),
    )
    .expect("fixtures/code-bevy/expected.json");
    assert_eq!(on["framework_roles"], expected["code_roles"], "{on}");
}
