//! AC-05 of docs/features/spec-check.md (the core half): the check tables of
//! `specengine.toml`. The Data example loads; an unknown key or class, a
//! wrong type, a cap below 1 and an unknown mode fail as
//! `specengine.toml:<line>: message`; `[paths]` gains `tier0`, `tier1_name`
//! and `index`. The re-parse half (only new tables edited → nothing
//! re-parsed) is in the store's `tests/check_config.rs`.

mod common;

use specengine_core::check::config::{
    DEFAULT_DECISION_BYTES, DEFAULT_INDEX_BYTES, DEFAULT_TIER0_BYTES, DEFAULT_TIER1_BYTES,
};
use specengine_core::check::{CheckConfig, ClassContract, DocClass, Mode};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;

/// The Data example of the spec, verbatim but for the elided keys.
const DATA_EXAMPLE: &str = r#"[paths]                       # + existing keys
records    = "docs/decisions" # file named after its id
tier0      = "CLAUDE.md"      # the only canon tier 0
tier1_name = "README.md"      # canon tier 1 only so named
index      = "docs/index.md"  # capped by index_bytes
exclude    = ["**/_*.md"]     # + SKIP_DIRS, AC-19

[ids]
ADR = { kind = "decision", width = 4 }

[budgets]                     # bytes; absent → §4, canon_bytes → none
tier0_bytes    = 16384
tier1_bytes    = 10240
index_bytes    = 10240
decision_bytes = 1536
canon_bytes    = 12288

[classes]                     # replaces that default; here all four, closed,
                              # as docs/README.md "Front-matter contract"
decision = { required = ["class", "id", "title", "status", "date", "scope"], optional = ["canon", "supersedes", "ref"], closed = true }

[check]
mode = "enforce"              # observe | enforce (default)
"#;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn the_data_example_loads() {
    let config = CheckConfig::from_toml(DATA_EXAMPLE)
        .unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    assert_eq!(config.budgets.tier0_bytes, 16384);
    assert_eq!(config.budgets.tier1_bytes, 10240);
    assert_eq!(config.budgets.index_bytes, 10240);
    assert_eq!(config.budgets.decision_bytes, 1536);
    assert_eq!(config.budgets.canon_bytes, Some(12288));
    assert_eq!(config.budgets.bundle_node, None);
    assert_eq!(config.mode, Mode::Enforce);
    let decision = config.classes.get(DocClass::Decision);
    assert_eq!(
        decision,
        &ClassContract {
            required: strings(&["class", "id", "title", "status", "date", "scope"]),
            optional: strings(&["canon", "supersedes", "ref"]),
            closed: true,
        }
    );
    assert!(decision.allows("canon") && decision.allows("class"));
    assert!(!decision.allows("owner"));
    // A class not written keeps its default contract.
    assert_eq!(
        config.classes.get(DocClass::Canon),
        &ClassContract::default_for(DocClass::Canon)
    );

    let paths =
        Paths::from_toml(DATA_EXAMPLE).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    assert_eq!(paths.records, "docs/decisions");
    assert_eq!(paths.tier0.as_deref(), Some("CLAUDE.md"));
    assert_eq!(paths.tier1_name.as_deref(), Some("README.md"));
    assert_eq!(paths.index.as_deref(), Some("docs/index.md"));
    assert_eq!(paths.exclude, ["**/_*.md"]);
    assert!(!paths.roots_written, "no `roots` key in the example");
    let scheme = IdScheme::from_toml(DATA_EXAMPLE).expect("[ids]");
    assert_eq!(scheme.prefixes().len(), 1);
}

