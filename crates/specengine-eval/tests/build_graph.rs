//! AC-01 of docs/features/phase-0-spikes.md: the core build graph, read from
//! `cargo metadata` and `cargo tree` of this workspace; AC-01–AC-03 of
//! docs/features/spec-parser.md: `specengine-model` and `specengine-core`
//! join the default members and the core graph, the model's normal graph is
//! `serde` only, neither crate touches the file system, and the two parser
//! libraries are pinned exactly; AC-01 and AC-02 of
//! docs/features/spec-index.md: `specengine-store` is the eighth default
//! member, SQLite stays out of the model's and the core's graphs, the store
//! depends on no measurement crate, no `sqlx`, and `rusqlite` is pinned
//! exactly with `bundled`, one version each of `rusqlite`, `libsqlite3-sys`
//! and `blake3`; AC-01 of docs/features/spec-check.md: the file-access scan
//! reaches the check module, and no `[workspace.dependencies]` entry is
//! added against `main`; AC-01 of docs/features/spec-cli.md:
//! `specengine-cli` a default member, its normal graph is the
//! model, core and store (SQLite only through the store) and no measurement,
//! MCP or rust-analyzer crate, and every direct dependency is a workspace
//! entry. AC-12 of docs/features/spec-cli-switch.md: the eight specengine
//! crates are the default members, no second-implementation package is left
//! in the metadata or `Cargo.lock`, `.cargo/config.toml` holds only the
//! build directory, and `scripts/` only the hook installer.

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
fn default_members_are_exactly_the_eight_core_packages() {
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
            "specengine-cli",
            "specengine-code",
            "specengine-core",
            "specengine-eval",
            "specengine-import",
            "specengine-mcp",
            "specengine-model",
            "specengine-store",
        ]
    );
    // docs/features/spec-cli-switch.md AC-12: no `xtask` package at all.
    let packages: Vec<&str> = names.values().copied().collect();
    assert!(
        !packages.iter().any(|name| name.contains("xtask")),
        "{packages:?}"
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

/// AC-12 of docs/features/spec-cli-switch.md, the files: `Cargo.lock` has
/// no retired package, `.cargo/config.toml` keeps only `[build] target-dir`
/// (no alias), `scripts/` holds only `hooks-install.sh`.
#[test]
fn no_retired_package_alias_or_script_is_left() {
    let root = workspace_root();
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).expect("Cargo.lock");
    assert!(!lock.contains("xtask"), "Cargo.lock names xtask");
    assert!(!root.join("xtask").exists(), "xtask/ exists");

    let config =
        std::fs::read_to_string(root.join(".cargo/config.toml")).expect(".cargo/config.toml");
    let settings: Vec<&str> = config
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        settings,
        ["[build]", "target-dir = \"target.noindex\""],
        "{config}"
    );

    let mut scripts: Vec<String> = std::fs::read_dir(root.join("scripts"))
        .expect("scripts/")
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    scripts.sort();
    assert_eq!(scripts, ["hooks-install.sh"]);
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
            "-p",
            "specengine-model",
            "-p",
            "specengine-core",
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
        "specengine-model v",
        "specengine-core v",
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
                || *token == "specengine-ra"
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
        "-p",
        "specengine-model",
        "-p",
        "specengine-core",
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
        "specengine-code v",
        "specengine-mcp v",
        "specengine-import v",
        "specengine-eval v",
        "specengine-model v",
        "specengine-core v",
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

// ---------------------------------------------------------------------------
// docs/features/spec-parser.md AC-02 (layering) and AC-03 (pins).
// ---------------------------------------------------------------------------

/// `(name, version)` of every crate in the normal-edge graph of `package`,
/// the package itself excluded.
fn normal_graph(package: &str) -> Vec<(String, String)> {
    let tree = cargo_tree(&["-p", package, "-e", "normal"]);
    let crates = tree_crates(&tree);
    assert!(
        crates.first().is_some_and(|(name, _)| name == package),
        "cargo tree -p {package} lists the package first:\n{tree}"
    );
    crates.into_iter().skip(1).collect()
}

