//! The UI's file-level policies (docs/features/ui-shell.md AC-01, AC-03,
//! AC-04, AC-05, AC-06, AC-08, AC-15, AC-19, AC-20, AC-21; rules in
//! `ui/README.md` "Dependencies", "Gates", "Laptop rules"), read as text from
//! `ui/` without running pnpm: the gates themselves stay commands.
//!
//! - AC-01: `package.json` names exactly the owner's 15, each at an exact
//!   `x.y.z`, equal to the README table once it holds versions ("pinned at
//!   install" is the placeholder until then); `react` = `react-dom`, 19.x;
//!   `packageManager` an exact pnpm; the lockfile present and its importer
//!   specifiers equal to `package.json`; `.npmrc` per policy; no install
//!   script, no `onlyBuiltDependencies`.
//! - AC-03, AC-21: the scripts, `test` = `vitest run`, Vitest `watch: false`
//!   with at most 2 workers.
//! - AC-04, AC-08, AC-15, AC-19: forbidden text in `ui/src` (browser
//!   dialogs, colour literals outside `tokens.css`, the words for a hold on
//!   work, a mock kind quoted outside the mocks and tests).
//! - AC-05, AC-06: the seam's imports, the provisional header, a citation per
//!   exported type, `kind` and `contour` plain strings. The cited headings
//!   themselves are checked by `doc_pointers.rs`.
//!
//! Named mutations (each turns this red): `globals` added; one `^`; the
//! lockfile deleted; `node-linker=hoisted`; a `postinstall`; `window.confirm`
//! in a component; `color: #fff` in a component; a "Blocking" label;
//! `kind === "mechanic"` in a component; the header dropped; `kind` a union;
//! `test` = `vitest`; the worker cap removed; `ui` out of roots.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn ui_dir() -> PathBuf {
    repository_root().join("ui")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The owner's allowlist of 2026-10-05 (`ui/README.md` "Dependencies").
const ALLOWLIST: [&str; 15] = [
    "react",
    "react-dom",
    "@tanstack/react-query",
    "@xyflow/react",
    "vite",
    "@vitejs/plugin-react",
    "typescript",
    "@types/react",
    "@types/react-dom",
    "eslint",
    "typescript-eslint",
    "eslint-plugin-react-hooks",
    "vitest",
    "@testing-library/react",
    "jsdom",
];

/// The README's version cell before `spec-writer` records the pins.
const PLACEHOLDER: &str = "pinned at install";

/// `x.y.z`, each part decimal digits without a leading zero: no range, tag,
/// prerelease, URL, `file:`, `link:` or alias.
fn is_exact_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
        })
}

fn package_json() -> Value {
    let path = ui_dir().join("package.json");
    serde_json::from_str(&read(&path)).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// `dependencies` and `devDependencies` together, name → version text; a
/// name in both is reported.
fn declared_dependencies(package: &Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for field in ["dependencies", "devDependencies"] {
        let Some(table) = package.get(field) else {
            continue;
        };
        let table = table
            .as_object()
            .unwrap_or_else(|| panic!("package.json {field} is not an object"));
        for (name, version) in table {
            let version = version
                .as_str()
                .unwrap_or_else(|| panic!("package.json {field}.{name} is not a string"));
            let previous = out.insert(name.clone(), version.to_owned());
            assert!(
                previous.is_none(),
                "{name} is in both dependencies and devDependencies"
            );
        }
    }
    out
}

/// The `ui/README.md` "Dependencies" table: package → version cell.
fn readme_table() -> BTreeMap<String, String> {
    let text = read(&ui_dir().join("README.md"));
    let mut in_section = false;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            in_section = line == "## Dependencies";
            continue;
        }
        if !in_section || !line.starts_with("| `") {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // ["", name, version, role, ""]
        assert!(cells.len() >= 4, "a table row without cells: {line}");
        let name = cells[1].trim_matches('`').to_owned();
        let version = cells[2].trim_matches('`').to_owned();
        assert!(
            out.insert(name.clone(), version).is_none(),
            "{name} twice in the table"
        );
    }
    out
}