#[test]
fn defaults_without_the_tables() {
    let config = CheckConfig::from_toml("[ids]\nX = { kind = \"x\" }\n").expect("no check tables");
    assert_eq!(config, CheckConfig::default());
    assert_eq!(config.mode, Mode::Enforce, "enforce is the default mode");
    assert_eq!(config.budgets.tier0_bytes, DEFAULT_TIER0_BYTES);
    assert_eq!(config.budgets.tier1_bytes, DEFAULT_TIER1_BYTES);
    assert_eq!(config.budgets.index_bytes, DEFAULT_INDEX_BYTES);
    assert_eq!(config.budgets.decision_bytes, DEFAULT_DECISION_BYTES);
    assert_eq!(
        (
            DEFAULT_TIER0_BYTES,
            DEFAULT_TIER1_BYTES,
            DEFAULT_INDEX_BYTES,
            DEFAULT_DECISION_BYTES
        ),
        (16384, 10240, 10240, 1536),
        "the caps of ADR-0022"
    );
    assert_eq!(
        config.budgets.canon_bytes, None,
        "canon_bytes absent: no cap"
    );
    // Default contracts: open, the load-bearing keys plus `class`.
    let required = |class| config.classes.get(class).required.clone();
    assert_eq!(
        required(DocClass::Canon),
        strings(&["class", "owner", "reviewed"])
    );
    assert_eq!(
        required(DocClass::Decision),
        strings(&["class", "id", "status", "scope"])
    );
    assert_eq!(
        required(DocClass::Spec),
        strings(&["class", "status", "scope"])
    );
    assert_eq!(required(DocClass::Generated), strings(&["class"]));
    for class in DocClass::ALL {
        assert!(
            !config.classes.get(class).closed,
            "{class}: open by default"
        );
        assert!(config.classes.get(class).allows("anything"));
    }
    let paths = Paths::from_toml("").expect("empty");
    assert_eq!(
        (paths.tier0, paths.tier1_name, paths.index),
        (None, None, None)
    );
}

#[test]
fn token_bundles_and_observe_are_accepted() {
    let config = CheckConfig::from_toml(
        "[budgets]\nbundle_node = 4000\nbundle_task = 12000\n\n[check]\nmode = \"observe\"\n",
    )
    .expect("valid");
    assert_eq!(config.budgets.bundle_node, Some(4000));
    assert_eq!(config.budgets.bundle_task, Some(12000));
    assert_eq!(config.mode, Mode::Observe);
}

/// `(case, text, line)`: each must fail at that line of the file.
const INVALID: &[(&str, &str, usize)] = &[
    (
        "unknown key in [budgets]",
        "[ids]\nX = { kind = \"x\" }\n\n[budgets]\ntier0_bytes = 100\ntier9_bytes = 5\n",
        6,
    ),
    (
        "unknown key in [check]",
        "[check]\nmode = \"enforce\"\nstrict = true\n",
        3,
    ),
    (
        "unknown key in a contract",
        "[classes]\ncanon = { required = [\"owner\"] }\nspec = { required = [\"status\"], forbidden = [\"x\"] }\n",
        3,
    ),
    (
        "unknown class",
        "[classes]\ncanon = { required = [\"owner\"] }\n\nmemo = { required = [\"class\"] }\n",
        4,
    ),
    (
        "wrong type of a cap",
        "[budgets]\n\ntier1_bytes = \"10k\"\n",
        3,
    ),
    (
        "wrong type of closed",
        "[classes]\ndecision = { closed = \"yes\" }\n",
        2,
    ),
    ("tier0_bytes = 0", "[budgets]\ntier0_bytes = 0\n", 2),
    ("a negative cap", "[budgets]\n\n\ndecision_bytes = -1\n", 4),
    ("mode = strict", "[ids]\n\n[check]\nmode = \"strict\"\n", 4),
];

