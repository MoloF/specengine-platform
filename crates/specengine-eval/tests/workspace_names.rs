//! docs/features/workspaces.md (RM-0081, v4 step 1a) "Acceptance criteria",
//! WS-6 (b): no workspace, module or preset flow name is code. No string
//! literal in `crates/*/src` (Rust, `"…"`) or in `ui/src`'s app code (TS, any
//! of the three quotes; not `src/mocks/`, `src/test/` or `*.test.ts(x)`) is
//! exactly one of the names: every `name` of a `[[workspace]]` or `[[module]]`
//! table in every repository `specengine.toml` (its own and each under
//! `fixtures/`) and in `scripts/presets/*.toml`, and every `[[card.flow]]`
//! name of a preset, read with the TOML reader when the test runs (W5: names
//! are data, shown or sent, never compared to a literal; the core names no
//! trade). Comment lines are no literal. A positive control on a scratch tree
//! shows each form caught and each exemption left. M14: `"narrative"` in core
//! as a default; M13's and M15's kind, `name === "newsroom"` in a component.
//!
//! docs/features/board-handoff.md (RM-0085) BH-13: the names gain each preset
//! flow's `return` column (W11: `sent-back`, since `returned` is already a
//! literal of `crates/specengine-cli/src/task.rs`); a fixture's `return` is
//! its own word, as its flow names are.
//!
//! docs/features/department-split.md (RM-0100) "Acceptance criteria", S9: the
//! preset's sixth flow and fifth workspace, `sound`, and its title join the
//! names as read; no source holds a `sound` literal.
//!
//! docs/features/departments.md (RM-0096) DP-6: the names gain every `kinds`
//! entry of every `[[module]]` (a direction's kinds, as written) in the same
//! configs and presets, and each preset flow's `title`; a fixture flow's
//! `title` is its own word, as its flow names are. M: the guard reading
//! `root` instead of `kinds`; preset titles not read.
//!
//! The daemon's word scan is `specengine-http`'s `tests/no_domain.rs` (WS-6
//! (a)); the mock's names `ui/src/policy.test.ts`'s (WS-6 (c)).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// Every `specengine.toml` under `dir`, hidden, `target*` and `node_modules`
/// directories skipped.
fn configs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !(name.starts_with('.') || name.starts_with("target") || name == "node_modules") {
                configs(&path, out);
            }
        } else if name == "specengine.toml" {
            out.push(path);
        }
    }
}

/// The `.toml` files directly under `dir`, sorted.
fn presets(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    files.sort();
    files
}