/// The lockfile's root importer: name → specifier, both sections together.
fn lockfile_specifiers(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut in_root = false;
    let mut in_section = false;
    let mut current: Option<String> = None;
    for line in text.lines() {
        if !line.starts_with(' ') && !line.is_empty() {
            in_root = false;
            in_section = false;
        }
        if line == "  .:" {
            in_root = true;
            continue;
        }
        if line.starts_with("  ") && !line.starts_with("   ") && line != "  .:" {
            in_root = false;
        }
        if !in_root {
            continue;
        }
        if line == "    dependencies:" || line == "    devDependencies:" {
            in_section = true;
            continue;
        }
        if line.starts_with("    ") && !line.starts_with("     ") {
            in_section = false;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(name) = line
            .strip_prefix("      ")
            .filter(|rest| !rest.starts_with(' '))
        {
            let name = name.trim_end_matches(':').trim_matches('\'');
            current = Some(name.to_owned());
        } else if let Some(specifier) = line.trim_start().strip_prefix("specifier: ") {
            let name = current.take().expect("a specifier under a package name");
            out.insert(name, specifier.trim_matches('\'').to_owned());
        }
    }
    out
}

#[test]
fn package_json_pins_exactly_the_fifteen_at_exact_versions() {
    let package = package_json();
    let declared = declared_dependencies(&package);
    let names: BTreeSet<&str> = declared.keys().map(String::as_str).collect();
    let allowed: BTreeSet<&str> = ALLOWLIST.into_iter().collect();
    assert_eq!(names, allowed, "package.json names against the allowlist");
    let inexact: Vec<String> = declared
        .iter()
        .filter(|(_, version)| !is_exact_version(version))
        .map(|(name, version)| format!("{name}: {version:?}"))
        .collect();
    assert!(inexact.is_empty(), "not an exact x.y.z: {inexact:?}");
    for field in [
        "peerDependencies",
        "optionalDependencies",
        "bundleDependencies",
        "bundledDependencies",
    ] {
        assert!(
            package.get(field).is_none(),
            "package.json declares {field}: the 15 live in dependencies and devDependencies"
        );
    }
    let react = &declared["react"];
    assert_eq!(react, &declared["react-dom"], "react and react-dom differ");
    assert!(react.starts_with("19."), "react {react} is not 19.x");
    assert_eq!(
        package["private"],
        Value::Bool(true),
        "package.json private"
    );
    assert_eq!(package["type"], Value::from("module"), "package.json type");
}

#[test]
fn the_readme_table_names_the_same_fifteen_and_its_versions_once_filled() {
    let declared = declared_dependencies(&package_json());
    let table = readme_table();
    let table_names: BTreeSet<&str> = table.keys().map(String::as_str).collect();
    let allowed: BTreeSet<&str> = ALLOWLIST.into_iter().collect();
    assert_eq!(
        table_names, allowed,
        "the README table against the allowlist"
    );
    let mut placeholders = 0;
    for (name, cell) in &table {
        if cell == PLACEHOLDER {
            placeholders += 1;
            continue;
        }
        assert!(
            is_exact_version(cell),
            "README {name}: {cell:?} is neither a version nor {PLACEHOLDER:?}"
        );
        assert_eq!(
            Some(cell),
            declared.get(name),
            "README {name} against package.json"
        );
    }
    if placeholders > 0 {
        eprintln!("{placeholders} README version cells still say {PLACEHOLDER:?}");
    }
}

#[test]
fn package_manager_is_an_exact_pnpm() {
    let package = package_json();
    let manager = package["packageManager"]
        .as_str()
        .expect("packageManager is a string");
    let version = manager
        .strip_prefix("pnpm@")
        .unwrap_or_else(|| panic!("packageManager {manager:?} is not pnpm"));
    assert!(
        is_exact_version(version),
        "packageManager {manager:?} is not an exact pnpm"
    );
}

#[test]
fn the_lockfile_is_present_and_matches_package_json() {
    let path = ui_dir().join("pnpm-lock.yaml");
    assert!(path.is_file(), "{} is missing", path.display());
    let text = read(&path);
    assert!(
        text.starts_with("lockfileVersion: '9.0'"),
        "unexpected lockfile header: {:?}",
        text.lines().next()
    );
    let declared = declared_dependencies(&package_json());
    assert_eq!(
        lockfile_specifiers(&text),
        declared,
        "the lockfile's root importer against package.json (relock needed)"
    );
}

#[test]
fn the_lockfile_reader_takes_only_the_root_importer() {
    let text = concat!(
        "lockfileVersion: '9.0'\n",
        "\n",
        "importers:\n",
        "\n",
        "  .:\n",
        "    dependencies:\n",
        "      '@scope/a':\n",
        "        specifier: 1.2.3\n",
        "        version: 1.2.3(b@4.5.6)\n",
        "    devDependencies:\n",
        "      b:\n",
        "        specifier: 4.5.6\n",
        "        version: 4.5.6\n",
        "\n",
        "  other:\n",
        "    dependencies:\n",
        "      c:\n",
        "        specifier: 7.8.9\n",
        "        version: 7.8.9\n",
        "\n",
        "packages:\n",
        "\n",
        "  'd@1.0.0':\n",
        "    resolution: {integrity: x}\n",
    );
    let expected: BTreeMap<String, String> = [("@scope/a", "1.2.3"), ("b", "4.5.6")]
        .into_iter()
        .map(|(name, version)| (name.to_owned(), version.to_owned()))
        .collect();
    assert_eq!(lockfile_specifiers(text), expected);
}

#[test]
fn exact_versions_reject_ranges_tags_and_paths() {
    for good in ["0.0.0", "19.3.0", "10.28.2", "5.104.0"] {
        assert!(is_exact_version(good), "{good}");
    }
    for bad in [
        "^19.3.0",
        "~19.3.0",
        ">=19",
        "19.x",
        "19.3",
        "latest",
        "19.3.0-rc.1",
        "01.2.3",
        "file:../x",
        "link:../x",
        "npm:react@19.3.0",
        "https://example.invalid/x.tgz",
        "19.3.0 || 19.4.0",
    ] {
        assert!(!is_exact_version(bad), "{bad}");
    }
}

/// `key=value` lines of an `.npmrc`, comments and blanks skipped, keys
/// lower-cased.
fn npmrc_entries(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with(';'))
        .map(|line| match line.split_once('=') {
            Some((key, value)) => (key.trim().to_lowercase(), value.trim().to_owned()),
            None => (line.to_lowercase(), String::new()),
        })
        .collect()
}