#[test]
fn model_normal_graph_has_no_specengine_crate_and_no_parser_library() {
    let graph = normal_graph("specengine-model");
    let offenders: Vec<&(String, String)> = graph
        .iter()
        .filter(|(name, _)| {
            name.starts_with("specengine-") || name == "pulldown-cmark" || name == "serde-saphyr"
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "specengine-model's normal graph must be serde only: {offenders:?}"
    );
    assert!(
        graph.iter().any(|(name, _)| name == "serde"),
        "serde in the model graph: {graph:?}"
    );
}

#[test]
fn core_normal_graph_has_no_other_specengine_crate_but_the_model() {
    let graph = normal_graph("specengine-core");
    let offenders: Vec<&(String, String)> = graph
        .iter()
        .filter(|(name, _)| {
            [
                "specengine-code",
                "specengine-import",
                "specengine-mcp",
                "specengine-eval",
                "specengine-ra",
            ]
            .contains(&name.as_str())
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "specengine-core depends on {offenders:?}"
    );
    for wanted in ["specengine-model", "pulldown-cmark", "serde-saphyr"] {
        assert!(
            graph.iter().any(|(name, _)| name == wanted),
            "{wanted} missing from specengine-core's graph: {graph:?}"
        );
    }
}

#[test]
fn model_and_core_sources_do_no_file_io() {
    let pattern = ["std::fs", "File::", "OpenOptions"];
    let mut offenders = Vec::new();
    let mut scanned: Vec<PathBuf> = Vec::new();
    for krate in ["specengine-model", "specengine-core"] {
        let src = workspace_root().join("crates").join(krate).join("src");
        let mut stack = vec![src];
        let mut files = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("src readable") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                files += 1;
                let text = std::fs::read_to_string(&path).expect("UTF-8 source");
                scanned.push(path.clone());
                for (number, line) in text.lines().enumerate() {
                    if pattern.iter().any(|p| line.contains(p)) {
                        offenders.push(format!(
                            "{}:{}: {}",
                            path.display(),
                            number + 1,
                            line.trim()
                        ));
                    }
                }
            }
        }
        assert!(files > 0, "{krate}: no sources found");
    }
    assert!(
        offenders.is_empty(),
        "file access in model/core:\n{}",
        offenders.join("\n")
    );
    // docs/features/spec-check.md AC-01: the scan reaches every source of
    // the check module.
    let check_dir = workspace_root().join("crates/specengine-core/src/check");
    let check_sources: Vec<PathBuf> = std::fs::read_dir(&check_dir)
        .expect("the check module is a directory")
        .map(|entry| entry.expect("entry").path())
        .collect();
    assert!(check_sources.len() >= 5, "{check_sources:?}");
    // docs/features/spec-check-graph.md AC-01: the renderer and the rules of
    // increment 2 are among them.
    for name in ["render.rs", "generated.rs", "graph.rs", "resolve.rs"] {
        assert!(
            check_sources.iter().any(|path| path.ends_with(name)),
            "{name} in the check module: {check_sources:?}"
        );
    }
    for source in check_sources {
        assert!(
            scanned.contains(&source),
            "{} is not scanned for file access",
            source.display()
        );
    }
}

/// docs/features/spec-check.md AC-01, amended by AC-01 of
/// docs/features/spec-check-graph.md (owner's answer Q-C): against `main`,
/// the only `[workspace.dependencies]` key that may be added is `petgraph`,
/// pinned `=0.8.3` with default features off; it is a normal dependency of
/// `specengine-core` alone among the workspace members, and no feature of it
/// is enabled in the default members' graph.
#[test]
fn no_workspace_dependency_is_added_against_main() {
    let now_text =
        std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root manifest");
    let now_manifest: toml::Table = toml::from_str(&now_text).expect("a TOML manifest");
    let now_dependencies = now_manifest["workspace"]["dependencies"]
        .as_table()
        .expect("[workspace.dependencies]");

    // The pin itself, whatever `main` holds.
    let petgraph = now_dependencies
        .get("petgraph")
        .and_then(toml::Value::as_table)
        .expect("petgraph = { version = .., default-features = false }");
    assert_eq!(
        petgraph.get("version").and_then(toml::Value::as_str),
        Some("=0.8.3"),
        "petgraph is `=`-pinned at 0.8.3"
    );
    assert_eq!(
        petgraph
            .get("default-features")
            .and_then(toml::Value::as_bool),
        Some(false),
        "petgraph's default features are off"
    );
    assert!(
        petgraph
            .get("features")
            .and_then(toml::Value::as_array)
            .is_none_or(Vec::is_empty),
        "petgraph enables no feature: {petgraph:?}"
    );
    let unexpected: Vec<&String> = petgraph
        .keys()
        .filter(|key| !["version", "default-features"].contains(&key.as_str()))
        .collect();
    assert!(unexpected.is_empty(), "petgraph: {unexpected:?}");

    // Only `specengine-core` depends on it directly, as a normal dependency.
    let output = cargo()
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata runs");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let mut direct: Vec<(String, String)> = Vec::new();
    for package in metadata["packages"].as_array().expect("packages") {
        for dependency in package["dependencies"].as_array().expect("dependencies") {
            if dependency["name"] == "petgraph" {
                direct.push((
                    package["name"].as_str().unwrap().to_owned(),
                    dependency["kind"].as_str().unwrap_or("normal").to_owned(),
                ));
            }
        }
    }
    assert_eq!(
        direct,
        [("specengine-core".to_owned(), "normal".to_owned())],
        "direct dependents of petgraph"
    );

    // No feature of petgraph is on in the default members' graph.
    let features = cargo_tree(&["-e", "features", "-i", "petgraph"]);
    let on: Vec<&str> = features
        .lines()
        .filter(|line| line.contains("petgraph feature"))
        .collect();
    assert!(
        on.is_empty(),
        "petgraph features enabled: {on:?}\n{features}"
    );
    assert!(
        tree_crates(features.lines().next().unwrap_or_default())
            .contains(&("petgraph".to_owned(), "0.8.3".to_owned())),
        "petgraph resolves at 0.8.3:\n{features}"
    );

    // Against `main`: nothing else is added.
    let output = Command::new("git")
        .current_dir(workspace_root())
        .args(["show", "main:Cargo.toml"])
        .output()
        .expect("git runs");
    if !output.status.success() {
        // No `main` here (a shallow or detached clone): nothing to compare.
        eprintln!(
            "no main:Cargo.toml: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let keys = |text: &str| -> Vec<String> {
        let manifest: toml::Table = toml::from_str(text).expect("a TOML manifest");
        let mut keys: Vec<String> = manifest["workspace"]["dependencies"]
            .as_table()
            .expect("[workspace.dependencies]")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    };
    let on_main = keys(&String::from_utf8_lossy(&output.stdout));
    let now = keys(&now_text);
    let added: Vec<&String> = now.iter().filter(|key| !on_main.contains(key)).collect();
    assert!(
        added.iter().all(|key| *key == "petgraph"),
        "[workspace.dependencies] entries added against main beyond petgraph: {added:?}"
    );
    assert!(
        now.iter().any(|key| key == "serde_json"),
        "serde_json is pinned"
    );
}

/// The parser libraries of 04 §6 / Q1, exact and resolved at one version.
const PARSER_PINS: [(&str, &str); 2] = [("pulldown-cmark", "0.13.4"), ("serde-saphyr", "1.3.0")];

#[test]
fn parser_libraries_are_pinned_exactly_and_resolve_at_the_pin() {
    let manifest =
        std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root manifest");
    for (name, version) in PARSER_PINS {
        let line = manifest
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("{name} ")))
            .unwrap_or_else(|| panic!("{name} is not in [workspace.dependencies]"));
        assert!(
            line.contains(&format!("\"={version}\"")),
            "{name} is not `=`-pinned at {version} in the root manifest: {line}"
        );
        // No `-p`/`--workspace`: the default members, as the criterion runs it.
        // (`--workspace` adds `specengine-ra`, whose `ra_ap_ide` brings
        // pulldown-cmark 0.9.6 outside the core graph.)
        let inverted = cargo_tree(&["-i", name]);
        let head = tree_crates(inverted.lines().next().unwrap_or_default());
        assert_eq!(
            head,
            [(name.to_owned(), version.to_owned())],
            "cargo tree -i {name}:\n{inverted}"
        );
        let other: Vec<(String, String)> = tree_crates(&cargo_tree(&[]))
            .into_iter()
            .filter(|(n, v)| n == name && v != version)
            .collect();
        assert!(
            other.is_empty(),
            "{name} at another version in the default members: {other:?}"
        );
        assert!(
            locked_versions(name).iter().any(|v| v == version),
            "Cargo.lock lacks {name} {version}"
        );
    }
}

