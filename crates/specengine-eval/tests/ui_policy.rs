//! The UI's file-level policies (docs/features/ui-shell.md AC-01, AC-03,
//! AC-04, AC-05, AC-06, AC-08, AC-15, AC-19, AC-20, AC-21; rules in
//! `ui/README.md` "Dependencies", "Gates", "Laptop rules"), read as text from
//! `ui/` without running pnpm: the gates themselves stay commands.
//!
//! - AC-01: `package.json` names exactly the owner's allowlist (17,
//!   ADR-0036), each at an exact `x.y.z`, equal to the README table once it
//!   holds versions ("pinned at install" is the placeholder until then);
//!   `react` = `react-dom`, 19.x;
//!   `packageManager` an exact pnpm; the lockfile present and its importer
//!   specifiers equal to `package.json`; `.npmrc` per policy; no install
//!   script, no `onlyBuiltDependencies`.
//! - AC-03, AC-21: the scripts, `test` = `vitest run`, Vitest `watch: false`
//!   with at most 2 workers.
//! - AC-04, AC-08, AC-15, AC-19: forbidden text in `ui/src` (browser
//!   dialogs, colour literals outside `tokens.css`, the words for a hold on
//!   work, a mock kind quoted outside the mocks and tests).
//! - docs/features/ui-health.md AC-11, narrowing AC-15: `blocked` passes
//!   only on the single `KnownCheckVerdict` line of `src/api/provisional.ts`,
//!   under `src/mocks/` and `src/test/` and in `*.test.ts(x)`; any other
//!   `blocked`, and `blocking`, `blocker`, `unblock` anywhere, case-insensitive,
//!   still fail; no provisional key starts with `block`. The exempt `blocked`
//!   is a verdict value of `spec check` the UI reads and labels, not a hold on
//!   work: nothing is blocked by a discrepancy (ADR-0012).
//! - AC-05, AC-06: the seam's imports, the provisional header, a citation per
//!   exported type, `kind` and `contour` plain strings. The cited headings
//!   themselves are checked by `doc_pointers.rs`.
//!
//! - docs/features/ui-tree-node.md AC-07: no HTML sink of its "Rules and edge
//!   cases" in `ui/src/**/*.ts(x)`, word-bounded, so the names spelled in
//!   pieces by `ui/src/policy.test.ts` stay clean.
//! - docs/features/ui-markdown.md AC-01: the allowlist above, 15 plus
//!   `react-markdown` and `remark-gfm` (ADR-0036), in `package.json`, the
//!   README table and the lockfile. AC-02: only `ui/src/markdown/` imports
//!   the two (and something there does); `rehype-raw`, `rehypeRaw`,
//!   `allowDangerousHtml` on no line of `ui/src/**`. AC-14: ADR-0036 within
//!   1 536 B, with a cost and `canon:` at `docs/canon/architecture.md#ui`.
//!
//! Named mutations (each turns this red): `globals` added; one `^`; the
//! lockfile deleted; `node-linker=hoisted`; a `postinstall`; `window.confirm`
//! in a component; `color: #fff` in a component; a "Blocking" label;
//! `kind === "mechanic"` in a component; the header dropped; `kind` a union;
//! `test` = `vitest`; the worker cap removed; `ui` out of roots; the snippet
//! rendered through `dangerouslySetInnerHTML` (run on a scratch copy of `ui/`
//! named by `UI_POLICY_SINK_UI_DIR`, never by editing `ui/src`);
//! `react-markdown` at `^10.1.0`; an 18th package; `import "react-markdown"`
//! in `ui/src/tree/TextPanel.tsx`; `allowDangerousHtml` in a component; the
//! `blocked` exemption widened to all of `src`, to all of `provisional.ts` or
//! to the other hold words.

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