/// The `name` of each table in the array `key` of `table`.
fn table_names(table: &toml::Table, key: &str) -> Vec<String> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(toml::Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The `return` column of each `[[card.flow]]` of `card` that names one.
fn return_columns(card: Option<&toml::Table>) -> Vec<String> {
    card.and_then(|card| card.get("flow"))
        .and_then(toml::Value::as_array)
        .map(|flows| {
            flows
                .iter()
                .filter_map(|flow| flow.get("return").and_then(toml::Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// docs/features/departments.md DP-6: every `kinds` entry of each
/// `[[module]]` of `table`, as written.
fn module_kinds(table: &toml::Table) -> Vec<String> {
    table
        .get("module")
        .and_then(toml::Value::as_array)
        .map(|modules| {
            modules
                .iter()
                .filter_map(|module| module.get("kinds").and_then(toml::Value::as_array))
                .flatten()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// docs/features/departments.md DP-6: the `title` of each `[[card.flow]]` of
/// `card` that has one.
fn flow_titles(card: Option<&toml::Table>) -> Vec<String> {
    card.and_then(|card| card.get("flow"))
        .and_then(toml::Value::as_array)
        .map(|flows| {
            flows
                .iter()
                .filter_map(|flow| flow.get("title").and_then(toml::Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The names of WS-6 (b) under `root`, and the files they came from. A config
/// the TOML reader refuses (a fixture's deliberately broken one) holds none.
fn names_under(root: &Path) -> (BTreeSet<String>, Vec<PathBuf>) {
    let mut files = Vec::new();
    if root.join("specengine.toml").is_file() {
        files.push(root.join("specengine.toml"));
    }
    configs(&root.join("fixtures"), &mut files);
    let preset_files = presets(&root.join("scripts/presets"));
    files.extend(preset_files.iter().cloned());
    files.sort();
    let mut names = BTreeSet::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("a config");
        let Ok(table) = text.parse::<toml::Table>() else {
            continue;
        };
        names.extend(table_names(&table, "workspace"));
        names.extend(table_names(&table, "module"));
        // docs/features/departments.md DP-6: a direction's kinds.
        names.extend(module_kinds(&table));
        if preset_files.contains(file) {
            let card = table.get("card").and_then(toml::Value::as_table);
            let flows = card
                .map(|card| table_names(card, "flow"))
                .unwrap_or_default();
            names.extend(flows);
            // docs/features/board-handoff.md BH-13: each preset flow's
            // `return` column is a name too, so no code compares to it.
            names.extend(return_columns(card));
            // docs/features/departments.md DP-6: and each preset flow's
            // `title`.
            names.extend(flow_titles(card));
        }
    }
    (names, files)
}

/// A path of `ui/src` the rule leaves alone: the mocks and the tests.
fn is_mock_or_test(path: &str) -> bool {
    path.starts_with("ui/src/mocks/")
        || path.starts_with("ui/src/test/")
        || path.ends_with(".test.ts")
        || path.ends_with(".test.tsx")
}

/// Every file under `dir` with one of `extensions`, as (root-relative path
/// with `/`, text), sorted; `node_modules` and `target*` skipped.
fn sources(root: &Path, dir: &Path, extensions: &[&str], out: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if path.is_dir() {
            if name != "node_modules" && !name.starts_with("target") {
                sources(root, &path, extensions, out);
            }
        } else if path
            .extension()
            .is_some_and(|ext| extensions.iter().any(|want| ext == *want))
        {
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, fs::read_to_string(&path).expect("UTF-8 source")));
        }
    }
}

/// Whether `line` is a comment line (`//`, `/*`, `*`, `#!`/`#[` are code).
fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
}

/// `path:line: name` for each literal under `root` that is exactly one of
/// `names`: `"name"` in `crates/*/src/**/*.rs`; `"name"`, `'name'` or
/// `` `name` `` in `ui/src/**/*.ts(x)` outside the mocks and tests.
fn literal_hits(root: &Path, names: &BTreeSet<String>) -> (Vec<String>, usize) {
    let mut files = Vec::new();
    if let Ok(crates) = fs::read_dir(root.join("crates")) {
        let mut dirs: Vec<PathBuf> = crates.flatten().map(|entry| entry.path()).collect();
        dirs.sort();
        for dir in dirs {
            sources(root, &dir.join("src"), &["rs"], &mut files);
        }
    }
    let rust = files.len();
    sources(root, &root.join("ui/src"), &["ts", "tsx"], &mut files);
    let mut hits = Vec::new();
    for (index, (path, text)) in files.iter().enumerate() {
        let quotes: &[char] = if index < rust {
            &['"']
        } else if is_mock_or_test(path) {
            continue;
        } else {
            &['"', '\'', '`']
        };
        for (number, line) in text.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            for name in names {
                if quotes
                    .iter()
                    .any(|quote| line.contains(&format!("{quote}{name}{quote}")))
                {
                    hits.push(format!("{path}:{}: {name}", number + 1));
                }
            }
        }
    }
    (hits, files.len())
}

/// WS-6 (b): the names are read (card-press's, studio-lamp's, the preset's
/// workspaces and flows) and none is a literal of `crates/*/src` or of the UI's
/// app code. M14.
#[test]
fn ws6b_no_workspace_module_or_preset_flow_name_is_a_literal() {
    let root = repository_root();
    let (names, files) = names_under(&root);
    assert!(
        files
            .iter()
            .any(|file| file.ends_with("scripts/presets/gamedev.toml")),
        "the preset is read: {files:?}"
    );
    for name in [
        "newsroom",
        "pressroom",
        "weekly",
        "rulings",
        "writing",
        "development",
        "art",
        "production",
        "shore",
        "island",
        "narrative",
        "level-design",
        // docs/features/board-handoff.md BH-13: the preset's return column.
        "sent-back",
        // docs/features/departments.md DP-6: studio-lamp's direction's kind
        // and the preset's five flow titles.
        "scene",
        "Narrative",
        "Programming",
        "Art",
        "Level design",
        "Production",
        // docs/features/department-split.md "Acceptance criteria", S9: the
        // preset's `sound` flow and workspace, and its title.
        "sound",
        "Sound",
    ] {
        assert!(names.contains(name), "`{name}` is read: {names:?}");
    }
    assert!(
        !names.contains("copy") && !names.contains("plates"),
        "a fixture's card flows are not the preset's: {names:?}"
    );
    assert!(
        !names.contains("Copy desk"),
        "a fixture flow's title is its own word (DP-6): {names:?}"
    );
    assert!(
        !names.contains("CHP-SHORE") && !names.contains("ISL-LAMP"),
        "a module's root is no name (DP-6's M: `root` read instead of `kinds`): {names:?}"
    );
    let (hits, scanned) = literal_hits(&root, &names);
    eprintln!(
        "{} names from {} files; {scanned} sources scanned",
        names.len(),
        files.len()
    );
    assert!(scanned > 100, "the walk reads the sources: {scanned}");
    assert!(
        hits.is_empty(),
        "a workspace, module or preset flow name is a literal (W5):\n{}",
        hits.join("\n")
    );
}

/// A scratch tree under the system temp directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-workspace-names-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("a parent")).expect("parent directory");
        fs::write(&path, text).expect("scratch file written");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The positive control on a scratch tree: a config's workspace and module
/// names and a preset's flow names are read (a fixture's flow is not); a
/// default `"narrative"` in a crate's source, `name === "newsroom"` and
/// `'shore'` in a component are caught; a comment, a mock, a test, a crate's
/// `tests/`, a word holding a name and a name inside a longer literal are not.
/// docs/features/departments.md DP-6: a preset flow's `title = "Kiln work"`
/// with `"Kiln work"` in a crate's source, and a fixture direction's `kinds =
/// ["quarry"]` with `'quarry'` in a component, are each caught; a fixture
/// flow's `title` (`Press desk`) and a module's `root` (`SHT-FRONT`) are not
/// read.
#[test]
fn ws6b_the_scan_catches_each_form_and_leaves_each_exemption() {
    let tree = Scratch::new("probe");
    tree.write(
        "fixtures/press/specengine.toml",
        "[project]\nslug = \"press\"\n\n[[card.flow]]\nname = \"copy\"\ntitle = \"Press \
         desk\"\nreturn = \"blocked\"\n\n[[workspace]]\nname = \"newsroom\"\n\n[[module]]\nname = \
         \"shore\"\nroot = \"SHT-FRONT\"\n\n[[module]]\nname = \"pit\"\nkinds = [\"quarry\"]\n",
    );
    tree.write("fixtures/broken/specengine.toml", "[[[ broken\n");
    tree.write(
        "scripts/presets/trade.toml",
        "[[card.flow]]\nname = \"narrative\"\ntitle = \"Kiln work\"\nreturn = \
         \"sent-back\"\n\n[[workspace]]\nname = \"writing\"\n",
    );
    tree.write(
        "crates/core/src/lens.rs",
        "// \"writing\" in a comment\nconst DEFAULT: &str = \"narrative\";\nconst WIDER: &str = \
         \"rewriting\";\nconst LINE: &str = \"the newsroom tab\";\n",
    );
    // docs/features/board-handoff.md BH-13: a preset's return column in a
    // crate's source is caught; a fixture's (`blocked`) is no name.
    tree.write(
        "crates/core/src/back.rs",
        "const BACK: &str = \"sent-back\";\nconst SENT: &str = \"sent-back-later\";\n",
    );
    // docs/features/departments.md DP-6: a preset flow's title in a crate's
    // source, a fixture direction's kind in a component; a fixture flow's
    // title and a root are no names.
    tree.write(
        "crates/core/src/kiln.rs",
        "const TAB: &str = \"Kiln work\";\nconst DESK: &str = \"Press desk\";\nconst ROOT: &str = \
         \"SHT-FRONT\";\n",
    );
    tree.write(
        "ui/src/app/Quarry.tsx",
        "export const pick = (kind: string) => kind === 'quarry';\n",
    );
    tree.write(
        "crates/core/tests/lens.rs",
        "const FIXTURE: &str = \"newsroom\";\n",
    );
    tree.write(
        "ui/src/app/Shell.tsx",
        "export const pick = (name: string) => name === \"newsroom\";\nconst bar = ['shore'];\n// \
         `writing` in a comment\n",
    );
    tree.write("ui/src/mocks/lens.ts", "export const NAME = \"writing\";\n");
    tree.write("ui/src/test/lens.ts", "export const NAME = \"writing\";\n");
    tree.write("ui/src/app/Shell.test.tsx", "const name = `writing`;\n");
    let (names, _) = names_under(&tree.0);
    let want: BTreeSet<String> = [
        "Kiln work",
        "narrative",
        "newsroom",
        "pit",
        "quarry",
        "sent-back",
        "shore",
        "writing",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(names, want);
    let (hits, _) = literal_hits(&tree.0, &names);
    assert_eq!(
        hits,
        [
            "crates/core/src/back.rs:1: sent-back",
            "crates/core/src/kiln.rs:1: Kiln work",
            "crates/core/src/lens.rs:2: narrative",
            "ui/src/app/Quarry.tsx:1: quarry",
            "ui/src/app/Shell.tsx:1: newsroom",
            "ui/src/app/Shell.tsx:2: shore",
        ]
    );
}