// ---------------------------------------------------------------------------
// docs/features/spec-index.md AC-01 (layering) and AC-02 (pins).
// ---------------------------------------------------------------------------

/// The SQLite side of the store: never in the model's or the core's graph.
const SQLITE_CRATES: [&str; 3] = ["rusqlite", "libsqlite3-sys", "specengine-store"];

#[test]
fn model_and_core_normal_graphs_have_no_sqlite() {
    for package in ["specengine-model", "specengine-core"] {
        let graph = normal_graph(package);
        let offenders: Vec<&(String, String)> = graph
            .iter()
            .filter(|(name, _)| SQLITE_CRATES.contains(&name.as_str()) || name == "sqlx")
            .collect();
        assert!(
            offenders.is_empty(),
            "{package}'s normal graph reaches SQLite: {offenders:?}"
        );
    }
}

#[test]
fn store_depends_on_model_core_and_sqlite_but_no_measurement_crate() {
    let graph = normal_graph("specengine-store");
    let offenders: Vec<&(String, String)> = graph
        .iter()
        .filter(|(name, _)| {
            [
                "specengine-code",
                "specengine-import",
                "specengine-mcp",
                "specengine-eval",
                "specengine-ra",
            ]
            .contains(&name.as_str())
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "specengine-store depends on {offenders:?}"
    );
    for wanted in [
        "specengine-model",
        "specengine-core",
        "rusqlite",
        "libsqlite3-sys",
        "blake3",
        "serde_json",
    ] {
        assert!(
            graph.iter().any(|(name, _)| name == wanted),
            "{wanted} missing from specengine-store's graph: {graph:?}"
        );
    }
}

#[test]
fn no_sqlx_anywhere() {
    for args in [
        &[][..],
        &["--workspace"][..],
        &["--workspace", "-e", "all"][..],
    ] {
        let crates = tree_crates(&cargo_tree(args));
        let sqlx: Vec<&(String, String)> = crates
            .iter()
            .filter(|(name, _)| name == "sqlx" || name.starts_with("sqlx-"))
            .collect();
        assert!(sqlx.is_empty(), "cargo tree {args:?} lists {sqlx:?}");
    }
    let lock = std::fs::read_to_string(workspace_root().join("Cargo.lock")).expect("Cargo.lock");
    assert!(
        !lock.contains("name = \"sqlx"),
        "Cargo.lock locks a sqlx crate"
    );
}

#[test]
fn rusqlite_is_pinned_exactly_with_bundled_and_sqlite_resolves_once() {
    let manifest =
        std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root manifest");
    let section = manifest
        .split("[workspace.dependencies]")
        .nth(1)
        .expect("[workspace.dependencies] in the root manifest");
    let section = section.split("\n[").next().unwrap_or(section);
    let line = section
        .lines()
        .find(|line| line.trim_start().starts_with("rusqlite "))
        .expect("rusqlite in [workspace.dependencies]");
    assert!(
        line.contains("version = \"=0.40.2\""),
        "rusqlite is not `=`-pinned at 0.40.2: {line}"
    );
    assert!(
        line.contains("default-features = false"),
        "rusqlite's default features are not off: {line}"
    );
    assert!(
        line.contains("features = [\"bundled\"]"),
        "rusqlite is not built with exactly `bundled`: {line}"
    );
    let metadata = workspace_metadata();
    let declared = declared_dependency(&metadata, "specengine-store", "rusqlite");
    assert_eq!(
        declared["req"], "=0.40.2",
        "specengine-store's rusqlite requirement"
    );
    for name in ["rusqlite", "libsqlite3-sys", "blake3"] {
        let locked = locked_versions(name);
        assert_eq!(locked.len(), 1, "Cargo.lock holds {name} at {locked:?}");
        let versions: std::collections::BTreeSet<String> =
            tree_crates(&cargo_tree(&["--workspace"]))
                .into_iter()
                .filter(|(n, _)| n == name)
                .map(|(_, v)| v)
                .collect();
        assert_eq!(
            versions.len(),
            1,
            "cargo tree --workspace: {name} at {versions:?}"
        );
    }
    assert_eq!(locked_versions("rusqlite"), ["0.40.2"]);
}

/// AC-12 of docs/features/phase1-cleanup.md (S2): `float_roundtrip` is set
/// once, on the root `serde_json` pin, so every build of any member parses
/// floats the same; no member manifest sets a `serde_json` feature.
#[test]
fn serde_json_float_roundtrip_is_set_on_the_workspace_pin_only() {
    let root: toml::Table = toml::from_str(
        &std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root manifest"),
    )
    .expect("root manifest is TOML");
    let pin = &root["workspace"]["dependencies"]["serde_json"];
    let features: Vec<&str> = pin
        .get("features")
        .and_then(|features| features.as_array())
        .map(|features| features.iter().filter_map(|f| f.as_str()).collect())
        .unwrap_or_default();
    assert!(
        features.contains(&"float_roundtrip"),
        "root serde_json pin: {pin:?}"
    );
    assert_eq!(
        pin.get("version").and_then(|v| v.as_str()),
        Some("=1.0.151"),
        "the pin itself is unchanged: {pin:?}"
    );

    /// Every `serde_json` entry of a manifest table and its `target.*`
    /// tables, as `(table, entry)`.
    fn serde_json_entries(manifest: &toml::Table) -> Vec<(String, toml::Value)> {
        let mut out = Vec::new();
        let mut tables: Vec<(String, &toml::Table)> = vec![("".to_owned(), manifest)];
        if let Some(targets) = manifest.get("target").and_then(|t| t.as_table()) {
            for (name, target) in targets {
                if let Some(target) = target.as_table() {
                    tables.push((format!("target.{name}."), target));
                }
            }
        }
        for (prefix, table) in tables {
            for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(entry) = table
                    .get(kind)
                    .and_then(|deps| deps.as_table())
                    .and_then(|deps| deps.get("serde_json"))
                {
                    out.push((format!("{prefix}{kind}"), entry.clone()));
                }
            }
        }
        out
    }

    let metadata = workspace_metadata();
    let mut checked = 0;
    for package in metadata["packages"].as_array().expect("packages") {
        let manifest_path = package["manifest_path"].as_str().expect("manifest_path");
        let manifest: toml::Table =
            toml::from_str(&std::fs::read_to_string(manifest_path).expect("member manifest"))
                .expect("member manifest is TOML");
        for (table, entry) in serde_json_entries(&manifest) {
            checked += 1;
            assert!(
                entry.get("features").is_none() && entry.get("default-features").is_none(),
                "{manifest_path} [{table}] sets serde_json features: {entry:?}"
            );
            assert_eq!(
                entry.get("workspace").and_then(|w| w.as_bool()),
                Some(true),
                "{manifest_path} [{table}] serde_json is the workspace pin: {entry:?}"
            );
        }
    }
    assert!(checked >= 4, "serde_json entries checked: {checked}");
}