#[test]
fn npmrc_and_workspace_settings_follow_the_policy() {
    let ui = ui_dir();
    let npmrc = npmrc_entries(&read(&ui.join(".npmrc")));
    assert_eq!(
        npmrc.get("save-exact").map(String::as_str),
        Some("true"),
        ".npmrc save-exact"
    );
    assert_eq!(
        npmrc.get("strict-peer-dependencies").map(String::as_str),
        Some("true"),
        ".npmrc strict-peer-dependencies"
    );
    if let Some(linker) = npmrc.get("node-linker") {
        assert_eq!(linker, "isolated", ".npmrc node-linker={linker}");
    }
    for key in [
        "shamefully-hoist",
        "only-built-dependencies",
        "only-built-dependencies-file",
        "dangerously-allow-all-builds",
    ] {
        assert!(
            !npmrc.contains_key(key),
            ".npmrc sets {key}: dependency install scripts stay off, the linker isolated"
        );
    }
    let workspace = ui.join("pnpm-workspace.yaml");
    if workspace.is_file() {
        let text = read(&workspace);
        for key in [
            "nodeLinker",
            "shamefullyHoist",
            "onlyBuiltDependencies",
            "onlyBuiltDependenciesFile",
            "dangerouslyAllowAllBuilds",
        ] {
            assert!(
                !text
                    .lines()
                    .any(|line| line.trim_start().starts_with(&format!("{key}:"))),
                "pnpm-workspace.yaml sets {key}"
            );
        }
    }
    let package = package_json();
    if let Some(pnpm) = package.get("pnpm") {
        for key in ["onlyBuiltDependencies", "onlyBuiltDependenciesFile"] {
            assert!(pnpm.get(key).is_none(), "package.json pnpm.{key}");
        }
    }
}

