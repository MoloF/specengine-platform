//! AC-01 of docs/features/phase-0-spikes.md: the core build graph, read from
//! `cargo metadata` and `cargo tree` of this workspace.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root exists")
}

fn cargo() -> Command {
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command.current_dir(workspace_root());
    command
}

#[test]
fn default_members_are_exactly_the_five_core_packages() {
    let output = cargo()
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let names: HashMap<&str, &str> = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["name"].as_str().unwrap()))
        .collect();
    let mut defaults: Vec<&str> = metadata["workspace_default_members"]
        .as_array()
        .expect("workspace_default_members (cargo >= 1.71)")
        .iter()
        .map(|id| names[id.as_str().unwrap()])
        .collect();
    defaults.sort_unstable();
    assert_eq!(
        defaults,
        [
            "specengine-code",
            "specengine-eval",
            "specengine-import",
            "specengine-mcp",
            "xtask",
        ]
    );
    let members: Vec<&str> = metadata["workspace_members"]
        .as_array()
        .expect("workspace_members")
        .iter()
        .map(|id| names[id.as_str().unwrap()])
        .collect();
    assert!(
        !members
            .iter()
            .any(|m| m.starts_with("bevy") || *m == "bevy-mini"),
        "a Bevy fixture entered the workspace: {members:?}"
    );
}