// ---------------------------------------------------------------------------
// docs/features/spec-cli.md AC-01 (the `spec` binary's graph).
// ---------------------------------------------------------------------------

/// Crates the CLI's normal graph must never reach (by exact name); every
/// `ra_ap_*` is checked by prefix. `syn` is checked apart: it enters the
/// compile-time graph through the `clap_derive` and `serde_derive`
/// proc-macros (as it does the model's and the core's), so the check is on
/// the linked graph (`-e normal,no-proc-macro`), as
/// `core_tree_has_no_rust_analyzer_syn3_or_bevy` does.
const CLI_FORBIDDEN: [&str; 7] = [
    "specengine-code",
    "specengine-import",
    "specengine-mcp",
    "specengine-eval",
    "specengine-ra",
    "tokio",
    "rmcp",
];

#[test]
fn cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer() {
    let graph = normal_graph("specengine-cli");
    for wanted in [
        "specengine-model",
        "specengine-core",
        "specengine-store",
        "clap",
        "serde",
        "serde_json",
    ] {
        assert!(
            graph.iter().any(|(name, _)| name == wanted),
            "{wanted} missing from specengine-cli's normal graph: {graph:?}"
        );
    }
    let offenders: Vec<&(String, String)> = graph
        .iter()
        .filter(|(name, _)| CLI_FORBIDDEN.contains(&name.as_str()) || name.starts_with("ra_ap_"))
        .collect();
    assert!(
        offenders.is_empty(),
        "specengine-cli's normal graph reaches {offenders:?}"
    );
    let linked = tree_crates(&cargo_tree(&[
        "-p",
        "specengine-cli",
        "-e",
        "normal,no-proc-macro",
    ]));
    assert!(
        linked
            .first()
            .is_some_and(|(name, _)| name == "specengine-cli"),
        "cargo tree lists specengine-cli first: {linked:?}"
    );
    let linked_offenders: Vec<&(String, String)> = linked
        .iter()
        .filter(|(name, _)| {
            name == "syn" || CLI_FORBIDDEN.contains(&name.as_str()) || name.starts_with("ra_ap_")
        })
        .collect();
    assert!(
        linked_offenders.is_empty(),
        "specengine-cli's linked graph reaches {linked_offenders:?}"
    );
}