/// npm lifecycle scripts that run on install.
const INSTALL_SCRIPTS: [&str; 7] = [
    "preinstall",
    "install",
    "postinstall",
    "prepare",
    "preprepare",
    "postprepare",
    "prepublish",
];

fn scripts() -> BTreeMap<String, String> {
    package_json()["scripts"]
        .as_object()
        .expect("package.json scripts")
        .iter()
        .map(|(name, command)| {
            (
                name.clone(),
                command.as_str().expect("a script is a string").to_owned(),
            )
        })
        .collect()
}

#[test]
fn scripts_are_the_gates_plus_dev_and_none_runs_on_install() {
    let scripts = scripts();
    let install: Vec<&String> = scripts
        .keys()
        .filter(|name| INSTALL_SCRIPTS.contains(&name.as_str()))
        .collect();
    assert!(install.is_empty(), "install scripts: {install:?}");
    let names: Vec<&str> = scripts.keys().map(String::as_str).collect();
    assert_eq!(names, ["build", "dev", "lint", "test"], "the scripts");
    let lint = &scripts["lint"];
    assert!(
        lint.starts_with("eslint ") && lint.contains("--max-warnings=0"),
        "lint {lint:?} does not fail on a warning"
    );
    let build = &scripts["build"];
    assert!(
        build.contains("tsc --noEmit") && build.contains("vite build"),
        "build {build:?} is not `tsc --noEmit` and `vite build`"
    );
}

#[test]
fn tests_run_once_with_at_most_two_workers() {
    let scripts = scripts();
    assert_eq!(scripts["test"], "vitest run", "test is not `vitest run`");
    let config = read(&ui_dir().join("vite.config.ts"));
    assert!(
        config.lines().any(|line| line.trim() == "watch: false,"),
        "vite.config.ts does not set `watch: false`"
    );
    let workers: Vec<u32> = config
        .lines()
        .filter_map(|line| line.trim().strip_prefix("maxWorkers:"))
        .map(|rest| {
            rest.trim()
                .trim_end_matches(',')
                .parse()
                .unwrap_or_else(|_| panic!("maxWorkers {rest:?} is not a number"))
        })
        .collect();
    assert!(
        matches!(workers.as_slice(), [n] if (1..=2).contains(n)),
        "vite.config.ts maxWorkers: {workers:?}, want one value <= 2"
    );
}

/// Every file under `ui/src`, as (path relative to `ui/` with `/`, text),
/// sorted by path.
fn ui_sources() -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
            .map(|entry| entry.expect("directory entry").path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            if path.is_dir() {
                if name.as_deref() != Some("node_modules") {
                    walk(root, &path, out);
                }
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("under ui/")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((relative, read(&path)));
            }
        }
    }
    let ui = ui_dir();
    let mut out = Vec::new();
    walk(&ui, &ui.join("src"), &mut out);
    assert!(
        out.len() > 20 && out.iter().any(|(path, _)| path == "src/main.tsx"),
        "the walk found only {} files under ui/src",
        out.len()
    );
    out
}