/// The version constants in `specengine_code::grammar` are synced by hand
/// (Cargo exposes no dependency versions at build time); they enter the
/// recipe header, so a drift from the resolved crates would be invisible.
#[test]
fn grammar_version_constants_match_the_resolved_dependencies() {
    let output = cargo()
        .args(["metadata", "--format-version", "1", "--locked"])
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let versions_of = |name: &str| -> Vec<String> {
        metadata["packages"]
            .as_array()
            .expect("packages")
            .iter()
            .filter(|p| p["name"] == name)
            .map(|p| p["version"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        versions_of("tree-sitter"),
        [specengine_code::grammar::TREE_SITTER_VERSION],
        "grammar::TREE_SITTER_VERSION drifted from the resolved `tree-sitter`"
    );
    assert_eq!(
        versions_of("tree-sitter-rust"),
        [specengine_code::grammar::TREE_SITTER_RUST_VERSION],
        "grammar::TREE_SITTER_RUST_VERSION drifted from the resolved `tree-sitter-rust`"
    );
    let info = specengine_code::grammar::grammar_info();
    assert_eq!(
        info.tree_sitter,
        specengine_code::grammar::TREE_SITTER_VERSION
    );
    assert_eq!(
        info.tree_sitter_rust,
        specengine_code::grammar::TREE_SITTER_RUST_VERSION
    );
}

#[test]
fn core_tree_has_no_rust_analyzer_syn3_or_bevy() {
    let output = cargo()
        .args([
            "tree",
            "-e",
            "normal,no-proc-macro",
            "-p",
            "specengine-code",
            "-p",
            "specengine-mcp",
            "-p",
            "specengine-import",
        ])
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8_lossy(&output.stdout);
    for root in [
        "specengine-code v",
        "specengine-mcp v",
        "specengine-import v",
    ] {
        assert!(
            tree.contains(root),
            "cargo tree did not list {root}: {tree}"
        );
    }
    let mut offenders = Vec::new();
    for line in tree.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        for (index, token) in tokens.iter().enumerate() {
            let version = tokens.get(index + 1).copied().unwrap_or("");
            let is_crate =
                version.starts_with('v') && version[1..].starts_with(|c: char| c.is_ascii_digit());
            if !is_crate {
                continue;
            }
            if token.starts_with("ra_ap_")
                || token.starts_with("bevy")
                || (*token == "syn" && version.starts_with("v3."))
            {
                offenders.push(line.trim().to_owned());
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "forbidden crates in the core build graph:\n{}",
        offenders.join("\n")
    );
}

// ---------------------------------------------------------------------------
// AC-01, the `--features ra` part (layer C of 05 §5.1,
// `crates/specengine-ra/README.md`): `ra_ap_*` enters only through feature `ra`
// of `specengine-eval` → `specengine-ra`, at the one pinned release, and never
// the core graph, the default `specengine-eval` graph or the default members'
// graph. `cargo tree` resolves without compiling, so these run on the
// stable toolchain too (the `ra_ap_*` set itself needs rustc >= 1.98 to build).
// ---------------------------------------------------------------------------

/// The pinned `ra_ap_*` release (root `Cargo.toml`, `specengine_ra::RA_AP_VERSION`).
const RA_AP_VERSION: &str = "0.0.352";

/// The `ra_ap_*` crates `specengine-ra` depends on directly.
const RA_AP_DIRECT: [&str; 9] = [
    "ra_ap_load-cargo",
    "ra_ap_project_model",
    "ra_ap_ide",
    "ra_ap_ide_db",
    "ra_ap_hir_expand",
    "ra_ap_vfs",
    "ra_ap_paths",
    "ra_ap_syntax",
    "ra_ap_proc_macro_api",
];

/// `cargo tree --locked` in the workspace root (never rewrites `Cargo.lock`).
fn cargo_tree(args: &[&str]) -> String {
    let output = cargo()
        .arg("tree")
        .arg("--locked")
        .args(args)
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("cargo tree prints UTF-8")
}

/// `(name, version)` of every crate a `cargo tree` listing names, versions
/// without the leading `v`.
fn tree_crates(tree: &str) -> Vec<(String, String)> {
    let mut crates = Vec::new();
    for line in tree.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        for (index, token) in tokens.iter().enumerate() {
            let Some(version) = tokens.get(index + 1).and_then(|v| v.strip_prefix('v')) else {
                continue;
            };
            if version.starts_with(|c: char| c.is_ascii_digit()) {
                crates.push(((*token).to_owned(), version.to_owned()));
            }
        }
    }
    crates
}

/// Lines of `tree` naming `ra_ap_*` or `specengine-ra`.
fn rust_analyzer_lines(tree: &str) -> Vec<String> {
    tree.lines()
        .filter(|line| {
            tree_crates(line)
                .iter()
                .any(|(name, _)| name.starts_with("ra_ap_") || name == "specengine-ra")
        })
        .map(|line| line.trim().to_owned())
        .collect()
}

#[test]
fn ra_feature_pulls_ra_ap_only_at_the_pinned_release_through_specengine_ra() {
    let tree = cargo_tree(&["-p", "specengine-eval", "--features", "ra"]);
    let crates = tree_crates(&tree);
    assert!(
        crates
            .iter()
            .any(|(name, version)| name == "specengine-ra" && version == "0.1.0"),
        "`--features ra` did not pull specengine-ra:\n{tree}"
    );
    let ra_ap: Vec<&(String, String)> = crates
        .iter()
        .filter(|(name, _)| name.starts_with("ra_ap_"))
        .collect();
    for direct in RA_AP_DIRECT {
        assert!(
            ra_ap.iter().any(|(name, _)| name == direct),
            "`--features ra` did not pull {direct}:\n{tree}"
        );
    }
    let off_release: Vec<String> = ra_ap
        .iter()
        .filter(|(_, version)| version != RA_AP_VERSION)
        .map(|(name, version)| format!("{name} v{version}"))
        .collect();
    assert!(
        off_release.is_empty(),
        "ra_ap_* outside the pinned {RA_AP_VERSION}: {off_release:?}"
    );
    assert_eq!(
        root_manifest_ra_ap_pin(),
        RA_AP_VERSION,
        "the test's pin and the root manifest's pin disagree"
    );

    // Only through `specengine-ra`: no `ra_ap_*` is a direct dependency of the harness.
    let direct = cargo_tree(&["-p", "specengine-eval", "--features", "ra", "--depth", "1"]);
    let direct_crates = tree_crates(&direct);
    assert!(
        direct_crates
            .iter()
            .any(|(name, _)| name == "specengine-ra"),
        "specengine-ra is not a direct dependency under `--features ra`:\n{direct}"
    );
    let bypass: Vec<&(String, String)> = direct_crates
        .iter()
        .filter(|(name, _)| name.starts_with("ra_ap_"))
        .collect();
    assert!(
        bypass.is_empty(),
        "ra_ap_* reaches specengine-eval directly, not through specengine-ra: {bypass:?}"
    );
}

/// The pin as the root manifest states it for every `ra_ap_*` entry.
fn root_manifest_ra_ap_pin() -> String {
    let manifest =
        std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root manifest");
    let pins: Vec<&str> = manifest
        .lines()
        .filter(|line| line.trim_start().starts_with("ra_ap_"))
        .filter_map(|line| line.split('"').nth(1))
        .collect();
    assert!(
        pins.len() >= RA_AP_DIRECT.len(),
        "root manifest pins fewer ra_ap_* crates than specengine-ra uses: {pins:?}"
    );
    let mut distinct: Vec<&str> = pins.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 1, "ra_ap_* pins disagree: {pins:?}");
    distinct[0]
        .strip_prefix('=')
        .unwrap_or_else(|| panic!("ra_ap_* pin is not exact (`=`): {}", distinct[0]))
        .to_owned()
}

#[test]
fn default_specengine_eval_graph_has_no_rust_analyzer() {
    let tree = cargo_tree(&["-p", "specengine-eval"]);
    assert!(
        tree.contains("specengine-eval v"),
        "cargo tree did not list specengine-eval: {tree}"
    );
    let offenders = rust_analyzer_lines(&tree);
    assert!(
        offenders.is_empty(),
        "ra_ap_* / specengine-ra in the default specengine-eval graph:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn core_graph_with_every_default_edge_kind_has_no_rust_analyzer() {
    // Default edges: normal, build and dev dependencies (the proc-macro-free
    // normal graph is `core_tree_has_no_rust_analyzer_syn3_or_bevy`).
    let tree = cargo_tree(&[
        "-p",
        "specengine-code",
        "-p",
        "specengine-mcp",
        "-p",
        "specengine-import",
    ]);
    let offenders = rust_analyzer_lines(&tree);
    assert!(
        offenders.is_empty(),
        "ra_ap_* / specengine-ra in the core graph:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn default_members_graph_has_no_rust_analyzer() {
    // No `-p`: a virtual workspace's commands act on `default-members`.
    let tree = cargo_tree(&[]);
    for root in [
        "xtask v",
        "specengine-code v",
        "specengine-mcp v",
        "specengine-import v",
        "specengine-eval v",
    ] {
        assert!(
            tree.contains(root),
            "cargo tree over the default members did not list {root}: {tree}"
        );
    }
    let offenders = rust_analyzer_lines(&tree);
    assert!(
        offenders.is_empty(),
        "ra_ap_* / specengine-ra in the default members' graph (is specengine-ra a default member?):\n{}",
        offenders.join("\n")
    );
    // The full workspace does carry it: the check above is not vacuous.
    let workspace = cargo_tree(&["--workspace"]);
    assert!(
        !rust_analyzer_lines(&workspace).is_empty(),
        "`cargo tree --workspace` lists no ra_ap_*: the parser above cannot see them"
    );
}

// ---------------------------------------------------------------------------
// Exact pins of 04 §6 (owner's decision 2026-09-29): `salsa`, `salsa-macros`,
// `salsa-macro-rules` =0.28.2 and `unicode-ident` =1.0.24 as direct exact
// dependencies of `specengine-ra`; `libc` =0.2.189 direct in `specengine-eval`
// only behind feature `ra`; `fixtures/ra-mini` outside the workspace.
// ---------------------------------------------------------------------------

/// Transitive crates of `ra_ap_*` =0.0.352 held at exact versions.
const RA_TRANSITIVE_PINS: [(&str, &str); 4] = [
    ("salsa", "0.28.2"),
    ("salsa-macros", "0.28.2"),
    ("salsa-macro-rules", "0.28.2"),
    ("unicode-ident", "1.0.24"),
];

const LIBC_PIN: &str = "0.2.189";

/// `cargo metadata --no-deps` of the workspace.
fn workspace_metadata() -> Value {
    let output = cargo()
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON")
}

/// The declared dependency `name` of workspace package `package`.
fn declared_dependency<'a>(metadata: &'a Value, package: &str, name: &str) -> &'a Value {
    let package = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|p| p["name"] == package)
        .unwrap_or_else(|| panic!("no workspace package {package}"));
    let found: Vec<&Value> = package["dependencies"]
        .as_array()
        .expect("dependencies")
        .iter()
        .filter(|d| d["name"] == name)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{package} declares {name} {} times: {found:?}",
        found.len()
    );
    found[0]
}