/// `rusqlite` reaches the CLI only through the store: it is in the graph,
/// but not at depth 1, and every path to it runs through `specengine-store`
/// (`cargo tree -i rusqlite` lists no other dependent inside the CLI graph).
#[test]
fn cli_reaches_rusqlite_only_through_the_store() {
    let direct = cargo_tree(&["-p", "specengine-cli", "-e", "normal", "--depth", "1"]);
    let direct_crates = tree_crates(&direct);
    assert!(
        direct_crates
            .first()
            .is_some_and(|(name, _)| name == "specengine-cli"),
        "cargo tree lists specengine-cli first:\n{direct}"
    );
    let direct_names: Vec<&str> = direct_crates
        .iter()
        .skip(1)
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(
        !direct_names
            .iter()
            .any(|name| *name == "rusqlite" || *name == "libsqlite3-sys"),
        "specengine-cli depends on SQLite directly: {direct_names:?}"
    );
    assert!(
        direct_names.contains(&"specengine-store"),
        "specengine-cli does not depend on the store directly: {direct_names:?}"
    );
    // Who depends on rusqlite within the CLI's graph: the store alone.
    let inverted = cargo_tree(&[
        "-p",
        "specengine-cli",
        "-e",
        "normal",
        "-i",
        "rusqlite",
        "--depth",
        "1",
    ]);
    let dependents: Vec<String> = tree_crates(&inverted)
        .into_iter()
        .skip(1)
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        dependents,
        ["specengine-store"],
        "rusqlite's dependents in the CLI graph:\n{inverted}"
    );
}

/// Every direct dependency of `specengine-cli` is either a path dependency
/// on a workspace crate or a `[workspace.dependencies]` entry
/// (`name.workspace = true`): no version is pinned in the member manifest.
#[test]
fn cli_direct_dependencies_are_all_workspace_entries() {
    let manifest_path = workspace_root().join("crates/specengine-cli/Cargo.toml");
    let text = std::fs::read_to_string(&manifest_path).expect("crates/specengine-cli/Cargo.toml");
    let manifest: toml::Table = text.parse().expect("the CLI manifest is TOML");
    let root: toml::Table = std::fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("Cargo.toml")
        .parse()
        .expect("the root manifest is TOML");
    let pins = root["workspace"]["dependencies"]
        .as_table()
        .expect("[workspace.dependencies]");
    let mut seen = Vec::new();
    for table in ["dependencies", "build-dependencies"] {
        let Some(deps) = manifest.get(table).and_then(toml::Value::as_table) else {
            continue;
        };
        for (name, spec) in deps {
            seen.push(name.clone());
            let spec = spec
                .as_table()
                .unwrap_or_else(|| panic!("{table}.{name} pins a version in the member: {spec:?}"));
            if let Some(path) = spec.get("path").and_then(toml::Value::as_str) {
                assert!(
                    name.starts_with("specengine-") && path == format!("../{name}"),
                    "{table}.{name}: a path dependency outside the workspace crates: {path}"
                );
                assert!(
                    spec.keys().all(|key| key == "path"),
                    "{table}.{name}: a path dependency with extra keys: {spec:?}"
                );
                continue;
            }
            assert_eq!(
                spec.get("workspace").and_then(toml::Value::as_bool),
                Some(true),
                "{table}.{name} is not a workspace entry: {spec:?}"
            );
            assert!(
                spec.get("version").is_none(),
                "{table}.{name} pins its own version: {spec:?}"
            );
            assert!(
                pins.contains_key(name),
                "{table}.{name} has no [workspace.dependencies] entry"
            );
        }
    }
    seen.sort_unstable();
    assert_eq!(
        seen,
        [
            "clap",
            "serde",
            "serde_json",
            "specengine-core",
            "specengine-model",
            "specengine-store",
        ],
        "the CLI's direct dependencies (docs/features/spec-cli.md, Crate)"
    );
}