#[test]
fn invalid_check_tables_fail_with_file_line_message() {
    let mut failures = Vec::new();
    for (case, text, line) in INVALID {
        match CheckConfig::from_toml(text) {
            Ok(config) => failures.push(format!("{case}: accepted as {config:?}")),
            Err(error) => {
                let shown = error.at("specengine.toml");
                let prefix = format!("specengine.toml:{line}: ");
                if !shown.starts_with(&prefix) || shown.len() <= prefix.len() {
                    failures.push(format!("{case}: {shown:?} does not start with {prefix:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_messages_name_the_problem() {
    let message = |text: &str| {
        CheckConfig::from_toml(text)
            .unwrap_err()
            .at("specengine.toml")
    };
    let unknown = message("[budgets]\ntier9_bytes = 5\n");
    assert!(unknown.contains("tier9_bytes"), "{unknown}");
    let class = message("[classes]\nmemo = {}\n");
    assert!(class.contains("memo"), "{class}");
    let cap = message("[budgets]\ntier0_bytes = 0\n");
    assert!(cap.contains("tier0_bytes"), "{cap}");
    let mode = message("[check]\nmode = \"strict\"\n");
    assert!(mode.contains("strict"), "{mode}");
}

#[test]
fn invalid_new_paths_keys_fail_with_file_line_message() {
    for (case, text, line) in [
        (
            "tier1_name with a slash",
            "[paths]\n\ntier1_name = \"docs/README.md\"\n",
            3,
        ),
        ("tier0 absolute", "[paths]\ntier0 = \"/CLAUDE.md\"\n", 2),
        (
            "index with ..",
            "[paths]\nindex = \"../docs/index.md\"\n",
            2,
        ),
        ("unknown paths key", "[paths]\ntier2 = \"x.md\"\n", 2),
    ] {
        let error = Paths::from_toml(text)
            .err()
            .unwrap_or_else(|| panic!("{case}: accepted"));
        let shown = error.at("specengine.toml");
        assert!(
            shown.starts_with(&format!("specengine.toml:{line}: ")),
            "{case}: {shown}"
        );
    }
}

// ------------------------------------------------------------------ the generator registry
// AC-05 of docs/features/spec-check-graph.md (the core half): the
// `[[generators]]` table. The rule half is in `check_generated.rs`, the
// re-parse half in the store's `tests/check_config.rs`.

/// The registry example of spec-check-graph's Data, verbatim.
const REGISTRY_EXAMPLE: &str = r#"[paths]
index = "docs/index.md"

[[generators]]                              # §11.6; absent → the generator rules are off
command = "cargo xtask docs index --write"  # a `generator:` value, compared byte for byte
writes  = ["docs/index.md"]                 # root-relative, the [paths] path rules
index   = true                              # optional: SpecEngine renders this output itself (§11.5)
gate    = "cargo xtask docs check"          # optional, index entry only; default "spec check"
"#;

#[test]
fn the_registry_example_loads() {
    use specengine_core::check::{DEFAULT_GATE, Generator};
    let config = CheckConfig::from_toml(REGISTRY_EXAMPLE)
        .unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    let want = Generator {
        command: "cargo xtask docs index --write".to_owned(),
        writes: strings(&["docs/index.md"]),
        index: true,
        gate: Some("cargo xtask docs check".to_owned()),
        line: 4,
    };
    assert_eq!(
        config.generators.as_deref(),
        Some(std::slice::from_ref(&want))
    );
    assert_eq!(config.index_generator(), Some(&want));
    assert_eq!(want.gate(), "cargo xtask docs check");
    assert_eq!(DEFAULT_GATE, "spec check");
    // The other tables keep their defaults.
    assert_eq!(config.mode, Mode::Enforce);
    assert_eq!(config.budgets, CheckConfig::default().budgets);
}

#[test]
fn registry_entries_keep_their_order_lines_and_default_gate() {
    let text = "\
[paths]
index = \"site/toc.md\"

[[generators]]
command = \"make toc\"
writes = [\"site/toc.md\", \"site/extra.md\", \"site/toc.md\"]
index = true

[[generators]]
command = \"tool gen\"
writes = [\"b.md\"]
index = false
";
    let config =
        CheckConfig::from_toml(text).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    let generators = config.generators.as_deref().expect("the registry");
    let seen: Vec<(&str, Vec<&str>, bool, usize)> = generators
        .iter()
        .map(|g| {
            (
                g.command.as_str(),
                g.writes.iter().map(String::as_str).collect(),
                g.index,
                g.line,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("make toc", vec!["site/toc.md", "site/extra.md"], true, 4),
            ("tool gen", vec!["b.md"], false, 9),
        ],
        "order written, repeats in `writes` dropped, the header's line"
    );
    let index = config.index_generator().expect("the index entry");
    assert_eq!(index.command, "make toc");
    assert_eq!(index.gate, None);
    assert_eq!(index.gate(), "spec check", "the default gate");
    // No `index = true` entry: none.
    let plain = CheckConfig::from_toml("[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\n")
        .expect("valid");
    assert_eq!(plain.index_generator(), None);
}

#[test]
fn absent_and_empty_registries_differ() {
    let absent = CheckConfig::from_toml("[ids]\nX = { kind = \"x\" }\n").expect("valid");
    assert_eq!(absent.generators, None, "no table: the rules are off");
    let empty =
        CheckConfig::from_toml("generators = []\n\n[ids]\nX = { kind = \"x\" }\n").expect("valid");
    assert_eq!(
        empty.generators,
        Some(Vec::new()),
        "empty: the rules are on"
    );
    assert_eq!(empty.index_generator(), None);
}

/// `(case, text, line, fragment of the message)`: each must fail as
/// `specengine.toml:<line>: message`, the message naming the problem.
const INVALID_REGISTRY: &[(&str, &str, usize, &str)] = &[
    (
        "unknown key",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nrun = true\n",
        4,
        "run",
    ),
    (
        "command of the wrong type",
        "[[generators]]\ncommand = 5\nwrites = [\"a.md\"]\n",
        2,
        "",
    ),
    (
        "writes of the wrong type",
        "[[generators]]\ncommand = \"a\"\nwrites = \"a.md\"\n",
        3,
        "",
    ),
    (
        "a writes item of the wrong type",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\", 3]\n",
        3,
        "",
    ),
    (
        "index of the wrong type",
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = \"yes\"\n",
        7,
        "",
    ),
    (
        "gate of the wrong type",
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\ngate = 1\n",
        8,
        "",
    ),
    (
        "a [generators] table instead of an array of tables",
        "[ids]\nX = { kind = \"x\" }\n\n[generators]\ncommand = \"a\"\nwrites = [\"a.md\"]\n",
        4,
        "",
    ),
    (
        "command missing",
        "[ids]\n\n[[generators]]\nwrites = [\"a.md\"]\n",
        3,
        "command",
    ),
    (
        "command blank",
        "[[generators]]\nwrites = [\"a.md\"]\ncommand = \"  \"\n",
        3,
        "command",
    ),
    (
        "command empty",
        "[[generators]]\ncommand = \"\"\nwrites = [\"a.md\"]\n",
        2,
        "command",
    ),
    (
        "command repeated",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"b.md\"]\n",
        6,
        "repeated",
    ),
    (
        "writes missing",
        "[[generators]]\ncommand = \"a\"\n",
        1,
        "writes",
    ),
    (
        "writes empty",
        "[[generators]]\ncommand = \"a\"\n\nwrites = []\n",
        4,
        "writes",
    ),
    (
        "writes absolute",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"/a.md\"]\n",
        3,
        "writes",
    ),
    (
        "writes with ..",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"../a.md\"]\n",
        3,
        "writes",
    ),
    (
        "writes with a . component",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"docs/./a.md\"]\n",
        3,
        "writes",
    ),
    (
        "writes with an empty component",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"docs//a.md\"]\n",
        3,
        "writes",
    ),
    (
        "writes a directory",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"docs/\"]\n",
        3,
        "writes",
    ),
    (
        "writes empty string",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"\"]\n",
        3,
        "writes",
    ),
    (
        "writes shared with another entry",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\n\n[[generators]]\ncommand = \"b\"\nwrites = [\"b.md\",\n  \"a.md\"]\n",
        8,
        "a.md",
    ),
    (
        "index = true twice",
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\n\n[[generators]]\ncommand = \"b\"\nwrites = [\"b.md\"]\nindex = true\n",
        12,
        "index",
    ),
    (
        "gate without index",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\ngate = \"make check\"\n",
        4,
        "gate",
    ),
    (
        "gate with index = false",
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = false\ngate = \"make check\"\n",
        8,
        "gate",
    ),
    (
        "index = true without [paths] index",
        "[paths]\nrecords = \"r\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\n",
        7,
        "[paths] index",
    ),
    (
        "index = true without any [paths]",
        "[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\n",
        4,
        "[paths] index",
    ),
    (
        "index = true while writes lacks [paths] index",
        "[paths]\nindex = \"docs/index.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"docs/other.md\"]\nindex = true\n",
        7,
        "docs/index.md",
    ),
];

#[test]
fn invalid_registries_fail_with_file_line_message() {
    let mut failures = Vec::new();
    for (case, text, line, fragment) in INVALID_REGISTRY {
        match CheckConfig::from_toml(text) {
            Ok(config) => failures.push(format!("{case}: accepted as {config:?}")),
            Err(error) => {
                let shown = error.at("specengine.toml");
                let prefix = format!("specengine.toml:{line}: ");
                if !shown.starts_with(&prefix) || shown.len() <= prefix.len() {
                    failures.push(format!("{case}: {shown:?} does not start with {prefix:?}"));
                } else if !shown[prefix.len()..].contains(fragment) {
                    failures.push(format!("{case}: {shown:?} does not name {fragment:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A `[paths]` error is `Paths::from_toml`'s to report: the registry's
/// cross-check waits for it rather than guessing the index path.
#[test]
fn a_paths_error_is_reported_by_paths_not_by_the_registry() {
    let text = "[paths]\nindex = \"../x.md\"\n\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\nindex = true\n";
    assert!(Paths::from_toml(text).is_err());
    let config =
        CheckConfig::from_toml(text).unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
    assert!(config.index_generator().is_some());
}

/// The registry is not part of the `[ids]` scheme: editing it changes no
/// scheme (so no fingerprint; the store half checks `parsed: 0`).
#[test]
fn the_registry_does_not_touch_the_scheme() {
    let base = "[ids]\nADR = { kind = \"decision\", width = 4 }\n";
    let with = format!(
        "{base}\n[[generators]]\ncommand = \"a\"\nwrites = [\"a.md\"]\n\n[[generators]]\ncommand = \"b\"\nwrites = [\"b.md\"]\n"
    );
    assert_eq!(
        IdScheme::from_toml(base).expect("valid"),
        IdScheme::from_toml(&with).expect("valid")
    );
}

// ------------------------------------------------------------------ iteration 2
// `command` and `gate` are plain YAML scalars that read back as themselves
// from the rendered `generator:` line; a blank `gate` is an error.

/// `(value as a TOML basic string body, the value, the problem named)`.
const NOT_PLAIN: &[(&str, &str, &str)] = &[
    (
        "just docs # regen",
        "just docs # regen",
        "contains `: ` or ` #`",
    ),
    ("a: b", "a: b", "contains `: ` or ` #`"),
    ("make docs: all", "make docs: all", "contains `: ` or ` #`"),
    (" x", " x", "has leading or trailing whitespace"),
    ("x ", "x ", "has leading or trailing whitespace"),
    ("x\\t", "x\t", "contains a newline or a control character"),
    ("-x", "-x", "starts with the YAML indicator `-`"),
    ("[x]", "[x]", "starts with the YAML indicator `[`"),
    ("x:", "x:", "ends with `:`"),
    ("make docs:", "make docs:", "ends with `:`"),
    ("a\\nb", "a\nb", "contains a newline or a control character"),
    ("a\\tb", "a\tb", "contains a newline or a control character"),
    (
        "a\\u0007b",
        "a\u{7}b",
        "contains a newline or a control character",
    ),
    ("a\\rb", "a\rb", "contains a newline or a control character"),
];

/// Plain scalars that must be accepted (and read back as themselves).
const PLAIN: &[&str] = &[
    "cargo xtask docs index --write",
    "cargo xtask docs check",
    "make docs",
    "npm run docs:build",
    "a:b",
    "x#y",
    "tool --out=docs/index.md",
    "a - b",
    "x?",
    "just docs-index",
    "spec check",
];

fn command_entry(body: &str) -> String {
    format!("[[generators]]\ncommand = \"{body}\"\nwrites = [\"a.md\"]\n")
}

fn gate_entry(body: &str) -> String {
    format!(
        "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"make\"\nwrites = [\"a.md\"]\nindex = true\ngate = \"{body}\"\n"
    )
}

#[test]
fn command_and_gate_must_be_plain_scalars() {
    let mut failures = Vec::new();
    for (key, entry, line) in [
        ("command", command_entry as fn(&str) -> String, 2),
        ("gate", gate_entry, 8),
    ] {
        for (body, value, problem) in NOT_PLAIN {
            let text = entry(body);
            match CheckConfig::from_toml(&text) {
                Ok(config) => failures.push(format!("{key} {value:?}: accepted as {config:?}")),
                Err(error) => {
                    let shown = error.at("specengine.toml");
                    let prefix =
                        format!("specengine.toml:{line}: generator `{key}` {value:?} {problem}");
                    if !shown.starts_with(&prefix) {
                        failures.push(format!("{key} {value:?}: {shown:?}\n  want {prefix:?}"));
                    }
                }
            }
        }
        // Every YAML indicator, first.
        for indicator in [
            '-', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`', '#',
            '?', ':',
        ] {
            let value = format!("{indicator}x");
            let body = if indicator == '"' {
                "\\\"x".to_owned()
            } else {
                value.clone()
            };
            let want = format!(
                "specengine.toml:{line}: generator `{key}` {value:?} starts with the YAML indicator `{indicator}`"
            );
            match CheckConfig::from_toml(&entry(&body)) {
                Ok(_) => failures.push(format!("{key} {value:?}: accepted")),
                Err(error) if !error.at("specengine.toml").starts_with(&want) => {
                    failures.push(format!("{key} {value:?}: {}", error.at("specengine.toml")))
                }
                Err(_) => {}
            }
        }
        for value in PLAIN {
            if let Err(error) = CheckConfig::from_toml(&entry(value)) {
                failures.push(format!(
                    "{key} {value:?} rejected: {}",
                    error.at("specengine.toml")
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn an_accepted_command_reads_back_from_the_rendered_header() {
    use specengine_core::check::{CheckInput, render_index};
    let scheme = IdScheme::from_toml("").expect("empty scheme");
    for value in PLAIN {
        let config = CheckConfig::from_toml(&format!(
            "[paths]\nindex = \"a.md\"\n\n[[generators]]\ncommand = \"{value}\"\nwrites = [\"a.md\"]\nindex = true\ngate = \"{value}\"\n"
        ))
        .unwrap_or_else(|e| panic!("{}", e.at("specengine.toml")));
        let generator = config.index_generator().unwrap();
        let render = render_index(&CheckInput::default(), "a.md", generator);
        let parsed = specengine_core::parse("a.md", render.as_bytes(), &scheme);
        let codes: Vec<&str> = parsed.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert!(
            !codes.iter().any(|code| code.starts_with("frontmatter-")),
            "{value:?}: {codes:?}"
        );
        let generator_read = parsed
            .document()
            .and_then(|d| d.fields.as_ref())
            .and_then(|f| f.generator.as_deref());
        assert_eq!(generator_read, Some(*value), "`generator:` reads back");
        assert!(
            render.contains(&format!("and `{value}` rejects them")),
            "the gate as written"
        );
    }
}

#[test]
fn a_blank_gate_is_an_error() {
    for (body, line) in [("", 8), ("   ", 8), ("\\t", 8)] {
        let text = gate_entry(body);
        let error = CheckConfig::from_toml(&text)
            .err()
            .unwrap_or_else(|| panic!("gate {body:?}: accepted"));
        assert_eq!(
            error.at("specengine.toml"),
            format!("specengine.toml:{line}: generator `gate` is blank"),
            "gate {body:?}"
        );
    }
    // A blank gate without `index = true` is still the `gate`-without-index
    // error (checked first).
    let error = CheckConfig::from_toml(
        "[[generators]]\ncommand = \"make\"\nwrites = [\"a.md\"]\ngate = \"\"\n",
    )
    .unwrap_err()
    .at("specengine.toml");
    assert!(error.starts_with("specengine.toml:4: "), "{error}");
    assert!(error.contains("`gate` is only for"), "{error}");
}

// ------------------------------------------------------------------ iteration 3
// Values the YAML 1.2 core schema reads as a null, a boolean or a number,
// and values holding `-->`, are rejected for `command` and `gate`; every
// accepted value must read back as the same string through this parser
// (serde-saphyr), from the rendered `generator:` line.

const NON_STRING: &str = "is read by YAML as a null, a boolean or a number, not a string";
const CLOSES_COMMENT: &str = "contains `-->`, which would close the index header's comment";

/// `(value, problem)`: rejected for both keys.
const REJECTED_3: &[(&str, &str)] = &[
    ("true", NON_STRING),
    ("True", NON_STRING),
    ("FALSE", NON_STRING),
    ("null", NON_STRING),
    ("NULL", NON_STRING),
    ("~", NON_STRING),
    ("123", NON_STRING),
    ("+1", NON_STRING),
    ("0", NON_STRING),
    ("007", NON_STRING),
    ("0x1F", NON_STRING),
    ("0o17", NON_STRING),
    ("1.5", NON_STRING),
    ("1e3", NON_STRING),
    ("1E-3", NON_STRING),
    ("+1.5e+3", NON_STRING),
    (".5", NON_STRING),
    ("5.", NON_STRING),
    (".inf", NON_STRING),
    ("+.INF", NON_STRING),
    (".Inf", NON_STRING),
    (".nan", NON_STRING),
    (".NaN", NON_STRING),
    (".NAN", NON_STRING),
    // Iteration 4: what serde-saphyr reads as a non-string beyond the core
    // schema (any-case booleans and null, uppercase and binary prefixes,
    // `_` separators, an any-case infinity normalised to `.inf`).
    ("tRUE", NON_STRING),
    ("TrUe", NON_STRING),
    ("nULL", NON_STRING),
    ("1_000", NON_STRING),
    ("0b101", NON_STRING),
    ("0O17", NON_STRING),
    ("0X1F", NON_STRING),
    (".INf", NON_STRING),
    ("a-->b", CLOSES_COMMENT),
    ("make --> docs", CLOSES_COMMENT),
];

/// The ending each key's message carries after the problem.
fn ending(key: &str) -> &'static str {
    if key == "command" {
        ": the index header carries it as written (`generator:` and the build comment)"
    } else {
        ": the index header names it as written"
    }
}

#[test]
fn non_string_and_comment_closing_values_are_rejected() {
    let mut failures = Vec::new();
    for (key, entry, line) in [
        ("command", command_entry as fn(&str) -> String, 2),
        ("gate", gate_entry, 8),
    ] {
        for (value, problem) in REJECTED_3 {
            let want = format!(
                "specengine.toml:{line}: generator `{key}` {value:?} {problem}{}",
                ending(key)
            );
            match CheckConfig::from_toml(&entry(value)) {
                Ok(_) => failures.push(format!("{key} {value:?}: accepted")),
                Err(error) if error.at("specengine.toml") != want => failures.push(format!(
                    "{key} {value:?}: {:?}\n  want {want:?}",
                    error.at("specengine.toml")
                )),
                Err(_) => {}
            }
        }
        // The earlier problems carry the same ending.
        let error = CheckConfig::from_toml(&entry("a: b"))
            .unwrap_err()
            .at("specengine.toml");
        assert!(error.ends_with(ending(key)), "{error}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Accepted near-misses of the core schema, and YAML 1.1 spellings: each
/// reads back through serde-saphyr as the same string.
const NEAR_MISSES: &[&str] = &[
    // The coordinator's list (`tRUE` moved to the rejected table).
    "make e1",
    "v1.0",
    "1e",
    "0x",
    "0o8",
    "e3",
    ".",
    "1.2.3",
    "nan",
    "inf",
    // YAML 1.1 and other spellings the rule accepts.
    "yes",
    "No",
    "on",
    "OFF",
    "y",
    "1:30",
    "2026-09-29",
    "NaN",
    "+inf",
    "0x1G",
    "1e+",
    ".e3",
    "1__0",
];

/// What this parser reads from `generator: <value>` in the rendered header:
/// `Ok(string)`, or `Err(what it read instead)`.
fn read_back(value: &str) -> Result<String, String> {
    use specengine_core::check::{CheckInput, Generator, render_index};
    let generator = Generator {
        command: value.to_owned(),
        writes: vec!["a.md".to_owned()],
        index: true,
        gate: None,
        line: 1,
    };
    let render = render_index(&CheckInput::default(), "a.md", &generator);
    let scheme = IdScheme::from_toml("").expect("empty scheme");
    let parsed = specengine_core::parse("a.md", render.as_bytes(), &scheme);
    let document = parsed.document().expect("a document");
    if let Some(read) = document.fields.as_ref().and_then(|f| f.generator.clone()) {
        return Ok(read);
    }
    let extra = document
        .extra
        .iter()
        .flatten()
        .find(|entry| entry.key == "generator")
        .map(|entry| format!("{:?}", entry.value));
    Err(extra.unwrap_or_else(|| "null (no value)".to_owned()))
}

#[test]
fn every_accepted_near_miss_reads_back_as_the_same_string() {
    let mut mismatches = Vec::new();
    for value in NEAR_MISSES {
        for (key, entry) in [
            ("command", command_entry as fn(&str) -> String),
            ("gate", gate_entry),
        ] {
            if let Err(error) = CheckConfig::from_toml(&entry(value)) {
                mismatches.push(format!(
                    "{key} {value:?} rejected: {}",
                    error.at("specengine.toml")
                ));
            }
        }
        match read_back(value) {
            Ok(read) if read == *value => {}
            Ok(read) => mismatches.push(format!(
                "{value:?} accepted, but `generator:` reads back as the string {read:?}"
            )),
            Err(read) => mismatches.push(format!(
                "{value:?} accepted, but `generator:` reads back as {read}, not a string"
            )),
        }
    }
    assert!(
        mismatches.is_empty(),
        "the rule and serde-saphyr disagree:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn rejected_values_do_not_read_back_but_the_normalised_spellings() {
    // Rejecting is safe; this pins where the rule is stricter than the
    // parser: `.inf` and `.nan` are the spellings the parser normalises
    // every infinity and NaN to, so they alone read back as themselves.
    let mut reads_back: Vec<&str> = REJECTED_3
        .iter()
        .filter(|(_, problem)| *problem == NON_STRING)
        .map(|(value, _)| *value)
        .filter(|value| read_back(value).as_deref() == Ok(*value))
        .collect();
    reads_back.sort_unstable();
    assert_eq!(reads_back, [".inf", ".nan"]);
}

// ------------------------------------------------------------------ link_base
// AC-04 of docs/features/spec-check-links.md (the core half): the optional
// `[paths] link_base`, checked like a `roots` item; no default. The
// re-parse and cannot-check halves are in the store's `tests/check_config.rs`.

#[test]
fn link_base_is_an_optional_root_relative_directory_without_default() {
    let base = Paths::from_toml("[paths]\nroots = [\"docs\"]\nlink_base = \"docs\"\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(base.link_base.as_deref(), Some("docs"));
    let slash = Paths::from_toml("[paths]\nroots = [\"docs\"]\nlink_base = \"docs/\"\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(slash, base, "one trailing `/` dropped: equal to `docs`");
    let nested = Paths::from_toml("[paths]\nlink_base = \"docs/spec\"\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(nested.link_base.as_deref(), Some("docs/spec"));
    // Existence is not checked: a base outside every root loads.
    let elsewhere = Paths::from_toml("[paths]\nroots = [\"docs\"]\nlink_base = \"wiki\"\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(elsewhere.link_base.as_deref(), Some("wiki"));
    // No default anywhere (ADR-0008).
    assert_eq!(Paths::default().link_base, None);
    for text in ["", "[paths]\n", "[paths]\nroots = [\"docs\"]\n"] {
        let paths = Paths::from_toml(text).expect("loads");
        assert_eq!(paths.link_base, None, "{text:?}");
    }
    // The other readers ignore the key: the scheme and the check tables load.
    let text = "[paths]\nroots = [\"docs\"]\nlink_base = \"docs\"\n\n[ids]\nADR = { kind = \"decision\", width = 4 }\n";
    let scheme =
        IdScheme::from_toml(text).unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    let without = IdScheme::from_toml("[ids]\nADR = { kind = \"decision\", width = 4 }\n")
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    assert_eq!(scheme, without);
    CheckConfig::from_toml(text).unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
}

#[test]
fn an_invalid_link_base_fails_with_file_line_message() {
    for (case, value) in [
        ("absolute", "\"/docs\""),
        ("parent", "\"../x\""),
        ("dot component", "\"a/./b\""),
        ("empty", "\"\""),
        ("a number", "1"),
        ("a list", "[\"docs\"]"),
        ("a bool", "true"),
        ("an empty component", "\"docs//spec\""),
        ("a parent inside", "\"docs/../x\""),
        ("a lone dot", "\".\""),
        ("a lone slash", "\"/\""),
    ] {
        let text = format!("[paths]\nroots = [\"docs\"]\n\nlink_base = {value}\n");
        let error = Paths::from_toml(&text)
            .err()
            .unwrap_or_else(|| panic!("{case}: accepted"));
        let shown = error.at("specengine.toml");
        assert!(shown.starts_with("specengine.toml:4: "), "{case}: {shown}");
        assert!(
            shown.len() > "specengine.toml:4: ".len(),
            "{case}: a message: {shown}"
        );
    }
}