/// Versions of every `[[package]]` named `name` in the workspace `Cargo.lock`.
fn locked_versions(name: &str) -> Vec<String> {
    let lock = std::fs::read_to_string(workspace_root().join("Cargo.lock")).expect("Cargo.lock");
    let mut versions = Vec::new();
    for block in lock.split("[[package]]").skip(1) {
        let field = |key: &str| {
            block.lines().find_map(|line| {
                line.strip_prefix(key)
                    .and_then(|rest| rest.trim().strip_prefix('='))
                    .map(|value| value.trim().trim_matches('"').to_owned())
            })
        };
        if field("name ").as_deref() == Some(name) {
            versions.push(field("version ").expect("version of a locked package"));
        }
    }
    versions
}

#[test]
fn specengine_ra_holds_salsa_and_unicode_ident_as_direct_exact_pins() {
    let metadata = workspace_metadata();
    for (name, version) in RA_TRANSITIVE_PINS {
        let dependency = declared_dependency(&metadata, "specengine-ra", name);
        assert_eq!(
            dependency["req"],
            format!("={version}"),
            "specengine-ra's {name} is not pinned exactly at {version}: {dependency}"
        );
        assert!(
            dependency["kind"].is_null(),
            "specengine-ra's {name} is not a normal dependency: {dependency}"
        );
        assert_eq!(
            dependency["optional"], false,
            "specengine-ra's {name} is optional: {dependency}"
        );
        assert_eq!(
            locked_versions(name),
            [version],
            "Cargo.lock does not hold exactly {name} {version}"
        );
    }
    // Resolved: the direct edges of `specengine-ra` land on the pinned versions.
    let direct = tree_crates(&cargo_tree(&[
        "-p",
        "specengine-ra",
        "-e",
        "normal",
        "--depth",
        "1",
    ]));
    for (name, version) in RA_TRANSITIVE_PINS {
        assert!(
            direct.iter().any(|(n, v)| n == name && v == version),
            "specengine-ra has no direct edge to {name} v{version}: {direct:?}"
        );
    }
    // And the whole `--features ra` graph carries no other version of them.
    let graph = tree_crates(&cargo_tree(&["-p", "specengine-eval", "--features", "ra"]));
    for (name, version) in RA_TRANSITIVE_PINS {
        let other: Vec<&(String, String)> = graph
            .iter()
            .filter(|(n, v)| n == name && v != version)
            .collect();
        assert!(other.is_empty(), "{name} at another version: {other:?}");
    }
}