/// `path:line: text` for each line of `files` that `needle` accepts.
fn hits(
    files: &[(String, String)],
    skip: impl Fn(&str) -> bool,
    needle: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut out = Vec::new();
    for (path, text) in files {
        if skip(path) {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if needle(line) {
                out.push(format!("ui/{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    out
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// `name(` in `line` as a call: not the tail of a longer identifier.
fn calls(line: &str, name: &str) -> bool {
    let needle = format!("{name}(");
    line.match_indices(&needle)
        .any(|(at, _)| at == 0 || !is_identifier_byte(line.as_bytes()[at - 1]))
}

#[test]
fn no_browser_dialog_is_called() {
    let files = ui_sources();
    let found = hits(
        &files,
        |_| false,
        |line| {
            ["alert", "confirm", "prompt"]
                .iter()
                .any(|name| calls(line, name))
        },
    );
    assert!(
        found.is_empty(),
        "browser dialogs (AC-04):\n{}",
        found.join("\n")
    );
}

/// The only file allowed to hold colour literals.
const TOKENS: &str = "src/styles/tokens.css";

/// A CSS colour literal: a 3, 4, 6 or 8 digit hex colour not inside a longer
/// word or an HTML character reference, or a colour function.
fn has_colour_literal(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (at, _) in line.match_indices('#') {
        if at > 0 && (is_identifier_byte(bytes[at - 1]) || bytes[at - 1] == b'&') {
            continue;
        }
        let digits = bytes[at + 1..]
            .iter()
            .take_while(|b| b.is_ascii_hexdigit())
            .count();
        let after = bytes.get(at + 1 + digits).copied();
        let ends = after.is_none_or(|b| !is_identifier_byte(b) && b != b'-');
        if matches!(digits, 3 | 4 | 6 | 8) && ends {
            return true;
        }
    }
    let lower = line.to_lowercase();
    ["rgb", "rgba", "hsl", "hsla", "oklch", "color-mix"]
        .iter()
        .any(|name| calls(&lower, name))
}

#[test]
fn the_colour_detector_reads_hex_and_functions_only() {
    for colour in [
        "color: #fff;",
        "  --text-secondary: #5a6b85;",
        "background:#00000080",
        "border: 1px solid #ABCD",
        "box-shadow: 0 0 0 2px rgb(0 0 0);",
        "--scrim: rgba(0, 0, 0, 0.6);",
        "color: hsl(210 40% 50%)",
        "color: oklch(70% 0.1 250)",
        "color: color-mix(in srgb, var(--a), var(--b))",
        "style={{ color: \"#fff\" }}",
    ] {
        assert!(has_colour_literal(colour), "{colour}");
    }
    for plain in [
        "href=\"#main\"",
        "window.location.hash = \"#/harbor-sim/inbox\"",
        "&#160;",
        "id: \"PR-0046\"",
        "color: var(--text-primary);",
        "#12 and #12345",
        "a#fff-b",
        "toRgb(x)",
    ] {
        assert!(!has_colour_literal(plain), "{plain}");
    }
}

#[test]
fn colour_literals_live_only_in_the_token_file() {
    let files = ui_sources();
    let found = hits(&files, |path| path == TOKENS, has_colour_literal);
    assert!(
        found.is_empty(),
        "colour literals outside {TOKENS} (AC-08):\n{}",
        found.join("\n")
    );
    let in_tokens = hits(&files, |path| path != TOKENS, has_colour_literal);
    assert!(
        in_tokens.len() >= 20,
        "{TOKENS} holds only {} colour lines: wrong file or detector?",
        in_tokens.len()
    );
}

/// The words for a hold on work (ADR-0012), assembled so this file does not
/// trip the repository's own greps.
fn hold_words() -> [String; 4] {
    let stem = "block";
    [
        format!("{stem}ed"),
        format!("{stem}ing"),
        format!("{stem}er"),
        format!("un{stem}"),
    ]
}

#[test]
fn nothing_is_worded_as_a_hold_on_work() {
    let files = ui_sources();
    let words = hold_words();
    let found = hits(
        &files,
        |_| false,
        |line| {
            let lower = line.to_lowercase();
            words.iter().any(|word| lower.contains(word.as_str()))
        },
    );
    assert!(found.is_empty(), "AC-15:\n{}", found.join("\n"));
    let provisional = read(&ui_dir().join("src/api/provisional.ts"));
    let keys = interface_keys(&provisional);
    assert!(
        keys.len() >= 20,
        "only {} provisional keys read",
        keys.len()
    );
    let held: Vec<&String> = keys
        .iter()
        .filter(|key| key.to_lowercase().starts_with("block"))
        .collect();
    assert!(
        held.is_empty(),
        "provisional keys starting `block`: {held:?}"
    );
}

/// Property keys of the object types in a TypeScript source: `key:` or
/// `key?:` at the start of a line, `readonly` and quotes stripped.
fn interface_keys(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let trimmed = trimmed.strip_prefix("readonly ").unwrap_or(trimmed);
        let Some(colon) = trimmed.find(':') else {
            continue;
        };
        let key = trimmed[..colon]
            .trim_end_matches('?')
            .trim_matches(['"', '\'']);
        if !key.is_empty() && key.bytes().all(is_identifier_byte) {
            out.push(key.to_owned());
        }
    }
    out
}

/// The two mock projects and the kinds the spec gives them (Data, "Mocks").
const MOCK_KINDS: [(&str, [&str; 3]); 2] = [
    ("harbor-sim", ["domain", "mechanic", "rule"]),
    ("ledger-api", ["service", "endpoint", "policy"]),
];

/// The string literals of the `nodeKinds` declaration in a `kinds.ts`.
fn declared_kinds(text: &str) -> Vec<String> {
    let line = text
        .lines()
        .find(|line| line.contains("nodeKinds"))
        .expect("a nodeKinds declaration");
    let start = line.find('[').expect("an array of kinds");
    let end = line.rfind(']').expect("an array of kinds");
    line[start + 1..end]
        .split(',')
        .map(|item| item.trim().trim_matches(['"', '\'']).to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

/// A path the kind rule leaves alone: the mocks and the tests.
fn is_mock_or_test(path: &str) -> bool {
    path.starts_with("src/mocks/")
        || path.starts_with("src/test/")
        || path.ends_with(".test.ts")
        || path.ends_with(".test.tsx")
}

#[test]
fn mock_kinds_are_disjoint_and_never_quoted_in_app_code() {
    let ui = ui_dir();
    let mut all: Vec<String> = Vec::new();
    for (slug, expected) in MOCK_KINDS {
        let kinds = declared_kinds(&read(&ui.join(format!("src/mocks/{slug}/kinds.ts"))));
        assert_eq!(kinds, expected, "{slug} nodeKinds");
        all.extend(kinds);
    }
    let unique: BTreeSet<&String> = all.iter().collect();
    assert_eq!(
        unique.len(),
        all.len(),
        "the two kind sets overlap: {all:?}"
    );
    let files = ui_sources();
    let found = hits(&files, is_mock_or_test, |line| {
        all.iter().any(|kind| {
            ['"', '\'', '`']
                .iter()
                .any(|quote| line.contains(&format!("{quote}{kind}{quote}")))
        })
    });
    assert!(
        found.is_empty(),
        "a project's kind quoted in app code (AC-19, ADR-0031):\n{}",
        found.join("\n")
    );
}

/// The module specifiers a TypeScript line imports from.
fn import_specifiers(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for marker in ["from \"", "import \"", "import(\""] {
        for (at, _) in line.match_indices(marker) {
            let rest = &line[at + marker.len()..];
            if let Some(end) = rest.find('"') {
                out.push(&rest[..end]);
            }
        }
    }
    out
}

#[test]
fn only_the_bootstrap_imports_the_mocks_and_only_src_api_the_provisional_types() {
    let mut wrong = Vec::new();
    let mut imports = 0;
    for (path, text) in ui_sources() {
        if !(path.ends_with(".ts") || path.ends_with(".tsx")) {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            for specifier in import_specifiers(line) {
                imports += 1;
                let segments: Vec<&str> = specifier.split('/').collect();
                let mocks = segments.contains(&"mocks");
                if mocks && path != "src/main.tsx" && !path.starts_with("src/mocks/") {
                    wrong.push(format!("ui/{path}:{}: {specifier}", index + 1));
                }
                let provisional = segments.last() == Some(&"provisional");
                if provisional && !path.starts_with("src/api/") {
                    wrong.push(format!("ui/{path}:{}: {specifier}", index + 1));
                }
            }
        }
    }
    assert!(imports > 50, "only {imports} imports read");
    assert!(
        wrong.is_empty(),
        "the seam is crossed (AC-05):\n{}",
        wrong.join("\n")
    );
}

/// The first line of `src/api/provisional.ts` (Data, "Contract seam").
const HEADER: &str = "// PROVISIONAL — hand-written until `spec serve` generates types from Rust; replace, do not extend; generated types win.";

/// A Data-form citation: a backticked `.md` path, one space, a quoted heading.
fn has_citation(text: &str) -> bool {
    text.match_indices(".md` \"").any(|(at, _)| {
        text[..at].rfind('`').is_some_and(|open| {
            let path = &text[open + 1..at];
            !path.is_empty() && !path.contains(char::is_whitespace)
        }) && text[at + ".md` \"".len()..].contains('"')
    })
}

#[test]
fn provisional_types_carry_the_header_a_citation_each_and_plain_kinds() {
    let text = read(&ui_dir().join("src/api/provisional.ts"));
    assert_eq!(text.lines().next(), Some(HEADER), "the PROVISIONAL header");
    let lines: Vec<&str> = text.lines().collect();
    let mut exported = 0;
    let mut uncited = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(rest) = line
            .strip_prefix("export type ")
            .or_else(|| line.strip_prefix("export interface "))
        else {
            continue;
        };
        exported += 1;
        let comment: Vec<&str> = lines[..index]
            .iter()
            .rev()
            .take_while(|above| {
                let above = above.trim_start();
                above.starts_with("/**") || above.starts_with('*') || above.starts_with("//")
            })
            .copied()
            .collect();
        if !comment.iter().any(|above| has_citation(above)) {
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric()).collect();
            uncited.push(format!("ui/src/api/provisional.ts:{}: {name}", index + 1));
        }
    }
    assert!(exported >= 15, "only {exported} exported types read");
    assert!(
        uncited.is_empty(),
        "exported types without a `<path>` \"Heading\" citation:\n{}",
        uncited.join("\n")
    );
    let mut vocabulary = 0;
    for line in &lines {
        let trimmed = line.trim_start();
        let trimmed = trimmed.strip_prefix("readonly ").unwrap_or(trimmed);
        for key in ["kind", "contour"] {
            let Some(rest) = trimmed
                .strip_prefix(key)
                .and_then(|rest| rest.strip_prefix('?').or(Some(rest)))
                .and_then(|rest| rest.strip_prefix(':'))
            else {
                continue;
            };
            vocabulary += 1;
            let ty = rest.trim().trim_end_matches(';').trim();
            assert!(
                ty == "string" || ty == "string | null",
                "provisional `{key}: {ty}` is not a plain string (ADR-0031)"
            );
        }
    }
    assert!(vocabulary >= 2, "only {vocabulary} kind/contour keys read");
}

#[test]
fn the_ui_is_a_documentation_root_with_its_readme_and_adr_in_budget() {
    let root = repository_root();
    let config: toml::Value =
        toml::from_str(&read(&root.join("specengine.toml"))).expect("specengine.toml is TOML");
    let roots: Vec<&str> = config["paths"]["roots"]
        .as_array()
        .expect("[paths] roots")
        .iter()
        .filter_map(toml::Value::as_str)
        .collect();
    assert!(
        roots.contains(&"ui"),
        "`ui` is not in [paths] roots: {roots:?}"
    );
    let readme = read(&root.join("ui/README.md"));
    assert!(
        readme.starts_with("---\n") && readme.lines().any(|line| line == "tier: 1"),
        "ui/README.md is not a Tier 1 document"
    );
    assert!(
        readme.len() <= 8192,
        "ui/README.md is {} B, over 8 192 B",
        readme.len()
    );
    let adr = read(&root.join("docs/decisions/ADR-0033.md"));
    assert!(
        adr.len() <= 1536,
        "ADR-0033 is {} B, over 1 536 B",
        adr.len()
    );
    let canon = adr
        .lines()
        .find_map(|line| line.strip_prefix("canon: "))
        .expect("ADR-0033 has canon:");
    let (file, anchor) = canon.split_once('#').expect("canon: names an anchor");
    assert_eq!(anchor, "ui", "ADR-0033 canon: {canon}");
    assert!(
        read(&root.join(file)).contains("<a id=\"ui\"></a>"),
        "{file} has no `ui` anchor"
    );
    assert!(adr.contains("**Cost.**"), "ADR-0033 names no cost");
}