/// The owner's allowlist of 2026-10-05, amended by ADR-0036 (2026-10-06)
/// with the last two (`ui/README.md` "Dependencies").
const ALLOWLIST: [&str; 17] = [
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
    "react-markdown",
    "remark-gfm",
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
fn package_json_pins_exactly_the_allowlist_at_exact_versions() {
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
            "package.json declares {field}: the allowlist lives in dependencies and devDependencies"
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
fn the_readme_table_names_the_allowlist_and_its_versions_once_filled() {
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
    ui_sources_in(&ui_dir())
}

/// Every file under `<ui>/src`, as (path relative to `ui` with `/`, text),
/// sorted by path.
fn ui_sources_in(ui: &Path) -> Vec<(String, String)> {
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
    let mut out = Vec::new();
    walk(ui, &ui.join("src"), &mut out);
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

/// The HTML sinks of `docs/features/ui-tree-node.md` "Rules and edge cases":
/// corpus and daemon text is shown as text, never parsed as HTML.
const HTML_SINKS: [&str; 7] = [
    "dangerouslySetInnerHTML",
    "innerHTML",
    "outerHTML",
    "insertAdjacentHTML",
    "document.write",
    "createContextualFragment",
    "srcdoc",
];

/// The first sink `line` names: word-bounded (no identifier byte on either
/// side) and ASCII case-insensitive, so React's `srcDoc` prop counts while a
/// name spelled in pieces (`"inner", "HTML"`) or inside a longer identifier
/// (`SetInnerHTML`, `innerHTMLLength`) does not.
fn html_sink(line: &str) -> Option<&'static str> {
    let lower = line.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    HTML_SINKS.into_iter().find(|sink| {
        let needle = sink.to_ascii_lowercase();
        lower.match_indices(&needle).any(|(at, _)| {
            let end = at + needle.len();
            (at == 0 || !is_identifier_byte(bytes[at - 1]))
                && (end == bytes.len() || !is_identifier_byte(bytes[end]))
        })
    })
}

#[test]
fn the_sink_detector_flags_each_use_and_no_name_in_pieces() {
    let uses = [
        (
            "<pre dangerouslySetInnerHTML={{ __html: snippet }} />",
            "dangerouslySetInnerHTML",
        ),
        ("element.innerHTML = text;", "innerHTML"),
        ("innerHTML", "innerHTML"),
        ("const html = node.outerHTML;", "outerHTML"),
        (
            "list.insertAdjacentHTML(\"beforeend\", row);",
            "insertAdjacentHTML",
        ),
        ("window.document.write(text);", "document.write"),
        (
            "range.createContextualFragment(text);",
            "createContextualFragment",
        ),
        ("<iframe srcDoc={text} />", "srcdoc"),
        ("frame.srcdoc = text;", "srcdoc"),
    ];
    for (line, sink) in uses {
        assert_eq!(html_sink(line), Some(sink), "{line}");
    }
    let clean = [
        // The pieces `ui/src/policy.test.ts` joins at run time.
        "      [\"dangerously\", \"SetInnerHTML\"],",
        "      [\"inner\", \"HTML\"],",
        "      [\"outer\", \"HTML\"],",
        "      [\"insertAdjacent\", \"HTML\"],",
        "      [\"document\", \".write\"],",
        "      [\"createContextual\", \"Fragment\"],",
        "      [\"src\", \"doc\"],",
        // A sink's name inside a longer identifier.
        "const myInnerHTML = 1;",
        "const innerHTMLLength = 1;",
        "documentWriter.write(text);",
        "const srcdocs = [];",
        "<a href={sectionHash(slug, id)}>{title}</a>",
    ];
    for line in clean {
        assert_eq!(html_sink(line), None, "{line}");
    }
}

/// The `ui/` the sink scan reads: this repository's, or a scratch copy (with
/// its `src/`) named by `UI_POLICY_SINK_UI_DIR` for the named-mutation run.
fn sink_scan_ui_dir() -> PathBuf {
    std::env::var_os("UI_POLICY_SINK_UI_DIR").map_or_else(ui_dir, PathBuf::from)
}

#[test]
fn no_html_sink_in_ui_sources() {
    let files: Vec<(String, String)> = ui_sources_in(&sink_scan_ui_dir())
        .into_iter()
        .filter(|(path, _)| path.ends_with(".ts") || path.ends_with(".tsx"))
        .collect();
    assert!(
        files.len() > 50,
        "only {} .ts/.tsx files read under ui/src",
        files.len()
    );
    let found = hits(&files, |_| false, |line| html_sink(line).is_some());
    assert!(
        found.is_empty(),
        "HTML sinks in ui/src (ui-tree-node AC-07):\n{}",
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

/// The one file outside the mocks and tests whose text may spell the first
/// hold word, and the start of its one line that may: the verdict list of
/// `spec check` (docs/features/ui-health.md AC-11), a value the UI reads and
/// labels, not a hold on work.
const VERDICT_FILE: &str = "src/api/provisional.ts";
const VERDICT_LINE: &str = "export type KnownCheckVerdict";

/// `line` declares the verdict type: it starts with [`VERDICT_LINE`], not
/// with a longer name of which that is the head.
fn is_verdict_line(line: &str) -> bool {
    line.strip_prefix(VERDICT_LINE)
        .is_some_and(|rest| !rest.bytes().next().is_some_and(is_identifier_byte))
}

/// `path:line: text` for each line of `files` that words a hold on work
/// (ui-shell AC-15 as narrowed by ui-health AC-11): the last three hold words
/// anywhere, case-insensitive; the first one anywhere but in the mocks, the
/// tests and the single verdict line of [`VERDICT_FILE`].
fn hold_hits(files: &[(String, String)]) -> Vec<String> {
    let [first, rest @ ..] = hold_words();
    let mut out = Vec::new();
    for (path, text) in files {
        let verdict_lines = if path == VERDICT_FILE {
            text.lines().filter(|line| is_verdict_line(line)).count()
        } else {
            0
        };
        for (index, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            let exempt = is_mock_or_test(path) || (verdict_lines == 1 && is_verdict_line(line));
            if rest.iter().any(|word| lower.contains(word.as_str()))
                || (!exempt && lower.contains(first.as_str()))
            {
                out.push(format!("ui/{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    out
}

/// A scratch `ui/` under the system temp dir, removed on drop: `src/main.tsx`
/// and enough filler for [`ui_sources_in`], plus `files`.
struct ScratchUi(PathBuf);

impl ScratchUi {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "specengine-eval-ui-policy-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let filler: Vec<(String, &str)> = (0..24)
            .map(|n| (format!("src/filler/f{n:02}.ts"), "export const n = 0;\n"))
            .collect();
        let all = filler
            .iter()
            .map(|(path, text)| (path.as_str(), *text))
            .chain([("src/main.tsx", "export {};\n")])
            .chain(files.iter().copied());
        for (relative, text) in all {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().expect("a parent")).expect("scratch directory");
            fs::write(&path, text).expect("scratch file written");
        }
        Self(root)
    }
}

impl Drop for ScratchUi {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// ui-health AC-11 on a scratch `ui/`: the first hold word passes on the
/// verdict line, in the mocks and in the tests, and fails on any other line,
/// even of `provisional.ts`; the other three fail everywhere, the mocks and
/// tests included; a "Blocking" label in `src/health/` fails.
#[test]
fn the_hold_scan_exempts_the_verdict_line_mocks_and_tests_only() {
    let [first, ing, er, un] = hold_words();
    let verdict = format!(
        "{VERDICT_LINE} = (typeof KNOWN_CHECK_VERDICTS)[number]; export const KNOWN_CHECK_VERDICTS = [\"clean\", \"observed\", \"{first}\", \"cannot-check\"] as const;"
    );
    let provisional = format!(
        "// Provisional.\n{verdict}\nexport type Hold = \"{first}\";\n{VERDICT_LINE}Hold = \"{first}\";\n"
    );
    let label = format!("export const L = () => <span>B{}</span>;\n", &ing[1..]);
    let mocks = format!("export const v = {{ verdict: \"{first}\" }};\n// A {er}.\n");
    let unit = format!("expect(v).toBe(\"{first}\");\nit(\"does not {un}\", () => {{}});\n");
    let capital = format!(
        "expect(screen.getByText(\"B{}\")).toBeNull();\n",
        &first[1..]
    );
    let setup = format!("export const verdict = \"{first}\";\n");
    let elsewhere = format!("export const v = \"{first}\";\n");
    let types = format!("{VERDICT_LINE} = \"{first}\";\n");
    let ui = ScratchUi::new(
        "hold",
        &[
            ("src/api/provisional.ts", provisional.as_str()),
            ("src/api/types.ts", types.as_str()),
            ("src/health/x.tsx", label.as_str()),
            ("src/health/mocks.ts", elsewhere.as_str()),
            ("src/mocks/m.ts", mocks.as_str()),
            ("src/test/setup.ts", setup.as_str()),
            ("src/a.test.ts", unit.as_str()),
            ("src/b.test.tsx", capital.as_str()),
        ],
    );
    let mut found = hold_hits(&ui_sources_in(&ui.0));
    found.sort();
    let mut want = vec![
        format!("ui/src/a.test.ts:2: it(\"does not {un}\", () => {{}});"),
        format!("ui/src/api/provisional.ts:3: export type Hold = \"{first}\";"),
        format!("ui/src/api/provisional.ts:4: {VERDICT_LINE}Hold = \"{first}\";"),
        format!("ui/src/api/types.ts:1: {}", types.trim()),
        format!("ui/src/health/mocks.ts:1: {}", elsewhere.trim()),
        format!("ui/src/health/x.tsx:1: {}", label.trim()),
        format!("ui/src/mocks/m.ts:2: // A {er}."),
    ];
    want.sort();
    assert_eq!(found, want);

    // Two verdict lines in `provisional.ts`: neither is the single one.
    let twice = format!("{verdict}\n{verdict}\n");
    let ui = ScratchUi::new("hold-twice", &[("src/api/provisional.ts", twice.as_str())]);
    assert_eq!(
        hold_hits(&ui_sources_in(&ui.0)),
        [1, 2]
            .map(|n| format!("ui/src/api/provisional.ts:{n}: {verdict}"))
            .to_vec()
    );
}

#[test]
fn nothing_is_worded_as_a_hold_on_work() {
    let files = ui_sources();
    let found = hold_hits(&files);
    assert!(
        found.is_empty(),
        "ui-shell AC-15, ui-health AC-11:\n{}",
        found.join("\n")
    );
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

/// A path the kind rule leaves alone, and whose lines may spell the first
/// hold word as data (ui-health AC-11): the mocks and the tests.
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

/// The two packages ADR-0036 adds (docs/features/ui-markdown.md AC-02).
const MARKDOWN_PACKAGES: [&str; 2] = ["react-markdown", "remark-gfm"];

/// The only folder that may import them, relative to `ui/`.
const MARKDOWN_DIR: &str = "src/markdown/";

/// The ADR-0036 package a module specifier names, as the ESLint rule reads
/// it: `^(react-markdown|remark-gfm)(/|$)`.
fn markdown_package(specifier: &str) -> Option<&'static str> {
    MARKDOWN_PACKAGES.into_iter().find(|name| {
        specifier
            .strip_prefix(name)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}

/// The module specifiers a line loads, in any quote style: the string
/// literal right after the word `from` or `import` (CSS `@import` too), or
/// as the first argument of `import(`, `require(` or Vitest's `mock(`,
/// `doMock(`, `importActual(`, `importMock(`. A string anywhere else (an
/// expected value in a test) loads nothing.
fn module_specifiers(line: &str) -> Vec<&str> {
    const WORDS: [&str; 2] = ["from", "import"];
    const CALLS: [&str; 6] = [
        "import",
        "require",
        "mock",
        "doMock",
        "importActual",
        "importMock",
    ];
    let mut out = Vec::new();
    for (at, quote) in line.char_indices() {
        if !matches!(quote, '"' | '\'' | '`') {
            continue;
        }
        let before = line[..at].trim_end();
        let (before, keywords): (&str, &[&str]) = match before.strip_suffix('(') {
            Some(callee) => (callee.trim_end(), &CALLS),
            None => (before, &WORDS),
        };
        let loads = keywords.iter().any(|keyword| {
            before.strip_suffix(keyword).is_some_and(|head| {
                head.as_bytes()
                    .last()
                    .is_none_or(|&byte| !is_identifier_byte(byte))
            })
        });
        if !loads {
            continue;
        }
        let rest = &line[at + 1..];
        if let Some(end) = rest.find(quote) {
            out.push(&rest[..end]);
        }
    }
    out
}

#[test]
fn the_import_reader_takes_each_form_and_quote_and_no_plain_string() {
    for (line, specifier) in [
        (
            "import ReactMarkdown from \"react-markdown\";",
            "react-markdown",
        ),
        ("import remarkGfm from 'remark-gfm';", "remark-gfm"),
        (
            "import type { Options } from \"react-markdown\";",
            "react-markdown",
        ),
        ("} from \"remark-gfm\";", "remark-gfm"),
        (
            "export { default } from \"react-markdown\";",
            "react-markdown",
        ),
        ("import \"remark-gfm\";", "remark-gfm"),
        (
            "const m = await import(`react-markdown`);",
            "react-markdown",
        ),
        (
            "const m = import( \"react-markdown/lib\" );",
            "react-markdown/lib",
        ),
        (
            "vi.mock(\"react-markdown\", async (importOriginal) => {",
            "react-markdown",
        ),
        ("const gfm = require(\"remark-gfm\");", "remark-gfm"),
        (
            "@import \"react-markdown/style.css\";",
            "react-markdown/style.css",
        ),
    ] {
        assert_eq!(module_specifiers(line), vec![specifier], "{line}");
    }
    for line in [
        "expect(importsOf(text)).toEqual(expect.arrayContaining([\"react-markdown\", \"remark-gfm\"]));",
        "const name = \"react-markdown\";",
        "myimport(\"react-markdown\");",
        "const reimport = 'remark-gfm';",
    ] {
        assert!(module_specifiers(line).is_empty(), "{line}");
    }
    for (specifier, package) in [
        ("react-markdown", Some("react-markdown")),
        ("remark-gfm/lib/index.js", Some("remark-gfm")),
        ("react-markdown-extra", None),
        ("remark-gfm2", None),
        ("@scope/react-markdown", None),
        ("rehype-raw", None),
        ("react", None),
    ] {
        assert_eq!(markdown_package(specifier), package, "{specifier}");
    }
}

#[test]
fn only_src_markdown_imports_react_markdown_and_remark_gfm() {
    let mut wrong = Vec::new();
    let mut inside: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (path, text) in ui_sources() {
        for (index, line) in text.lines().enumerate() {
            for specifier in module_specifiers(line) {
                let Some(package) = markdown_package(specifier) else {
                    continue;
                };
                let place = format!("ui/{path}:{}: {specifier}", index + 1);
                if path.starts_with(MARKDOWN_DIR) {
                    inside.entry(package).or_default().push(place);
                } else {
                    wrong.push(place);
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "react-markdown or remark-gfm imported outside ui/{MARKDOWN_DIR} (ui-markdown AC-02, ADR-0036):\n{}",
        wrong.join("\n")
    );
    let missing: Vec<&str> = MARKDOWN_PACKAGES
        .into_iter()
        .filter(|package| !inside.contains_key(package))
        .collect();
    assert!(
        missing.is_empty(),
        "nothing under ui/{MARKDOWN_DIR} imports {missing:?}: the renderer is gone or the import reader is blind"
    );
}

/// The packages' raw-HTML switches (ADR-0036: raw HTML stays text).
const RAW_HTML_SWITCHES: [&str; 3] = ["rehype-raw", "rehypeRaw", "allowDangerousHtml"];

/// The first raw-HTML switch `line` names, ASCII case-insensitive and not
/// word-bounded, so an alias (`RehypeRaw`) or a key inside an options
/// object counts, while a name spelled in pieces (`"rehype" + "Raw"`, as
/// `ui/src/policy.test.ts` does) does not.
fn raw_html_switch(line: &str) -> Option<&'static str> {
    let lower = line.to_ascii_lowercase();
    RAW_HTML_SWITCHES
        .into_iter()
        .find(|name| lower.contains(&name.to_ascii_lowercase()))
}

#[test]
fn the_raw_html_reader_flags_each_switch_and_no_name_in_pieces() {
    for (line, name) in [
        ("import rehypeRaw from \"rehype-raw\";", "rehype-raw"),
        ("<ReactMarkdown rehypePlugins={[rehypeRaw]}>", "rehypeRaw"),
        ("const { default: RehypeRaw } = mod;", "rehypeRaw"),
        (
            "remarkRehypeOptions={{ allowDangerousHtml: true }}",
            "allowDangerousHtml",
        ),
        ("<ReactMarkdown allowDangerousHtml>", "allowDangerousHtml"),
        ("// allowDangerousHtml stays off", "allowDangerousHtml"),
    ] {
        assert_eq!(raw_html_switch(line), Some(name), "{line}");
    }
    for line in [
        "const raw = [\"rehype\" + \"-raw\", \"rehype\" + \"Raw\", \"allowDangerous\" + \"Html\"];",
        "type ToHastOptions = NonNullable<Options[\"remarkRehypeOptions\"]>;",
        "<ReactMarkdown skipHtml>",
    ] {
        assert_eq!(raw_html_switch(line), None, "{line}");
    }
}

#[test]
fn no_raw_html_switch_in_ui_sources() {
    let found = hits(
        &ui_sources(),
        |_| false,
        |line| raw_html_switch(line).is_some(),
    );
    assert!(
        found.is_empty(),
        "a raw-HTML switch in ui/src (ui-markdown AC-02, ADR-0036):\n{}",
        found.join("\n")
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
    // ADR-0033 founds the UI; ADR-0036 amends its allowlist (ui-markdown
    // AC-14). Both change the canon's `ui` section.
    for id in ["ADR-0033", "ADR-0036"] {
        let adr = read(&root.join(format!("docs/decisions/{id}.md")));
        assert!(adr.len() <= 1536, "{id} is {} B, over 1 536 B", adr.len());
        let canon = adr
            .lines()
            .find_map(|line| line.strip_prefix("canon: "))
            .unwrap_or_else(|| panic!("{id} has no canon:"));
        let (file, anchor) = canon
            .split_once('#')
            .unwrap_or_else(|| panic!("{id} canon: {canon} names no anchor"));
        assert_eq!(
            (file, anchor),
            ("docs/canon/architecture.md", "ui"),
            "{id} canon: {canon}"
        );
        assert!(
            read(&root.join(file)).contains("<a id=\"ui\"></a>"),
            "{file} has no `ui` anchor"
        );
        assert!(adr.contains("**Cost.**"), "{id} names no cost");
    }
}