#[test]
fn libc_is_a_direct_dependency_of_specengine_eval_only_with_feature_ra() {
    let metadata = workspace_metadata();
    let libc = declared_dependency(&metadata, "specengine-eval", "libc");
    assert_eq!(libc["req"], format!("={LIBC_PIN}"), "{libc}");
    assert_eq!(libc["optional"], true, "libc must be optional: {libc}");
    assert!(
        libc["kind"].is_null(),
        "libc is not a normal dependency: {libc}"
    );
    let eval = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|p| p["name"] == "specengine-eval")
        .expect("specengine-eval");
    let enabling: Vec<&str> = eval["features"]
        .as_object()
        .expect("features")
        .iter()
        .filter(|(_, enables)| {
            enables
                .as_array()
                .is_some_and(|list| list.iter().any(|e| e == "dep:libc"))
        })
        .map(|(feature, _)| feature.as_str())
        .collect();
    assert_eq!(enabling, ["ra"], "features enabling dep:libc: {enabling:?}");

    let depth_one = |features: &[&str]| {
        let mut args = vec!["-p", "specengine-eval", "-e", "normal", "--depth", "1"];
        args.extend_from_slice(features);
        tree_crates(&cargo_tree(&args))
    };
    let default = depth_one(&[]);
    assert!(
        !default.iter().any(|(name, _)| name == "libc"),
        "libc is a direct dependency of specengine-eval without --features ra: {default:?}"
    );
    let ra = depth_one(&["--features", "ra"]);
    assert!(
        ra.iter()
            .any(|(name, version)| name == "libc" && version == LIBC_PIN),
        "--features ra does not make libc v{LIBC_PIN} a direct dependency: {ra:?}"
    );
    assert_eq!(locked_versions("libc"), [LIBC_PIN], "Cargo.lock libc");
}

#[test]
fn ra_mini_fixture_stays_outside_the_workspace() {
    let metadata = workspace_metadata();
    let members: Vec<String> = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .map(|p| p["name"].as_str().expect("name").to_owned())
        .collect();
    for fixture_crate in ["arena", "arena-derive"] {
        assert!(
            !members.iter().any(|m| m == fixture_crate),
            "the ra-mini crate {fixture_crate} entered the workspace: {members:?}"
        );
    }
    let manifest =
        std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root manifest");
    let exclude_line = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("exclude"))
        .expect("root manifest has an `exclude` list");
    assert!(
        exclude_line.contains("\"fixtures/ra-mini\""),
        "root `exclude` does not list fixtures/ra-mini: {exclude_line}"
    );
    assert!(
        workspace_root()
            .join("fixtures/ra-mini/Cargo.lock")
            .is_file(),
        "fixtures/ra-mini carries its own Cargo.lock"
    );
}