// ---------------------------------------------------------------------------
// docs/features/spec-cli-check.md AC-01 (pass 2a.1 adds no dependency).
// ---------------------------------------------------------------------------

/// `spec check` and `spec export index` add nothing to the CLI's graph:
/// its direct dependencies stay pass 1's
/// (`cli_direct_dependencies_are_all_workspace_entries`), and no git
/// library reaches its normal graph (2a.2 reads git through
/// `std::process`, 05 §9).
#[test]
fn cli_normal_graph_has_no_git_library() {
    let graph = normal_graph("specengine-cli");
    let git: Vec<&(String, String)> = graph
        .iter()
        .filter(|(name, _)| {
            name == "git2" || name == "libgit2-sys" || name == "gix" || name.starts_with("gix-")
        })
        .collect();
    assert!(
        git.is_empty(),
        "specengine-cli's normal graph reaches a git library: {git:?}"
    );
}

// ---------------------------------------------------------------------------
// docs/features/spec-cli-staged.md AC-01 (pass 2a.2 adds no dependency).
// ---------------------------------------------------------------------------

/// The `[dependencies]` keys of `crates/<package>/Cargo.toml`, sorted, and
/// whether the manifest has a target-specific dependency table.
fn declared_normal_dependencies(package: &str) -> (Vec<String>, bool) {
    let path = workspace_root()
        .join("crates")
        .join(package)
        .join("Cargo.toml");
    let text = std::fs::read_to_string(&path).expect("the member's manifest");
    let manifest: toml::Table = text.parse().expect("a TOML manifest");
    let mut names: Vec<String> = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .map(|deps| deps.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    let targeted = manifest
        .get("target")
        .and_then(toml::Value::as_table)
        .is_some_and(|targets| {
            targets.values().any(|target| {
                target
                    .as_table()
                    .is_some_and(|table| table.contains_key("dependencies"))
            })
        });
    (names, targeted)
}

/// Git libraries and process crates: `spec check --staged` reads git
/// through `std::process` only (05 §9).
fn git_or_process_crate(name: &str) -> bool {
    name == "git2"
        || name == "libgit2-sys"
        || name == "gix"
        || name.starts_with("gix-")
        || [
            "duct",
            "subprocess",
            "command-group",
            "os_pipe",
            "shared_child",
            "process_control",
            "wait-timeout",
            "which",
            "tokio",
            "async-process",
        ]
        .contains(&name)
}

/// The store's and the CLI's normal dependencies stay 2a.1's: the same
/// `[dependencies]` keys, no target-specific table, and no git library or
/// process crate anywhere in their normal graphs.
#[test]
fn store_and_cli_normal_dependencies_are_2a1_s() {
    for (package, expected) in [
        (
            "specengine-store",
            &[
                "blake3",
                "rusqlite",
                "serde",
                "serde_json",
                "specengine-core",
                "specengine-model",
            ][..],
        ),
        (
            "specengine-cli",
            &[
                "clap",
                "serde",
                "serde_json",
                "specengine-core",
                "specengine-model",
                "specengine-store",
            ][..],
        ),
    ] {
        let (names, targeted) = declared_normal_dependencies(package);
        assert_eq!(names, expected, "{package}'s [dependencies]");
        assert!(
            !targeted,
            "{package} has a target-specific dependency table"
        );
        let graph = normal_graph(package);
        let offenders: Vec<&(String, String)> = graph
            .iter()
            .filter(|(name, _)| git_or_process_crate(name))
            .collect();
        assert!(
            offenders.is_empty(),
            "{package}'s normal graph reaches {offenders:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// docs/features/mcp-read.md AC-17: the MCP server reads through the CLI
// library only.
// ---------------------------------------------------------------------------

/// The MCP server's `[dependencies]`, exactly.
const MCP_DEPENDENCIES: [&str; 7] = [
    "clap",
    "getrandom",
    "rmcp",
    "serde",
    "serde_json",
    "specengine-cli",
    "tokio",
];

/// `specengine-mcp` declares exactly [`MCP_DEPENDENCIES`]: `specengine-cli`
/// by path, every other a `[workspace.dependencies]` entry with no version,
/// feature or default-features of its own (no tokio feature added);
/// `getrandom` is optional and only feature `probes` enables it, which is no
/// default feature; no target-specific table. The workspace pins of `rmcp`,
/// `tokio` and `getrandom` are `main`'s.
#[test]
fn mcp_direct_dependencies_are_the_cli_library_and_the_protocol_crates() {
    let (names, targeted) = declared_normal_dependencies("specengine-mcp");
    assert_eq!(
        names, MCP_DEPENDENCIES,
        "specengine-mcp's [dependencies] (docs/features/mcp-read.md AC-17)"
    );
    assert!(
        !targeted,
        "specengine-mcp has a target-specific dependency table"
    );
    let text = std::fs::read_to_string(workspace_root().join("crates/specengine-mcp/Cargo.toml"))
        .expect("crates/specengine-mcp/Cargo.toml");
    let manifest: toml::Table = text.parse().expect("the MCP manifest is TOML");
    let deps = manifest["dependencies"].as_table().expect("[dependencies]");
    for (name, spec) in deps {
        let spec = spec
            .as_table()
            .unwrap_or_else(|| panic!("dependencies.{name} pins a version: {spec:?}"));
        if name == "specengine-cli" {
            assert_eq!(
                spec.get("path").and_then(toml::Value::as_str),
                Some("../specengine-cli"),
                "{spec:?}"
            );
            assert_eq!(spec.len(), 1, "specengine-cli: extra keys {spec:?}");
            continue;
        }
        assert_eq!(
            spec.get("workspace").and_then(toml::Value::as_bool),
            Some(true),
            "dependencies.{name} is not a workspace entry: {spec:?}"
        );
        let allowed: &[&str] = if name == "getrandom" {
            &["workspace", "optional"]
        } else {
            &["workspace"]
        };
        let extra: Vec<&String> = spec
            .keys()
            .filter(|key| !allowed.contains(&key.as_str()))
            .collect();
        assert!(extra.is_empty(), "dependencies.{name}: {extra:?}");
    }
    assert_eq!(
        deps["getrandom"]
            .get("optional")
            .and_then(toml::Value::as_bool),
        Some(true),
        "getrandom is optional"
    );
    let features = manifest["features"].as_table().expect("[features]");
    assert_eq!(
        features.get("probes"),
        Some(&toml::Value::Array(vec![toml::Value::String(
            "dep:getrandom".to_owned()
        )])),
        "probes enables getrandom"
    );
    let default: Vec<String> = features
        .get("default")
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        default.is_empty(),
        "default features of specengine-mcp: {default:?}"
    );

    // The workspace pins the MCP server takes are main's.
    let output = Command::new("git")
        .current_dir(workspace_root())
        .args(["show", "main:Cargo.toml"])
        .output()
        .expect("git runs");
    if !output.status.success() {
        eprintln!(
            "no main:Cargo.toml: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let on_main: toml::Table =
        toml::from_str(&String::from_utf8_lossy(&output.stdout)).expect("main's manifest");
    let now: toml::Table = toml::from_str(
        &std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root manifest"),
    )
    .expect("the root manifest");
    for name in ["rmcp", "tokio", "getrandom", "clap", "serde", "serde_json"] {
        assert_eq!(
            now["workspace"]["dependencies"].get(name),
            on_main["workspace"]["dependencies"].get(name),
            "[workspace.dependencies].{name} changed against main"
        );
    }
}

/// `rusqlite` and the store reach the MCP server only through the CLI
/// library: neither is a direct dependency, the store's only dependent in
/// the MCP graph is `specengine-cli`, rusqlite's is the store; `getrandom`
/// is direct only with feature `probes`.
#[test]
fn mcp_reaches_the_store_and_rusqlite_only_through_the_cli() {
    for features in [&[][..], &["--features", "specengine-mcp/probes"][..]] {
        let mut args = vec!["-p", "specengine-mcp", "-e", "normal", "--depth", "1"];
        args.extend_from_slice(features);
        let direct = cargo_tree(&args);
        let crates = tree_crates(&direct);
        assert!(
            crates
                .first()
                .is_some_and(|(name, _)| name == "specengine-mcp"),
            "cargo tree lists specengine-mcp first:\n{direct}"
        );
        let mut names: Vec<&str> = crates
            .iter()
            .skip(1)
            .map(|(name, _)| name.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        let want: Vec<&str> = MCP_DEPENDENCIES
            .iter()
            .copied()
            .filter(|name| !features.is_empty() || *name != "getrandom")
            .collect();
        assert_eq!(
            names, want,
            "direct dependencies with {features:?}:\n{direct}"
        );
        for (inverted, dependents) in [
            ("specengine-store", ["specengine-cli"]),
            ("rusqlite", ["specengine-store"]),
        ] {
            let mut args = vec![
                "-p",
                "specengine-mcp",
                "-e",
                "normal",
                "-i",
                inverted,
                "--depth",
                "1",
            ];
            args.extend_from_slice(features);
            let tree = cargo_tree(&args);
            let found: Vec<String> = tree_crates(&tree)
                .into_iter()
                .skip(1)
                .map(|(name, _)| name)
                .collect();
            assert_eq!(found, dependents, "{inverted}'s dependents:\n{tree}");
        }
    }
}

/// docs/features/layer-a-identity.md AC-14: `specengine-code` keeps exactly
/// `tree-sitter`, `tree-sitter-rust` and `blake3` as `[dependencies]`, each a
/// workspace entry, and nothing under a target table or as a build
/// dependency (the target table and the cargo call live in the harness).
#[test]
fn code_crate_depends_on_exactly_the_parser_and_the_hash() {
    let text = std::fs::read_to_string(workspace_root().join("crates/specengine-code/Cargo.toml"))
        .expect("specengine-code manifest");
    let manifest: toml::Table = toml::from_str(&text).expect("a TOML manifest");
    let dependencies = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .expect("[dependencies]");
    let mut names: Vec<&str> = dependencies.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["blake3", "tree-sitter", "tree-sitter-rust"],
        "specengine-code [dependencies]"
    );
    for (name, value) in dependencies {
        let table = value.as_table();
        assert!(
            table
                .is_some_and(|t| t.len() == 1
                    && t.get("workspace").and_then(toml::Value::as_bool) == Some(true)),
            "{name} must be `.workspace = true` only, got {value:?}"
        );
    }
    assert!(
        !manifest.contains_key("build-dependencies") && !manifest.contains_key("target"),
        "no build or target-specific dependency in specengine-code"
    );

    let metadata = workspace_metadata();
    let package = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|p| p["name"] == "specengine-code")
        .expect("specengine-code in the workspace");
    let mut normal: Vec<&str> = package["dependencies"]
        .as_array()
        .expect("dependencies")
        .iter()
        .filter(|d| d["kind"].is_null())
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    normal.sort_unstable();
    assert_eq!(normal, ["blake3", "tree-sitter", "tree-sitter-rust"]);
}

// ---------------------------------------------------------------------------
// docs/features/pilot-w.md AC-09: `w` reaches `spec bundle` and `spec show`
// through the CLI library; eval gains exactly that edge, the CLI nothing.
// ---------------------------------------------------------------------------

/// Direct dependencies of a workspace package by kind (`normal`, `dev`,
/// `build`) and whether optional: (name, kind, optional), sorted.
fn direct_dependencies(metadata: &Value, package: &str) -> Vec<(String, String, bool)> {
    let package = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|p| p["name"] == package)
        .unwrap_or_else(|| panic!("no workspace package {package}"));
    let mut out: Vec<(String, String, bool)> = package["dependencies"]
        .as_array()
        .expect("dependencies")
        .iter()
        .map(|d| {
            (
                d["name"].as_str().unwrap().to_owned(),
                d["kind"].as_str().unwrap_or("normal").to_owned(),
                d["optional"].as_bool().unwrap_or(false),
            )
        })
        .collect();
    out.sort();
    out
}

/// AC-09: eval's direct normal dependencies are the set it had before `w`
/// plus `specengine-cli` — a path dependency on the workspace crate, no
/// features, not optional (M2: another dependency → red); its optional
/// ones are unchanged.
#[test]
fn eval_direct_dependencies_gain_exactly_the_cli_library() {
    let metadata = workspace_metadata();
    let direct = direct_dependencies(&metadata, "specengine-eval");
    let normal: Vec<&str> = direct
        .iter()
        .filter(|(_, kind, optional)| kind == "normal" && !optional)
        .map(|(name, _, _)| name.as_str())
        .collect();
    assert_eq!(
        normal,
        [
            "clap",
            "serde",
            "serde_json",
            "specengine-cli",
            "specengine-code",
            "specengine-core",
            "specengine-import",
            "specengine-model",
            "specengine-store",
            "toml",
        ],
        "eval's direct normal dependencies"
    );
    let optional: Vec<&str> = direct
        .iter()
        .filter(|(_, kind, optional)| kind == "normal" && *optional)
        .map(|(name, _, _)| name.as_str())
        .collect();
    assert_eq!(
        optional,
        ["libc", "proc-macro2", "quote", "specengine-ra", "syn"],
        "eval's optional dependencies"
    );
    let cli = declared_dependency(&metadata, "specengine-eval", "specengine-cli");
    assert!(cli["source"].is_null(), "a path dependency: {cli}");
    let path = cli["path"].as_str().expect("a path dependency");
    assert!(
        Path::new(path).ends_with("crates/specengine-cli"),
        "the workspace crate: {path}"
    );
    assert_eq!(
        cli["features"].as_array().map(Vec::len),
        Some(0),
        "no features: {cli}"
    );
    assert_eq!(cli["uses_default_features"], true, "{cli}");
    assert_eq!(cli["optional"], false, "{cli}");
    assert_eq!(cli["req"], "*", "no version requirement: {cli}");

    // The manifest entry itself is `{ path = "../specengine-cli" }`.
    let text = std::fs::read_to_string(workspace_root().join("crates/specengine-eval/Cargo.toml"))
        .expect("the eval manifest");
    let manifest: toml::Table = text.parse().expect("TOML");
    let entry = manifest["dependencies"]["specengine-cli"]
        .as_table()
        .expect("a table entry");
    assert_eq!(entry.len(), 1, "{entry:?}");
    assert_eq!(
        entry.get("path").and_then(toml::Value::as_str),
        Some("../specengine-cli")
    );
}

/// AC-09: the CLI's own direct dependencies are unchanged by `w`: the
/// model, core and store, `clap`, `serde`, `serde_json`, nothing else of
/// any kind (its normal graph is pinned by
/// `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`,
/// `CLI_FORBIDDEN` holding `specengine-eval`).
#[test]
fn cli_direct_dependencies_are_unchanged_by_w() {
    let metadata = workspace_metadata();
    let direct = direct_dependencies(&metadata, "specengine-cli");
    let normal: Vec<&str> = direct
        .iter()
        .filter(|(_, kind, _)| kind == "normal")
        .map(|(name, _, _)| name.as_str())
        .collect();
    assert_eq!(
        normal,
        [
            "clap",
            "serde",
            "serde_json",
            "specengine-core",
            "specengine-model",
            "specengine-store",
        ],
        "the CLI's direct normal dependencies"
    );
    assert!(
        direct.iter().all(|(_, _, optional)| !optional),
        "{direct:?}"
    );
    assert!(CLI_FORBIDDEN.contains(&"specengine-eval"));
}
